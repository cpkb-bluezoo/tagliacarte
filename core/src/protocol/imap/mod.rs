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

mod client;

pub use client::{
    connect_and_authenticate, AuthenticatedSession, FetchSummary, ImapClientError, ImapLine,
    ImapLineWithLiteral, ListEntry, SelectEvent, SelectResult,
};

use crate::message_id::{imap_message_id, MessageId};
use crate::mime::{extract_structured_body, parse_envelope, parse_thread_headers, EmailAddress, EnvelopeHeaders};
use crate::store::{Address, Attachment, ConversationSummary, DateTime, Envelope, Flag, Message};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::{ThreadId, ThreadSummary};
use crate::sasl::SaslMechanism;
use std::ops::Range;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300;

/// Shared state for IMAP: persistent session, idle timeout, reconnect. Store and folders hold Arc<this>.
struct ImapStoreState {
    host: String,
    port: u16,
    use_implicit_tls: RwLock<bool>,
    use_starttls: RwLock<bool>,
    auth: RwLock<Option<(String, String, SaslMechanism)>>,
    username: RwLock<String>,
    idle_timeout_secs: RwLock<u64>,
    runtime: once_cell::sync::OnceCell<tokio::runtime::Runtime>,
    connection_state: Arc<Mutex<(Option<AuthenticatedSession>, Instant)>>,
}

impl ImapStoreState {
    fn runtime(&self) -> Result<&tokio::runtime::Runtime, StoreError> {
        self.runtime.get_or_try_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| StoreError::new(e.to_string()))
        })
    }

    /// Run an async operation with the shared session. Takes session out (or connects), runs f, puts session back.
    /// The closure receives (mailbox, session): mailbox is Some(&str) when a folder was requested (session is already selected);
    /// use it only to build the future (e.g. session.select(mb)). The future must not capture any reference from outside this call.
    fn with_session<F, R>(&self, mailbox: Option<&str>, f: F) -> Result<R, StoreError>
    where
        F: for<'s> FnOnce(Option<&'s str>, &'s mut AuthenticatedSession) -> Pin<Box<dyn std::future::Future<Output = Result<R, client::ImapClientError>> + Send + 's>>,
        R: Send,
    {
        let rt = self.runtime()?;
        let state = Arc::clone(&self.connection_state);
        let host = self.host.clone();
        let port = self.port;
        let use_implicit_tls = *self.use_implicit_tls.read().map_err(|e| StoreError::new(e.to_string()))?;
        let use_starttls = *self.use_starttls.read().map_err(|e| StoreError::new(e.to_string()))?;
        let auth = self.auth.read().map_err(|e| StoreError::new(e.to_string()))?.as_ref().map(|(u, p, m)| (u.clone(), p.clone(), *m));
        let idle_timeout = Duration::from_secs(*self.idle_timeout_secs.read().map_err(|e| StoreError::new(e.to_string()))?);
        let mailbox = mailbox.map(|s| s.to_string());

        rt.block_on(async move {
            let mut session = {
                let mut guard = state.lock().map_err(|e| StoreError::new(e.to_string()))?;
                let expired = guard.0.as_ref().map_or(true, |_| guard.1.elapsed() > idle_timeout);
                if expired {
                    guard.0 = None;
                }
                let s = guard.0.take();
                drop(guard);
                s
            };
            if session.is_none() {
                let auth_ref = auth.as_ref().map(|(u, p, m)| (u.as_str(), p.as_str(), *m));
                session = Some(
                    connect_and_authenticate(&host, port, use_implicit_tls, use_starttls, auth_ref)
                        .await
                        .map_err(|e| StoreError::new(e.to_string()))?,
                );
            }
            let mut session = session.unwrap();
            if let Some(ref mb) = mailbox {
                session
                    .select(mb)
                    .await
                    .map_err(|e| StoreError::new(e.to_string()))?;
            }
            let result = {
                let mut fut = f(mailbox.as_deref(), &mut session);
                fut.as_mut().await.map_err(|e| StoreError::new(e.to_string()))?
            };
            let mut guard = state.lock().map_err(|e| StoreError::new(e.to_string()))?;
            guard.0 = Some(session);
            guard.1 = Instant::now();
            Ok(result)
        })
    }
}

/// IMAP store. Holds persistent client (connection reuse, idle timeout, reconnect).
pub struct ImapStore {
    state: Arc<ImapStoreState>,
}

