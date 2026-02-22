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

//! IMAP4rev2 client (Store + Folder). Persistent connection with idle timeout and reconnect.
//! Store and folders share one session via ImapStoreState.
//!
//! All trait methods are fully callback-driven and return immediately.

mod client;

pub use client::{
    connect_and_authenticate, connect_and_start_pipeline, AuthenticatedSession, FetchSummary,
    ImapClientError, ImapConnection, ImapLine, ImapLineWithLiteral, ListEntry, SelectEvent,
    SelectResult,
};

use crate::message_id::{imap_message_id, MessageId};
use crate::mime::{parse_envelope, parse_thread_headers, EmailAddress, EnvelopeHeaders};
use crate::store::{Address, ConversationSummary, DateTime, Envelope, Flag};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::{ThreadId, ThreadSummary};
use crate::sasl::SaslMechanism;
use std::ops::Range;
use std::sync::{Arc, Mutex, RwLock};

/// IMAP delete mode: how the delete button works for IMAP folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImapDeleteMode {
    /// Mark messages with \Deleted flag (user can expunge later).
    MarkDeleted,
    /// Copy to trash folder, mark \Deleted on source, then UID EXPUNGE source.
    MoveToTrash,
}

/// Shared state for IMAP: persistent connection via async pipeline. Store and folders hold Arc<this>.
struct ImapStoreState {
    host: String,
    port: u16,
    use_implicit_tls: RwLock<bool>,
    use_starttls: RwLock<bool>,
    auth: RwLock<Option<(String, String, SaslMechanism)>>,
    username: RwLock<String>,
    /// Handle to the shared tokio runtime (set by FFI layer at creation).
    runtime_handle: tokio::runtime::Handle,
    /// Live connection to the IMAP server (pipeline task).
    connection: Mutex<Option<ImapConnection>>,
    /// Cached hierarchy delimiter from LIST responses.
    cached_delimiter: Mutex<Option<char>>,
    /// Registered callbacks for folder list events.
    folder_list_callbacks: RwLock<Option<FolderListCallbacksInternal>>,
    /// Delete mode for this IMAP store.
    delete_mode: RwLock<ImapDeleteMode>,
    /// Trash folder name for move-to-trash deletion (e.g. "Trash").
    trash_folder: RwLock<String>,
}

/// Internal folder list callbacks stored in ImapStoreState.
#[derive(Clone)]
struct FolderListCallbacksInternal {
    on_folder_found: Arc<dyn Fn(FolderInfo) + Send + Sync>,
    on_folder_removed: Arc<dyn Fn(&str) + Send + Sync>,
}

impl ImapStoreState {
    /// Ensure a live connection exists and return a clone of the ImapConnection handle.
    fn ensure_connection(&self) -> Result<ImapConnection, StoreError> {
        let mut guard = self.connection.lock().map_err(|e| StoreError::new(e.to_string()))?;
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        // Need to connect: build auth and spawn the pipeline
        let host = self.host.clone();
        let port = self.port;
        let use_implicit_tls = *self.use_implicit_tls.read().map_err(|e| StoreError::new(e.to_string()))?;
        let use_starttls = *self.use_starttls.read().map_err(|e| StoreError::new(e.to_string()))?;
        let auth = self.auth.read().map_err(|e| StoreError::new(e.to_string()))?.clone();
        if auth.is_none() {
            let username = self.username.read().map_err(|e| StoreError::new(e.to_string()))?.clone();
            let is_plaintext = !use_implicit_tls && !use_starttls;
            return Err(StoreError::NeedsCredential { username, is_plaintext });
        }
        let (user, pass, mechanism) = auth.unwrap();

        // Use block_on on the shared runtime to connect and authenticate.
        // This is called from the FFI layer (UI thread) but only once per store
        // when the connection needs to be established.
        let conn = self.runtime_handle.block_on(async move {
            connect_and_start_pipeline(
                &host,
                port,
                use_implicit_tls,
                use_starttls,
                Some((&user, &pass, mechanism)),
            )
            .await
            .map_err(|e| StoreError::new(e.to_string()))
        })?;
        *guard = Some(conn.clone());
        Ok(conn)
    }
}

/// IMAP store. Holds persistent client (connection reuse, idle timeout, reconnect).
pub struct ImapStore {
    state: Arc<ImapStoreState>,
}

