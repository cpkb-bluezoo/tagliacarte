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

//! Microsoft Graph REST API protocol: GraphStore (Store) and GraphTransport (Transport).
//!
//! Uses OAuth2 bearer tokens obtained from the shared OAuth infrastructure
//! (`core/src/oauth/`). Communicates with `https://graph.microsoft.com/v1.0`
//! for mail operations (mailFolders, messages, sendMail).
//!
//! Architecture follows the IMAP pipeline pattern:
//! - Persistent HTTPS connection to `graph.microsoft.com:443`
//! - Commands queued via `mpsc::UnboundedSender<GraphCommand>` — fire-and-forget
//! - Pipeline loop processes commands sequentially on the same connection
//! - JSON responses parsed with the in-tree push parser (no serde_json)
//! - JSON request bodies built with `JsonWriter` (no serde_json)
//! - No reqwest, no external HTTP client
//!
//! All trait methods are fully callback-driven and return immediately.

pub mod connection;
pub mod json_handlers;
pub mod requests;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use crate::config::default_credentials_path;
use crate::message_id::MessageId;
use crate::oauth::provider::OAuthProvider;
use crate::oauth::token_store::get_valid_access_token;
use crate::oauth::MicrosoftOAuthProvider;
use crate::store::{
    ConversationSummary, DateTime, Envelope, Flag, Folder, FolderInfo,
    OpenFolderEvent, SendPayload, Store, StoreError, StoreKind, Transport, TransportKind,
};

use connection::{connect_and_start_pipeline, GraphCommand, GraphConnection};
use json_handlers::GraphFolderEntry;

// ── GraphStore ────────────────────────────────────────────────────────

/// Microsoft Graph mail store. Implements the `Store` trait.
///
/// Uses a persistent pipeline connection to graph.microsoft.com (like the IMAP
/// module). Commands are queued via the connection handle; all trait methods are
/// fully callback-driven and return immediately.
pub struct GraphStore {
    email: String,
    uri: String,
    client_id: String,
    runtime_handle: tokio::runtime::Handle,
    credentials_path: PathBuf,
    /// Live pipeline connection.
    connection: Mutex<Option<GraphConnection>>,
    /// Cache of folder id → folder metadata (populated on list_folders).
    folder_cache: Arc<RwLock<Vec<GraphFolderEntry>>>,
}

