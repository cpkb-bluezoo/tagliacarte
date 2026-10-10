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

//! IMAP4rev2 client (Store + Folder) on hopf (`hopf-imap`).
//!
//! One persistent [`ImapSession`] per store (reconnected when it dies) runs
//! every command as a queued job with its own callback; see `session.rs`.
//! A second, dedicated session serves the mail-body HTTPS server's chunked
//! fetches (`body_stream.rs`). All trait methods return at once; the
//! `*_blocking` helpers exist for the app's worker threads.

mod bodystructure;
pub mod body_stream;
pub mod session;
pub mod trace;

pub use body_stream::{Lease, LiteralSink, MailBodySession, Reads, Resume};
pub use bodystructure::{
    part_bytes_to_string, plan_body_fetch, AttachmentPlan, BodyFetchPlan, CidPartInfo, DisplayFetch,
};
pub use hopf_imap::ImapListEntry as ListEntry;
pub use session::{FetchItem, ImapClientError, ImapSession, ImapSessionConfig, Job};

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
use hopf_imap::{ImapCapabilities, ImapMailboxInfo};
use session::{probe_capabilities_blocking, FetchStream};

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;


/// Per-command wait in the blocking helpers.
const T_CMD: Duration = Duration::from_secs(120);
/// Login wait before the first command of a blocking helper.
const T_LOGIN: Duration = Duration::from_secs(60);

/// IMAP delete mode: how the delete button works for IMAP folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImapDeleteMode {
    /// Mark messages with \Deleted flag (user can expunge later).
    MarkDeleted,
    /// Copy to trash folder, mark \Deleted on source, then UID EXPUNGE source.
    MoveToTrash,
}

/// Which RFC822 header fields to request in `BODY.PEEK[HEADER.FIELDS (...)]` for batched summaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryHeaderFields {
    /// Flat list rows: from, sender, subject, date.
    List,
    /// Thread grouping only (subject + Message-ID / References / In-Reply-To).
    ThreadIndex,
    /// Rows inside a thread: envelope fields for the UI plus threading headers.
    ThreadDrillDown,
}

impl SummaryHeaderFields {
    fn body_peek_clause(self) -> &'static str {
        match self {
            Self::List => "BODY.PEEK[HEADER.FIELDS (FROM SENDER SUBJECT DATE)]",
            Self::ThreadIndex => "BODY.PEEK[HEADER.FIELDS (SUBJECT MESSAGE-ID REFERENCES IN-REPLY-TO)]",
            Self::ThreadDrillDown => {
                "BODY.PEEK[HEADER.FIELDS (FROM SENDER TO CC SUBJECT DATE MESSAGE-ID REFERENCES IN-REPLY-TO)]"
            }
        }
    }

    fn fetch_items(self) -> String {
        format!("(UID FLAGS RFC822.SIZE {})", self.body_peek_clause())
    }
}

/// IMAP peek for the detail-pane envelope (not the full RFC822 header block).
const MESSAGE_DETAIL_ENVELOPE_PEEK: &str = "(BODY.PEEK[HEADER.FIELDS (SUBJECT FROM SENDER TO CC DATE)])";

/// Shared state for IMAP: the persistent session. Store and folders hold Arc<this>.
struct ImapStoreState {
    host: String,
    port: u16,
    use_implicit_tls: RwLock<bool>,
    use_starttls: RwLock<bool>,
    auth: RwLock<Option<(String, String, SaslMechanism)>>,
    username: RwLock<String>,
    /// Live session, replaced when it dies.
    session: Mutex<Option<Arc<ImapSession>>>,
    /// Cached hierarchy delimiter from LIST responses.
    cached_delimiter: Mutex<Option<char>>,
    /// Registered callbacks for folder list events.
    folder_list_callbacks: RwLock<Option<FolderListCallbacksInternal>>,
    delete_mode: RwLock<ImapDeleteMode>,
    trash_folder: RwLock<String>,
    /// Second session for mail-body HTTPS streaming (chunked FETCH).
    body_session: Mutex<Option<MailBodySession>>,
    /// Last `UID SORT` result; invalidated on reconnect and when mailbox snapshot changes.
    sorted_uid_cache: Mutex<Option<SortedUidListCache>>,
    /// Set when IDLE / EXISTS suggests refreshing folder list + message windows (Flutter polls).
    folder_list_stale: Arc<AtomicBool>,
    /// Minimum quiet seconds on the wire before auto-IDLE (per account; default 120).
    imap_min_idle_secs: Arc<AtomicU32>,
}

impl Drop for ImapStoreState {
    fn drop(&mut self) {
        if let Ok(guard) = self.session.lock() {
            if let Some(s) = guard.as_ref() {
                s.close();
            }
        }
    }
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

fn imap_sort_parentheses_for_symbolic(symbolic: &str) -> Option<&'static str> {
    match symbolic.trim() {
        "date_asc" | "date_desc" | "dateAsc" | "dateDesc" => Some("(DATE)"),
        "from_asc" | "from_desc" | "fromAsc" | "fromDesc" => Some("(FROM)"),
        "subject_asc" | "subject_desc" | "subAsc" | "subDesc" => Some("(SUBJECT)"),
        _ => None,
    }
}

fn store_err(e: ImapClientError) -> StoreError {
    StoreError::new(e.message)
}

impl ImapStoreState {
    fn user_at_host(&self) -> String {
        let username = {
            let u = self.username.read().unwrap().clone();
            if u.is_empty() {
                self.auth.read().unwrap().as_ref().map(|(u, _, _)| u.clone()).unwrap_or_default()
            } else {
                u
            }
        };
        if username.contains('@') {
            username
        } else {
            format!("{}@{}", username, self.host)
        }
    }

    fn session_config(&self, auto_idle: bool) -> Result<ImapSessionConfig, StoreError> {
        let implicit_tls = *self.use_implicit_tls.read().unwrap();
        let starttls = *self.use_starttls.read().unwrap();
        let auth = self.auth.read().unwrap().clone();
        let cfg = ImapSessionConfig {
            host: self.host.clone(),
            port: self.port,
            implicit_tls,
            starttls,
            auth: None,
            min_idle_secs: Arc::clone(&self.imap_min_idle_secs),
            folder_list_stale: Arc::clone(&self.folder_list_stale),
            auto_idle,
        };
        match auth {
            Some(a) => Ok(ImapSessionConfig { auth: Some(a), ..cfg }),
            None => {
                // No credentials: tell the caller, with what the server
                // offers, so the prompt can show the mechanisms.
                let caps = probe_capabilities_blocking(&cfg, Duration::from_secs(30)).ok();
                Err(StoreError::NeedsCredential {
                    username: self.username.read().unwrap().clone(),
                    is_plaintext: !implicit_tls && !starttls,
                    advertised_capabilities: caps,
                })
            }
        }
    }