impl ImapStore {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 993;
        let state = ImapStoreState {
            host: host.clone(),
            port,
            use_implicit_tls: RwLock::new(use_implicit_tls),
            use_starttls: RwLock::new(true),
            auth: RwLock::new(None),
            username: RwLock::new(String::new()),
            idle_timeout_secs: RwLock::new(DEFAULT_IDLE_TIMEOUT_SECS),
            runtime: once_cell::sync::OnceCell::new(),
            connection_state: Arc::new(Mutex::new((None, Instant::now()))),
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

    pub fn set_username(&mut self, user_at_host: impl Into<String>) -> &mut Self {
        *self.state.username.write().unwrap() = user_at_host.into();
        self
    }

    /// Set idle timeout in seconds; connection is dropped after this period of inactivity. Default 300.
    pub fn set_idle_timeout_secs(&mut self, secs: u64) -> &mut Self {
        *self.state.idle_timeout_secs.write().unwrap() = secs;
        self
    }
}

impl Store for ImapStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn list_folders(&self) -> Result<Vec<FolderInfo>, StoreError> {
        let entries = self.state.with_session(None, |_mb, session| Box::pin(session.list_folders()))?;
        let delimiter = entries.first().and_then(|e| e.delimiter);
        Ok(entries
            .into_iter()
            .map(|e| FolderInfo {
                name: e.name,
                delimiter,
                attributes: e.attributes,
            })
            .collect())
    }

    fn refresh_folders_streaming(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        self.state.with_session(None, move |_mb, session| {
            let on_folder = on_folder;
            let on_complete = on_complete;
            Box::pin(async move {
                session
                    .list_folders_streaming(|entry| {
                        on_folder(FolderInfo {
                            name: entry.name.clone(),
                            delimiter: entry.delimiter,
                            attributes: entry.attributes.clone(),
                        });
                    })
                    .await?;
                on_complete(Ok(()));
                Ok(())
            })
        })?;
        Ok(())
    }

