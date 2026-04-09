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

mod bodystructure;
mod client;
pub mod trace;

pub use client::{
    connect_and_authenticate, connect_and_start_pipeline, parse_sort_response_line,
    AuthenticatedSession, FetchSummary, ImapClientError, ImapConnection, ImapLine,
    ImapLineWithLiteral, ListEntry, PipelineIdleHooks, SelectEvent, SelectResult,
    StreamingLiteralState, SummaryHeaderFields,
};

use crate::message_id::{imap_message_id, MessageId};
use crate::mime::{
    decode_content_transfer_encoding, extract_structured_body, parse_envelope,
    parse_thread_headers, EmailAddress, EnvelopeHeaders,
};
use crate::sasl::SaslMechanism;
use crate::store::{
    message_for_display_from_raw, sort_conversation_summaries_for_window, Address,
    ConversationSummary, DateTime, Envelope, Flag, MessageAttachmentRef, MessageForDisplay,
};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::{ThreadId, ThreadSummary};
pub use bodystructure::{
    part_bytes_to_string, plan_body_fetch, AttachmentPlan, BodyFetchPlan, CidPartInfo, DisplayFetch,
};

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use tokio::sync::Mutex as TokioSessionMutex;

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
    /// Second IMAP session for mail-body HTTPS streaming (chunked FETCH), separate from pipeline.
    streaming_session: Arc<TokioSessionMutex<Option<AuthenticatedSession>>>,
    /// Post-auth capabilities from the pipeline connection (includes `SORT` when supported).
    imap_capabilities: Mutex<Vec<String>>,
    /// Last `UID SORT` result; invalidated on reconnect and when mailbox snapshot changes.
    sorted_uid_cache: Mutex<Option<SortedUidListCache>>,
    /// Set when IDLE / EXISTS suggests refreshing folder list + message windows (Flutter polls).
    folder_list_stale: Arc<AtomicBool>,
    /// Minimum quiet seconds on the wire before auto-IDLE (per account; default 120).
    imap_min_idle_secs: Arc<AtomicU32>,
}

/// Internal folder list callbacks stored in ImapStoreState.
#[derive(Clone)]
struct FolderListCallbacksInternal {
    on_folder_found: Arc<dyn Fn(FolderInfo) + Send + Sync>,
    on_folder_removed: Arc<dyn Fn(&str) + Send + Sync>,
}

/// Cached `UID SORT` result for the current mailbox snapshot.
struct SortedUidListCache {
    mailbox: String,
    uid_validity: Option<u32>,
    exists: u32,
    sort_parentheses: String,
    uids: Vec<u32>,
}

fn imap_user_at_host(state: &ImapStoreState) -> String {
    let host = state.host.clone();
    let username = if state.username.read().unwrap().is_empty() {
        state
            .auth
            .read()
            .unwrap()
            .as_ref()
            .map(|(u, _p, _m)| u.clone())
            .unwrap_or_default()
    } else {
        state.username.read().unwrap().clone()
    };
    if username.contains('@') {
        username
    } else {
        format!("{}@{}", username, host)
    }
}

fn imap_sort_parentheses_for_symbolic(symbolic: &str) -> Option<&'static str> {
    match symbolic.trim() {
        "date_asc" | "date_desc" | "dateAsc" | "dateDesc" => Some("(DATE)"),
        "from_asc" | "from_desc" | "fromAsc" | "fromDesc" => Some("(FROM)"),
        "subject_asc" | "subject_desc" | "subAsc" | "subDesc" => Some("(SUBJECT)"),
        _ => None,
    }
}

/// Stderr only (visible in `flutter run` / Xcode / device logs), not the Dart debug console.
fn eprint_imap_list_messages_window_err(
    host: &str,
    port: u16,
    mailbox: &str,
    sort: &str,
    start_index: u64,
    limit: u64,
    phase: &str,
    detail: &str,
) {
    eprintln!(
        "[imap] list_folder_messages_window host={host}:{port} mailbox={mailbox:?} sort={sort} start_index={start_index} limit={limit} phase={phase}: {detail}",
    );
}

impl ImapStoreState {
    /// TLS + greeting + CAPABILITY without LOGIN; connection is closed before return.
    /// When `TAGLIACARTE_TRACE` includes `imap`, this path emits the same wire trace as a full session.
    fn probe_capabilities_without_login(&self) -> Result<Vec<String>, StoreError> {
        let host = self.host.clone();
        let port = self.port;
        let use_implicit_tls = *self
            .use_implicit_tls
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?;
        let use_starttls = *self
            .use_starttls
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?;
        let session = self
            .runtime_handle
            .block_on(async move {
                connect_and_authenticate(
                    &host,
                    port,
                    use_implicit_tls,
                    use_starttls,
                    None,
                )
                .await
            })
            .map_err(|e| StoreError::new(e.to_string()))?;
        Ok(session.capabilities().to_vec())
    }

    fn needs_credential_error(&self, advertised_capabilities: Option<Vec<String>>) -> StoreError {
        let username = self.username.read().unwrap().clone();
        let use_implicit_tls = *self.use_implicit_tls.read().unwrap();
        let use_starttls = *self.use_starttls.read().unwrap();
        let is_plaintext = !use_implicit_tls && !use_starttls;
        StoreError::NeedsCredential {
            username,
            is_plaintext,
            advertised_capabilities,
        }
    }