impl ImapStore {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self::with_runtime_handle(host, port, tokio::runtime::Handle::current())
    }

    /// Create an ImapStore with an explicit tokio runtime handle (used by FFI with the shared runtime).
    pub fn with_runtime_handle(host: impl Into<String>, port: u16, handle: tokio::runtime::Handle) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 993;
        let state = ImapStoreState {
            host: host.clone(),
            port,
            use_implicit_tls: RwLock::new(use_implicit_tls),
            use_starttls: RwLock::new(true),
            auth: RwLock::new(None),
            username: RwLock::new(String::new()),
            runtime_handle: handle,
            connection: Mutex::new(None),
            cached_delimiter: Mutex::new(None),
            folder_list_callbacks: RwLock::new(None),
            delete_mode: RwLock::new(ImapDeleteMode::MoveToTrash),
            trash_folder: RwLock::new("Trash".to_string()),
        };
        Self {
            state: Arc::new(state),
        }
    }

    pub fn set_implicit_tls(&mut self, use_tls: bool) -> &mut Self {
        *self.state.use_implicit_tls.write().unwrap() = use_tls;
        self
    }

    pub fn set_use_starttls(&mut self, use_starttls: bool) -> &mut Self {
        *self.state.use_starttls.write().unwrap() = use_starttls;
        self
    }

    pub fn set_auth(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
        mechanism: SaslMechanism,
    ) -> &mut Self {
        let u = username.into();
        if self.state.username.read().unwrap().is_empty() {
            *self.state.username.write().unwrap() = u.clone();
        }
        *self.state.auth.write().unwrap() = Some((u, password.into(), mechanism));
        self
    }

    /// Set OAuth2 access token for XOAUTH2 authentication (Gmail, Outlook).
    /// `email` is the user's email address; `access_token` is the OAuth2 bearer token.
    pub fn set_oauth_token(
        &mut self,
        email: impl Into<String>,
        access_token: impl Into<String>,
    ) -> &mut Self {
        let e = email.into();
        if self.state.username.read().unwrap().is_empty() {
            *self.state.username.write().unwrap() = e.clone();
        }
        // For XOAUTH2, the "password" slot carries the access token.
        *self.state.auth.write().unwrap() = Some((e, access_token.into(), SaslMechanism::XOAuth2));
        self
    }

    pub fn set_username(&mut self, user_at_host: impl Into<String>) -> &mut Self {
        *self.state.username.write().unwrap() = user_at_host.into();
        self
    }

    /// Username (authcid) for this store, for credential request callback.
    pub fn username(&self) -> String {
        self.state.username.read().unwrap().clone()
    }

    /// Configure the delete mode for this IMAP store.
    pub fn set_delete_mode(&mut self, mode: ImapDeleteMode) -> &mut Self {
        *self.state.delete_mode.write().unwrap() = mode;
        self
    }

    /// Configure the trash folder name for move-to-trash deletion.
    pub fn set_trash_folder(&mut self, name: impl Into<String>) -> &mut Self {
        *self.state.trash_folder.write().unwrap() = name.into();
        self
    }

    /// Set folder list callbacks for reactive UI updates (create/rename/delete).
    pub fn set_folder_list_callbacks(
        &self,
        on_folder_found: Arc<dyn Fn(FolderInfo) + Send + Sync>,
        on_folder_removed: Arc<dyn Fn(&str) + Send + Sync>,
    ) {
        *self.state.folder_list_callbacks.write().unwrap() = Some(FolderListCallbacksInternal {
            on_folder_found,
            on_folder_removed,
        });
    }
}

