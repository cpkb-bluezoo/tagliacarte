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
pub mod crypto_store;
pub mod device;
pub mod encrypted_attachments;
pub mod json_handlers;
pub mod key_backup;
pub mod requests;
pub mod types;
pub mod verification;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::message_id::{self, MessageId};
use crate::store::{
    Address, ConversationSummary, DateTime, Envelope, Folder, FolderInfo, OpenFolderEvent,
    SendPayload, Store, StoreError, StoreKind, Transport, TransportKind,
};

use connection::{connect_and_start_pipeline, MatrixCommand, MatrixConnection};
use crypto::CryptoMachine;
use device::DeviceTracker;
use types::{
    RoomEvent, RoomSummary, EVENT_ROOM_ENCRYPTED, EVENT_ROOM_MESSAGE,
    ALGORITHM_MEGOLM,
};

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
        self.access_token.read().unwrap().clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.user_id.clone(),
                is_plaintext: false,
            })
    }

    /// Initialize the E2EE crypto machine. Called after login or credential set.
    pub fn init_crypto(&self, device_id: &str) -> Result<(), StoreError> {
        let token = self.get_token()?;
        *self.device_id.write().unwrap() = Some(device_id.to_string());
        let machine = Arc::new(CryptoMachine::new_or_load(
            &self.user_id, device_id, &token,
        )?);
        *self.crypto.write().unwrap() = Some(machine);
        Ok(())
    }

    pub fn get_crypto(&self) -> Option<Arc<CryptoMachine>> {
        self.crypto.read().unwrap().clone()
    }

    pub fn device_fingerprint(&self) -> Option<String> {
        self.get_crypto().map(|cm| cm.ed25519_key().to_base64())
    }

    pub fn access_token(&self) -> Option<String> {
        self.access_token.read().unwrap().clone()
    }

    /// Perform m.login.password, store resulting access_token and init crypto.
    pub fn login(&self, password: &str) -> Result<types::LoginResponse, StoreError> {
        eprintln!("[matrix] login: connecting to {}", self.homeserver);
        let conn = self.ensure_connection()?;
        eprintln!("[matrix] login: connected, sending login for {}", self.user_id);
        let (tx, rx) = std::sync::mpsc::channel();
        let user = self.user_id.clone();
        let pw = password.to_string();
        conn.send(MatrixCommand::Login {
            user,
            password: pw,
            on_complete: Box::new(move |result| {
                eprintln!("[matrix] login response: {}", if result.is_ok() { "ok" } else { "error" });
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
        let conn = self.runtime_handle.block_on(
            connect_and_start_pipeline(&self.homeserver)
        )?;
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
            on_complete: Box::new(move |r| { let _ = tx.send(r); }),
        });
        let version_info = rx.recv()
            .map_err(|_| StoreError::new("backup version channel closed"))??;
        let (version, algorithm) = match version_info {
            Some(v) => v,
            None => return Ok(0),
        };
        if algorithm != "m.megolm_backup.v1.curve25519-aes-sha2" {
            return Err(StoreError::new(format!("unsupported backup algorithm: {}", algorithm)));
        }
        let backup_version = version.clone();

        let (tx2, rx2) = std::sync::mpsc::channel();
        conn.send(MatrixCommand::DownloadRoomKeys {
            token,
            version,
            on_complete: Box::new(move |r| { let _ = tx2.send(r); }),
        });
        let body = rx2.recv()
            .map_err(|_| StoreError::new("download room keys channel closed"))??;
        let body_str = String::from_utf8_lossy(&body);

        let cm = self.get_crypto()
            .ok_or_else(|| StoreError::new("crypto not initialized"))?;
        let mut restored = 0usize;
        let parsed: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|e| StoreError::new(format!("parse backup: {}", e)))?;
        if let Some(rooms) = parsed.get("rooms").and_then(|v| v.as_object()) {
            for (room_id, room_val) in rooms {
                if let Some(sessions) = room_val.get("sessions").and_then(|v| v.as_object()) {
                    for (session_id, sess_val) in sessions {
                        let sd = match sess_val.get("session_data") {
                            Some(v) => v,
                            None => continue,
                        };
                        let ephemeral = match sd.get("ephemeral").and_then(|v| v.as_str()) {
                            Some(v) => v,
                            None => continue,
                        };
                        let ciphertext = match sd.get("ciphertext").and_then(|v| v.as_str()) {
                            Some(v) => v,
                            None => continue,
                        };
                        let mac = match sd.get("mac").and_then(|v| v.as_str()) {
                            Some(v) => v,
                            None => continue,
                        };
                        match key_backup::decrypt_backup_session(&recovery, ephemeral, ciphertext, mac) {
                            Ok(plaintext) => {
                                let pt_str = String::from_utf8_lossy(&plaintext);
                                if let Some(session_key_b64) = extract_json_string(&pt_str, "session_key") {
                                    match vodozemac::megolm::SessionKey::from_base64(&session_key_b64) {
                                        Ok(sk) => {
                                            if let Err(e) = cm.add_inbound_group_session(room_id, session_id, &sk) {
                                                eprintln!("[matrix] restore session {}/{}: {}", room_id, session_id, e);
                                            } else {
                                                restored += 1;
                                            }
                                        }
                                        Err(e) => eprintln!("[matrix] invalid session key {}/{}: {}", room_id, session_id, e),
                                    }
                                }
                            }
                            Err(e) => eprintln!("[matrix] decrypt backup session {}/{}: {}", room_id, session_id, e),
                        }
                    }
                }
            }
        }
        eprintln!("[matrix] restored {} sessions from backup", restored);
        *self.backup_info.write().unwrap() = Some((backup_version, recovery));
        Ok(restored)
    }

    /// Upload a single session key to the server backup (if backup is active).
    pub fn upload_session_to_backup(
        &self,
        room_id: &str,
        session_id: &str,
        session_key_b64: &str,
    ) {
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

        // E2EE: capture to-device events and OTK counts from sync
        let crypto = self.get_crypto();
        let crypto_for_to_device = crypto.clone();
        let backup_for_td = self.backup_info.clone();
        let conn_for_td = conn.clone();
        let token_for_td = token.clone();
        let on_to_device: Arc<dyn Fn(String, String, String) + Send + Sync> = Arc::new(move |event_type, sender, content_json| {
            if let Some(ref cm) = crypto_for_to_device {
                process_to_device_event(cm, &event_type, &sender, &content_json,
                    &backup_for_td, &conn_for_td, &token_for_td);
            }
        });

        let otk_count: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
        let device_lists_changed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let conn_for_keys = conn.clone();
        let token_for_keys = token.clone();
        let crypto_for_keys = crypto.clone();
        let device_tracker = self.device_tracker.clone();
        let otk_count_clone = otk_count.clone();
        let device_lists_clone = device_lists_changed.clone();

        conn.send(MatrixCommand::Sync {
            token,
            since,
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

                        // E2EE: upload keys if needed
                        if let Some(ref cm) = crypto_for_keys {
                            let count = otk_count_clone.lock().unwrap().take();
                            let otk = cm.generate_one_time_keys_if_needed(count.unwrap_or(0));
                            if !otk.is_empty() {
                                let otk_json = cm.one_time_keys_json(&otk);
                                let body = device::build_keys_upload_body(None, Some(&otk_json));
                                let cm_clone = cm.clone();
                                conn_for_keys.send(MatrixCommand::UploadKeys {
                                    token: token_for_keys.clone(),
                                    body,
                                    on_complete: Box::new(move |result| {
                                        if result.is_ok() {
                                            cm_clone.mark_keys_as_published();
                                        }
                                    }),
                                });
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

        let crypto = self.crypto.clone();
        let room_id_owned = self.room_id.clone();
        self.connection.send(MatrixCommand::GetEvent {
            token: self.token.clone(),
            room_id: self.room_id.clone(),
            event_id: event_id.to_string(),
            on_complete: Box::new(move |result| {
                match result {
                    Ok(Some(event)) => {
                        let display_event = if event.event_type == EVENT_ROOM_ENCRYPTED {
                            try_decrypt_room_event(&crypto, &room_id_owned, &event)
                                .unwrap_or_else(|| {
                                    let mut fallback = event.clone();
                                    fallback.body = Some("[Encrypted message]".to_string());
                                    fallback.event_type = EVENT_ROOM_MESSAGE.to_string();
                                    fallback
                                })
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
        self.access_token.read().unwrap().clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.user_id.clone(),
                is_plaintext: false,
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
        let txn_id = self.next_txn_id();

        // Check if room is encrypted and we have crypto
        let is_encrypted = self.encrypted_rooms.read().unwrap().contains(&room_id);
        let crypto = self.crypto.read().unwrap().clone();

        if is_encrypted {
            if let Some(ref cm) = crypto {
                let content_json = requests::build_text_message_body(body_text);
                match cm.megolm_encrypt(&room_id, "m.room.message", &content_json) {
                    Ok(encrypted) => {
                        let body = requests::build_encrypted_event_body(&encrypted);
                        conn.send(MatrixCommand::SendMessage {
                            token,
                            room_id,
                            body,
                            txn_id,
                            on_complete,
                        });
                        return;
                    }
                    Err(e) => {
                        eprintln!("[matrix] encrypt failed, sending plaintext: {}", e);
                    }
                }
            }
        }

        let body = requests::build_text_message_body(body_text);
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

type BackupInfoRef = Arc<RwLock<Option<(String, key_backup::RecoveryKey)>>>;

/// Process a to-device event from sync (E2EE key sharing, etc.).
fn process_to_device_event(
    crypto: &Arc<CryptoMachine>,
    event_type: &str,
    sender: &str,
    content_json: &str,
    backup_info: &BackupInfoRef,
    conn: &MatrixConnection,
    token: &str,
) {
    match event_type {
        "m.room.encrypted" => {
            if let Some((algorithm, sender_key, ciphertext_type, ciphertext_body)) =
                parse_olm_to_device(content_json)
            {
                if algorithm != types::ALGORITHM_OLM {
                    return;
                }
                let sender_curve = match vodozemac::Curve25519PublicKey::from_base64(&sender_key) {
                    Ok(k) => k,
                    Err(_) => return,
                };
                let olm_msg = match vodozemac::olm::OlmMessage::from_parts(
                    ciphertext_type as usize,
                    ciphertext_body.as_bytes(),
                ) {
                    Ok(m) => m,
                    Err(_) => return,
                };

                match crypto.olm_decrypt(&sender_curve, &olm_msg) {
                    Ok(plaintext) => {
                        let pt_str = String::from_utf8_lossy(&plaintext);
                        process_decrypted_to_device(crypto, &pt_str, backup_info, conn, token);
                    }
                    Err(e) => eprintln!("[matrix] failed to decrypt to-device from {}: {}", sender, e),
                }
            }
        }
        "m.room_key" => {
            process_room_key_event(crypto, content_json, backup_info, conn, token);
        }
        _ => {}
    }
}

/// Parse an Olm-encrypted to-device event's content.
/// Returns (algorithm, sender_key, ciphertext_type, ciphertext_body).
fn parse_olm_to_device(content_json: &str) -> Option<(String, String, u64, String)> {
    // Minimal JSON extraction without serde
    let algorithm = extract_json_string(content_json, "algorithm")?;
    let sender_key = extract_json_string(content_json, "sender_key")?;
    // The ciphertext object contains our key; we need to find our curve25519 key's entry
    // For simplicity, extract the first ciphertext entry
    let ct_start = content_json.find("\"ciphertext\"")?;
    let rest = &content_json[ct_start..];
    // Find the "type" and "body" inside the ciphertext nested object
    let msg_type_str = extract_json_string(rest, "type")
        .or_else(|| extract_json_number(rest, "type"))?;
    let msg_type: u64 = msg_type_str.parse().unwrap_or(0);
    let body = extract_json_string(rest, "body")?;
    Some((algorithm, sender_key, msg_type, body))
}

/// Process a decrypted to-device message (inner plaintext from Olm).
fn process_decrypted_to_device(
    crypto: &Arc<CryptoMachine>,
    plaintext: &str,
    backup_info: &BackupInfoRef,
    conn: &MatrixConnection,
    token: &str,
) {
    if let Some(event_type) = extract_json_string(plaintext, "type") {
        if event_type == "m.room_key" {
            if let Some(content_start) = plaintext.find("\"content\"") {
                let rest = &plaintext[content_start..];
                process_room_key_event(crypto, rest, backup_info, conn, token);
            }
        }
    }
}

/// Process an m.room_key event content — add the inbound Megolm session.
fn process_room_key_event(
    crypto: &Arc<CryptoMachine>,
    content_json: &str,
    backup_info: &BackupInfoRef,
    conn: &MatrixConnection,
    token: &str,
) {
    let algorithm = match extract_json_string(content_json, "algorithm") {
        Some(a) => a,
        None => return,
    };
    if algorithm != ALGORITHM_MEGOLM {
        return;
    }
    let room_id = match extract_json_string(content_json, "room_id") {
        Some(r) => r,
        None => return,
    };
    let session_id = match extract_json_string(content_json, "session_id") {
        Some(s) => s,
        None => return,
    };
    let session_key_b64 = match extract_json_string(content_json, "session_key") {
        Some(k) => k,
        None => return,
    };
    let session_key = match vodozemac::megolm::SessionKey::from_base64(&session_key_b64) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[matrix] invalid room key session_key: {}", e);
            return;
        }
    };
    if let Err(e) = crypto.add_inbound_group_session(&room_id, &session_id, &session_key) {
        eprintln!("[matrix] failed to add inbound group session: {}", e);
    } else {
        eprintln!("[matrix] added inbound group session for room={} session={}", room_id, session_id);
        // Upload to server backup if active
        if let Ok(info) = backup_info.read() {
            if let Some((version, recovery)) = info.as_ref() {
                let plaintext = format!(
                    "{{\"algorithm\":\"m.megolm.v1.aes-sha2\",\"room_id\":\"{}\",\"session_id\":\"{}\",\"session_key\":\"{}\"}}",
                    room_id, session_id, session_key_b64
                );
                if let Ok(encrypted) = key_backup::encrypt_backup_session(recovery, plaintext.as_bytes()) {
                    let mut sessions = std::collections::HashMap::new();
                    let mut room_sessions = std::collections::HashMap::new();
                    room_sessions.insert(session_id.clone(), encrypted);
                    sessions.insert(room_id.clone(), room_sessions);
                    let body = key_backup::build_upload_room_keys_body(&sessions);
                    conn.send(MatrixCommand::UploadRoomKeys {
                        token: token.to_string(),
                        version: version.clone(),
                        body,
                        on_complete: Box::new(|result| {
                            if let Err(e) = result {
                                eprintln!("[matrix] auto backup upload failed: {}", e);
                            }
                        }),
                    });
                }
            }
        }
    }
}

/// Try to decrypt an `m.room.encrypted` event into a plaintext `RoomEvent`.
fn try_decrypt_room_event(
    crypto: &Option<Arc<CryptoMachine>>,
    room_id: &str,
    event: &RoomEvent,
) -> Option<RoomEvent> {
    let cm = crypto.as_ref()?;

    let algorithm = event.algorithm.as_deref()?;
    if algorithm != ALGORITHM_MEGOLM {
        return None;
    }

    let session_id = event.session_id.as_deref()?;
    let ciphertext = event.ciphertext.as_deref()?;

    match cm.megolm_decrypt(room_id, session_id, ciphertext) {
        Ok(decrypted) => {
            let plaintext = String::from_utf8_lossy(&decrypted.plaintext);
            let body = extract_json_string(&plaintext, "body");
            let msgtype = extract_json_string(&plaintext, "msgtype");
            let url = extract_json_string(&plaintext, "url");
            Some(RoomEvent {
                event_id: event.event_id.clone(),
                event_type: extract_json_string(&plaintext, "type")
                    .unwrap_or_else(|| EVENT_ROOM_MESSAGE.to_string()),
                sender: event.sender.clone(),
                origin_server_ts: event.origin_server_ts,
                body,
                msgtype,
                url,
                room_id: event.room_id.clone(),
                algorithm: None,
                sender_key: None,
                session_id: None,
                ciphertext: None,
                device_id: None,
            })
        }
        Err(e) => {
            eprintln!("[matrix] megolm decrypt failed for session={}: {}", session_id, e);
            None
        }
    }
}

/// Minimal JSON string extraction: find `"key":"value"` and return value.
pub(super) fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\"", key);
    let pos = json.find(&search)?;
    let rest = &json[pos + search.len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') { return None; }
    let rest = rest[1..].trim_start();
    if !rest.starts_with('"') { return None; }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Extract a JSON number value as a string.
fn extract_json_number(json: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\"", key);
    let pos = json.find(&search)?;
    let rest = &json[pos + search.len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') { return None; }
    let rest = rest[1..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-' && c != '.')?;
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