    /// Ensure a live connection exists and return a clone of the ImapConnection handle.
    fn ensure_connection(&self) -> Result<ImapConnection, StoreError> {
        let mut guard = self
            .connection
            .lock()
            .map_err(|e| StoreError::new(e.to_string()))?;
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let host = self.host.clone();
        let port = self.port;
        let use_implicit_tls = *self
            .use_implicit_tls
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?;
        let use_starttls = *self
            .use_starttls
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?;
        let auth = self
            .auth
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?
            .clone();
        if auth.is_none() {
            let caps = self.probe_capabilities_without_login()?;
            return Err(self.needs_credential_error(Some(caps)));
        }
        let (user, pass, mechanism) = auth.unwrap();

        // Use block_on on the shared runtime to connect and authenticate.
        // This is called from the FFI layer (UI thread) but only once per store
        // when the connection needs to be established.
        let stale = Arc::clone(&self.folder_list_stale);
        let min_idle = Arc::clone(&self.imap_min_idle_secs);
        let mailbox_selected = Arc::new(AtomicBool::new(false));
        let in_idle = Arc::new(AtomicBool::new(false));
        let tag_counter_placeholder = Arc::new(AtomicU32::new(0));
        let hooks = PipelineIdleHooks {
            folder_list_stale: stale,
            supports_idle: false,
            mailbox_selected: Arc::clone(&mailbox_selected),
            min_idle_secs: min_idle,
            in_idle: Arc::clone(&in_idle),
            tag_counter: Arc::clone(&tag_counter_placeholder),
        };
        let (conn, caps) = self.runtime_handle.block_on(async move {
            connect_and_start_pipeline(
                &host,
                port,
                use_implicit_tls,
                use_starttls,
                Some((&user, &pass, mechanism)),
                Some(hooks),
            )
            .await
            .map_err(|e| StoreError::new(e.to_string()))
        })?;
        {
            let mut cg = self
                .imap_capabilities
                .lock()
                .map_err(|e| StoreError::new(e.to_string()))?;
            *cg = caps;
        }
        {
            let mut sc = self
                .sorted_uid_cache
                .lock()
                .map_err(|e| StoreError::new(e.to_string()))?;
            *sc = None;
        }
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
    pub fn with_runtime_handle(
        host: impl Into<String>,
        port: u16,
        handle: tokio::runtime::Handle,
    ) -> Self {
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
            streaming_session: Arc::new(TokioSessionMutex::new(None)),
            imap_capabilities: Mutex::new(Vec::new()),
            sorted_uid_cache: Mutex::new(None),
            folder_list_stale: Arc::new(AtomicBool::new(false)),
            imap_min_idle_secs: Arc::new(AtomicU32::new(120)),
        };
        Self {
            state: Arc::new(state),
        }
    }

    /// Clear and return whether folder/message views should refresh (IMAP EXISTS / IDLE).
    pub fn take_folder_list_stale(&self) -> bool {
        self.state
            .folder_list_stale
            .swap(false, Ordering::AcqRel)
    }

    pub fn set_imap_min_idle_secs(&self, secs: u32) {
        let s = secs.max(15).min(864_000);
        self.state.imap_min_idle_secs.store(s, Ordering::Release);
    }