impl Store for ImapStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn set_credential(&self, username: Option<&str>, password: &str) {
        let u = username
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.state.username.read().unwrap().clone());
        if u.is_empty() {
            return;
        }
        // Preserve existing mechanism if set (e.g. XOAuth2); default to SCRAM-SHA-256.
        let existing_mechanism = self.state.auth.read().unwrap()
            .as_ref()
            .map(|(_, _, m)| *m)
            .unwrap_or(SaslMechanism::ScramSha256);
        *self.state.auth.write().unwrap() = Some((u, password.to_string(), existing_mechanism));
    }

    fn set_oauth_credential(&self, email: &str, token: &str) {
        *self.state.username.write().unwrap() = email.to_string();
        *self.state.auth.write().unwrap() = Some((
            email.to_string(),
            token.to_string(),
            SaslMechanism::XOAuth2,
        ));
        // Drop stale connection so next operation reconnects with the new token.
        if let Ok(mut guard) = self.state.connection.lock() {
            *guard = None;
        }
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let delim_state = Arc::clone(&self.state);

        conn.list_folders_streaming(
            move |entry| {
                // Cache delimiter from first entry
                if let Some(d) = entry.delimiter {
                    if let Ok(mut guard) = delim_state.cached_delimiter.lock() {
                        if guard.is_none() {
                            *guard = Some(d);
                        }
                    }
                }
                on_folder(FolderInfo {
                    name: entry.name.clone(),
                    delimiter: entry.delimiter,
                    attributes: entry.attributes.clone(),
                });
            },
            move |result| {
                on_complete(result.map_err(|e| StoreError::new(e.to_string())));
            },
        );
    }

    fn open_folder(
        &self,
        name: &str,
        on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let name_owned = name.to_string();
        let state = Arc::clone(&self.state);
        let host = self.state.host.clone();
        let username = if self.state.username.read().unwrap().is_empty() {
            self.state
                .auth
                .read()
                .unwrap()
                .as_ref()
                .map(|(u, _p, _m)| u.clone())
                .unwrap_or_default()
        } else {
            self.state.username.read().unwrap().clone()
        };
        let user_at_host = if username.contains('@') {
            username
        } else {
            format!("{}@{}", username, host)
        };

        conn.select_streaming(
            name,
            move |ev| {
                let open_ev = match ev {
                    SelectEvent::Exists(n) => OpenFolderEvent::Exists(n),
                    SelectEvent::Recent(n) => OpenFolderEvent::Recent(n),
                    SelectEvent::Flags(f) => OpenFolderEvent::Flags(f),
                    SelectEvent::UidValidity(n) => OpenFolderEvent::UidValidity(n),
                    SelectEvent::UidNext(n) => OpenFolderEvent::UidNext(n),
                    SelectEvent::PermanentFlags(f) => OpenFolderEvent::Flags(f),
                    SelectEvent::Other(s) => OpenFolderEvent::Other(s),
                };
                on_event(open_ev);
            },
            move |result| {
                match result {
                    Ok(select_result) => {
                        let folder = Box::new(ImapFolder {
                            state,
                            user_at_host,
                            mailbox: name_owned,
                            exists: select_result.exists,
                        }) as Box<dyn Folder>;
                        on_complete(Ok(folder));
                    }
                    Err(e) => {
                        on_complete(Err(StoreError::new(e.to_string())));
                    }
                }
            },
        );
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        self.state.cached_delimiter.lock().ok().and_then(|g| *g).or(Some('/'))
    }

    fn default_folder(&self) -> Option<&str> {
        Some("INBOX")
    }

    fn create_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let name_owned = name.to_string();
        let callbacks = self.state.folder_list_callbacks.read().ok().and_then(|g| g.clone());
        let delimiter = self.hierarchy_delimiter();

        conn.create_mailbox(name, move |result| {
            match result {
                Ok(()) => {
                    // Fire on_folder_found so the UI reactively adds the item
                    if let Some(ref cbs) = callbacks {
                        (cbs.on_folder_found)(FolderInfo {
                            name: name_owned,
                            delimiter,
                            attributes: vec![],
                        });
                    }
                    on_complete(Ok(()));
                }
                Err(e) => {
                    on_complete(Err(StoreError::new(e.to_string())));
                }
            }
        });
    }

    fn rename_folder(
        &self,
        old_name: &str,
        new_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let old_owned = old_name.to_string();
        let new_owned = new_name.to_string();
        let callbacks = self.state.folder_list_callbacks.read().ok().and_then(|g| g.clone());
        let delimiter = self.hierarchy_delimiter();

        conn.rename_mailbox(old_name, new_name, move |result| {
            match result {
                Ok(()) => {
                    if let Some(ref cbs) = callbacks {
                        (cbs.on_folder_removed)(&old_owned);
                        (cbs.on_folder_found)(FolderInfo {
                            name: new_owned,
                            delimiter,
                            attributes: vec![],
                        });
                    }
                    on_complete(Ok(()));
                }
                Err(e) => {
                    on_complete(Err(StoreError::new(e.to_string())));
                }
            }
        });
    }

    fn delete_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let name_owned = name.to_string();
        let callbacks = self.state.folder_list_callbacks.read().ok().and_then(|g| g.clone());

        conn.delete_mailbox(name, move |result| {
            match result {
                Ok(()) => {
                    if let Some(ref cbs) = callbacks {
                        (cbs.on_folder_removed)(&name_owned);
                    }
                    on_complete(Ok(()));
                }
                Err(e) => {
                    on_complete(Err(StoreError::new(e.to_string())));
                }
            }
        });
    }

    fn set_delete_config(&self, mode: i32, trash_folder: &str) {
        let dm = if mode == 0 {
            ImapDeleteMode::MarkDeleted
        } else {
            ImapDeleteMode::MoveToTrash
        };
        *self.state.delete_mode.write().unwrap() = dm;
        if !trash_folder.is_empty() {
            *self.state.trash_folder.write().unwrap() = trash_folder.to_string();
        }
    }
}