    /// The live session, dialling a new one when there is none or it died.
    fn ensure_session(&self) -> Result<Arc<ImapSession>, StoreError> {
        {
            let guard = self.session.lock().unwrap();
            if let Some(s) = guard.as_ref() {
                if s.is_alive() {
                    return Ok(Arc::clone(s));
                }
            }
        }
        let cfg = self.session_config(true)?;
        let s = ImapSession::connect(cfg);
        *self.session.lock().unwrap() = Some(Arc::clone(&s));
        *self.sorted_uid_cache.lock().unwrap() = None;
        Ok(s)
    }

    /// Session plus the capabilities it logged in with (blocking helpers).
    fn ready_session(&self) -> Result<(Arc<ImapSession>, ImapCapabilities), StoreError> {
        let s = self.ensure_session()?;
        let caps = s.wait_ready(T_LOGIN).map_err(store_err)?;
        Ok((s, caps))
    }

    fn note_delimiter(&self, entry: &ListEntry) {
        if let Some(d) = entry.delimiter.as_deref().and_then(|d| d.chars().next()) {
            let mut g = self.cached_delimiter.lock().unwrap();
            if g.is_none() {
                *g = Some(d);
            }
        }
    }

    fn folder_info(entry: &ListEntry) -> FolderInfo {
        FolderInfo {
            name: entry.name.clone(),
            delimiter: entry.delimiter.as_deref().and_then(|d| d.chars().next()),
            attributes: entry.attributes.clone(),
        }
    }

    /// CREATE, treating "already exists" as success.
    fn ensure_mailbox_exists(session: &ImapSession, mailbox: &str, done: session::Done<()>) {
        session.submit(Job::Create {
            mailbox: mailbox.to_string(),
            done: Box::new(move |r| match r {
                Ok(()) => done(Ok(())),
                Err(e) => {
                    let m = e.message.to_ascii_lowercase();
                    if m.contains("exist") {
                        done(Ok(()))
                    } else {
                        done(Err(e))
                    }
                }
            }),
        });
    }
}

/// IMAP store. Holds the persistent session (connection reuse, idle, reconnect).
pub struct ImapStore {
    state: Arc<ImapStoreState>,
}