    /// Lock the dedicated mail-body IMAP session (second TCP connection), creating it on first use.
    /// Hold the guard across `await` while issuing SELECT/FETCH.
    pub async fn lock_mail_body_streaming_session(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<AuthenticatedSession>>, StoreError> {
        let mut guard = self.state.streaming_session.lock().await;
        if guard.is_none() {
            let host = self.state.host.clone();
            let port = self.state.port;
            let use_implicit_tls = *self
                .state
                .use_implicit_tls
                .read()
                .map_err(|e| StoreError::new(e.to_string()))?;
            let use_starttls = *self
                .state
                .use_starttls
                .read()
                .map_err(|e| StoreError::new(e.to_string()))?;
            let auth = self
                .state
                .auth
                .read()
                .map_err(|e| StoreError::new(e.to_string()))?
                .clone();
            let Some((user, pass, mechanism)) = auth else {
                let caps = self.state.probe_capabilities_without_login()?;
                return Err(self.state.needs_credential_error(Some(caps)));
            };
            let session = connect_and_authenticate(
                &host,
                port,
                use_implicit_tls,
                use_starttls,
                Some((&user, &pass, mechanism)),
            )
            .await
            .map_err(|e| StoreError::new(e.to_string()))?;
            *guard = Some(session);
        }
        Ok(guard)
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

    /// STATUS (UNSEEN) for a mailbox without SELECT (blocking wait).
    pub fn mailbox_status_unseen_blocking(&self, mailbox: &str) -> Result<u32, StoreError> {
        let conn = self.state.ensure_connection()?;
        let (tx, rx) = mpsc::sync_channel::<Result<u32, ImapClientError>>(1);
        conn.mailbox_status_unseen(mailbox, move |r| {
            let _ = tx.send(r);
        });
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(Ok(n)) => Ok(n),
            Ok(Err(e)) => Err(StoreError::new(e.to_string())),
            Err(_) => Err(StoreError::new("timeout STATUS UNSEEN (60s)")),
        }
    }

    fn list_folders_list_status_unseen_blocking(
        &self,
        conn: &ImapConnection,
    ) -> Result<Vec<(ListEntry, u32)>, StoreError> {
        let rows: Arc<Mutex<Vec<(ListEntry, u32)>>> = Arc::new(Mutex::new(Vec::new()));
        let r2 = rows.clone();
        let state = Arc::clone(&self.state);
        let (tx, rx) = mpsc::sync_channel::<Result<(), ImapClientError>>(1);
        conn.list_folders_return_status_unseen_streaming(
            move |entry, unseen| {
                if let Ok(mut g) = state.cached_delimiter.lock() {
                    if g.is_none() {
                        if let Some(d) = entry.delimiter {
                            *g = Some(d);
                        }
                    }
                }
                r2.lock().unwrap().push((entry, unseen));
            },
            move |res| {
                let _ = tx.send(res);
            },
        );
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(())) => Ok(rows.lock().unwrap().clone()),
            Ok(Err(e)) => Err(StoreError::new(e.to_string())),
            Err(_) => Err(StoreError::new(
                "timeout LIST RETURN (STATUS (UNSEEN)) (120s)",
            )),
        }
    }

    fn list_folders_plain_then_unseen_blocking(
        &self,
        conn: &ImapConnection,
    ) -> Result<(Vec<String>, Option<char>, HashMap<String, u32>), StoreError> {
        let names_acc = Arc::new(Mutex::new(Vec::<String>::new()));
        let n2 = names_acc.clone();
        let state = Arc::clone(&self.state);
        let (tx, rx) = mpsc::sync_channel::<Result<(), ImapClientError>>(1);
        conn.list_folders_streaming(
            move |entry| {
                if let Ok(mut g) = state.cached_delimiter.lock() {
                    if g.is_none() {
                        if let Some(d) = entry.delimiter {
                            *g = Some(d);
                        }
                    }
                }
                n2.lock().unwrap().push(entry.name);
            },
            move |res| {
                let _ = tx.send(res);
            },
        );
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(StoreError::new(e.to_string())),
            Err(_) => return Err(StoreError::new("timeout listing folders (120s)")),
        }
        let names = names_acc.lock().unwrap().clone();
        let delim = *self
            .state
            .cached_delimiter
            .lock()
            .map_err(|e| StoreError::new(e.to_string()))?;
        let mut map = HashMap::new();
        for n in &names {
            map.insert(n.clone(), self.mailbox_status_unseen_blocking(n)?);
        }
        Ok((names, delim, map))
    }

    /// Folder names, cached hierarchy delimiter, and UNSEEN count per folder name.
    ///
    /// When the server advertises `LIST-STATUS` (RFC 5819), uses a single
    /// `LIST "" "*" RETURN (STATUS (UNSEEN))` instead of one `STATUS` per mailbox.
    /// If that command fails, falls back to plain `LIST` plus per-mailbox `STATUS`.
    pub fn list_folders_and_unread_blocking(
        &self,
    ) -> Result<(Vec<String>, Option<char>, HashMap<String, u32>), StoreError> {
        let conn = self.state.ensure_connection()?;
        let list_status = self
            .state
            .imap_capabilities
            .lock()
            .map_err(|e| StoreError::new(e.to_string()))?
            .iter()
            .any(|c| c.eq_ignore_ascii_case("LIST-STATUS"));
        if list_status {
            match self.list_folders_list_status_unseen_blocking(&conn) {
                Ok(rows) => {
                    let mut names = Vec::with_capacity(rows.len());
                    let mut map = HashMap::new();
                    let mut delim = None;
                    for (e, u) in rows {
                        if delim.is_none() {
                            delim = e.delimiter;
                        }
                        names.push(e.name.clone());
                        map.insert(e.name, u);
                    }
                    Ok((names, delim, map))
                }
                Err(_) => self.list_folders_plain_then_unseen_blocking(&conn),
            }
        } else {
            self.list_folders_plain_then_unseen_blocking(&conn)
        }
    }

    fn inbox_first_mailbox_order(names: &mut Vec<String>) {
        if let Some(pos) = names.iter().position(|n| n.eq_ignore_ascii_case("INBOX")) {
            let inbox = names.remove(pos);
            names.insert(0, inbox);
        }
    }

    /// Subscribed mailboxes from `LSUB "" "*"` (blocking).
    pub fn lsub_folder_names_blocking(&self) -> Result<Vec<String>, StoreError> {
        let conn = self.state.ensure_connection()?;
        let names_acc = Arc::new(Mutex::new(Vec::<String>::new()));
        let n2 = names_acc.clone();
        let state = Arc::clone(&self.state);
        let (tx, rx) = mpsc::sync_channel::<Result<(), ImapClientError>>(1);
        conn.lsub_folders_streaming(
            move |entry| {
                if let Ok(mut g) = state.cached_delimiter.lock() {
                    if g.is_none() {
                        if let Some(d) = entry.delimiter {
                            *g = Some(d);
                        }
                    }
                }
                n2.lock().unwrap().push(entry.name);
            },
            move |res| {
                let _ = tx.send(res);
            },
        );
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(StoreError::new(e.to_string())),
            Err(_) => return Err(StoreError::new("timeout LSUB (120s)")),
        }
        let mut names = names_acc.lock().unwrap().clone();
        Self::inbox_first_mailbox_order(&mut names);
        Ok(names)
    }

    pub fn subscribe_mailbox_blocking(&self, mailbox: &str) -> Result<(), StoreError> {
        let conn = self.state.ensure_connection()?;
        let (tx, rx) = mpsc::sync_channel::<Result<(), ImapClientError>>(1);
        let m = mailbox.to_string();
        conn.subscribe_mailbox(&m, move |r| {
            let _ = tx.send(r);
        });
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(StoreError::new(e.to_string())),
            Err(_) => Err(StoreError::new("timeout SUBSCRIBE (120s)")),
        }
    }

    pub fn unsubscribe_mailbox_blocking(&self, mailbox: &str) -> Result<(), StoreError> {
        let conn = self.state.ensure_connection()?;
        let (tx, rx) = mpsc::sync_channel::<Result<(), ImapClientError>>(1);
        let m = mailbox.to_string();
        conn.unsubscribe_mailbox(&m, move |r| {
            let _ = tx.send(r);
        });
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(StoreError::new(e.to_string())),
            Err(_) => Err(StoreError::new("timeout UNSUBSCRIBE (120s)")),
        }
    }

    /// `(LSUB names, hierarchy delimiter, UNSEEN only for subscribed, Available rows from full LIST)`.
    pub fn subscription_snapshot_blocking(
        &self,
    ) -> Result<
        (
            Vec<String>,
            Option<char>,
            HashMap<String, u32>,
            Vec<(String, bool, u32)>,
        ),
        StoreError,
    > {
        let subscribed = self.lsub_folder_names_blocking()?;
        let (all_names, delim, unread_all) = self.list_folders_and_unread_blocking()?;
        let lsub_set: HashSet<String> = subscribed.iter().cloned().collect();
        let mut available: Vec<(String, bool, u32)> = Vec::with_capacity(all_names.len());
        for name in &all_names {
            let unseen = unread_all.get(name).copied().unwrap_or(0);
            let is_sub = lsub_set.contains(name);
            available.push((name.clone(), is_sub, unseen));
        }
        let mut unread_subscribed = HashMap::new();
        for name in &subscribed {
            unread_subscribed.insert(
                name.clone(),
                unread_all.get(name).copied().unwrap_or(0),
            );
        }
        Ok((subscribed, delim, unread_subscribed, available))
    }

    /// SELECT [mailbox], then return summaries for `[start_index, start_index + limit)` in **ascending**
    /// sort order for the given symbolic sort (matches Flutter `messageListSort`).
    /// Strategy: `"imapSort"` when `UID SORT` + `UID FETCH` succeed; otherwise `"fullScan"`.
    pub fn list_folder_messages_window_blocking(
        &self,
        mailbox: &str,
        start_index: u64,
        limit: u64,
        sort_symbolic: &str,
    ) -> Result<(u64, u64, Vec<ConversationSummary>, &'static str), StoreError> {
        let limit = limit.max(1).min(10_000);
        let conn = self.state.ensure_connection()?;
        let conn_idle = conn.clone();
        let mb_owned = mailbox.to_string();
        let (tx_sel, rx_sel) = mpsc::sync_channel::<Result<SelectResult, ImapClientError>>(1);
        conn.select_streaming(mailbox, |_| {}, move |res| {
            match &res {
                Ok(_) => conn_idle.set_idle_mailbox_selected(true),
                Err(_) => conn_idle.set_idle_mailbox_selected(false),
            }
            let _ = tx_sel.send(res);
        });
        let sel = match rx_sel.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(sr)) => sr,
            Ok(Err(e)) => {
                let msg = e.to_string();
                eprint_imap_list_messages_window_err(
                    &self.state.host,
                    self.state.port,
                    mailbox,
                    sort_symbolic,
                    start_index,
                    limit,
                    "SELECT",
                    &msg,
                );
                return Err(StoreError::new(msg));
            }
            Err(_) => {
                eprint_imap_list_messages_window_err(
                    &self.state.host,
                    self.state.port,
                    mailbox,
                    sort_symbolic,
                    start_index,
                    limit,
                    "SELECT",
                    "timeout waiting for tagged response (120s)",
                );
                return Err(StoreError::new("timeout SELECT (120s)"));
            }
        };
        let total = sel.exists as u64;
        if total == 0 || limit == 0 {
            return Ok((total, start_index, Vec::new(), "fullScan"));
        }
        if start_index >= total {
            return Ok((total, start_index, Vec::new(), "fullScan"));
        }

        let user_at_host = imap_user_at_host(&self.state);
        let sort_paren = imap_sort_parentheses_for_symbolic(sort_symbolic);
        let supports_sort = self
            .state
            .imap_capabilities
            .lock()
            .map_err(|e| StoreError::new(e.to_string()))?
            .iter()
            .any(|c| c == "SORT");

        if supports_sort {
            if let Some(sp) = sort_paren {
                let (uids, from_cache) = {
                    let cache_guard = self
                        .state
                        .sorted_uid_cache
                        .lock()
                        .map_err(|e| StoreError::new(e.to_string()))?;
                    if let Some(ref c) = *cache_guard {
                        if c.mailbox == mb_owned
                            && c.uid_validity == sel.uid_validity
                            && c.exists == sel.exists
                            && c.sort_parentheses == sp
                        {
                            (c.uids.clone(), true)
                        } else {
                            (Vec::new(), false)
                        }
                    } else {
                        (Vec::new(), false)
                    }
                };

                let uids = if !uids.is_empty() {
                    uids
                } else {
                    let (tx_sort, rx_sort) =
                        mpsc::sync_channel::<Result<Vec<u32>, ImapClientError>>(1);
                    conn.uid_sort_all(sp, move |r| {
                        let _ = tx_sort.send(r);
                    });
                    match rx_sort.recv_timeout(Duration::from_secs(120)) {
                        Ok(Ok(u)) => u,
                        Ok(Err(_)) | Err(_) => Vec::new(),
                    }
                };

                let sort_matches_mailbox =
                    !uids.is_empty() && uids.len() as u32 == sel.exists;
                if sort_matches_mailbox {
                    if !from_cache {
                        let mut cache_guard = self
                            .state
                            .sorted_uid_cache
                            .lock()
                            .map_err(|e| StoreError::new(e.to_string()))?;
                        *cache_guard = Some(SortedUidListCache {
                            mailbox: mb_owned.clone(),
                            uid_validity: sel.uid_validity,
                            exists: sel.exists,
                            sort_parentheses: sp.to_string(),
                            uids: uids.clone(),
                        });
                    }
                    let slice_end = (start_index + limit).min(total) as usize;
                    let win: Vec<u32> = uids[start_index as usize..slice_end].to_vec();
                    let (tx_f, rx_f) =
                        mpsc::sync_channel::<Result<Vec<FetchSummary>, ImapClientError>>(1);
                    conn.fetch_uid_set_summaries(&win, SummaryHeaderFields::List, move |r| {
                        let _ = tx_f.send(r);
                    });
                    let rows = match rx_f.recv_timeout(Duration::from_secs(120)) {
                        Ok(Ok(v)) => v,
                        Ok(Err(e)) => {
                            let msg = e.to_string();
                            eprint_imap_list_messages_window_err(
                                &self.state.host,
                                self.state.port,
                                mailbox,
                                sort_symbolic,
                                start_index,
                                limit,
                                "UID_FETCH",
                                &msg,
                            );
                            return Err(StoreError::new(msg));
                        }
                        Err(_) => {
                            eprint_imap_list_messages_window_err(
                                &self.state.host,
                                self.state.port,
                                mailbox,
                                sort_symbolic,
                                start_index,
                                limit,
                                "UID_FETCH",
                                "timeout waiting for tagged response (120s)",
                            );
                            return Err(StoreError::new(
                                "timeout UID FETCH summaries (120s)",
                            ));
                        }
                    };
                    let mut out = Vec::with_capacity(rows.len());
                    for s in rows {
                        let envelope =
                            envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
                        let id = imap_message_id(&user_at_host, &mb_owned, s.uid);
                        let flags = imap_flags_to_store(&s.flags);
                        out.push(ConversationSummary {
                            id,
                            envelope,
                            flags,
                            size: s.size as u64,
                        });
                    }
                    return Ok((total, start_index, out, "imapSort"));
                }
            }
        }

        let collected: Arc<Mutex<Vec<ConversationSummary>>> = Arc::new(Mutex::new(Vec::new()));
        let c2 = Arc::clone(&collected);
        let ua = user_at_host.clone();
        let mb2 = mb_owned.clone();
        let (tx_f, rx_f) = mpsc::sync_channel::<Result<(), ImapClientError>>(1);
        conn.fetch_summaries_streaming(
            1,
            sel.exists,
            SummaryHeaderFields::List,
            move |s| {
                let envelope =
                    envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
                let id = imap_message_id(&ua, &mb2, s.uid);
                let flags = imap_flags_to_store(&s.flags);
                c2.lock()
                    .expect("imap window full scan")
                    .push(ConversationSummary {
                        id,
                        envelope,
                        flags,
                        size: s.size as u64,
                    });
            },
            move |res| {
                let _ = tx_f.send(res);
            },
        );
        match rx_f.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let msg = e.to_string();
                eprint_imap_list_messages_window_err(
                    &self.state.host,
                    self.state.port,
                    mailbox,
                    sort_symbolic,
                    start_index,
                    limit,
                    "FETCH_STREAM",
                    &msg,
                );
                return Err(StoreError::new(msg));
            }
            Err(_) => {
                eprint_imap_list_messages_window_err(
                    &self.state.host,
                    self.state.port,
                    mailbox,
                    sort_symbolic,
                    start_index,
                    limit,
                    "FETCH_STREAM",
                    "timeout waiting for tagged response (120s)",
                );
                return Err(StoreError::new("timeout FETCH summaries (120s)"));
            }
        }
        let mut all = std::mem::take(&mut *collected.lock().expect("imap window full scan"));
        sort_conversation_summaries_for_window(&mut all, sort_symbolic);
        let slice_end = (start_index + limit).min(total) as usize;
        let slice = all[start_index as usize..slice_end].to_vec();
        Ok((total, start_index, slice, "fullScan"))
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
        let existing_mechanism = self
            .state
            .auth
            .read()
            .unwrap()
            .as_ref()
            .map(|(_, _, m)| *m)
            .unwrap_or(SaslMechanism::ScramSha256);
        *self.state.auth.write().unwrap() = Some((u, password.to_string(), existing_mechanism));
    }

    fn set_oauth_credential(&self, email: &str, token: &str) {
        *self.state.username.write().unwrap() = email.to_string();
        *self.state.auth.write().unwrap() =
            Some((email.to_string(), token.to_string(), SaslMechanism::XOAuth2));
        // Drop stale connection so next operation reconnects with the new token.
        if let Ok(mut guard) = self.state.connection.lock() {
            *guard = None;
        }
        if let Ok(mut c) = self.state.sorted_uid_cache.lock() {
            *c = None;
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
        let conn_idle = conn.clone();
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
                match &result {
                    Ok(_) => conn_idle.set_idle_mailbox_selected(true),
                    Err(_) => conn_idle.set_idle_mailbox_selected(false),
                }
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
        self.state
            .cached_delimiter
            .lock()
            .ok()
            .and_then(|g| *g)
            .or(Some('/'))
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
        let callbacks = self
            .state
            .folder_list_callbacks
            .read()
            .ok()
            .and_then(|g| g.clone());
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
        let callbacks = self
            .state
            .folder_list_callbacks
            .read()
            .ok()
            .and_then(|g| g.clone());
        let delimiter = self.hierarchy_delimiter();

        conn.rename_mailbox(old_name, new_name, move |result| match result {
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
        let callbacks = self
            .state
            .folder_list_callbacks
            .read()
            .ok()
            .and_then(|g| g.clone());

        conn.delete_mailbox(name, move |result| match result {
            Ok(()) => {
                if let Some(ref cbs) = callbacks {
                    (cbs.on_folder_removed)(&name_owned);
                }
                on_complete(Ok(()));
            }
            Err(e) => {
                on_complete(Err(StoreError::new(e.to_string())));
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
            SummaryHeaderFields::List,
            move |s| {
                let envelope =
                    envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
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

    fn message_count(&self, on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>) {
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

    fn get_message_display(
        &self,
        id: &MessageId,
        on_done: Box<dyn FnOnce(Result<MessageForDisplay, StoreError>) + Send>,
    ) {
        let uid = match parse_uid_from_imap_id(id) {
            Some(u) => u,
            None => {
                on_done(Err(StoreError::new("invalid message id")));
                return;
            }
        };
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_done(Err(e));
                return;
            }
        };

        let structure_line: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let sl = structure_line.clone();
        let conn_a = conn.clone();
        conn.send(
            &format!("UID FETCH {} (BODYSTRUCTURE)", uid),
            move |line, _lit| {
                if line.contains(" FETCH (") && line.contains("BODYSTRUCTURE") {
                    *sl.lock().unwrap() = Some(line.to_string());
                }
            },
            move |ok, _raw| {
                if !ok {
                    imap_fallback_full_fetch(conn_a, uid, on_done);
                    return;
                }
                let line = structure_line.lock().unwrap().take().unwrap_or_default();
                let Some(plan) = plan_body_fetch(&line) else {
                    imap_fallback_full_fetch(conn_a, uid, on_done);
                    return;
                };
                if matches!(plan.display, DisplayFetch::None) {
                    imap_fallback_full_fetch(conn_a, uid, on_done);
                    return;
                }
                imap_run_body_plan(conn_a, uid, plan, on_done);
            },
        );
    }

    fn fetch_message_part(
        &self,
        id: &MessageId,
        imap_section: &str,
        transfer_encoding: &str,
        on_done: Box<dyn FnOnce(Result<Vec<u8>, StoreError>) + Send>,
    ) {
        let uid = match parse_uid_from_imap_id(id) {
            Some(u) => u,
            None => {
                on_done(Err(StoreError::new("invalid message id")));
                return;
            }
        };
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => {
                on_done(Err(e));
                return;
            }
        };
        let sec = imap_section.to_string();
        let enc = transfer_encoding.to_string();
        let buf = Arc::new(Mutex::new(Vec::new()));
        let buf2 = buf.clone();
        conn.send(
            &format!("UID FETCH {} (BODY.PEEK[{sec}])", uid),
            move |_line, lit| {
                if let Some(b) = lit {
                    buf2.lock().unwrap().extend_from_slice(b);
                }
            },
            move |ok, _| {
                if !ok {
                    on_done(Err(StoreError::new("UID FETCH body part failed")));
                    return;
                }
                let raw = buf.lock().unwrap().clone();
                let decoded = decode_content_transfer_encoding(&enc, &raw);
                on_done(Ok(decoded));
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
                // 1. CREATE trash mailbox if missing, then UID COPY
                // 2. UID STORE +FLAGS (\Deleted) on source
                // 3. UID EXPUNGE source UIDs
                let uid_set2 = uid_set.clone();
                let uid_set3 = uid_set.clone();
                let trash = trash_folder.clone();
                let conn_ensure = conn.clone();
                let conn2 = conn.clone();
                let conn3 = conn.clone();
                conn.ensure_mailbox_exists(&trash_folder, move |ens| match ens {
                    Ok(()) => conn_ensure.copy_uids(
                        &uid_set,
                        &trash,
                        move |copy_result| match copy_result {
                            Ok(()) => {
                                conn2.store_flags(
                                    &uid_set2,
                                    r"+FLAGS (\Deleted)",
                                    move |store_result| match store_result {
                                        Ok(()) => {
                                            conn3.uid_expunge(&uid_set3, move |exp_result| {
                                                on_complete(
                                                    exp_result
                                                        .map_err(|e| StoreError::new(e.to_string())),
                                                );
                                            });
                                        }
                                        Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
                                    },
                                );
                            }
                            Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
                        },
                    ),
                    Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
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

        conn.fetch_summaries_streaming(
            1,
            exists,
            SummaryHeaderFields::ThreadIndex,
            move |s| {
                if let Ok(mut guard) = summaries_cb.lock() {
                    guard.push(s);
                }
            },
            move |result| match result.map_err(|e| StoreError::new(e.to_string())) {
                Ok(()) => {
                    let summaries = summaries.lock().unwrap();
                    let mut thread_groups: std::collections::HashMap<
                        String,
                        (Option<String>, u64),
                    > = std::collections::HashMap::new();
                    for s in summaries.iter() {
                        let th = parse_thread_headers(&s.header).unwrap_or_default();
                        let root = th
                            .references
                            .first()
                            .cloned()
                            .or(th.message_id.clone())
                            .unwrap_or_else(|| {
                                format!("s:{}", th.subject.as_deref().unwrap_or(""))
                            });
                        let entry = thread_groups.entry(root).or_insert((th.subject.clone(), 0));
                        entry.1 += 1;
                    }
                    let mut threads: Vec<(String, Option<String>, u64)> = thread_groups
                        .into_iter()
                        .map(|(id, (subject, count))| (id, subject, count))
                        .collect();
                    threads.sort_by(|a, b| a.0.cmp(&b.0));
                    let start = range.start.min(threads.len() as u64) as usize;
                    let end = range.end.min(threads.len() as u64) as usize;
                    for t in threads
                        .into_iter()
                        .skip(start)
                        .take(end.saturating_sub(start))
                    {
                        on_thread(ThreadSummary {
                            id: ThreadId(t.0),
                            subject: t.1,
                            message_count: t.2,
                        });
                    }
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(e)),
            },
        );
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

        conn.fetch_summaries_streaming(
            1,
            exists,
            SummaryHeaderFields::ThreadDrillDown,
            move |s| {
                if let Ok(mut guard) = summaries_cb.lock() {
                    guard.push(s);
                }
            },
            move |result| match result.map_err(|e| StoreError::new(e.to_string())) {
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
                            .unwrap_or_else(|| {
                                format!("s:{}", th.subject.as_deref().unwrap_or(""))
                            });
                        if root != thread_id_str {
                            continue;
                        }
                        let envelope =
                            envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
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
                    for s in in_thread
                        .into_iter()
                        .skip(start)
                        .take(end.saturating_sub(start))
                    {
                        on_summary(s);
                    }
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(e)),
            },
        );
    }

    fn append_message(
        &self,
        _data: &[u8],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // APPEND requires literal syntax which the pipeline model handles differently.
        // For now, return an error; this can be enhanced later.
        on_complete(Err(StoreError::new(
            "APPEND via pipeline not yet supported",
        )));
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
        let uid_set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let dest = dest_folder_name.to_string();
        let c = conn.clone();
        conn.ensure_mailbox_exists(dest_folder_name, move |ens| match ens {
            Ok(()) => c.copy_uids(&uid_set, &dest, move |result| {
                on_complete(result.map_err(|e| StoreError::new(e.to_string())));
            }),
            Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
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
        let uid_set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let dest = dest_folder_name.to_string();
        let uid_set_for_fallback = uid_set.clone();
        let dest_for_fallback = dest.clone();
        let conn_for_fallback = conn.clone();
        let conn_ensure = conn.clone();
        // Ensure destination exists (e.g. Trash/Junk not yet on server), then UID MOVE / COPY fallback.
        conn.ensure_mailbox_exists(dest_folder_name, move |ens| match ens {
            Ok(()) => conn_ensure.move_uids(&uid_set, &dest, move |result| {
                match result {
                    Ok(()) => on_complete(Ok(())),
                    Err(_) => {
                        let uid_set2 = uid_set_for_fallback.clone();
                        let conn2 = conn_for_fallback.clone();
                        conn_for_fallback.copy_uids(
                            &uid_set_for_fallback,
                            &dest_for_fallback,
                            move |copy_result| match copy_result {
                                Ok(()) => {
                                    let uid_set3 = uid_set2.clone();
                                    let conn3 = conn2.clone();
                                    conn2.store_flags(
                                        &uid_set2,
                                        r"+FLAGS (\Deleted)",
                                        move |store_result| match store_result {
                                            Ok(()) => {
                                                conn3.uid_expunge(&uid_set3, move |exp_result| {
                                                    on_complete(
                                                        exp_result.map_err(|e| {
                                                            StoreError::new(e.to_string())
                                                        }),
                                                    );
                                                });
                                            }
                                            Err(e) => {
                                                on_complete(Err(StoreError::new(e.to_string())));
                                            }
                                        },
                                    );
                                }
                                Err(e) => {
                                    on_complete(Err(StoreError::new(e.to_string())));
                                }
                            },
                        );
                    }
                }
            }),
            Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
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
        let uid_set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

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
            conn.store_flags(&uid_set, &add_action, move |result| match result {
                Ok(()) => {
                    conn2.store_flags(&uid_set2, &remove_action, move |result2| {
                        on_complete(result2.map_err(|e| StoreError::new(e.to_string())));
                    });
                }
                Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
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

    fn expunge(&self, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
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

    fn mark_all_read(&self, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
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

/// IMAP peek for the detail-pane envelope (not the full RFC822 header block).
const MESSAGE_DETAIL_ENVELOPE_PEEK: &str =
    "BODY.PEEK[HEADER.FIELDS (SUBJECT FROM SENDER TO CC DATE)]";

fn imap_attachment_refs_from_plan(plan: &BodyFetchPlan) -> Vec<MessageAttachmentRef> {
    plan.attachments
        .iter()
        .map(|a| MessageAttachmentRef {
            filename: a.filename.clone(),
            content_type: a.content_type.clone(),
            size_bytes: a.size,
            transfer_encoding: a.encoding.clone(),
            imap_section: Some(a.section.clone()),
            content_id: a.content_id.clone(),
            data: None,
        })
        .collect()
}

fn imap_fallback_full_fetch(
    conn: ImapConnection,
    uid: u32,
    on_done: Box<dyn FnOnce(Result<MessageForDisplay, StoreError>) + Send>,
) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let buf2 = buf.clone();
    conn.fetch_body_by_uid_streaming(
        uid,
        move |chunk| {
            buf2.lock().unwrap().extend_from_slice(chunk);
        },
        move |result| match result {
            Ok(()) => {
                let raw = buf.lock().unwrap().clone();
                on_done(Ok(message_for_display_from_raw(&raw)));
            }
            Err(e) => on_done(Err(StoreError::new(e.to_string()))),
        },
    );
}

fn imap_run_body_plan(
    conn: ImapConnection,
    uid: u32,
    plan: BodyFetchPlan,
    on_done: Box<dyn FnOnce(Result<MessageForDisplay, StoreError>) + Send>,
) {
    let header_buf = Arc::new(Mutex::new(Vec::new()));
    let hb = header_buf.clone();
    let conn_b = conn.clone();
    conn.send(
        &format!("UID FETCH {} ({MESSAGE_DETAIL_ENVELOPE_PEEK})", uid),
        move |_line, lit| {
            if let Some(b) = lit {
                hb.lock().unwrap().extend_from_slice(b);
            }
        },
        move |ok, _| {
            if !ok {
                imap_fallback_full_fetch(conn_b.clone(), uid, on_done);
                return;
            }
            let headers = header_buf.lock().unwrap().clone();
            let env = envelope_from_header(&headers).unwrap_or_else(|_| default_envelope());
            let attachments = imap_attachment_refs_from_plan(&plan);

            match plan.display {
                DisplayFetch::None => imap_fallback_full_fetch(conn_b, uid, on_done),
                DisplayFetch::TextPart {
                    section,
                    encoding,
                    is_html,
                    charset_hint,
                } => {
                    let body_buf = Arc::new(Mutex::new(Vec::new()));
                    let bb = body_buf.clone();
                    let conn_c = conn_b.clone();
                    conn_b.send(
                        &format!("UID FETCH {} (BODY.PEEK[{section}])", uid),
                        move |_line, lit| {
                            if let Some(b) = lit {
                                bb.lock().unwrap().extend_from_slice(b);
                            }
                        },
                        move |ok2, _| {
                            if !ok2 {
                                imap_fallback_full_fetch(conn_c, uid, on_done);
                                return;
                            }
                            let raw_body = body_buf.lock().unwrap().clone();
                            let decoded = decode_content_transfer_encoding(&encoding, &raw_body);
                            let text = bodystructure::part_bytes_to_string(
                                &decoded,
                                charset_hint.as_deref(),
                            );
                            let (body_plain, body_html) = if is_html {
                                (None, Some(text))
                            } else {
                                (Some(text), None)
                            };
                            on_done(Ok(MessageForDisplay {
                                envelope: env,
                                body_plain,
                                body_html,
                                attachments,
                            }));
                        },
                    );
                }
                DisplayFetch::NestedMessage { section, encoding } => {
                    let body_buf = Arc::new(Mutex::new(Vec::new()));
                    let bb = body_buf.clone();
                    let conn_c = conn_b.clone();
                    conn_b.send(
                        &format!("UID FETCH {} (BODY.PEEK[{section}])", uid),
                        move |_line, lit| {
                            if let Some(b) = lit {
                                bb.lock().unwrap().extend_from_slice(b);
                            }
                        },
                        move |ok2, _| {
                            if !ok2 {
                                imap_fallback_full_fetch(conn_c, uid, on_done);
                                return;
                            }
                            let raw_body = body_buf.lock().unwrap().clone();
                            let decoded = decode_content_transfer_encoding(&encoding, &raw_body);
                            let (plain, html, _) =
                                extract_structured_body(&decoded).unwrap_or((None, None, vec![]));
                            on_done(Ok(MessageForDisplay {
                                envelope: env,
                                body_plain: plain,
                                body_html: html,
                                attachments,
                            }));
                        },
                    );
                }
            }
        },
    );
}

/// IMAP ids are `imap://{user_at_host}/{mailbox}/{uid}` where `mailbox` may contain `/`.
fn parse_uid_from_imap_id(id: &MessageId) -> Option<u32> {
    let s = id.as_str();
    let rest = s.strip_prefix("imap://")?;
    let last_slash = rest.rfind('/')?;
    rest[last_slash + 1..].parse().ok()
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
        in_reply_to: rfc.in_reply_to.clone(),
        references: rfc.references.clone(),
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
        in_reply_to: None,
        references: None,
    }
}

#[cfg(test)]
mod imap_message_id_parse_tests {
    use super::parse_uid_from_imap_id;
    use crate::message_id::imap_message_id;

    #[test]
    fn uid_is_last_path_segment() {
        let id = imap_message_id("alice@ex.com", "INBOX", 42);
        assert_eq!(parse_uid_from_imap_id(&id), Some(42));
        let nested = imap_message_id("alice@ex.com", "Clients/Acme/INBOX", 7);
        assert_eq!(parse_uid_from_imap_id(&nested), Some(7));
    }
}
