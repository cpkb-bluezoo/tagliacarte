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

//! POP3 client (Store with single INBOX folder), on hopf (`hopf-pop3`).
//!
//! POP3 is session-based: every operation is one `Pop3Script` session
//! (connect, CAPA, STLS per policy, authenticate, run the commands, QUIT)
//! whose results arrive in one callback on hopf's reactor. The Store/Folder
//! methods return as soon as the dial is started.

use crate::message_id::{pop3_message_id, MessageId};
use crate::mime::{parse_envelope, EmailAddress, EnvelopeHeaders};
use crate::net::{hopf_runtime, hopf_tls_connector};
use crate::store::{
    Address, ConversationSummary, DateTime, Envelope, Folder, FolderInfo, OpenFolderEvent, Store,
    StoreError, StoreKind, ThreadId,
};
use hopf_pop3::{Pop3Client, Pop3ClientTimeouts, Pop3Op, Pop3OpResult, Pop3Script, Pop3ScriptOutcome};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, RwLock};

type Done = Box<dyn FnOnce(Result<Vec<Pop3OpResult>, StoreError>) + Send>;

/// Shared state for POP3: host, port, TLS, auth. No persistent session; one session per operation.
struct Pop3StoreState {
    host: String,
    port: u16,
    use_implicit_tls: RwLock<bool>,
    /// On plain TCP (not implicit TLS), upgrade with RFC 2595 `STLS` before credentials when true.
    use_stls: RwLock<bool>,
    auth: RwLock<Option<(String, String)>>,
    username: RwLock<String>,
}

impl Pop3StoreState {
    /// Run `ops` in a fresh session. `on_done` fires once, with every result in
    /// op order, on hopf's reactor thread (or on this thread when the
    /// credentials are missing or the dial is refused outright).
    fn run(&self, ops: Vec<Pop3Op>, on_done: Done) {
        let use_tls = *self.use_implicit_tls.read().unwrap();
        let use_stls = *self.use_stls.read().unwrap();
        let Some((username, password)) = self.auth.read().unwrap().clone() else {
            let username = self.username.read().unwrap().clone();
            on_done(Err(StoreError::NeedsCredential {
                username,
                is_plaintext: !use_tls && !use_stls,
                advertised_capabilities: None,
            }));
            return;
        };
        crate::trace_log!(
            "pop3",
            "session host={} port={} implicit_tls={} stls={} ops={}",
            self.host,
            self.port,
            use_tls,
            use_stls,
            ops.len()
        );
        let reporter = Arc::new(Mutex::new(Some(on_done)));
        let from_script = Arc::clone(&reporter);
        let script = Pop3Script::new()
            .credentials(username, password)
            .require_stls(!use_tls && use_stls)
            .ops(ops)
            .on_result(Box::new(move |outcome| {
                let cb = from_script.lock().unwrap().take();
                if let Some(cb) = cb {
                    cb(match outcome {
                        Pop3ScriptOutcome::Done(results) => Ok(results),
                        Pop3ScriptOutcome::AuthFailed(m) => {
                            Err(StoreError::new(format!("POP3 authentication failed: {m}")))
                        }
                        Pop3ScriptOutcome::Failed { error, .. } => Err(StoreError::new(error)),
                    });
                }
            }));
        let client = Pop3Client::new(self.host.as_str(), self.port)
            .timeouts(Pop3ClientTimeouts::default())
            .resolver(crate::net::hopf_dns_resolver());
        let client = if use_tls {
            client.implicit_tls(hopf_tls_connector(), self.host.as_str())
        } else if use_stls {
            client.stls(hopf_tls_connector(), self.host.as_str())
        } else {
            client
        };
        if let Err(e) = client.connect(&hopf_runtime(), Arc::new(script)) {
            let cb = reporter.lock().unwrap().take();
            if let Some(cb) = cb {
                cb(Err(StoreError::new(e.to_string())));
            }
        }
    }