impl ImapStore {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 993;
        let state = ImapStoreState {
            host,
            port,
            use_implicit_tls: RwLock::new(use_implicit_tls),
            use_starttls: RwLock::new(true),
            auth: RwLock::new(None),
            username: RwLock::new(String::new()),
            session: Mutex::new(None),
            cached_delimiter: Mutex::new(None),
            folder_list_callbacks: RwLock::new(None),
            delete_mode: RwLock::new(ImapDeleteMode::MoveToTrash),
            trash_folder: RwLock::new("Trash".to_string()),
            body_session: Mutex::new(None),
            sorted_uid_cache: Mutex::new(None),
            folder_list_stale: Arc::new(AtomicBool::new(false)),
            imap_min_idle_secs: Arc::new(AtomicU32::new(120)),
        };
        Self { state: Arc::new(state) }
    }

    /// Clear and return whether folder/message views should refresh (IMAP EXISTS / IDLE).
    pub fn take_folder_list_stale(&self) -> bool {
        self.state.folder_list_stale.swap(false, Ordering::AcqRel)
    }

    pub fn set_imap_min_idle_secs(&self, secs: u32) {
        let s = secs.clamp(15, 864_000);
        self.state.imap_min_idle_secs.store(s, Ordering::Release);
    }

    /// The dedicated mail-body IMAP session (second TCP connection),
    /// created on first use and reconnected once it has dropped. Requests
    /// take turns on it through [`MailBodySession::acquire`].
    pub fn mail_body_session(&self) -> Result<MailBodySession, StoreError> {
        let mut guard = self.state.body_session.lock().unwrap();
        if guard.as_ref().map(|s| !s.is_alive()).unwrap_or(true) {
            let cfg = self.state.session_config(false)?;
            *guard = Some(MailBodySession::connect(cfg));
        }
        Ok(guard.as_ref().expect("just connected").clone())
    }

    pub fn set_implicit_tls(&mut self, use_tls: bool) -> &mut Self {
        *self.state.use_implicit_tls.write().unwrap() = use_tls;
        self
    }

    pub fn set_use_starttls(&mut self, use_starttls: bool) -> &mut Self {
        *self.state.use_starttls.write().unwrap() = use_starttls;
        self
    }

    pub fn set_auth(&mut self, username: impl Into<String>, password: impl Into<String>, mechanism: SaslMechanism) -> &mut Self {
        let u = username.into();
        if self.state.username.read().unwrap().is_empty() {
            *self.state.username.write().unwrap() = u.clone();
        }
        *self.state.auth.write().unwrap() = Some((u, password.into(), mechanism));
        self
    }

    /// Set OAuth2 access token for XOAUTH2 authentication (Gmail, Outlook).
    pub fn set_oauth_token(&mut self, email: impl Into<String>, access_token: impl Into<String>) -> &mut Self {
        let e = email.into();
        if self.state.username.read().unwrap().is_empty() {
            *self.state.username.write().unwrap() = e.clone();
        }
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

    pub fn set_delete_mode(&mut self, mode: ImapDeleteMode) -> &mut Self {
        *self.state.delete_mode.write().unwrap() = mode;
        self
    }

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
        *self.state.folder_list_callbacks.write().unwrap() =
            Some(FolderListCallbacksInternal { on_folder_found, on_folder_removed });
    }

    /// LOGOUT the persistent session (a new one is dialled on next use).
    pub fn close(&self) {
        if let Some(s) = self.state.session.lock().unwrap().take() {
            s.close();
        }
    }

    // ── Blocking helpers (app worker threads) ───────────────────────────

    /// STATUS (UNSEEN) for a mailbox without SELECT (blocking wait).
    pub fn mailbox_status_unseen_blocking(&self, mailbox: &str) -> Result<u32, StoreError> {
        let (s, _) = self.state.ready_session()?;
        let mailbox = mailbox.to_string();
        s.run_blocking(|done| Job::Status { mailbox, done }, T_CMD)
            .map(|st| st.unseen.unwrap_or(0))
            .map_err(store_err)
    }

    /// `LIST "" "*"` entries (blocking).
    pub fn list_folders_blocking(&self) -> Result<Vec<ListEntry>, StoreError> {
        let (s, _) = self.state.ready_session()?;
        let rows: Arc<Mutex<Vec<ListEntry>>> = Arc::new(Mutex::new(Vec::new()));
        let r2 = Arc::clone(&rows);
        let state = Arc::clone(&self.state);
        s.run_blocking(
            |done| Job::List {
                pattern: "*".into(),
                status_unseen: false,
                on_row: Box::new(move |e, _| {
                    state.note_delimiter(e);
                    r2.lock().unwrap().push(e.clone());
                }),
                done,
            },
            T_CMD,
        )
        .map_err(store_err)?;
        let rows = std::mem::take(&mut *rows.lock().unwrap());
        Ok(rows)
    }

    /// Folder names, cached hierarchy delimiter, and UNSEEN count per folder name.
    ///
    /// With `LIST-STATUS` (RFC 5819) this is one `LIST "" "*" RETURN (STATUS (UNSEEN))`;
    /// otherwise plain `LIST` plus one `STATUS` per mailbox.
    pub fn list_folders_and_unread_blocking(&self) -> Result<(Vec<String>, Option<char>, HashMap<String, u32>), StoreError> {
        let (s, caps) = self.state.ready_session()?;
        let rows: Arc<Mutex<Vec<(ListEntry, Option<u32>)>>> = Arc::new(Mutex::new(Vec::new()));
        let r2 = Arc::clone(&rows);
        let state = Arc::clone(&self.state);
        let status_unseen = caps.list_status;
        let listed = s.run_blocking(
            |done| Job::List {
                pattern: "*".into(),
                status_unseen,
                on_row: Box::new(move |e, unseen| {
                    state.note_delimiter(e);
                    r2.lock().unwrap().push((e.clone(), unseen));
                }),
                done,
            },
            T_CMD,
        );
        let rows = match listed {
            Ok(()) => std::mem::take(&mut *rows.lock().unwrap()),
            Err(e) if status_unseen => {
                // LIST-STATUS advertised but refused: the plain way.
                crate::trace_log!("imap", "LIST RETURN (STATUS) failed ({e}); falling back to LIST + STATUS");
                let plain = self.list_folders_blocking()?;
                plain.into_iter().map(|e| (e, None)).collect()
            }
            Err(e) => return Err(store_err(e)),
        };
        let delim = *self.state.cached_delimiter.lock().unwrap();
        let mut names = Vec::with_capacity(rows.len());
        let mut map = HashMap::new();
        for (e, unseen) in rows {
            let unseen = match unseen {
                Some(u) => u,
                None => self.mailbox_status_unseen_blocking(&e.name).unwrap_or(0),
            };
            names.push(e.name.clone());
            map.insert(e.name, unseen);
        }
        Ok((names, delim, map))
    }

    fn inbox_first_mailbox_order(names: &mut Vec<String>) {
        if let Some(pos) = names.iter().position(|n| n.eq_ignore_ascii_case("INBOX")) {
            let inbox = names.remove(pos);
            names.insert(0, inbox);
        }
    }

    /// Subscribed mailboxes from `LSUB "" "*"` (blocking).
    pub fn lsub_folder_names_blocking(&self) -> Result<Vec<String>, StoreError> {
        let (s, _) = self.state.ready_session()?;
        let names: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let n2 = Arc::clone(&names);
        let state = Arc::clone(&self.state);
        s.run_blocking(
            |done| Job::Lsub {
                on_row: Box::new(move |e| {
                    state.note_delimiter(e);
                    n2.lock().unwrap().push(e.name.clone());
                }),
                done,
            },
            T_CMD,
        )
        .map_err(store_err)?;
        let mut names = std::mem::take(&mut *names.lock().unwrap());
        Self::inbox_first_mailbox_order(&mut names);
        Ok(names)
    }

    pub fn subscribe_mailbox_blocking(&self, mailbox: &str) -> Result<(), StoreError> {
        let (s, _) = self.state.ready_session()?;
        let mailbox = mailbox.to_string();
        s.run_blocking(|done| Job::Subscribe { mailbox, done }, T_CMD).map_err(store_err)
    }

    pub fn unsubscribe_mailbox_blocking(&self, mailbox: &str) -> Result<(), StoreError> {
        let (s, _) = self.state.ready_session()?;
        let mailbox = mailbox.to_string();
        s.run_blocking(|done| Job::Unsubscribe { mailbox, done }, T_CMD).map_err(store_err)
    }

    /// `(LSUB names, hierarchy delimiter, UNSEEN only for subscribed, Available rows from full LIST)`.
    pub fn subscription_snapshot_blocking(
        &self,
    ) -> Result<(Vec<String>, Option<char>, HashMap<String, u32>, Vec<(String, bool, u32)>), StoreError> {
        let subscribed = self.lsub_folder_names_blocking()?;
        let (all_names, delim, unread_all) = self.list_folders_and_unread_blocking()?;
        let lsub_set: HashSet<String> = subscribed.iter().cloned().collect();
        let available: Vec<(String, bool, u32)> = all_names
            .iter()
            .map(|name| (name.clone(), lsub_set.contains(name), unread_all.get(name).copied().unwrap_or(0)))
            .collect();
        let unread_subscribed =
            subscribed.iter().map(|n| (n.clone(), unread_all.get(n).copied().unwrap_or(0))).collect();
        Ok((subscribed, delim, unread_subscribed, available))
    }

    /// `UID SEARCH HEADER name value` in `mailbox` (blocking).
    pub fn uid_search_header_blocking(&self, mailbox: &str, header: &str, value: &str) -> Result<Vec<u32>, StoreError> {
        let (s, _) = self.state.ready_session()?;
        let mailbox = mailbox.to_string();
        let criteria = format!("HEADER {} {}", header, quote_string(value));
        s.run_blocking(|done| Job::UidSearch { mailbox, criteria, done }, T_CMD).map_err(store_err)
    }

    /// APPEND a message; the new UID when the server reports APPENDUID.
    pub fn append_blocking(&self, mailbox: &str, flags: Option<&str>, data: &[u8]) -> Result<Option<u32>, StoreError> {
        let (s, _) = self.state.ready_session()?;
        let mailbox = mailbox.to_string();
        let flags = flags.map(|f| f.to_string());
        let data = data.to_vec();
        s.run_blocking(|done| Job::Append { mailbox, flags, data, done }, T_CMD)
            .map(|au| au.map(|a| a.uid))
            .map_err(store_err)
    }

    /// Flag `uid` deleted and `UID EXPUNGE` it (plain EXPUNGE on servers without UIDPLUS).
    pub fn uid_delete_permanent_blocking(&self, mailbox: &str, uid: u32) -> Result<(), StoreError> {
        let (s, caps) = self.state.ready_session()?;
        let mb = mailbox.to_string();
        let set = uid.to_string();
        s.run_blocking(
            |done| Job::UidStore { mailbox: mb, set, action: "+FLAGS".into(), flags: "(\\Deleted)".into(), done },
            T_CMD,
        )
        .map_err(store_err)?;
        let mb = mailbox.to_string();
        let r = if caps.uidplus {
            s.run_blocking(|done| Job::UidExpunge { mailbox: mb, set: uid.to_string(), done }, T_CMD)
        } else {
            s.run_blocking(|done| Job::Expunge { mailbox: mb, done }, T_CMD)
        };
        r.map_err(store_err)
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
        let limit = limit.clamp(1, 10_000);
        let (s, caps) = self.state.ready_session()?;
        let mb_owned = mailbox.to_string();
        let sel = s
            .run_blocking(|done| Job::Select { mailbox: mb_owned.clone(), done }, T_CMD)
            .map_err(|e| {
                eprintln!("[imap] list_folder_messages_window {}:{} {mailbox:?} SELECT: {e}", self.state.host, self.state.port);
                store_err(e)
            })?;
        let total = sel.exists as u64;
        if total == 0 || start_index >= total {
            return Ok((total, start_index, Vec::new(), "fullScan"));
        }
        let user_at_host = self.state.user_at_host();

        if caps.has("SORT") {
            if let Some(sp) = imap_sort_parentheses_for_symbolic(sort_symbolic) {
                let cached = {
                    let g = self.state.sorted_uid_cache.lock().unwrap();
                    g.as_ref()
                        .filter(|c| c.mailbox == mb_owned && c.uid_validity == sel.uid_validity && c.exists == sel.exists && c.sort_parentheses == sp)
                        .map(|c| c.uids.clone())
                };
                let t0 = std::time::Instant::now();
                let (uids, from_cache) = match cached {
                    Some(u) => (u, true),
                    None => (
                        s.run_blocking(|done| Job::UidSort { mailbox: mb_owned.clone(), criteria: sp.to_string(), done }, T_CMD)
                            .unwrap_or_default(),
                        false,
                    ),
                };
                crate::trace_log!("imap", "window {mb_owned:?}: sort uids={} exists={} cached={from_cache} ({} ms)", uids.len(), sel.exists, t0.elapsed().as_millis());
                if !uids.is_empty() && uids.len() as u32 == sel.exists {
                    if !from_cache {
                        *self.state.sorted_uid_cache.lock().unwrap() = Some(SortedUidListCache {
                            mailbox: mb_owned.clone(),
                            uid_validity: sel.uid_validity,
                            exists: sel.exists,
                            sort_parentheses: sp.to_string(),
                            uids: uids.clone(),
                        });
                    }
                    let slice_end = (start_index + limit).min(total) as usize;
                    let win: Vec<u32> = uids[start_index as usize..slice_end].to_vec();
                    let rows = self.fetch_uid_set_summaries(&s, &mb_owned, &win, SummaryHeaderFields::List)?;
                    let out = rows.iter().map(|it| item_to_summary(it, &user_at_host, &mb_owned)).collect();
                    return Ok((total, start_index, out, "imapSort"));
                }
            }
        }

        // No SORT (or it failed): fetch every summary and sort here.
        let collected: Arc<Mutex<Vec<ConversationSummary>>> = Arc::new(Mutex::new(Vec::new()));
        let c2 = Arc::clone(&collected);
        let ua = user_at_host.clone();
        let mb2 = mb_owned.clone();
        s.run_blocking(
            |done| Job::Fetch {
                mailbox: mb_owned.clone(),
                uid: false,
                set: format!("1:{}", sel.exists),
                items: SummaryHeaderFields::List.fetch_items(),
                on_item: Box::new(move |it| c2.lock().unwrap().push(item_to_summary(&it, &ua, &mb2))),
                stream: None,
                done,
            },
            T_CMD,
        )
        .map_err(|e| {
            eprintln!("[imap] list_folder_messages_window {}:{} {mailbox:?} FETCH: {e}", self.state.host, self.state.port);
            store_err(e)
        })?;
        let mut all = std::mem::take(&mut *collected.lock().unwrap());
        sort_conversation_summaries_for_window(&mut all, sort_symbolic);
        let slice_end = (start_index + limit).min(total) as usize;
        let slice = all[start_index as usize..slice_end].to_vec();
        Ok((total, start_index, slice, "fullScan"))
    }

    /// `UID FETCH uid,…` with list header fields; results are ordered like `uids`.
    fn fetch_uid_set_summaries(&self, s: &ImapSession, mailbox: &str, uids: &[u32], fields: SummaryHeaderFields) -> Result<Vec<FetchItem>, StoreError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        let got: Arc<Mutex<HashMap<u32, FetchItem>>> = Arc::new(Mutex::new(HashMap::new()));
        let g2 = Arc::clone(&got);
        s.run_blocking(
            |done| Job::Fetch {
                mailbox: mailbox.to_string(),
                uid: true,
                set,
                items: fields.fetch_items(),
                on_item: Box::new(move |it| {
                    if let Some(uid) = it.uid {
                        g2.lock().unwrap().insert(uid, it);
                    } else {
                        crate::trace_log!("imap", "FETCH item without UID (seq {}, {} byte literal)", it.seq, it.literal.len());
                    }
                }),
                stream: None,
                done,
            },
            T_CMD,
        )
        .map_err(|e| {
            crate::trace_log!("imap", "uid fetch of {} uids failed: {e}", uids.len());
            store_err(e)
        })?;
        let mut by_uid = std::mem::take(&mut *got.lock().unwrap());
        crate::trace_log!("imap", "uid fetch of {} uids returned {} items", uids.len(), by_uid.len());
        Ok(uids.iter().filter_map(|u| by_uid.remove(u)).collect())
    }
}

fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn item_to_summary(it: &FetchItem, user_at_host: &str, mailbox: &str) -> ConversationSummary {
    ConversationSummary {
        id: imap_message_id(user_at_host, mailbox, it.uid.unwrap_or(0)),
        envelope: envelope_from_header(&it.literal).unwrap_or_else(|_| default_envelope()),
        flags: imap_flags_to_store(&it.flags),
        size: it.size.unwrap_or(0),
    }
}

fn mailbox_info_events(info: &ImapMailboxInfo, on_event: &dyn Fn(OpenFolderEvent)) {
    if !info.flags.is_empty() {
        on_event(OpenFolderEvent::Flags(info.flags.clone()));
    }
    on_event(OpenFolderEvent::Exists(info.exists));
    on_event(OpenFolderEvent::Recent(info.recent));
    if let Some(v) = info.uid_validity {
        on_event(OpenFolderEvent::UidValidity(v));
    }
    if let Some(n) = info.uid_next {
        on_event(OpenFolderEvent::UidNext(n));
    }
    if let Some(rw) = info.read_write {
        on_event(OpenFolderEvent::Other(if rw { "READ-WRITE".into() } else { "READ-ONLY".into() }));
    }
}

impl Store for ImapStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn set_credential(&self, username: Option<&str>, password: &str) {
        let u = username.map(|s| s.to_string()).unwrap_or_else(|| self.state.username.read().unwrap().clone());
        if u.is_empty() {
            return;
        }
        // Preserve existing mechanism if set (e.g. XOAuth2); default to SCRAM-SHA-256.
        let existing = self.state.auth.read().unwrap().as_ref().map(|(_, _, m)| *m).unwrap_or(SaslMechanism::ScramSha256);
        *self.state.auth.write().unwrap() = Some((u, password.to_string(), existing));
        self.close();
    }

    fn set_oauth_credential(&self, email: &str, token: &str) {
        *self.state.username.write().unwrap() = email.to_string();
        *self.state.auth.write().unwrap() = Some((email.to_string(), token.to_string(), SaslMechanism::XOAuth2));
        // Drop the session so the next operation reconnects with the new token.
        self.close();
        *self.state.sorted_uid_cache.lock().unwrap() = None;
    }

    fn list_folders(&self, on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let state = Arc::clone(&self.state);
        s.submit(Job::List {
            pattern: "*".into(),
            status_unseen: false,
            on_row: Box::new(move |entry, _| {
                state.note_delimiter(entry);
                on_folder(ImapStoreState::folder_info(entry));
            }),
            done: Box::new(move |r| on_complete(r.map_err(store_err))),
        });
    }

    fn open_folder(
        &self,
        name: &str,
        on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let state = Arc::clone(&self.state);
        let user_at_host = self.state.user_at_host();
        let mailbox = name.to_string();
        let mb = mailbox.clone();
        s.submit(Job::Select {
            mailbox,
            done: Box::new(move |r| match r {
                Ok(info) => {
                    mailbox_info_events(&info, &*on_event);
                    on_complete(Ok(Box::new(ImapFolder { state, user_at_host, mailbox: mb, exists: info.exists }) as Box<dyn Folder>));
                }
                Err(e) => on_complete(Err(store_err(e))),
            }),
        });
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        self.state.cached_delimiter.lock().ok().and_then(|g| *g).or(Some('/'))
    }

    fn default_folder(&self) -> Option<&str> {
        Some("INBOX")
    }

    fn create_folder(&self, name: &str, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let name_owned = name.to_string();
        let callbacks = self.state.folder_list_callbacks.read().ok().and_then(|g| g.clone());
        let delimiter = self.hierarchy_delimiter();
        s.submit(Job::Create {
            mailbox: name.to_string(),
            done: Box::new(move |r| match r {
                Ok(()) => {
                    if let Some(cbs) = callbacks {
                        (cbs.on_folder_found)(FolderInfo { name: name_owned, delimiter, attributes: vec![] });
                    }
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(store_err(e))),
            }),
        });
    }

    fn rename_folder(&self, old_name: &str, new_name: &str, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let (old_owned, new_owned) = (old_name.to_string(), new_name.to_string());
        let callbacks = self.state.folder_list_callbacks.read().ok().and_then(|g| g.clone());
        let delimiter = self.hierarchy_delimiter();
        s.submit(Job::Rename {
            from: old_name.to_string(),
            to: new_name.to_string(),
            done: Box::new(move |r| match r {
                Ok(()) => {
                    if let Some(cbs) = callbacks {
                        (cbs.on_folder_removed)(&old_owned);
                        (cbs.on_folder_found)(FolderInfo { name: new_owned, delimiter, attributes: vec![] });
                    }
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(store_err(e))),
            }),
        });
    }

    fn delete_folder(&self, name: &str, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let name_owned = name.to_string();
        let callbacks = self.state.folder_list_callbacks.read().ok().and_then(|g| g.clone());
        s.submit(Job::Delete {
            mailbox: name.to_string(),
            done: Box::new(move |r| match r {
                Ok(()) => {
                    if let Some(cbs) = callbacks {
                        (cbs.on_folder_removed)(&name_owned);
                    }
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(store_err(e))),
            }),
        });
    }

    fn set_delete_config(&self, mode: i32, trash_folder: &str) {
        *self.state.delete_mode.write().unwrap() = if mode == 0 { ImapDeleteMode::MarkDeleted } else { ImapDeleteMode::MoveToTrash };
        if !trash_folder.is_empty() {
            *self.state.trash_folder.write().unwrap() = trash_folder.to_string();
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Folder backed by IMAP; uses the store's persistent session.
struct ImapFolder {
    state: Arc<ImapStoreState>,
    user_at_host: String,
    mailbox: String,
    exists: u32,
}

impl ImapFolder {
    fn uid_set(ids: &[&str]) -> Option<String> {
        let uids: Vec<String> = ids.iter().filter_map(|id| parse_uid_from_imap_id(&MessageId::new(*id))).map(|u| u.to_string()).collect();
        (!uids.is_empty()).then(|| uids.join(","))
    }

    /// Fetch one literal-bearing item for `uid`, accumulated.
    fn fetch_literal(s: &ImapSession, mailbox: &str, uid: u32, items: &str, done: Box<dyn FnOnce(Result<Vec<u8>, ImapClientError>) + Send>) {
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let b2 = Arc::clone(&buf);
        s.submit(Job::Fetch {
            mailbox: mailbox.to_string(),
            uid: true,
            set: uid.to_string(),
            items: items.to_string(),
            on_item: Box::new(move |it| {
                if it.uid == Some(uid) || it.uid.is_none() {
                    b2.lock().unwrap().extend_from_slice(&it.literal);
                }
            }),
            stream: None,
            done: Box::new(move |r| done(r.map(|_| std::mem::take(&mut *buf.lock().unwrap())))),
        });
    }

    /// Summaries for a sequence range, streamed as they arrive.
    fn fetch_range(&self, s: &ImapSession, start: u32, end: u32, fields: SummaryHeaderFields, on_item: Box<dyn Fn(FetchItem) + Send>, done: session::Done<()>) {
        s.submit(Job::Fetch {
            mailbox: self.mailbox.clone(),
            uid: false,
            set: format!("{start}:{end}"),
            items: fields.fetch_items(),
            on_item,
            stream: None,
            done,
        });
    }

    fn uid_store(s: &ImapSession, mailbox: &str, set: &str, action: &str, flags: &str, done: session::Done<()>) {
        s.submit(Job::UidStore { mailbox: mailbox.to_string(), set: set.to_string(), action: action.to_string(), flags: flags.to_string(), done });
    }

    /// Flag `set` deleted and expunge it (UID EXPUNGE with UIDPLUS, else EXPUNGE).
    fn delete_uids(s: Arc<ImapSession>, mailbox: String, set: String, done: session::Done<()>) {
        let s2 = Arc::clone(&s);
        let mb = mailbox.clone();
        let set_for_expunge = set.clone();
        Self::uid_store(
            &s,
            &mailbox,
            &set,
            "+FLAGS",
            "(\\Deleted)",
            Box::new(move |r| match r {
                Ok(()) => {
                    if s2.capabilities().uidplus {
                        s2.submit(Job::UidExpunge { mailbox: mb, set: set_for_expunge, done });
                    } else {
                        s2.submit(Job::Expunge { mailbox: mb, done });
                    }
                }
                Err(e) => done(Err(e)),
            }),
        );
    }
}

fn thread_root(header: &[u8]) -> (String, Option<String>) {
    let th = parse_thread_headers(header).unwrap_or_default();
    let root = th
        .references
        .first()
        .cloned()
        .or(th.message_id.clone())
        .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
    (root, th.subject)
}

impl Folder for ImapFolder {
    fn list_conversations(&self, range: Range<u64>, on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let exists = self.exists;
        let start = ((range.start + 1).min(exists as u64 + 1)) as u32;
        let end = (range.end.min(exists as u64)) as u32;
        if start > end {
            return on_complete(Ok(()));
        }
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let (user, mailbox) = (self.user_at_host.clone(), self.mailbox.clone());
        self.fetch_range(
            &s,
            start,
            end,
            SummaryHeaderFields::List,
            Box::new(move |it| on_summary(item_to_summary(&it, &user, &mailbox))),
            Box::new(move |r| on_complete(r.map_err(store_err))),
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
        let Some(uid) = parse_uid_from_imap_id(id) else {
            return on_complete(Err(StoreError::new("invalid message id")));
        };
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        // Stream the body; the envelope goes out as soon as the header
        // block is complete, the rest follows chunk by chunk.
        let header_done = Arc::new(AtomicBool::new(false));
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let on_metadata = Arc::new(on_metadata);
        let on_content_chunk = Arc::new(on_content_chunk);
        let (hd, b, om, oc) = (Arc::clone(&header_done), Arc::clone(&buf), Arc::clone(&on_metadata), Arc::clone(&on_content_chunk));
        let stream = FetchStream {
            on_begin: Box::new(|_| {}),
            on_chunk: Box::new(move |chunk, _ep| {
                if hd.load(Ordering::Relaxed) {
                    oc(chunk);
                    return;
                }
                let mut g = b.lock().unwrap();
                g.extend_from_slice(chunk);
                if let Some(sep) = g.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = g[..sep + 4].to_vec();
                    let rest = g[sep + 4..].to_vec();
                    g.clear();
                    drop(g);
                    om(envelope_from_raw(&header).unwrap_or_else(|_| default_envelope()));
                    oc(&header);
                    if !rest.is_empty() {
                        oc(&rest);
                    }
                    hd.store(true, Ordering::Relaxed);
                }
            }),
        };
        s.submit(Job::Fetch {
            mailbox: self.mailbox.clone(),
            uid: true,
            set: uid.to_string(),
            items: "(BODY[])".into(),
            on_item: Box::new(|_| {}),
            stream: Some(stream),
            done: Box::new(move |r| {
                if !header_done.load(Ordering::Relaxed) {
                    let g = buf.lock().unwrap();
                    if !g.is_empty() {
                        on_metadata(envelope_from_raw(&g).unwrap_or_else(|_| default_envelope()));
                        on_content_chunk(&g);
                    }
                }
                on_complete(r.map_err(store_err));
            }),
        });
    }

    fn get_message_display(&self, id: &MessageId, on_done: Box<dyn FnOnce(Result<MessageForDisplay, StoreError>) + Send>) {
        let Some(uid) = parse_uid_from_imap_id(id) else {
            return on_done(Err(StoreError::new("invalid message id")));
        };
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_done(Err(e)),
        };
        let mailbox = self.mailbox.clone();
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let c2 = Arc::clone(&captured);
        let s2 = Arc::clone(&s);
        let mb = mailbox.clone();
        s.submit(Job::Fetch {
            mailbox: mailbox.clone(),
            uid: true,
            set: uid.to_string(),
            items: "(UID BODYSTRUCTURE)".into(),
            on_item: Box::new(move |it| {
                if let Some(bs) = it.bodystructure {
                    *c2.lock().unwrap() = Some(format!("* {} FETCH (UID {uid} BODYSTRUCTURE {bs})", it.seq));
                }
            }),
            stream: None,
            done: Box::new(move |r| {
                let line = captured.lock().unwrap().take();
                let plan = match (r, line) {
                    (Ok(()), Some(line)) => plan_body_fetch(&line),
                    _ => None,
                };
                match plan {
                    Some(plan) if !matches!(plan.display, DisplayFetch::None) => imap_run_body_plan(s2, mb, uid, plan, on_done),
                    _ => imap_fallback_full_fetch(s2, mb, uid, on_done),
                }
            }),
        });
    }

    fn fetch_message_part(&self, id: &MessageId, imap_section: &str, transfer_encoding: &str, on_done: Box<dyn FnOnce(Result<Vec<u8>, StoreError>) + Send>) {
        let Some(uid) = parse_uid_from_imap_id(id) else {
            return on_done(Err(StoreError::new("invalid message id")));
        };
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_done(Err(e)),
        };
        let enc = transfer_encoding.to_string();
        Self::fetch_literal(
            &s,
            &self.mailbox,
            uid,
            &format!("(BODY.PEEK[{imap_section}])"),
            Box::new(move |r| on_done(r.map(|raw| decode_content_transfer_encoding(&enc, &raw)).map_err(store_err))),
        );
    }

    fn delete_message(&self, id: &MessageId, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let Some(uid) = parse_uid_from_imap_id(id) else {
            return on_complete(Err(StoreError::new("invalid message id")));
        };
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let set = uid.to_string();
        let mailbox = self.mailbox.clone();
        let done: session::Done<()> = Box::new(move |r| on_complete(r.map_err(store_err)));
        match *self.state.delete_mode.read().unwrap() {
            ImapDeleteMode::MarkDeleted => Self::uid_store(&s, &mailbox, &set, "+FLAGS", "(\\Deleted)", done),
            ImapDeleteMode::MoveToTrash => {
                let trash = self.state.trash_folder.read().unwrap().clone();
                let s2 = Arc::clone(&s);
                let trash2 = trash.clone();
                let mb = mailbox.clone();
                ImapStoreState::ensure_mailbox_exists(
                    &s,
                    &trash,
                    Box::new(move |r| match r {
                        Ok(()) => {
                            let s3 = Arc::clone(&s2);
                            let mb2 = mb.clone();
                            let set2 = set.clone();
                            s2.submit(Job::UidCopy {
                                mailbox: mb,
                                set,
                                dest: trash2,
                                done: Box::new(move |r| match r {
                                    Ok(_) => ImapFolder::delete_uids(s3, mb2, set2, done),
                                    Err(e) => done(Err(e)),
                                }),
                            });
                        }
                        Err(e) => done(Err(e)),
                    }),
                );
            }
        }
    }

    fn list_threads(&self, range: Range<u64>, on_thread: Box<dyn Fn(ThreadSummary) + Send + Sync>, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        if self.exists == 0 {
            return on_complete(Ok(()));
        }
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let headers: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let h2 = Arc::clone(&headers);
        self.fetch_range(
            &s,
            1,
            self.exists,
            SummaryHeaderFields::ThreadIndex,
            Box::new(move |it| h2.lock().unwrap().push(it.literal)),
            Box::new(move |r| {
                if let Err(e) = r {
                    return on_complete(Err(store_err(e)));
                }
                let mut groups: HashMap<String, (Option<String>, u64)> = HashMap::new();
                for h in headers.lock().unwrap().iter() {
                    let (root, subject) = thread_root(h);
                    groups.entry(root).or_insert((subject, 0)).1 += 1;
                }
                let mut threads: Vec<(String, Option<String>, u64)> = groups.into_iter().map(|(id, (subject, n))| (id, subject, n)).collect();
                threads.sort_by(|a, b| a.0.cmp(&b.0));
                let start = range.start.min(threads.len() as u64) as usize;
                let end = range.end.min(threads.len() as u64) as usize;
                for t in threads.into_iter().skip(start).take(end.saturating_sub(start)) {
                    on_thread(ThreadSummary { id: ThreadId(t.0), subject: t.1, message_count: t.2 });
                }
                on_complete(Ok(()));
            }),
        );
    }

    fn list_messages_in_thread(&self, thread_id: &ThreadId, range: Range<u64>, on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        if self.exists == 0 {
            return on_complete(Ok(()));
        }
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let (user, mailbox) = (self.user_at_host.clone(), self.mailbox.clone());
        let wanted = thread_id.as_str().to_string();
        let items: Arc<Mutex<Vec<FetchItem>>> = Arc::new(Mutex::new(Vec::new()));
        let i2 = Arc::clone(&items);
        self.fetch_range(
            &s,
            1,
            self.exists,
            SummaryHeaderFields::ThreadDrillDown,
            Box::new(move |it| i2.lock().unwrap().push(it)),
            Box::new(move |r| {
                if let Err(e) = r {
                    return on_complete(Err(store_err(e)));
                }
                let mut in_thread: Vec<ConversationSummary> = items
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|it| thread_root(&it.literal).0 == wanted)
                    .map(|it| item_to_summary(it, &user, &mailbox))
                    .collect();
                in_thread.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
                let start = range.start.min(in_thread.len() as u64) as usize;
                let end = range.end.min(in_thread.len() as u64) as usize;
                for sm in in_thread.into_iter().skip(start).take(end.saturating_sub(start)) {
                    on_summary(sm);
                }
                on_complete(Ok(()));
            }),
        );
    }

    fn append_message(&self, data: &[u8], on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        s.submit(Job::Append {
            mailbox: self.mailbox.clone(),
            flags: None,
            data: data.to_vec(),
            done: Box::new(move |r| on_complete(r.map(|_| ()).map_err(store_err))),
        });
    }

    fn copy_messages_to(&self, ids: &[&str], dest_folder_name: &str, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let Some(set) = Self::uid_set(ids) else {
            return on_complete(Err(StoreError::new("no valid UIDs to copy")));
        };
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let (mailbox, dest) = (self.mailbox.clone(), dest_folder_name.to_string());
        let s2 = Arc::clone(&s);
        ImapStoreState::ensure_mailbox_exists(
            &s,
            dest_folder_name,
            Box::new(move |r| match r {
                Ok(()) => s2.submit(Job::UidCopy { mailbox, set, dest, done: Box::new(move |r| on_complete(r.map(|_| ()).map_err(store_err))) }),
                Err(e) => on_complete(Err(store_err(e))),
            }),
        );
    }

    fn move_messages_to(&self, ids: &[&str], dest_folder_name: &str, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let Some(set) = Self::uid_set(ids) else {
            return on_complete(Err(StoreError::new("no valid UIDs to move")));
        };
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let (mailbox, dest) = (self.mailbox.clone(), dest_folder_name.to_string());
        let s2 = Arc::clone(&s);
        let done: session::Done<()> = Box::new(move |r| on_complete(r.map_err(store_err)));
        ImapStoreState::ensure_mailbox_exists(
            &s,
            dest_folder_name,
            Box::new(move |r| {
                if let Err(e) = r {
                    return done(Err(e));
                }
                // UID MOVE when offered; else COPY + \Deleted + expunge.
                if s2.capabilities().move_ {
                    s2.submit(Job::UidMove { mailbox, set, dest, done: Box::new(move |r| done(r.map(|_| ()))) });
                } else {
                    let s3 = Arc::clone(&s2);
                    let (mb2, set2) = (mailbox.clone(), set.clone());
                    s2.submit(Job::UidCopy {
                        mailbox,
                        set,
                        dest,
                        done: Box::new(move |r| match r {
                            Ok(_) => ImapFolder::delete_uids(s3, mb2, set2, done),
                            Err(e) => done(Err(e)),
                        }),
                    });
                }
            }),
        );
    }

    fn store_flags(&self, ids: &[&str], add: &[Flag], remove: &[Flag], on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let Some(set) = Self::uid_set(ids) else {
            return on_complete(Err(StoreError::new("no valid UIDs for store_flags")));
        };
        let add_flags: Vec<String> = add.iter().map(flag_to_imap_string).collect();
        let remove_flags: Vec<String> = remove.iter().map(flag_to_imap_string).collect();
        if add_flags.is_empty() && remove_flags.is_empty() {
            return on_complete(Ok(()));
        }
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        let mailbox = self.mailbox.clone();
        let done: session::Done<()> = Box::new(move |r| on_complete(r.map_err(store_err)));
        if add_flags.is_empty() {
            return Self::uid_store(&s, &mailbox, &set, "-FLAGS", &format!("({})", remove_flags.join(" ")), done);
        }
        let add_action = format!("({})", add_flags.join(" "));
        if remove_flags.is_empty() {
            return Self::uid_store(&s, &mailbox, &set, "+FLAGS", &add_action, done);
        }
        let s2 = Arc::clone(&s);
        let (mb2, set2) = (mailbox.clone(), set.clone());
        let remove_action = format!("({})", remove_flags.join(" "));
        Self::uid_store(
            &s,
            &mailbox,
            &set,
            "+FLAGS",
            &add_action,
            Box::new(move |r| match r {
                Ok(()) => ImapFolder::uid_store(&s2, &mb2, &set2, "-FLAGS", &remove_action, done),
                Err(e) => done(Err(e)),
            }),
        );
    }

    fn expunge(&self, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        s.submit(Job::Expunge { mailbox: self.mailbox.clone(), done: Box::new(move |r| on_complete(r.map_err(store_err))) });
    }

    fn mark_all_read(&self, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let s = match self.state.ensure_session() {
            Ok(s) => s,
            Err(e) => return on_complete(Err(e)),
        };
        Self::uid_store(&s, &self.mailbox, "1:*", "+FLAGS", "(\\Seen)", Box::new(move |r| on_complete(r.map_err(store_err))));
    }
}

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