/// Folder backed by IMAP; uses store's persistent session.
struct ImapFolder {
    state: Arc<ImapStoreState>,
    user_at_host: String,
    mailbox: String,
    exists: u32,
}

impl Folder for ImapFolder {
    fn list_conversations(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let exists = self.exists;
        let start = ((range.start + 1).min(exists as u64 + 1)) as u32;
        let end = (range.end.min(exists as u64)) as u32;
        if start > end {
            on_complete(Ok(()));
            return;
        }
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let user = self.user_at_host.clone();
        let mailbox_name = self.mailbox.clone();

        conn.fetch_summaries_streaming(
            start,
            end,
            move |s| {
                let envelope = envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
                let id = imap_message_id(&user, &mailbox_name, s.uid);
                let flags = imap_flags_to_store(&s.flags);
                on_summary(ConversationSummary {
                    id,
                    envelope,
                    flags,
                    size: s.size as u64,
                });
            },
            move |result| {
                on_complete(result.map_err(|e| StoreError::new(e.to_string())));
            },
        );
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        on_complete(Ok(self.exists as u64));
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let uid = match parse_uid_from_imap_id(id) {
            Some(u) => u,
            None => {
                on_complete(Err(StoreError::new("invalid message id")));
                return;
            }
        };
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };

        let header_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let buf = Arc::new(Mutex::new(Vec::new()));
        let header_done_for_chunk = header_done.clone();
        let buf_for_chunk = buf.clone();
        let on_metadata = Arc::new(on_metadata);
        let on_content_chunk = Arc::new(on_content_chunk);
        let on_metadata_for_chunk = on_metadata.clone();
        let on_content_chunk_for_chunk = on_content_chunk.clone();