    fn user_at_host(&self) -> String {
        let username = {
            let u = self.username.read().unwrap().clone();
            if u.is_empty() {
                self.auth.read().unwrap().as_ref().map(|(u, _)| u.clone()).unwrap_or_default()
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
}

/// POP3 store (single folder INBOX).
pub struct Pop3Store {
    state: Arc<Pop3StoreState>,
}

impl Pop3Store {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 995;
        Self {
            state: Arc::new(Pop3StoreState {
                host,
                port,
                use_implicit_tls: RwLock::new(use_implicit_tls),
                use_stls: RwLock::new(!use_implicit_tls),
                auth: RwLock::new(None),
                username: RwLock::new(String::new()),
            }),
        }
    }

    pub fn set_implicit_tls(&mut self, use_tls: bool) -> &mut Self {
        *self.state.use_implicit_tls.write().unwrap() = use_tls;
        self
    }

    /// Use `STLS` on plain POP3 (RFC 2595). Ignored when using implicit TLS (e.g. port 995).
    pub fn set_use_stls(&mut self, use_stls: bool) -> &mut Self {
        *self.state.use_stls.write().unwrap() = use_stls;
        self
    }

    pub fn set_auth(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> &mut Self {
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
        let state = Arc::clone(&self.state);
        let user_at_host = self.state.user_at_host();
        self.state.run(
            vec![Pop3Op::Stat, Pop3Op::Uidl, Pop3Op::List],
            Box::new(move |results| {
                let results = match results {
                    Ok(r) => r,
                    Err(e) => {
                        on_complete(Err(e));
                        return;
                    }
                };
                let mut count = 0u32;
                let mut uidl: Vec<(u32, String)> = Vec::new();
                let mut sizes: HashMap<u32, u64> = HashMap::new();
                for r in results {
                    match r {
                        Pop3OpResult::Stat { count: c, .. } => count = c,
                        Pop3OpResult::Uidl(u) => uidl = u,
                        Pop3OpResult::List(l) => sizes = l.into_iter().collect(),
                        _ => {}
                    }
                }
                let mut entries: Vec<(u32, String, u64)> = uidl
                    .into_iter()
                    .map(|(n, u)| (n, u, sizes.get(&n).copied().unwrap_or(0)))
                    .collect();
                entries.sort_by_key(|e| e.0);
                on_complete(Ok(Box::new(Pop3Folder {
                    state,
                    user_at_host,
                    count,
                    entries: Mutex::new(entries),
                })));
            }),
        );
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
        in_reply_to: None,
        references: None,
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

fn envelope_from_raw(raw: &[u8]) -> Envelope {
    parse_envelope(raw).map(|rfc| rfc5322_envelope_to_store(&rfc)).unwrap_or_else(|_| default_envelope())
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
        let slice: Vec<(u32, String, u64)> = {
            let entries = self.entries.lock().unwrap();
            let start = (range.start as usize).min(entries.len());
            let end = (range.end as usize).min(entries.len());
            entries[start..end].to_vec()
        };
        if slice.is_empty() {
            on_complete(Ok(()));
            return;
        }
        let user_at_host = self.user_at_host.clone();
        let ops = slice.iter().map(|(n, _, _)| Pop3Op::Top { message: *n, lines: 0 }).collect();
        self.state.run(
            ops,
            Box::new(move |results| {
                let results = match results {
                    Ok(r) => r,
                    Err(e) => {
                        on_complete(Err(e));
                        return;
                    }
                };
                let headers: HashMap<u32, Vec<u8>> = results
                    .into_iter()
                    .filter_map(|r| match r {
                        Pop3OpResult::Message { message, body, .. } => Some((message, body)),
                        _ => None,
                    })
                    .collect();
                for (msg_no, uidl, size) in &slice {
                    let envelope = headers.get(msg_no).map(|h| envelope_from_raw(h)).unwrap_or_else(default_envelope);
                    on_summary(ConversationSummary {
                        id: pop3_message_id(&user_at_host, uidl),
                        envelope,
                        flags: std::collections::HashSet::new(),
                        size: *size,
                    });
                }
                on_complete(Ok(()));
            }),
        );
    }

    fn message_count(&self, on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>) {
        on_complete(Ok(self.count as u64));
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let Some(uidl) = parse_uidl_from_pop3_id(id) else {
            on_complete(Err(StoreError::new("invalid pop3 message id")));
            return;
        };
        let msg_no = self.entries.lock().unwrap().iter().find(|(_, u, _)| u == &uidl).map(|(n, _, _)| *n);
        let Some(msg_no) = msg_no else {
            on_complete(Err(StoreError::new("message not found")));
            return;
        };
        self.state.run(
            vec![Pop3Op::Retr(msg_no)],
            Box::new(move |results| {
                let raw = match results {
                    Ok(r) => r.into_iter().find_map(|r| match r {
                        Pop3OpResult::Message { body, .. } => Some(body),
                        _ => None,
                    }),
                    Err(e) => {
                        on_complete(Err(e));
                        return;
                    }
                };
                let Some(raw) = raw else {
                    on_complete(Err(StoreError::new("RETR returned no message")));
                    return;
                };
                on_metadata(envelope_from_raw(&raw));
                on_content_chunk(&raw);
                on_complete(Ok(()));
            }),
        );
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