fn imap_fallback_full_fetch(s: Arc<ImapSession>, mailbox: String, uid: u32, on_done: Box<dyn FnOnce(Result<MessageForDisplay, StoreError>) + Send>) {
    ImapFolder::fetch_literal(
        &s,
        &mailbox,
        uid,
        "(BODY[])",
        Box::new(move |r| on_done(r.map(|raw| message_for_display_from_raw(&raw)).map_err(store_err))),
    );
}

fn imap_run_body_plan(s: Arc<ImapSession>, mailbox: String, uid: u32, plan: BodyFetchPlan, on_done: Box<dyn FnOnce(Result<MessageForDisplay, StoreError>) + Send>) {
    let s2 = Arc::clone(&s);
    let mb = mailbox.clone();
    ImapFolder::fetch_literal(
        &s,
        &mailbox,
        uid,
        MESSAGE_DETAIL_ENVELOPE_PEEK,
        Box::new(move |r| {
            let Ok(headers) = r else {
                return imap_fallback_full_fetch(s2, mb, uid, on_done);
            };
            let env = envelope_from_header(&headers).unwrap_or_else(|_| default_envelope());
            crate::trace_log!("imap", "body plan uid {uid}: header {} bytes, display {}", headers.len(), match &plan.display { DisplayFetch::None => "none".to_string(), DisplayFetch::TextPart { section, .. } => format!("text part {section}"), DisplayFetch::NestedMessage { section, .. } => format!("nested message {section}") });
            let attachments = imap_attachment_refs_from_plan(&plan);
            let s3 = Arc::clone(&s2);
            let mb2 = mb.clone();
            match plan.display {
                DisplayFetch::None => imap_fallback_full_fetch(s2, mb, uid, on_done),
                DisplayFetch::TextPart { section, encoding, is_html, charset_hint } => {
                    ImapFolder::fetch_literal(
                        &s2,
                        &mb,
                        uid,
                        &format!("(BODY.PEEK[{section}])"),
                        Box::new(move |r| {
                            let Ok(raw_body) = r else {
                                return imap_fallback_full_fetch(s3, mb2, uid, on_done);
                            };
                            let decoded = decode_content_transfer_encoding(&encoding, &raw_body);
                            let text = bodystructure::part_bytes_to_string(&decoded, charset_hint.as_deref());
                            crate::trace_log!("imap", "body part {section} uid {uid}: raw {} bytes, encoding {encoding:?}, decoded {} bytes, text {} chars, html={is_html}", raw_body.len(), decoded.len(), text.len());
                            let (body_plain, body_html) = if is_html { (None, Some(text)) } else { (Some(text), None) };
                            on_done(Ok(MessageForDisplay { envelope: env, body_plain, body_html, attachments }));
                        }),
                    );
                }
                DisplayFetch::NestedMessage { section, encoding } => {
                    ImapFolder::fetch_literal(
                        &s2,
                        &mb,
                        uid,
                        &format!("(BODY.PEEK[{section}])"),
                        Box::new(move |r| {
                            let Ok(raw_body) = r else {
                                return imap_fallback_full_fetch(s3, mb2, uid, on_done);
                            };
                            let decoded = decode_content_transfer_encoding(&encoding, &raw_body);
                            let (plain, html, _) = extract_structured_body(&decoded).unwrap_or((None, None, vec![]));
                            on_done(Ok(MessageForDisplay { envelope: env, body_plain: plain, body_html: html, attachments }));
                        }),
                    );
                }
            }
        }),
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
    Ok(rfc5322_envelope_to_store(&parse_envelope(header)?))
}

fn envelope_from_raw(raw: &[u8]) -> Result<Envelope, crate::mime::MimeParseError> {
    envelope_from_header(raw)
}

fn rfc5322_envelope_to_store(rfc: &EnvelopeHeaders) -> Envelope {
    Envelope {
        from: rfc.from.iter().map(email_to_address).collect(),
        to: rfc.to.iter().map(email_to_address).collect(),
        cc: rfc.cc.iter().map(email_to_address).collect(),
        date: rfc.date.map(|dt| DateTime { timestamp: dt.timestamp(), tz_offset_secs: Some(dt.offset().local_minus_utc()) }),
        subject: rfc.subject.clone(),
        message_id: rfc.message_id.as_ref().map(|c| c.to_string()),
        in_reply_to: rfc.in_reply_to.clone(),
        references: rfc.references.clone(),
    }
}

fn email_to_address(e: &EmailAddress) -> Address {
    Address { display_name: e.display_name.clone(), local_part: e.local_part.clone(), domain: Some(e.domain.clone()) }
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

fn imap_flags_to_store(flags: &[String]) -> HashSet<Flag> {
    flags
        .iter()
        .map(|s| {
            let s = s.trim_matches('\\');
            match s.to_uppercase().as_str() {
                "SEEN" => Flag::Seen,
                "ANSWERED" => Flag::Answered,
                "FLAGGED" => Flag::Flagged,
                "DELETED" => Flag::Deleted,
                "DRAFT" => Flag::Draft,
                _ => Flag::Custom(s.to_string()),
            }
        })
        .collect()
}

fn default_envelope() -> Envelope {
    Envelope { from: Vec::new(), to: Vec::new(), cc: Vec::new(), date: None, subject: None, message_id: None, in_reply_to: None, references: None }
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
