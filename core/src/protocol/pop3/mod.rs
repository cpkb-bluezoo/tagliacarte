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

//! POP3 client (Store with single INBOX folder).
//!
//! All trait methods are callback-driven and return immediately.
//! POP3 is session-based (connect per operation), so the callbacks fire inline.

mod client;

pub use client::{ListEntry, Pop3ClientError, Pop3Session, StatResponse, UidlEntry};

use crate::message_id::{pop3_message_id, MessageId};
use crate::mime::{parse_envelope, EmailAddress, EnvelopeHeaders};
use crate::store::{
    Address, ConversationSummary, DateTime, Envelope, Folder, FolderInfo,
    OpenFolderEvent, Store, StoreError, StoreKind, ThreadId,
};
use std::ops::Range;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};

/// Shared state for POP3: host, port, TLS, auth. No persistent session; connect per operation.
struct Pop3StoreState {
    host: String,
    port: u16,
    use_implicit_tls: RwLock<bool>,
    auth: RwLock<Option<(String, String)>>,
    username: RwLock<String>,
    /// Handle to the shared tokio runtime (set by FFI layer at creation).
    runtime_handle: tokio::runtime::Handle,
}

impl Pop3StoreState {
    /// Run an async operation with a fresh session: connect, greet, login, run f(session, data), quit.
    /// Pass data so the future can own it and avoid borrowing from the caller.
    fn with_session<D, F, R>(&self, data: D, f: F) -> Result<R, StoreError>
    where
        D: Send,
        F: for<'s> FnOnce(&'s mut Pop3Session, D) -> Pin<Box<dyn std::future::Future<Output = Result<R, Pop3ClientError>> + 's>>,
        R: Send,
    {
        let host = self.host.clone();
        let port = self.port;
        let use_tls = *self.use_implicit_tls.read().map_err(|e| StoreError::new(e.to_string()))?;
        let auth = self.auth.read().map_err(|e| StoreError::new(e.to_string()))?.clone();

        let (username, password) = match auth {
            Some((u, p)) => (u, p),
            None => {
                let username = self.username.read().map_err(|e| StoreError::new(e.to_string()))?.clone();
                let is_plaintext = !use_tls;
                return Err(StoreError::NeedsCredential { username, is_plaintext });
            }
        };

        self.runtime_handle.block_on(async move {
            let mut session = Pop3Session::connect(&host, port, use_tls).await.map_err(|e| StoreError::new(e.to_string()))?;
            session.read_greeting().await.map_err(|e| StoreError::new(e.to_string()))?;
            session.login(&username, &password).await.map_err(|e| StoreError::new(e.to_string()))?;
            let result = f(&mut session, data).await.map_err(|e| StoreError::new(e.to_string()))?;
            let _ = session.quit().await;
            Ok(result)
        })
    }
}

/// POP3 store (single folder INBOX).
pub struct Pop3Store {
    state: Arc<Pop3StoreState>,
}

impl Pop3Store {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self::with_runtime_handle(host, port, tokio::runtime::Handle::current())
    }

    /// Create a Pop3Store with an explicit tokio runtime handle (used by FFI with the shared runtime).
    pub fn with_runtime_handle(host: impl Into<String>, port: u16, handle: tokio::runtime::Handle) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 995;
        let state = Pop3StoreState {
            host: host.clone(),
            port,
            use_implicit_tls: RwLock::new(use_implicit_tls),
            auth: RwLock::new(None),
            username: RwLock::new(String::new()),
            runtime_handle: handle,
        };
        Self {
            state: Arc::new(state),
        }
    }

    pub fn set_implicit_tls(&mut self, use_tls: bool) -> &mut Self {
        *self.state.use_implicit_tls.write().unwrap() = use_tls;
        self
    }

    pub fn set_auth(&mut self, username: impl Into<String>, password: impl Into<String>) -> &mut Self {
        let u = username.into();
        if self.state.username.read().unwrap().is_empty() {
            *self.state.username.write().unwrap() = u.clone();
        }
        *self.state.auth.write().unwrap() = Some((u, password.into()));
        self
    }

    pub fn set_username(&mut self, user_at_host: impl Into<String>) -> &mut Self {
        *self.state.username.write().unwrap() = user_at_host.into();
        self
    }
}

