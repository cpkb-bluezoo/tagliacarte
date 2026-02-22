/*
 * mod.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Matrix backend (Store, Folder, Transport). Folder = one room. Connection reuse over HTTP;
//! semantic send; event-driven; MessageIds matrix://room/event; token refresh/re-login as needed.
//!
//! Architecture follows the Graph pipeline pattern:
//! - Persistent HTTPS connection to the homeserver
//! - Commands queued via `mpsc::UnboundedSender<MatrixCommand>` — fire-and-forget
//! - Pipeline loop processes commands sequentially on the same connection
//! - JSON responses parsed with the in-tree push parser (no serde_json)
//! - JSON request bodies built with `JsonWriter` (no serde_json)
//!
//! All trait methods are callback-driven and return immediately.

pub mod connection;
pub mod json_handlers;
pub mod requests;
pub mod types;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::message_id::{self, MessageId};
use crate::store::{
    Address, ConversationSummary, DateTime, Envelope, Folder, FolderInfo, OpenFolderEvent,
    SendPayload, Store, StoreError, StoreKind, Transport, TransportKind,
};

use connection::{connect_and_start_pipeline, MatrixCommand, MatrixConnection};
use types::{RoomEvent, RoomSummary, EVENT_ROOM_MESSAGE};

// ── MatrixStore ──────────────────────────────────────────────────────

/// Matrix store: homeserver + auth (user id, token). list_folders = joined rooms.
pub struct MatrixStore {
    uri: String,
    homeserver: String,
    user_id: String,
    access_token: RwLock<Option<String>>,
    runtime_handle: tokio::runtime::Handle,
    connection: Mutex<Option<MatrixConnection>>,
    /// Cached room metadata from sync.
    room_cache: Arc<RwLock<HashMap<String, RoomSummary>>>,
    /// next_batch token from last sync.
    sync_token: Arc<Mutex<Option<String>>>,
}

impl MatrixStore {
    pub fn new(
        homeserver: String,
        user_id: String,
        access_token: Option<String>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Result<Self, StoreError> {
        let uri = crate::uri::matrix_store_uri(&homeserver, &user_id);
        Ok(Self {
            uri,
            homeserver,
            user_id,
            access_token: RwLock::new(access_token),
            runtime_handle,
            connection: Mutex::new(None),
            room_cache: Arc::new(RwLock::new(HashMap::new())),
            sync_token: Arc::new(Mutex::new(None)),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn homeserver(&self) -> &str {
        &self.homeserver
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    fn get_token(&self) -> Result<String, StoreError> {
        self.access_token.read().unwrap().clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.user_id.clone(),
                is_plaintext: true,
            })
    }

    fn ensure_connection(&self) -> Result<MatrixConnection, StoreError> {
        let mut guard = self.connection.lock().unwrap();
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let conn = self.runtime_handle.block_on(
            connect_and_start_pipeline(&self.homeserver)
        )?;
        *guard = Some(conn.clone());
        Ok(conn)
    }
}

impl Store for MatrixStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Matrix
    }

    fn set_credential(&self, _username: Option<&str>, password: &str) {
        if let Ok(mut t) = self.access_token.write() {
            *t = Some(password.to_string());
        }
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(e) => { on_complete(Err(e)); return; }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => { on_complete(Err(e)); return; }
        };

        let room_cache = self.room_cache.clone();
        let sync_token = self.sync_token.clone();
        let since = sync_token.lock().unwrap().clone();

        let on_room: Arc<dyn Fn(RoomSummary) + Send + Sync> = Arc::new({
            let room_cache = room_cache.clone();
            move |room: RoomSummary| {
                let info = FolderInfo {
                    name: room.room_id.clone(),
                    delimiter: None,
                    attributes: if room.name.is_some() {
                        vec![format!("display_name={}", room.name.as_deref().unwrap_or(""))]
                    } else {
                        Vec::new()
                    },
                };
                on_folder(info);
                if let Ok(mut cache) = room_cache.write() {
                    cache.insert(room.room_id.clone(), room);
                }
            }
        });
        let on_event: Arc<dyn Fn(RoomEvent) + Send + Sync> = Arc::new(|_| {});

        conn.send(MatrixCommand::Sync {
            token,
            since,
            on_room,
            on_event,
            on_complete: Box::new(move |result| {
                match result {
                    Ok(next_batch) => {
                        if let Some(nb) = next_batch {
                            if let Ok(mut st) = sync_token.lock() {
                                *st = Some(nb);
                            }
                        }
                        on_complete(Ok(()));
                    }
                    Err(e) => on_complete(Err(e)),
                }
            }),
        });
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => { on_complete(Err(e)); return; }
        };
        let token = match self.get_token() {
            Ok(t) => t,
            Err(e) => { on_complete(Err(e)); return; }
        };

        let folder: Box<dyn Folder> = Box::new(MatrixFolder {
            room_id: name.to_string(),
            store_uri: self.uri.clone(),
            homeserver: self.homeserver.clone(),
            user_id: self.user_id.clone(),
            token,
            connection: conn,
        });
        on_complete(Ok(folder));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        None
    }
}

// ── MatrixFolder ─────────────────────────────────────────────────────

/// Folder = one Matrix room. Messages = room events (m.room.message).
struct MatrixFolder {
    room_id: String,
    #[allow(dead_code)]
    store_uri: String,
    #[allow(dead_code)]
    homeserver: String,
    #[allow(dead_code)]
    user_id: String,
    token: String,
    connection: MatrixConnection,
}

unsafe impl Send for MatrixFolder {}
unsafe impl Sync for MatrixFolder {}

fn room_event_to_summary(event: &RoomEvent) -> ConversationSummary {
    let (local_part, domain) = split_matrix_user_id(&event.sender);
    let from = Address {
        display_name: None,
        local_part,
        domain: Some(domain),
    };
    let timestamp = event.origin_server_ts / 1000;
    let date = DateTime {
        timestamp,
        tz_offset_secs: Some(0),
    };

    ConversationSummary {
        id: message_id::matrix_message_id(&event.room_id, &event.event_id),
        envelope: Envelope {
            from: vec![from],
            to: Vec::new(),
            cc: Vec::new(),
            date: Some(date),
            subject: event.body.clone(),
            message_id: Some(event.event_id.clone()),
        },
        flags: std::collections::HashSet::new(),
        size: event.body.as_ref().map_or(0, |b| b.len()) as u64,
    }
}

/// Split `@user:server` into `("user", "server")`.
fn split_matrix_user_id(user_id: &str) -> (String, String) {
    let s = user_id.strip_prefix('@').unwrap_or(user_id);
    if let Some(colon) = s.find(':') {
        (s[..colon].to_string(), s[colon + 1..].to_string())
    } else {
        (s.to_string(), String::new())
    }
}

impl Folder for MatrixFolder {
    fn list_conversations(
        &self,
        range: std::ops::Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let limit = range.end.saturating_sub(range.start);
        if limit == 0 {
            on_complete(Ok(()));
            return;
        }
        let on_event: Arc<dyn Fn(RoomEvent) + Send + Sync> = Arc::new(move |event| {
            if event.event_type == EVENT_ROOM_MESSAGE {
                on_summary(room_event_to_summary(&event));
            }
        });

        self.connection.send(MatrixCommand::RoomMessages {
            token: self.token.clone(),
            room_id: self.room_id.clone(),
            limit,
            from: None,
            on_event,
            on_complete: Box::new(|result| {
                on_complete(result.map(|_| ()));
            }),
        });
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        // Matrix doesn't have a direct message count endpoint for rooms.
        // Return 0; the UI counts as messages stream in.
        on_complete(Ok(0));
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // Parse the event_id from the MessageId (format: matrix://room_id/event_id)
        let id_str = id.as_str();
        let event_id = id_str.strip_prefix("matrix://")
            .and_then(|rest| rest.split('/').nth(1))
            .unwrap_or(id_str);

        self.connection.send(MatrixCommand::GetEvent {
            token: self.token.clone(),
            room_id: self.room_id.clone(),
            event_id: event_id.to_string(),
            on_complete: Box::new(move |result| {
                match result {
                    Ok(Some(event)) => {
                        let summary = room_event_to_summary(&event);
                        on_metadata(summary.envelope);
                        if let Some(ref body) = event.body {
                            on_content_chunk(body.as_bytes());
                        }
                        on_complete(Ok(()));
                    }
                    Ok(None) => {
                        on_complete(Err(StoreError::new("Matrix event not found")));
                    }
                    Err(e) => on_complete(Err(e)),
                }
            }),
        });
    }
}

// ── MatrixTransport ──────────────────────────────────────────────────

/// Matrix transport: send to room or user. Same account as store.
pub struct MatrixTransport {
    uri: String,
    homeserver: String,
    user_id: String,
    access_token: RwLock<Option<String>>,
    runtime_handle: tokio::runtime::Handle,
    connection: Mutex<Option<MatrixConnection>>,
    txn_counter: Mutex<u64>,
}

impl MatrixTransport {
    pub fn new(
        homeserver: String,
        user_id: String,
        access_token: Option<String>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Result<Self, StoreError> {
        let uri = crate::uri::matrix_transport_uri(&homeserver, &user_id);
        Ok(Self {
            uri,
            homeserver,
            user_id,
            access_token: RwLock::new(access_token),
            runtime_handle,
            connection: Mutex::new(None),
            txn_counter: Mutex::new(0),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn get_token(&self) -> Result<String, StoreError> {
        self.access_token.read().unwrap().clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.user_id.clone(),
                is_plaintext: true,
            })
    }

    fn ensure_connection(&self) -> Result<MatrixConnection, StoreError> {
        let mut guard = self.connection.lock().unwrap();
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let conn = self.runtime_handle.block_on(
            connect_and_start_pipeline(&self.homeserver)
        )?;
        *guard = Some(conn.clone());
        Ok(conn)
    }

    fn next_txn_id(&self) -> String {
        let mut counter = self.txn_counter.lock().unwrap();
        *counter += 1;
        format!("tc_{}", *counter)
    }
}

impl Transport for MatrixTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Matrix
    }

    fn send(
        &self,
        payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(e) => { on_complete(Err(e)); return; }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => { on_complete(Err(e)); return; }
        };

        // The "to" field should contain the room_id for Matrix.
        let room_id = match payload.to.first() {
            Some(addr) => {
                if addr.local_part.starts_with('!') {
                    addr.local_part.clone()
                } else if let Some(ref domain) = addr.domain {
                    format!("{}:{}", addr.local_part, domain)
                } else {
                    on_complete(Err(StoreError::new("Matrix send: no room ID in recipient")));
                    return;
                }
            }
            None => {
                on_complete(Err(StoreError::new("Matrix send: no recipient")));
                return;
            }
        };

        let body_text = payload.body_plain.as_deref()
            .or(payload.body_html.as_deref())
            .unwrap_or("");
        let body = requests::build_text_message_body(body_text);
        let txn_id = self.next_txn_id();

        conn.send(MatrixCommand::SendMessage {
            token,
            room_id,
            body,
            txn_id,
            on_complete,
        });
    }
}

// ── Utility ──────────────────────────────────────────────────────────

/// Parse a Matrix homeserver timestamp (milliseconds since epoch) to a DateTime.
pub fn matrix_timestamp_to_datetime(ts_ms: i64) -> DateTime {
    DateTime {
        timestamp: ts_ms / 1000,
        tz_offset_secs: Some(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_matrix_user_id() {
        let (user, server) = split_matrix_user_id("@alice:matrix.org");
        assert_eq!(user, "alice");
        assert_eq!(server, "matrix.org");
    }

    #[test]
    fn test_split_matrix_user_id_no_prefix() {
        let (user, server) = split_matrix_user_id("bob:example.com");
        assert_eq!(user, "bob");
        assert_eq!(server, "example.com");
    }
}