impl GraphStore {
    /// Create a new GraphStore.
    ///
    /// `email`: the user's Microsoft/Exchange email address.
    /// `client_id`: the registered Microsoft OAuth2 client ID.
    /// `runtime_handle`: tokio runtime handle for connection management and token refresh.
    pub fn new(
        email: impl Into<String>,
        client_id: impl Into<String>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Result<Self, StoreError> {
        let email = email.into();
        let uri = crate::uri::graph_store_uri(&email);
        let credentials_path = default_credentials_path()
            .ok_or_else(|| StoreError::new("no credentials path available"))?;
        Ok(Self {
            email,
            uri,
            client_id: client_id.into(),
            runtime_handle,
            credentials_path,
            connection: Mutex::new(None),
            folder_cache: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Obtain a valid access token, auto-refreshing if needed.
    /// Returns `StoreError::NeedsCredential` if tokens are unavailable or refresh fails.
    fn access_token(&self) -> Result<String, StoreError> {
        let provider = MicrosoftOAuthProvider::new(&self.client_id);
        get_valid_access_token(
            &self.credentials_path,
            &provider,
            &self.uri,
            &self.runtime_handle,
        )
        .or_else(|_| {
            let generic_key = format!("oauth:{}", provider.provider_id());
            get_valid_access_token(
                &self.credentials_path,
                &provider,
                &generic_key,
                &self.runtime_handle,
            )
        })
        .map_err(|_| StoreError::NeedsCredential {
            username: self.email.clone(),
            is_plaintext: false,
        })
    }

    /// Ensure the pipeline connection is alive, reconnecting if necessary.
    fn ensure_connection(&self) -> Result<GraphConnection, StoreError> {
        let mut guard = self.connection.lock().unwrap();
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let conn = self.runtime_handle.block_on(connect_and_start_pipeline())?;
        *guard = Some(conn.clone());
        Ok(conn)
    }

    /// Look up a folder's Graph ID by display name. Uses cached data.
    fn folder_id_by_name(&self, name: &str) -> Option<String> {
        let cache = self.folder_cache.read().ok()?;
        cache
            .iter()
            .find(|f| f.display_name == name)
            .map(|f| f.id.clone())
    }
}

impl Store for GraphStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let cache = self.folder_cache.clone();
        if let Ok(mut c) = cache.write() {
            c.clear();
        }
        let on_folder_arc: Arc<dyn Fn(FolderInfo, GraphFolderEntry) + Send + Sync> =
            Arc::new(move |info, entry| {
                on_folder(info);
                if let Ok(mut c) = cache.write() {
                    c.push(entry);
                }
            });

        conn.send(GraphCommand::ListFolders {
            token,
            on_folder: on_folder_arc,
            on_complete,
        });
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        // Graph doesn't have a SELECT-equivalent. Just look up the folder in cache.
        let folder_id = match self.folder_id_by_name(name) {
            Some(id) => id,
            None => {
                on_complete(Err(StoreError::new(format!("folder '{}' not found", name))));
                return;
            }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        on_complete(Ok(Box::new(GraphFolder {
            folder_id,
            folder_name: name.to_string(),
            email: self.email.clone(),
            uri: self.uri.clone(),
            client_id: self.client_id.clone(),
            runtime_handle: self.runtime_handle.clone(),
            credentials_path: self.credentials_path.clone(),
            connection: conn,
            folder_cache: self.folder_cache.clone(),
        })));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        Some('/')
    }

    fn default_folder(&self) -> Option<&str> {
        Some("Inbox")
    }

    fn create_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        conn.send(GraphCommand::CreateFolder {
            token,
            name: name.to_string(),
            on_complete,
        });
    }

    fn rename_folder(
        &self,
        old_name: &str,
        new_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let folder_id = match self.folder_id_by_name(old_name) {
            Some(id) => id,
            None => {
                on_complete(Err(StoreError::new(format!(
                    "folder '{}' not found",
                    old_name
                ))));
                return;
            }
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        conn.send(GraphCommand::RenameFolder {
            token,
            folder_id,
            new_name: new_name.to_string(),
            on_complete,
        });
    }

    fn delete_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let folder_id = match self.folder_id_by_name(name) {
            Some(id) => id,
            None => {
                on_complete(Err(StoreError::new(format!(
                    "folder '{}' not found",
                    name
                ))));
                return;
            }
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        conn.send(GraphCommand::DeleteFolder {
            token,
            folder_id,
            on_complete,
        });
    }
}

// ── GraphFolder ───────────────────────────────────────────────────────

/// A folder in a Graph mailbox.
struct GraphFolder {
    folder_id: String,
    #[allow(dead_code)]
    folder_name: String,
    email: String,
    uri: String,
    client_id: String,
    runtime_handle: tokio::runtime::Handle,
    credentials_path: PathBuf,
    connection: GraphConnection,
    folder_cache: Arc<RwLock<Vec<GraphFolderEntry>>>,
}

impl GraphFolder {
    fn folder_id_by_name(&self, name: &str) -> Option<String> {
        let cache = self.folder_cache.read().ok()?;
        cache.iter().find(|f| f.display_name == name).map(|f| f.id.clone())
    }

    fn access_token(&self) -> Result<String, StoreError> {
        let provider = MicrosoftOAuthProvider::new(&self.client_id);
        get_valid_access_token(
            &self.credentials_path,
            &provider,
            &self.uri,
            &self.runtime_handle,
        )
        .or_else(|_| {
            let generic_key = format!("oauth:{}", provider.provider_id());
            get_valid_access_token(
                &self.credentials_path,
                &provider,
                &generic_key,
                &self.runtime_handle,
            )
        })
        .map_err(|_| StoreError::NeedsCredential {
            username: self.email.clone(),
            is_plaintext: false,
        })
    }
}

unsafe impl Send for GraphFolder {}
unsafe impl Sync for GraphFolder {}

impl Folder for GraphFolder {
    fn list_conversations(
        &self,
        range: std::ops::Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let skip = range.start;
        let top = range.end.saturating_sub(range.start);
        if top == 0 {
            on_complete(Ok(()));
            return;
        }
        let folder_id = self.folder_id.clone();
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let on_summary: Arc<dyn Fn(ConversationSummary) + Send + Sync> = Arc::from(on_summary);
        self.connection.send(GraphCommand::ListMessages {
            token,
            folder_id,
            top,
            skip,
            on_summary,
            on_complete,
        });
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        let folder_id = self.folder_id.clone();
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        self.connection.send(GraphCommand::MessageCount {
            token,
            folder_id,
            on_complete,
        });
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let message_id = id.as_str().to_string();
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        self.connection.send(GraphCommand::GetMessage {
            token,
            message_id,
            on_complete: Box::new(move |result| {
                match result {
                    Ok(Some(msg)) => {
                        on_metadata(msg.envelope);
                        if let Some(ref raw) = msg.raw {
                            on_content_chunk(raw);
                        } else {
                            if let Some(ref b) = msg.body_plain {
                                on_content_chunk(b.as_bytes());
                            }
                            if let Some(ref b) = msg.body_html {
                                on_content_chunk(b.as_bytes());
                            }
                        }
                        on_complete(Ok(()));
                    }
                    Ok(None) => on_complete(Err(StoreError::new("message not found"))),
                    Err(e) => on_complete(Err(e)),
                }
            }),
        });
    }

    fn delete_message(
        &self,
        id: &MessageId,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let message_id = id.as_str().to_string();
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        self.connection.send(GraphCommand::DeleteMessage {
            token,
            message_id,
            on_complete,
        });
    }

    fn copy_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        if ids.is_empty() {
            on_complete(Ok(()));
            return;
        }
        let dest_folder_id = match self.folder_id_by_name(dest_folder_name) {
            Some(id) => id,
            None => {
                on_complete(Err(StoreError::new(format!("folder '{}' not found", dest_folder_name))));
                return;
            }
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let message_ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        self.connection.send(GraphCommand::CopyMessages {
            token,
            message_ids,
            dest_folder_id,
            on_complete,
        });
    }

    fn move_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        if ids.is_empty() {
            on_complete(Ok(()));
            return;
        }
        let dest_folder_id = match self.folder_id_by_name(dest_folder_name) {
            Some(id) => id,
            None => {
                on_complete(Err(StoreError::new(format!("folder '{}' not found", dest_folder_name))));
                return;
            }
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let message_ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        self.connection.send(GraphCommand::MoveMessages {
            token,
            message_ids,
            dest_folder_id,
            on_complete,
        });
    }

    fn store_flags(
        &self,
        ids: &[&str],
        add: &[Flag],
        remove: &[Flag],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let body = match requests::build_flag_patch_body(add, remove) {
            Some(b) => b,
            None => {
                on_complete(Ok(()));
                return;
            }
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let message_ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        self.connection.send(GraphCommand::StoreFlags {
            token,
            message_ids,
            body,
            on_complete,
        });
    }
}

// ── GraphTransport ────────────────────────────────────────────────────

/// Microsoft Graph mail transport. Sends messages via `POST /me/sendMail`.
pub struct GraphTransport {
    #[allow(dead_code)]
    email: String,
    uri: String,
    client_id: String,
    runtime_handle: tokio::runtime::Handle,
    credentials_path: PathBuf,
    connection: Mutex<Option<GraphConnection>>,
}

impl GraphTransport {
    pub fn new(
        email: impl Into<String>,
        client_id: impl Into<String>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Result<Self, StoreError> {
        let email = email.into();
        let uri = crate::uri::graph_transport_uri(&email);
        let credentials_path = default_credentials_path()
            .ok_or_else(|| StoreError::new("no credentials path available"))?;
        Ok(Self {
            email,
            uri,
            client_id: client_id.into(),
            runtime_handle,
            credentials_path,
            connection: Mutex::new(None),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn access_token(&self) -> Result<String, StoreError> {
        let provider = MicrosoftOAuthProvider::new(&self.client_id);
        get_valid_access_token(
            &self.credentials_path,
            &provider,
            &self.uri,
            &self.runtime_handle,
        )
        .or_else(|_| {
            let generic_key = format!("oauth:{}", provider.provider_id());
            get_valid_access_token(
                &self.credentials_path,
                &provider,
                &generic_key,
                &self.runtime_handle,
            )
        })
        .map_err(|_| StoreError::NeedsCredential {
            username: self.email.clone(),
            is_plaintext: false,
        })
    }

    fn ensure_connection(&self) -> Result<GraphConnection, StoreError> {
        let mut guard = self.connection.lock().unwrap();
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let conn = self.runtime_handle.block_on(connect_and_start_pipeline())?;
        *guard = Some(conn.clone());
        Ok(conn)
    }
}

impl Transport for GraphTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Email
    }

    fn send(
        &self,
        payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let body = requests::build_send_mail_body(payload);
        conn.send(GraphCommand::SendMail {
            token,
            body,
            on_complete,
        });
    }
}

// ── Utility functions ─────────────────────────────────────────────────

/// Parse a Graph ISO 8601 datetime string into a DateTime.
pub(crate) fn parse_graph_datetime(s: &str) -> Option<DateTime> {
    // Graph returns dates like "2026-01-15T10:30:00Z" or "2026-01-15T10:30:00.0000000Z".
    let s = s.trim_end_matches('Z');
    let parts: Vec<&str> = s.split('T').collect();
    if parts.len() != 2 {
        return None;
    }
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    let time_parts: Vec<&str> = parts[1].split('.').next()?.split(':').collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }
    let year: i64 = date_parts[0].parse().ok()?;
    let month: i64 = date_parts[1].parse().ok()?;
    let day: i64 = date_parts[2].parse().ok()?;
    let hour: i64 = time_parts[0].parse().ok()?;
    let min: i64 = time_parts[1].parse().ok()?;
    let sec: i64 = time_parts[2].parse().ok()?;

    // Simplified days-since-epoch calculation (good enough for email timestamps).
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    let month_days = [31, 28 + if is_leap_year(year) { 1 } else { 0 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 0..((month - 1) as usize) {
        days += month_days[m];
    }
    days += day - 1;

    let timestamp = days * 86400 + hour * 3600 + min * 60 + sec;
    Some(DateTime {
        timestamp,
        tz_offset_secs: Some(0), // Graph returns UTC.
    })
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Simple base64 decode (standard alphabet, with padding).
pub(crate) fn base64_decode(input: &str) -> Vec<u8> {
    const DECODE: [u8; 128] = {
        let mut t = [255u8; 128];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut i = 0u8;
        while i < 10 {
            t[(b'0' + i) as usize] = i + 52;
            i += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };

    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=' && b != b'\n' && b != b'\r').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let a = DECODE.get(chunk[0] as usize).copied().unwrap_or(0) as u32;
        let b = chunk.get(1).and_then(|&c| DECODE.get(c as usize)).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).and_then(|&c| DECODE.get(c as usize)).copied().unwrap_or(0) as u32;
        let d = chunk.get(3).and_then(|&c| DECODE.get(c as usize)).copied().unwrap_or(0) as u32;
        let n = (a << 18) | (b << 12) | (c << 6) | d;
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    out
}

/// Simple base64 encode (standard alphabet, with padding).
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (chunk.get(1).copied().unwrap_or(0) as u32) << 8
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(ALPHABET[(n >> 18) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_graph_datetime() {
        let dt = parse_graph_datetime("2026-01-15T10:30:00Z").unwrap();
        assert!(dt.timestamp > 0);
        assert_eq!(dt.tz_offset_secs, Some(0));
    }

    #[test]
    fn test_parse_graph_datetime_with_fractional() {
        let dt = parse_graph_datetime("2026-01-15T10:30:00.0000000Z").unwrap();
        assert!(dt.timestamp > 0);
    }

    #[test]
    fn test_base64_roundtrip() {
        let input = b"Hello, World!";
        let encoded = base64_encode(input);
        let decoded = base64_decode(&encoded);
        assert_eq!(decoded, input);
    }
}
