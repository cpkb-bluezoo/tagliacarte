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
pub mod crypto;
pub mod device;
pub mod encrypted_attachments;
pub mod json_handlers;
pub mod key_backup;
pub mod requests;
pub mod types;
pub mod verification;

mod parse;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use crate::message_id::{self, MessageId};
use crate::store::{
    Address, ConversationSummary, DateTime, Envelope, Folder, FolderInfo, OpenFolderEvent,
    SendPayload, Store, StoreError, StoreKind, Transport, TransportKind,
};

use connection::{connect_and_start_pipeline, MatrixCommand, MatrixConnection};
use crypto::CryptoMachine;
use device::DeviceTracker;
use types::{RoomEvent, RoomSummary, EVENT_ROOM_ENCRYPTED, EVENT_ROOM_MESSAGE};

// ── MatrixStore ──────────────────────────────────────────────────────

/// Matrix store: homeserver + auth (user id, token). list_folders = joined rooms.
pub struct MatrixStore {
    uri: String,
    homeserver: String,
    user_id: String,
    device_id: RwLock<Option<String>>,
    access_token: RwLock<Option<String>>,
    runtime_handle: tokio::runtime::Handle,
    connection: Mutex<Option<MatrixConnection>>,
    /// Cached room metadata from sync.
    room_cache: Arc<RwLock<HashMap<String, RoomSummary>>>,
    /// next_batch token from last sync.
    sync_token: Arc<Mutex<Option<String>>>,
    /// E2EE crypto machine, initialized after login/credential set.
    crypto: Arc<RwLock<Option<Arc<CryptoMachine>>>>,
    /// Device tracker for E2EE key management.
    device_tracker: Arc<DeviceTracker>,
    #[allow(dead_code)]
    encrypted_rooms: Arc<RwLock<std::collections::HashSet<String>>>,
    /// Active key backup: (version, recovery_key) — set after restore or setup.
    backup_info: Arc<RwLock<Option<(String, key_backup::RecoveryKey)>>>,
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
            device_id: RwLock::new(None),
            access_token: RwLock::new(access_token),
            runtime_handle,
            connection: Mutex::new(None),
            room_cache: Arc::new(RwLock::new(HashMap::new())),
            sync_token: Arc::new(Mutex::new(None)),
            crypto: Arc::new(RwLock::new(None)),
            device_tracker: Arc::new(DeviceTracker::new()),
            encrypted_rooms: Arc::new(RwLock::new(std::collections::HashSet::new())),
            backup_info: Arc::new(RwLock::new(None)),
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
        self.access_token
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.user_id.clone(),
                is_plaintext: false,
                advertised_capabilities: None,
            })
    }

    /// Record device id after login. Matrix E2EE (Olm/Megolm) is not active — no vodozemac backend.
    pub fn init_crypto(&self, device_id: &str) -> Result<(), StoreError> {
        let _ = self.get_token()?;
        *self.device_id.write().unwrap() = Some(device_id.to_string());
        *self.crypto.write().unwrap() = None;
        Ok(())
    }

    pub fn get_crypto(&self) -> Option<Arc<CryptoMachine>> {
        self.crypto.read().unwrap().clone()
    }

    pub fn device_fingerprint(&self) -> Option<String> {
        None
    }

    pub fn access_token(&self) -> Option<String> {
        self.access_token.read().unwrap().clone()
    }

    /// [`MatrixTransport`] sharing this account’s token, connection pool, and E2EE state.
    ///
    /// Outgoing room messages should use this transport (see the app’s `matrix_send` module)
    /// rather than treating send as a generic store operation.
    pub fn paired_transport(&self) -> Result<MatrixTransport, StoreError> {
        let token = self.get_token()?;
        let t = MatrixTransport::new(
            self.homeserver.clone(),
            self.user_id.clone(),
            Some(token),
            self.runtime_handle.clone(),
        )?;
        if let Some(cm) = self.get_crypto() {
            t.set_crypto(cm);
        }
        t.set_encrypted_rooms(Arc::clone(&self.encrypted_rooms));
        Ok(t)
    }

    /// Perform m.login.password, store resulting access_token and init crypto.
    pub fn login(&self, password: &str) -> Result<types::LoginResponse, StoreError> {
        eprintln!("[matrix] login: connecting to {}", self.homeserver);
        let conn = self.ensure_connection()?;
        eprintln!(
            "[matrix] login: connected, sending login for {}",
            self.user_id
        );
        let (tx, rx) = std::sync::mpsc::channel();
        let user = self.user_id.clone();
        let pw = password.to_string();
        conn.send(MatrixCommand::Login {
            user,
            password: pw,
            on_complete: Box::new(move |result| {
                eprintln!(
                    "[matrix] login response: {}",
                    if result.is_ok() { "ok" } else { "error" }
                );
                let _ = tx.send(result);
            }),
        });
        let resp = match rx.recv() {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                eprintln!("[matrix] login failed: {}", e);
                return Err(e);
            }
            Err(_) => {
                eprintln!("[matrix] login: channel closed unexpectedly");
                return Err(StoreError::new("login channel closed"));
            }
        };
        eprintln!("[matrix] login succeeded, device_id={}", resp.device_id);
        *self.access_token.write().unwrap() = Some(resp.access_token.clone());
        if let Err(e) = self.init_crypto(&resp.device_id) {
            eprintln!("[matrix] crypto init after login failed: {}", e);
        }
        Ok(resp)
    }

    fn ensure_connection(&self) -> Result<MatrixConnection, StoreError> {
        let mut guard = self.connection.lock().unwrap();
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let conn = self
            .runtime_handle
            .block_on(connect_and_start_pipeline(&self.homeserver))?;
        *guard = Some(conn.clone());
        Ok(conn)
    }

    pub fn ensure_connection_pub(&self) -> Result<MatrixConnection, StoreError> {
        self.ensure_connection()
    }

    /// Restore Megolm session keys from server-side backup using a recovery key.
    /// Returns the number of sessions restored.
    pub fn restore_backup(&self, recovery_key_b58: &str) -> Result<usize, StoreError> {
        let recovery = key_backup::RecoveryKey::from_base58(recovery_key_b58)?;
        let token = self.get_token()?;
        let conn = self.ensure_connection()?;

        let (tx, rx) = std::sync::mpsc::channel();
        conn.send(MatrixCommand::GetKeyBackupVersion {
            token: token.clone(),
            on_complete: Box::new(move |r| {
                let _ = tx.send(r);
            }),
        });
        let version_info = rx
            .recv()
            .map_err(|_| StoreError::new("backup version channel closed"))??;
        let (version, algorithm) = match version_info {
            Some(v) => v,
            None => return Ok(0),
        };
        if algorithm != "m.megolm_backup.v1.curve25519-aes-sha2" {
            return Err(StoreError::new(format!(
                "unsupported backup algorithm: {}",
                algorithm
            )));
        }
        let backup_version = version.clone();

        let (tx2, rx2) = std::sync::mpsc::channel();
        conn.send(MatrixCommand::DownloadRoomKeys {
            token,
            version,
            on_complete: Box::new(move |r| {
                let _ = tx2.send(r);
            }),
        });
        let body = rx2
            .recv()
            .map_err(|_| StoreError::new("download room keys channel closed"))??;
        let rows = parse::parse_room_keys_backup(&body)
            .map_err(|e| StoreError::new(format!("parse backup: {}", e)))?;
        eprintln!(
            "[matrix] key backup: downloaded {} session rows; Megolm import skipped (no E2EE backend)",
            rows.len()
        );
        *self.backup_info.write().unwrap() = Some((backup_version, recovery));
        Ok(0)
    }

    /// Upload a single session key to the server backup (if backup is active).
    pub fn upload_session_to_backup(&self, room_id: &str, session_id: &str, session_key_b64: &str) {
        let info = self.backup_info.read().unwrap();
        let (version, recovery) = match info.as_ref() {
            Some(v) => v,
            None => return,
        };
        let token = match self.access_token() {
            Some(t) => t,
            None => return,
        };
        let conn = match self.ensure_connection() {
            Ok(c) => c,
            Err(_) => return,
        };
        let plaintext = format!(
            "{{\"algorithm\":\"m.megolm.v1.aes-sha2\",\"room_id\":\"{}\",\"session_id\":\"{}\",\"session_key\":\"{}\"}}",
            room_id, session_id, session_key_b64
        );
        let encrypted = match key_backup::encrypt_backup_session(recovery, plaintext.as_bytes()) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[matrix] backup encrypt failed: {}", e);
                return;
            }
        };
        let mut sessions = std::collections::HashMap::new();
        let mut room_sessions = std::collections::HashMap::new();
        room_sessions.insert(session_id.to_string(), encrypted);
        sessions.insert(room_id.to_string(), room_sessions);
        let body = key_backup::build_upload_room_keys_body(&sessions);
        conn.send(MatrixCommand::UploadRoomKeys {
            token,
            version: version.clone(),
            body,
            on_complete: Box::new(|result| {
                if let Err(e) = result {
                    eprintln!("[matrix] backup upload failed: {}", e);
                }
            }),
        });
    }
}