    fn open_folder(&self, name: &str) -> Result<Box<dyn Folder>, StoreError> {
        let select_result = self.state.with_session(Some(name), |mb, session| {
            Box::pin(session.select(mb.unwrap()))
        })?;
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
            format!("{}@{}", username, self.state.host)
        };
        Ok(Box::new(ImapFolder {
            state: Arc::clone(&self.state),
            user_at_host,
            mailbox: name.to_string(),
            exists: select_result.exists,
        }))
    }

    fn start_open_folder_streaming(
        &self,
        name: &str,
        on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        let name = name.to_string();
        let name_for_session = name.clone();
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
        let select_result = state.with_session(Some(&name_for_session), move |mb, session| {
            Box::pin(async move {
                let select_result = session
                    .select_streaming(mb.unwrap(), |ev| {
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
                    })
                    .await?;
                Ok(select_result)
            })
        })?;
        let folder = Box::new(ImapFolder {
            state: Arc::clone(&self.state),
            user_at_host,
            mailbox: name,
            exists: select_result.exists,
        }) as Box<dyn Folder>;
        on_complete(Ok(folder));
        Ok(())
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        Some('/')
    }

    fn default_folder(&self) -> Option<&str> {
        Some("INBOX")
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
    fn list_conversations(&self, range: Range<u64>) -> Result<Vec<ConversationSummary>, StoreError> {
        let exists = self.exists;
        let start = ((range.start + 1).min(exists as u64 + 1)) as u32;
        let end = (range.end.min(exists as u64)) as u32;
        if start > end {
            return Ok(Vec::new());
        }
        let mailbox = self.mailbox.clone();
        let summaries = self.state.with_session(Some(&mailbox), |_mb, session| {
            Box::pin(session.fetch_summaries(start, end))
        })?;
        let user = self.user_at_host.clone();
        let mailbox = self.mailbox.clone();
        let mut out = Vec::new();
        for s in summaries {
            let envelope = envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
            let id = imap_message_id(&user, &mailbox, s.uid);
            let flags = imap_flags_to_store(&s.flags);
            out.push(ConversationSummary {
                id,
                envelope,
                flags,
                size: s.size as u64,
            });
        }
        Ok(out)
    }

    fn message_count(&self) -> Result<u64, StoreError> {
        Ok(self.exists as u64)
    }

    fn request_message_list_streaming(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        let exists = self.exists;
        let start = ((range.start + 1).min(exists as u64 + 1)) as u32;
        let end = (range.end.min(exists as u64)) as u32;
        if start > end {
            on_complete(Ok(()));
            return Ok(());
        }
        let mailbox = self.mailbox.clone();
        let user = self.user_at_host.clone();
        let mailbox_name = self.mailbox.clone();
        self.state.with_session(Some(&mailbox), move |_mb, session| {
            let user = user.clone();
            let mailbox_name = mailbox_name.clone();
            let on_summary = on_summary;
            let on_complete = on_complete;
            Box::pin(async move {
                session
                    .fetch_summaries_streaming(start, end, |s| {
                        let envelope = envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
                        let id = imap_message_id(&user, &mailbox_name, s.uid);
                        let flags = imap_flags_to_store(&s.flags);
                        on_summary(ConversationSummary {
                            id,
                            envelope,
                            flags,
                            size: s.size as u64,
                        });
                    })
                    .await?;
                on_complete(Ok(()));
                Ok(())
            })
        })?;
        Ok(())
    }

    fn request_message_streaming(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        let uid = match parse_uid_from_imap_id(id) {
            Some(u) => u,
            None => {
                on_complete(Err(StoreError::new("invalid message id")));
                return Ok(());
            }
        };
        let mailbox = self.mailbox.clone();
        const CHUNK_SIZE: usize = 8192;
        self.state.with_session(Some(&mailbox), move |_mb, session| {
            let on_metadata = on_metadata;
            let on_content_chunk = on_content_chunk;
            let on_complete = on_complete;
            Box::pin(async move {
                let mut header_done = false;
                let mut buf = Vec::new();
                session
                    .fetch_body_by_uid_streaming(uid, CHUNK_SIZE, |chunk| {
                        if !header_done {
                            buf.extend_from_slice(chunk);
                            if let Some(sep) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                let header_bytes = &buf[..sep + 4];
                                let body_start = &buf[sep + 4..];
                                if let Ok(env) = envelope_from_raw(header_bytes) {
                                    on_metadata(env);
                                } else {
                                    on_metadata(default_envelope());
                                }
                                on_content_chunk(header_bytes);
                                if !body_start.is_empty() {
                                    on_content_chunk(body_start);
                                }
                                header_done = true;
                                buf.clear();
                            }
                        } else {
                            on_content_chunk(chunk);
                        }
                    })
                    .await?;
                if !header_done && !buf.is_empty() {
                    if let Ok(env) = envelope_from_raw(&buf) {
                        on_metadata(env);
                    } else {
                        on_metadata(default_envelope());
                    }
                    on_content_chunk(&buf);
                }
                on_complete(Ok(()));
                Ok(())
            })
        })?;
        Ok(())
    }

    fn get_message(&self, id: &MessageId) -> Result<Option<Message>, StoreError> {
        let uid = match parse_uid_from_imap_id(id) {
            Some(u) => u,
            None => return Ok(None),
        };
        let mailbox = self.mailbox.clone();
        let raw = self
            .state
            .with_session(Some(&mailbox), |_mb, session| Box::pin(session.fetch_body_by_uid(uid)))?;
        let envelope = envelope_from_raw(&raw).unwrap_or_else(|_| default_envelope());
        let flags = std::collections::HashSet::new();
        let (body_plain, body_html, att_list) =
            extract_structured_body(&raw).unwrap_or((None, None, Vec::new()));
        let attachments: Vec<Attachment> = att_list
            .into_iter()
            .map(|(filename, mime_type, content)| Attachment {
                filename,
                mime_type,
                content,
            })
            .collect();
        Ok(Some(Message {
            id: id.clone(),
            envelope,
            flags,
            size: raw.len() as u64,
            body_plain,
            body_html,
            attachments,
            raw: Some(raw),
        }))
    }

    fn list_threads(&self, range: Range<u64>) -> Result<Vec<ThreadSummary>, StoreError> {
        let exists = self.exists;
        if exists == 0 {
            return Ok(Vec::new());
        }
        let mailbox = self.mailbox.clone();
        let user = self.user_at_host.clone();
        let summaries = self.state.with_session(Some(&mailbox), |_mb, session| {
            Box::pin(session.fetch_summaries(1, exists))
        })?;
        let mut thread_groups: std::collections::HashMap<String, (Option<String>, Vec<ConversationSummary>)> =
            std::collections::HashMap::new();
        for s in &summaries {
            let th = parse_thread_headers(&s.header).unwrap_or_default();
            let root = th
                .references
                .first()
                .cloned()
                .or(th.message_id.clone())
                .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
            let envelope = envelope_from_header(&s.header).unwrap_or_else(|_| default_envelope());
            let id = imap_message_id(&user, &mailbox, s.uid);
            let flags = imap_flags_to_store(&s.flags);
            let summary = ConversationSummary {
                id,
                envelope,
                flags,
                size: s.size as u64,
            };
            thread_groups
                .entry(root.clone())
                .or_insert((th.subject.clone(), Vec::new()))
                .1
                .push(summary);
        }
        let mut threads: Vec<ThreadSummary> = thread_groups
            .into_iter()
            .map(|(id, (subject, msgs))| ThreadSummary {
                id: ThreadId(id),
                subject,
                message_count: msgs.len() as u64,
            })
            .collect();
        threads.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        let start = range.start.min(threads.len() as u64) as usize;
        let end = range.end.min(threads.len() as u64) as usize;
        if start >= end {
            return Ok(Vec::new());
        }
        Ok(threads[start..end].to_vec())
    }

    fn list_messages_in_thread(
        &self,
        thread_id: &ThreadId,
        range: Range<u64>,
    ) -> Result<Vec<ConversationSummary>, StoreError> {
        let exists = self.exists;
        if exists == 0 {
            return Ok(Vec::new());
        }
        let mailbox = self.mailbox.clone();
        let user = self.user_at_host.clone();
        let summaries = self.state.with_session(Some(&mailbox), |_mb, session| {
            Box::pin(session.fetch_summaries(1, exists))
        })?;
        let mut in_thread = Vec::new();
        for s in &summaries {
            let th = parse_thread_headers(&s.header).unwrap_or_default();
            let root = th
                .references
                .first()
                .cloned()
                .or(th.message_id.clone())
                .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
            if root != thread_id.as_str() {
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
        if start >= end {
            return Ok(Vec::new());
        }
        Ok(in_thread[start..end].to_vec())
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