impl Store for Pop3Store {
    fn set_credential(&self, username: Option<&str>, password: &str) {
        let u = username
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.state.username.read().unwrap().clone());
        if u.is_empty() {
            return;
        }
        *self.state.auth.write().unwrap() = Some((u, password.to_string()));
    }
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_folder(FolderInfo {
            name: "INBOX".to_string(),
            delimiter: None,
            attributes: vec![],
        });
        on_complete(Ok(()));
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        if name != "INBOX" {
            on_complete(Err(StoreError::new("POP3 only has INBOX")));
            return;
        }
        let stat = match self.state.with_session((), |s, ()| Box::pin(async { s.stat().await })) {
            Ok(s) => s,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let uidl_list = match self.state.with_session((), |s, ()| Box::pin(async { s.uidl(None).await })) {
            Ok(u) => u,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let list_entries = match self.state.with_session((), |s, ()| Box::pin(async { s.list(None).await })) {
            Ok(l) => l,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let size_map: std::collections::HashMap<u32, u64> = list_entries.into_iter().map(|e| (e.msg_no, e.size)).collect();
        let mut entries: Vec<(u32, String, u64)> = uidl_list
            .into_iter()
            .map(|u| (u.msg_no, u.uidl, size_map.get(&u.msg_no).copied().unwrap_or(0)))
            .collect();
        entries.sort_by_key(|e| e.0);

        let username = if self.state.username.read().unwrap().is_empty() {
            self.state.auth.read().unwrap().as_ref().map(|(u, _)| u.clone()).unwrap_or_default()
        } else {
            self.state.username.read().unwrap().clone()
        };
        let user_at_host = if username.contains('@') {
            username
        } else {
            format!("{}@{}", username, self.state.host)
        };

        on_complete(Ok(Box::new(Pop3Folder {
            state: Arc::clone(&self.state),
            user_at_host,
            count: stat.count,
            entries: Mutex::new(entries),
        })));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        Some("INBOX")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Cached entry: (msg_no, uidl, size). Sorted by msg_no; index 0 = first message.
struct Pop3Folder {
    state: Arc<Pop3StoreState>,
    user_at_host: String,
    count: u32,
    entries: Mutex<Vec<(u32, String, u64)>>,
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

fn envelope_from_raw(raw: &[u8]) -> Result<Envelope, crate::mime::MimeParseError> {
    let rfc = parse_envelope(raw)?;
    Ok(rfc5322_envelope_to_store(&rfc))
}

/// Parse MessageId to get uidl. Format: pop3://user_at_host/uidl
fn parse_uidl_from_pop3_id(id: &MessageId) -> Option<String> {
    let s = id.as_str();
    let rest = s.strip_prefix("pop3://")?;
    let (_, uidl) = rest.rsplit_once('/')?;
    Some(uidl.to_string())
}

impl Folder for Pop3Folder {
    fn list_conversations(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let entries = match self.entries.lock() {
            Ok(e) => e,
            Err(e) => {
                on_complete(Err(StoreError::new(e.to_string())));
                return;
            }
        };
        let start = (range.start as usize).min(entries.len());
        let end = (range.end as usize).min(entries.len());
        if start >= end {
            on_complete(Ok(()));
            return;
        }
        let slice: Vec<(u32, String, u64)> = entries[start..end].to_vec();
        drop(entries);

        let user_at_host = self.user_at_host.clone();
        let summaries = match self.state.with_session((slice, user_at_host), |session, (slice, user_at_host)| {
            Box::pin(async move {
                let mut out = Vec::new();
                for (msg_no, uidl, size) in &slice {
                    let header_bytes = session.top(*msg_no, 0).await?;
                    let envelope = envelope_from_raw(&header_bytes).unwrap_or_else(|_| default_envelope());
                    let id = pop3_message_id(&user_at_host, uidl);
                    out.push(ConversationSummary {
                        id,
                        envelope,
                        flags: std::collections::HashSet::new(),
                        size: *size,
                    });
                }
                Ok(out)
            })
        }) {
            Ok(s) => s,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        for s in summaries {
            on_summary(s);
        }
        on_complete(Ok(()));
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        on_complete(Ok(self.count as u64));
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let uidl = match parse_uidl_from_pop3_id(id) {
            Some(u) => u,
            None => {
                on_complete(Err(StoreError::new("invalid pop3 message id")));
                return;
            }
        };
        let entries = match self.entries.lock() {
            Ok(e) => e,
            Err(e) => {
                on_complete(Err(StoreError::new(e.to_string())));
                return;
            }
        };
        let msg_no = entries.iter().find(|(_, u, _)| u == &uidl).map(|(mn, _, _)| *mn);
        drop(entries);
        let msg_no = match msg_no {
            Some(n) => n,
            None => {
                on_complete(Err(StoreError::new("message not found")));
                return;
            }
        };

        let raw = match self.state.with_session(msg_no, |s, msg_no| Box::pin(async move { s.retr(msg_no).await })) {
            Ok(r) => r,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let envelope = envelope_from_raw(&raw).unwrap_or_else(|_| default_envelope());
        on_metadata(envelope);
        on_content_chunk(&raw);
        on_complete(Ok(()));
    }

    fn list_messages_in_thread(
        &self,
        _thread_id: &ThreadId,
        _range: Range<u64>,
        _on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Ok(()));
    }
}