impl MatrixStore {
    /// Room ids listed in `m.direct` account data (DMs).
    pub fn direct_message_room_ids_blocking(&self) -> Result<HashSet<String>, StoreError> {
        let token = self.get_token()?;
        let conn = self.ensure_connection()?;
        let uid = self.user_id.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        conn.send(MatrixCommand::GetAccountData {
            token,
            user_id: uid,
            event_type: "m.direct".to_string(),
            on_complete: Box::new(move |r| {
                let _ = tx.send(r);
            }),
        });
        let res = rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .map_err(|_| StoreError::new("m.direct timeout"))?;
        match res {
            Ok(body) => Ok(parse::parse_m_direct_event_body(&body)),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("M_NOT_FOUND") {
                    Ok(HashSet::new())
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn public_rooms_blocking(
        &self,
        limit: u32,
        generic_search_term: Option<&str>,
    ) -> Result<Vec<(String, Option<String>)>, StoreError> {
        let token = self.get_token()?;
        let conn = self.ensure_connection()?;
        let body = requests::build_public_rooms_body(limit, generic_search_term);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        conn.send(MatrixCommand::PublicRooms {
            token,
            body,
            on_complete: Box::new(move |r| {
                let _ = tx.send(r);
            }),
        });
        let raw = rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .map_err(|_| StoreError::new("publicRooms timeout"))??;
        parse::parse_public_rooms_response(&raw)
    }

    pub fn join_room_blocking(&self, room_id_or_alias: &str) -> Result<(), StoreError> {
        let token = self.get_token()?;
        let conn = self.ensure_connection()?;
        let rid = room_id_or_alias.trim().to_string();
        if rid.is_empty() {
            return Err(StoreError::new("empty room id or alias"));
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        conn.send(MatrixCommand::JoinRoom {
            token,
            room_id_or_alias: rid,
            on_complete: Box::new(move |r| {
                let _ = tx.send(r);
            }),
        });
        rx.recv_timeout(std::time::Duration::from_secs(120))
            .map_err(|_| StoreError::new("join timeout"))??;
        Ok(())
    }

    pub fn leave_room_blocking(&self, room_id: &str) -> Result<(), StoreError> {
        let token = self.get_token()?;
        let conn = self.ensure_connection()?;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        conn.send(MatrixCommand::LeaveRoom {
            token,
            room_id: room_id.to_string(),
            on_complete: Box::new(move |r| {
                let _ = tx.send(r);
            }),
        });
        rx.recv_timeout(std::time::Duration::from_secs(120))
            .map_err(|_| StoreError::new("leave timeout"))??;
        Ok(())
    }
}

impl Store for MatrixStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Matrix
    }

    fn set_credential(&self, _username: Option<&str>, password: &str) {
        if let Err(e) = self.login(password) {
            eprintln!("[matrix] login failed: {}", e);
        }
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.get_token() {
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

        let room_cache = self.room_cache.clone();
        let sync_token = self.sync_token.clone();

        let on_room: Arc<dyn Fn(RoomSummary) + Send + Sync> = Arc::new({
            let room_cache = room_cache.clone();
            move |room: RoomSummary| {
                let info = FolderInfo {
                    name: room.room_id.clone(),
                    delimiter: None,
                    attributes: if room.name.is_some() {
                        vec![format!(
                            "display_name={}",
                            room.name.as_deref().unwrap_or("")
                        )]
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

        // To-device E2EE (Olm room keys, etc.) requires a crypto backend; none is linked.
        let on_to_device: Arc<dyn Fn(String, String, String) + Send + Sync> =
            Arc::new(|_event_type, _sender, _content_json| {});

        let otk_count: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
        let device_lists_changed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let device_tracker = self.device_tracker.clone();
        let device_lists_clone = device_lists_changed.clone();

        // Full sync (`since: None`) so `rooms.join` lists every joined room. Incremental sync
        // only includes rooms that changed, which would yield an empty folder list on refresh.
        conn.send(MatrixCommand::Sync {
            token,
            since: None,
            own_user_id: Some(self.user_id.clone()),
            on_room,
            on_event,
            otk_count,
            device_lists_changed,
            on_to_device,
            on_complete: Box::new(move |result| {
                match result {
                    Ok(next_batch) => {
                        if let Some(nb) = next_batch {
                            if let Ok(mut st) = sync_token.lock() {
                                *st = Some(nb);
                            }
                        }

                        // E2EE: process device list changes
                        if let Ok(changed) = device_lists_clone.lock() {
                            if !changed.is_empty() {
                                device_tracker.mark_users_dirty(&changed);
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
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let token = match self.get_token() {
            Ok(t) => t,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };

        let folder: Box<dyn Folder> = Box::new(MatrixFolder {
            room_id: name.to_string(),
            store_uri: self.uri.clone(),
            homeserver: self.homeserver.clone(),
            user_id: self.user_id.clone(),
            token,
            connection: conn,
            crypto: self.get_crypto(),
        });
        on_complete(Ok(folder));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
    crypto: Option<Arc<CryptoMachine>>,
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
            in_reply_to: None,
            references: None,
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
        let crypto = self.crypto.clone();
        let room_id = self.room_id.clone();
        let on_event: Arc<dyn Fn(RoomEvent) + Send + Sync> = Arc::new(move |event| {
            if event.event_type == EVENT_ROOM_MESSAGE {
                on_summary(room_event_to_summary(&event));
            } else if event.event_type == EVENT_ROOM_ENCRYPTED {
                if let Some(decrypted) = try_decrypt_room_event(&crypto, &room_id, &event) {
                    on_summary(room_event_to_summary(&decrypted));
                } else {
                    let mut fallback = event.clone();
                    fallback.body = Some("[Encrypted message]".to_string());
                    fallback.event_type = EVENT_ROOM_MESSAGE.to_string();
                    on_summary(room_event_to_summary(&fallback));
                }
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

    fn message_count(&self, on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>) {
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
        let event_id = id_str
            .strip_prefix("matrix://")
            .and_then(|rest| rest.split('/').nth(1))
            .unwrap_or(id_str);

        let crypto = self.crypto.clone();
        let room_id_owned = self.room_id.clone();
        self.connection.send(MatrixCommand::GetEvent {
            token: self.token.clone(),
            room_id: self.room_id.clone(),
            event_id: event_id.to_string(),
            on_complete: Box::new(move |result| match result {
                Ok(Some(event)) => {
                    let display_event = if event.event_type == EVENT_ROOM_ENCRYPTED {
                        try_decrypt_room_event(&crypto, &room_id_owned, &event).unwrap_or_else(
                            || {
                                let mut fallback = event.clone();
                                fallback.body = Some("[Encrypted message]".to_string());
                                fallback.event_type = EVENT_ROOM_MESSAGE.to_string();
                                fallback
                            },
                        )
                    } else {
                        event
                    };
                    let summary = room_event_to_summary(&display_event);
                    on_metadata(summary.envelope);
                    if let Some(ref body) = display_event.body {
                        on_content_chunk(body.as_bytes());
                    }
                    on_complete(Ok(()));
                }
                Ok(None) => {
                    on_complete(Err(StoreError::new("Matrix event not found")));
                }
                Err(e) => on_complete(Err(e)),
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
    crypto: Arc<RwLock<Option<Arc<CryptoMachine>>>>,
    encrypted_rooms: Arc<RwLock<std::collections::HashSet<String>>>,
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
            crypto: Arc::new(RwLock::new(None)),
            encrypted_rooms: Arc::new(RwLock::new(std::collections::HashSet::new())),
        })
    }

    /// Set the crypto machine (shared with the store).
    pub fn set_crypto(&self, crypto: Arc<CryptoMachine>) {
        *self.crypto.write().unwrap() = Some(crypto);
    }

    /// Share the encrypted rooms set with the store.
    pub fn set_encrypted_rooms(&self, rooms: Arc<RwLock<std::collections::HashSet<String>>>) {
        *self.encrypted_rooms.write().unwrap() = rooms.read().unwrap().clone();
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn get_token(&self) -> Result<String, StoreError> {
        self.access_token
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.user_id.clone(),
                is_plaintext: false,
                advertised_capabilities: None,
            })
    }

    fn ensure_connection(&self) -> Result<MatrixConnection, StoreError> {
        let mut guard = self.connection.lock().unwrap();
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let conn = self
            .runtime_handle
            .block_on(connect_and_start_pipeline(&self.homeserver))?;
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

        let content_json = matrix_room_message_content_json(payload);
        let txn_id = self.next_txn_id();

        let is_encrypted = self.encrypted_rooms.read().unwrap().contains(&room_id);

        if is_encrypted {
            eprintln!(
                "[matrix] room {} is encrypted; sending plaintext (no Matrix E2EE backend)",
                room_id
            );
        }

        conn.send(MatrixCommand::SendMessage {
            token,
            room_id,
            body: content_json,
            txn_id,
            on_complete,
        });
    }
}

fn matrix_room_message_content_json(payload: &SendPayload) -> Vec<u8> {
    let plain = payload.body_plain.as_deref().unwrap_or("").trim();
    let html = payload
        .body_html
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match html {
        Some(h) => requests::build_formatted_text_message_body(plain, h),
        None => {
            let fallback = if plain.is_empty() {
                payload.body_html.as_deref().unwrap_or("")
            } else {
                plain
            };
            requests::build_text_message_body(fallback)
        }
    }
}

// ── Utility ──────────────────────────────────────────────────────────

/// Megolm decryption is not available without an E2EE backend.
fn try_decrypt_room_event(
    _crypto: &Option<Arc<CryptoMachine>>,
    _room_id: &str,
    _event: &RoomEvent,
) -> Option<RoomEvent> {
    None
}

/// Minimal JSON string extraction: find `"key":"value"` and return value.
pub(super) fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\"", key);
    let pos = json.find(&search)?;
    let rest = &json[pos + search.len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

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