        conn.fetch_body_by_uid_streaming(
            uid,
            move |chunk| {
                if !header_done_for_chunk.load(std::sync::atomic::Ordering::Relaxed) {
                    let mut guard = buf_for_chunk.lock().unwrap();
                    guard.extend_from_slice(chunk);
                    if let Some(sep) = guard.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header_bytes = guard[..sep + 4].to_vec();
                        let body_start = guard[sep + 4..].to_vec();
                        if let Ok(env) = envelope_from_raw(&header_bytes) {
                            on_metadata_for_chunk(env);
                        } else {
                            on_metadata_for_chunk(default_envelope());
                        }
                        on_content_chunk_for_chunk(&header_bytes);
                        if !body_start.is_empty() {
                            on_content_chunk_for_chunk(&body_start);
                        }
                        header_done_for_chunk.store(true, std::sync::atomic::Ordering::Relaxed);
                        guard.clear();
                    }
                } else {
                    on_content_chunk_for_chunk(chunk);
                }
            },
            move |result| {
                if !header_done.load(std::sync::atomic::Ordering::Relaxed) {
                    let guard = buf.lock().unwrap();
                    if !guard.is_empty() {
                        if let Ok(env) = envelope_from_raw(&guard) {
                            on_metadata(env);
                        } else {
                            on_metadata(default_envelope());
                        }
                        on_content_chunk(&guard);
                    }
                }
                on_complete(result.map_err(|e| StoreError::new(e.to_string())));
            },
        );
    }

    fn delete_message(
        &self,
        id: &MessageId,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let uid = match parse_uid_from_imap_id(id) {
            Some(u) => u,
            None => {
                on_complete(Err(StoreError::new("invalid message id")));
                return;
            }
        };
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let uid_set = uid.to_string();
        let delete_mode = *self.state.delete_mode.read().unwrap();
        let trash_folder = self.state.trash_folder.read().unwrap().clone();

        match delete_mode {
            ImapDeleteMode::MarkDeleted => {
                // Just set \Deleted flag; no expunge
                conn.store_flags(&uid_set, r"+FLAGS (\Deleted)", move |result| {
                    on_complete(result.map_err(|e| StoreError::new(e.to_string())));
                });
            }
            ImapDeleteMode::MoveToTrash => {
                // 1. UID COPY to trash (messages arrive clean, without \Deleted)
                // 2. UID STORE +FLAGS (\Deleted) on source
                // 3. UID EXPUNGE source UIDs
                let uid_set2 = uid_set.clone();
                let uid_set3 = uid_set.clone();
                let conn2 = conn.clone();
                let conn3 = conn.clone();
                conn.copy_uids(&uid_set, &trash_folder, move |copy_result| {
                    match copy_result {
                        Ok(()) => {
                            conn2.store_flags(&uid_set2, r"+FLAGS (\Deleted)", move |store_result| {
                                match store_result {
                                    Ok(()) => {
                                        conn3.uid_expunge(&uid_set3, move |exp_result| {
                                            on_complete(exp_result.map_err(|e| StoreError::new(e.to_string())));
                                        });
                                    }
                                    Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
                                }
                            });
                        }
                        Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
                    }
                });
            }
        }
    }

    fn list_threads(
        &self,
        range: Range<u64>,
        on_thread: Box<dyn Fn(ThreadSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let exists = self.exists;
        if exists == 0 {
            on_complete(Ok(()));
            return;
        }
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let summaries = Arc::new(Mutex::new(Vec::new()));
        let summaries_cb = summaries.clone();

        conn.fetch_summaries_streaming(1, exists, move |s| {
            if let Ok(mut guard) = summaries_cb.lock() {
                guard.push(s);
            }
        }, move |result| {
            match result.map_err(|e| StoreError::new(e.to_string())) {
                Ok(()) => {
                    let summaries = summaries.lock().unwrap();
                    let mut thread_groups: std::collections::HashMap<String, (Option<String>, u64)> =
                        std::collections::HashMap::new();
                    for s in summaries.iter() {
                        let th = parse_thread_headers(&s.header).unwrap_or_default();
                        let root = th
                            .references
                            .first()
                            .cloned()
                            .or(th.message_id.clone())
                            .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
                        let entry = thread_groups
                            .entry(root)
                            .or_insert((th.subject.clone(), 0));
                        entry.1 += 1;
                    }
                    let mut threads: Vec<(String, Option<String>, u64)> = thread_groups
                        .into_iter()
                        .map(|(id, (subject, count))| (id, subject, count))
                        .collect();
                    threads.sort_by(|a, b| a.0.cmp(&b.0));
                    let start = range.start.min(threads.len() as u64) as usize;
                    let end = range.end.min(threads.len() as u64) as usize;
                    for t in threads.into_iter().skip(start).take(end.saturating_sub(start)) {
                        on_thread(ThreadSummary {
                            id: ThreadId(t.0),
                            subject: t.1,
                            message_count: t.2,
                        });
                    }
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(e)),
            }
        });
    }

    fn list_messages_in_thread(
        &self,
        thread_id: &ThreadId,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let exists = self.exists;
        if exists == 0 {
            on_complete(Ok(()));
            return;
        }
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let user = self.user_at_host.clone();
        let mailbox = self.mailbox.clone();
        let thread_id_str = thread_id.as_str().to_string();
        let summaries = Arc::new(Mutex::new(Vec::new()));
        let summaries_cb = summaries.clone();

        conn.fetch_summaries_streaming(1, exists, move |s| {
            if let Ok(mut guard) = summaries_cb.lock() {
                guard.push(s);
            }
        }, move |result| {
            match result.map_err(|e| StoreError::new(e.to_string())) {
                Ok(()) => {
                    let summaries = summaries.lock().unwrap();
                    let mut in_thread = Vec::new();
                    for s in summaries.iter() {
                        let th = parse_thread_headers(&s.header).unwrap_or_default();
                        let root = th
                            .references
                            .first()
                            .cloned()
                            .or(th.message_id.clone())
                            .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
                        if root != thread_id_str {
                            continue;
                        }
                        let envelope = envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
                        let id = imap_message_id(&user, &mailbox, s.uid);
                        let flags = imap_flags_to_store(&s.flags);
                        in_thread.push(ConversationSummary {
                            id,
                            envelope,
                            flags,
                            size: s.size as u64,
                        });
                    }
                    in_thread.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
                    let start = range.start.min(in_thread.len() as u64) as usize;
                    let end = range.end.min(in_thread.len() as u64) as usize;
                    for s in in_thread.into_iter().skip(start).take(end.saturating_sub(start)) {
                        on_summary(s);
                    }
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(e)),
            }
        });
    }

    fn append_message(
        &self,
        _data: &[u8],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // APPEND requires literal syntax which the pipeline model handles differently.
        // For now, return an error; this can be enhanced later.
        on_complete(Err(StoreError::new("APPEND via pipeline not yet supported")));
    }

    fn copy_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let uids: Vec<u32> = ids
            .iter()
            .filter_map(|id| {
                let mid = MessageId::new(*id);
                parse_uid_from_imap_id(&mid)
            })
            .collect();
        if uids.is_empty() {
            on_complete(Err(StoreError::new("no valid UIDs to copy")));
            return;
        }
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let uid_set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        conn.copy_uids(&uid_set, dest_folder_name, move |result| {
            on_complete(result.map_err(|e| StoreError::new(e.to_string())));
        });
    }

    fn move_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let uids: Vec<u32> = ids
            .iter()
            .filter_map(|id| {
                let mid = MessageId::new(*id);
                parse_uid_from_imap_id(&mid)
            })
            .collect();
        if uids.is_empty() {
            on_complete(Err(StoreError::new("no valid UIDs to move")));
            return;
        }
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let uid_set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        let dest = dest_folder_name.to_string();
        let uid_set_for_fallback = uid_set.clone();
        let dest_for_fallback = dest.clone();
        let conn_for_fallback = conn.clone();
        // Try UID MOVE first (RFC 6851)
        conn.move_uids(&uid_set, &dest, move |result| {
            match result {
                Ok(()) => on_complete(Ok(())),
                Err(_) => {
                    // Fallback: UID COPY + STORE \Deleted + EXPUNGE
                    let uid_set2 = uid_set_for_fallback.clone();
                    let conn2 = conn_for_fallback.clone();
                    conn_for_fallback.copy_uids(&uid_set_for_fallback, &dest_for_fallback, move |copy_result| {
                        match copy_result {
                            Ok(()) => {
                                let uid_set3 = uid_set2.clone();
                                let conn3 = conn2.clone();
                                conn2.store_flags(&uid_set2, r"+FLAGS (\Deleted)", move |store_result| {
                                    match store_result {
                                        Ok(()) => {
                                            conn3.uid_expunge(&uid_set3, move |exp_result| {
                                                on_complete(exp_result.map_err(|e| StoreError::new(e.to_string())));
                                            });
                                        }
                                        Err(e) => {
                                            on_complete(Err(StoreError::new(e.to_string())));
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                on_complete(Err(StoreError::new(e.to_string())));
                            }
                        }
                    });
                }
            }
        });
    }

    fn store_flags(
        &self,
        ids: &[&str],
        add: &[Flag],
        remove: &[Flag],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let uids: Vec<u32> = ids
            .iter()
            .filter_map(|id| {
                let mid = MessageId::new(*id);
                parse_uid_from_imap_id(&mid)
            })
            .collect();
        if uids.is_empty() {
            on_complete(Err(StoreError::new("no valid UIDs for store_flags")));
            return;
        }
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let uid_set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");

        // We may need to issue two commands: one for add, one for remove.
        // Use sequential chaining if both are non-empty.
        let add_flags: Vec<String> = add.iter().map(flag_to_imap_string).collect();
        let remove_flags: Vec<String> = remove.iter().map(flag_to_imap_string).collect();

        let has_add = !add_flags.is_empty();
        let has_remove = !remove_flags.is_empty();

        if !has_add && !has_remove {
            on_complete(Ok(()));
            return;
        }

        if has_add && has_remove {
            let add_action = format!("+FLAGS ({})", add_flags.join(" "));
            let remove_action = format!("-FLAGS ({})", remove_flags.join(" "));
            let uid_set2 = uid_set.clone();
            let conn2 = conn.clone();
            conn.store_flags(&uid_set, &add_action, move |result| {
                match result {
                    Ok(()) => {
                        conn2.store_flags(&uid_set2, &remove_action, move |result2| {
                            on_complete(result2.map_err(|e| StoreError::new(e.to_string())));
                        });
                    }
                    Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
                }
            });
        } else if has_add {
            let action = format!("+FLAGS ({})", add_flags.join(" "));
            conn.store_flags(&uid_set, &action, move |result| {
                on_complete(result.map_err(|e| StoreError::new(e.to_string())));
            });
        } else {
            let action = format!("-FLAGS ({})", remove_flags.join(" "));
            conn.store_flags(&uid_set, &action, move |result| {
                on_complete(result.map_err(|e| StoreError::new(e.to_string())));
            });
        }
    }

    fn expunge(
        &self,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        conn.expunge(move |result| {
            on_complete(result.map_err(|e| StoreError::new(e.to_string())));
        });
    }

    fn mark_all_read(
        &self,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        conn.store_flags("1:*", r"+FLAGS (\Seen)", move |result| {
            on_complete(result.map_err(|e| StoreError::new(e.to_string())));
        });
    }
}

fn parse_uid_from_imap_id(id: &MessageId) -> Option<u32> {
    let s = id.as_str();
    let rest = s.strip_prefix("imap://")?;
    let parts: Vec<&str> = rest.splitn(3, '/').collect();
    parts.get(2).and_then(|u| u.parse().ok())
}

fn envelope_from_header(header: &[u8]) -> Result<Envelope, crate::mime::MimeParseError> {
    let rfc = parse_envelope(header)?;
    Ok(rfc5322_envelope_to_store(&rfc))
}

fn envelope_from_raw(raw: &[u8]) -> Result<Envelope, crate::mime::MimeParseError> {
    let rfc = parse_envelope(raw)?;
    Ok(rfc5322_envelope_to_store(&rfc))
}

fn rfc5322_envelope_to_store(rfc: &EnvelopeHeaders) -> Envelope {
    Envelope {
        from: rfc.from.iter().map(email_to_address).collect(),
        to: rfc.to.iter().map(email_to_address).collect(),
        cc: rfc.cc.iter().map(email_to_address).collect(),
        date: rfc.date.map(|dt| DateTime {
            timestamp: dt.timestamp(),
            tz_offset_secs: Some(dt.offset().local_minus_utc()),
        }),
        subject: rfc.subject.clone(),
        message_id: rfc.message_id.as_ref().map(|c| c.to_string()),
    }
}

fn email_to_address(e: &EmailAddress) -> Address {
    Address {
        display_name: e.display_name.clone(),
        local_part: e.local_part.clone(),
        domain: Some(e.domain.clone()),
    }
}

fn flag_to_imap_string(flag: &Flag) -> String {
    match flag {
        Flag::Seen => r"\Seen".to_string(),
        Flag::Answered => r"\Answered".to_string(),
        Flag::Flagged => r"\Flagged".to_string(),
        Flag::Deleted => r"\Deleted".to_string(),
        Flag::Draft => r"\Draft".to_string(),
        Flag::Custom(s) => s.clone(),
    }
}

fn imap_flags_to_store(flags: &[String]) -> std::collections::HashSet<Flag> {
    flags
        .iter()
        .filter_map(|s| {
            let s = s.trim_matches('\\');
            Some(match s.to_uppercase().as_str() {
                "SEEN" => Flag::Seen,
                "ANSWERED" => Flag::Answered,
                "FLAGGED" => Flag::Flagged,
                "DELETED" => Flag::Deleted,
                "DRAFT" => Flag::Draft,
                _ => Flag::Custom(s.to_string()),
            })
        })
        .collect()
}

fn default_envelope() -> Envelope {
    Envelope {
        from: Vec::new(),
        to: Vec::new(),
        cc: Vec::new(),
        date: None,
        subject: None,
        message_id: None,
    }
}
