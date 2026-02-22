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

//! NNTP client (Store + Folder + Transport). Persistent connection via async pipeline.
//! Store and folders share one session via NntpStoreState.
//! Transport shares the same connection for POST.
//!
//! All trait methods are fully callback-driven and return immediately.

mod client;

pub use client::{
    connect_and_authenticate, connect_and_start_pipeline, AuthenticatedSession,
    GroupResult, NewsgroupEntry, NntpClientError, NntpConnection, OverviewEntry,
};

use crate::message_id::{nntp_message_id, MessageId};
use crate::mime::{parse_envelope, EmailAddress, EnvelopeHeaders};
use crate::store::{Address, ConversationSummary, DateTime, Envelope, Flag};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::TransportKind;
use crate::store::{SendPayload, Transport};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, RwLock};

// ======================================================================
// RangeSet for local read-state tracking
// ======================================================================

/// Compact set of article numbers stored as sorted, non-overlapping inclusive ranges.
#[derive(Debug, Clone, Default)]
pub struct RangeSet {
    ranges: Vec<(u64, u64)>,
}

impl RangeSet {
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    pub fn contains(&self, n: u64) -> bool {
        self.ranges.iter().any(|&(lo, hi)| n >= lo && n <= hi)
    }

    pub fn insert(&mut self, n: u64) {
        self.insert_range(n, n);
    }

    pub fn insert_range(&mut self, start: u64, end: u64) {
        if start > end {
            return;
        }
        self.ranges.push((start, end));
        self.normalize();
    }

    /// Mark all articles in [first, last] as read.
    pub fn insert_all(&mut self, first: u64, last: u64) {
        self.insert_range(first, last);
    }

    fn normalize(&mut self) {
        if self.ranges.len() <= 1 {
            return;
        }
        self.ranges.sort_by_key(|r| r.0);
        let mut merged: Vec<(u64, u64)> = Vec::new();
        for &(lo, hi) in &self.ranges {
            if let Some(last) = merged.last_mut() {
                // Merge if overlapping or adjacent (hi+1 >= lo means adjacent)
                if lo <= last.1 + 1 {
                    last.1 = last.1.max(hi);
                    continue;
                }
            }
            merged.push((lo, hi));
        }
        self.ranges = merged;
    }

    pub fn to_compact_string(&self) -> String {
        self.ranges
            .iter()
            .map(|&(lo, hi)| {
                if lo == hi {
                    lo.to_string()
                } else {
                    format!("{}-{}", lo, hi)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn parse(s: &str) -> Self {
        let mut set = Self::new();
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some(dash) = part.find('-') {
                let lo: u64 = part[..dash].trim().parse().unwrap_or(0);
                let hi: u64 = part[dash + 1..].trim().parse().unwrap_or(0);
                if lo > 0 && hi > 0 {
                    set.ranges.push((lo, hi));
                }
            } else if let Ok(n) = part.parse::<u64>() {
                if n > 0 {
                    set.ranges.push((n, n));
                }
            }
        }
        set.normalize();
        set
    }
}

/// Serialized read-state: group-keyed map of RangeSets.
/// Format: "group.name: 1-44, 60-1045\nother.group: 1-500"
pub fn parse_read_state(serialized: &str) -> HashMap<String, RangeSet> {
    let mut map = HashMap::new();
    for line in serialized.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(colon) = line.find(':') {
            let group = line[..colon].trim().to_string();
            let ranges_str = line[colon + 1..].trim();
            if !group.is_empty() {
                map.insert(group, RangeSet::parse(ranges_str));
            }
        }
    }
    map
}

pub fn serialize_read_state(state: &HashMap<String, RangeSet>) -> String {
    let mut lines: Vec<String> = state
        .iter()
        .filter(|(_, rs)| !rs.ranges.is_empty())
        .map(|(group, rs)| format!("{}: {}", group, rs.to_compact_string()))
        .collect();
    lines.sort();
    lines.join("\n")
}

// ======================================================================
// NntpStoreState (shared between Store, Folder, Transport)
// ======================================================================

pub struct NntpStoreState {
    host: String,
    port: u16,
    use_implicit_tls: RwLock<bool>,
    use_starttls: RwLock<bool>,
    auth: RwLock<Option<(String, String)>>,
    username: RwLock<String>,
    runtime_handle: tokio::runtime::Handle,
    connection: Mutex<Option<NntpConnection>>,
    pub read_state: RwLock<HashMap<String, RangeSet>>,
}

impl NntpStoreState {
    fn ensure_connection(&self) -> Result<NntpConnection, StoreError> {
        let mut guard = self.connection.lock().map_err(|e| StoreError::new(e.to_string()))?;
        if let Some(ref conn) = *guard {
            if conn.is_alive() {
                return Ok(conn.clone());
            }
        }
        let host = self.host.clone();
        let port = self.port;
        let use_implicit_tls = *self.use_implicit_tls.read().map_err(|e| StoreError::new(e.to_string()))?;
        let use_starttls = *self.use_starttls.read().map_err(|e| StoreError::new(e.to_string()))?;
        let auth = self.auth.read().map_err(|e| StoreError::new(e.to_string()))?.clone();
        if auth.is_none() {
            let username = self.username.read().map_err(|e| StoreError::new(e.to_string()))?.clone();
            if !username.is_empty() {
                let is_plaintext = !use_implicit_tls && !use_starttls;
                return Err(StoreError::NeedsCredential { username, is_plaintext });
            }
            // Anonymous access (no auth)
        }

        let auth_ref = auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));
        let conn = self.runtime_handle.block_on(async move {
            connect_and_start_pipeline(&host, port, use_implicit_tls, use_starttls, auth_ref)
                .await
                .map_err(|e| StoreError::new(e.to_string()))
        })?;
        *guard = Some(conn.clone());
        Ok(conn)
    }

    pub fn load_read_state(&self, serialized: &str) {
        if let Ok(mut guard) = self.read_state.write() {
            *guard = parse_read_state(serialized);
        }
    }

    pub fn serialize_read_state(&self) -> String {
        self.read_state.read().map(|g| serialize_read_state(&g)).unwrap_or_default()
    }
}

// ======================================================================
// NntpStore
// ======================================================================

pub struct NntpStore {
    state: Arc<NntpStoreState>,
}

impl NntpStore {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self::with_runtime_handle(host, port, tokio::runtime::Handle::current())
    }

    pub fn with_runtime_handle(host: impl Into<String>, port: u16, handle: tokio::runtime::Handle) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 563;
        let state = NntpStoreState {
            host,
            port,
            use_implicit_tls: RwLock::new(use_implicit_tls),
            use_starttls: RwLock::new(true),
            auth: RwLock::new(None),
            username: RwLock::new(String::new()),
            runtime_handle: handle,
            connection: Mutex::new(None),
            read_state: RwLock::new(HashMap::new()),
        };
        Self { state: Arc::new(state) }
    }

    pub fn set_username(&mut self, user: impl Into<String>) -> &mut Self {
        *self.state.username.write().unwrap() = user.into();
        self
    }

    pub fn username(&self) -> String {
        self.state.username.read().unwrap().clone()
    }

    /// Get a reference to the shared state (for NntpTransport).
    pub fn shared_state(&self) -> Arc<NntpStoreState> {
        Arc::clone(&self.state)
    }

    pub fn set_implicit_tls(&mut self, use_tls: bool) -> &mut Self {
        *self.state.use_implicit_tls.write().unwrap() = use_tls;
        self
    }

    pub fn set_use_starttls(&mut self, use_starttls: bool) -> &mut Self {
        *self.state.use_starttls.write().unwrap() = use_starttls;
        self
    }
}

impl Store for NntpStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Nntp
    }

    fn set_credential(&self, username: Option<&str>, password: &str) {
        let u = username
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.state.username.read().unwrap().clone());
        if u.is_empty() && password.is_empty() {
            return;
        }
        *self.state.auth.write().unwrap() = Some((u, password.to_string()));
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => { on_complete(Err(e)); return; }
        };
        conn.list_newsgroups_streaming(
            move |entry| {
                on_folder(FolderInfo {
                    name: entry.name,
                    delimiter: Some('.'),
                    attributes: vec![],
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
            Err(e) => { on_complete(Err(e)); return; }
        };
        let state = Arc::clone(&self.state);
        let host = self.state.host.clone();
        let username = self.state.username.read().unwrap().clone();
        let user_at_host = if username.contains('@') {
            username
        } else if !username.is_empty() {
            format!("{}@{}", username, host)
        } else {
            host.clone()
        };

        let group_name = name.to_string();
        conn.group(name, move |result| {
            match result {
                Ok(gr) => {
                    on_event(OpenFolderEvent::Exists(gr.count as u32));
                    let folder = Box::new(NntpFolder {
                        state,
                        user_at_host,
                        group: group_name,
                        first: gr.first,
                        last: gr.last,
                        count: gr.count,
                    }) as Box<dyn Folder>;
                    on_complete(Ok(folder));
                }
                Err(e) => {
                    on_complete(Err(StoreError::new(e.to_string())));
                }
            }
        });
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        Some('.')
    }

    fn default_folder(&self) -> Option<&str> {
        None
    }
}

// ======================================================================
// NntpFolder
// ======================================================================

struct NntpFolder {
    state: Arc<NntpStoreState>,
    user_at_host: String,
    group: String,
    first: u64,
    last: u64,
    count: u64,
}

impl Folder for NntpFolder {
    fn list_conversations(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        if self.count == 0 || self.first > self.last {
            on_complete(Ok(()));
            return;
        }
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => { on_complete(Err(e)); return; }
        };

        // Map the abstract range [start, end) to article numbers
        let over_first = self.first + range.start;
        let over_last = (self.first + range.end).min(self.last + 1).saturating_sub(1);
        if over_first > over_last {
            on_complete(Ok(()));
            return;
        }

        let user_at_host = self.user_at_host.clone();
        let group = self.group.clone();
        let read_state = Arc::clone(&self.state);

        conn.over_streaming(
            over_first,
            over_last,
            move |entry| {
                let envelope = envelope_from_overview(&entry);
                let id = nntp_message_id(&user_at_host, &group, entry.article_number);
                let mut flags = std::collections::HashSet::new();
                if let Ok(rs) = read_state.read_state.read() {
                    if let Some(set) = rs.get(&group) {
                        if set.contains(entry.article_number) {
                            flags.insert(Flag::Seen);
                        }
                    }
                }
                on_summary(ConversationSummary {
                    id,
                    envelope,
                    flags,
                    size: entry.bytes,
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
        on_complete(Ok(self.count));
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let article_num = match parse_article_number_from_id(id) {
            Some(n) => n,
            None => {
                on_complete(Err(StoreError::new("invalid NNTP message id")));
                return;
            }
        };
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => { on_complete(Err(e)); return; }
        };

        // Mark as read in local state
        if let Ok(mut rs) = self.state.read_state.write() {
            rs.entry(self.group.clone()).or_insert_with(RangeSet::new).insert(article_num);
        }

        let header_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let buf = Arc::new(Mutex::new(Vec::new()));
        let header_done2 = header_done.clone();
        let buf2 = buf.clone();
        let on_metadata = Arc::new(on_metadata);
        let on_content_chunk = Arc::new(on_content_chunk);
        let on_metadata2 = on_metadata.clone();
        let on_content_chunk2 = on_content_chunk.clone();

        conn.article_streaming(
            article_num,
            move |line| {
                let line_bytes = line.as_bytes();
                if !header_done2.load(std::sync::atomic::Ordering::Relaxed) {
                    let mut guard = buf2.lock().unwrap();
                    guard.extend_from_slice(line_bytes);
                    guard.extend_from_slice(b"\r\n");
                    // Empty line separates header from body
                    if line.is_empty() {
                        let header_bytes = guard.clone();
                        if let Ok(env) = envelope_from_raw(&header_bytes) {
                            on_metadata2(env);
                        } else {
                            on_metadata2(default_envelope());
                        }
                        on_content_chunk2(&header_bytes);
                        header_done2.store(true, std::sync::atomic::Ordering::Relaxed);
                        guard.clear();
                    }
                } else {
                    let mut chunk = line_bytes.to_vec();
                    chunk.extend_from_slice(b"\r\n");
                    on_content_chunk2(&chunk);
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

    fn mark_all_read(
        &self,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        if let Ok(mut rs) = self.state.read_state.write() {
            rs.entry(self.group.clone())
                .or_insert_with(RangeSet::new)
                .insert_all(self.first, self.last);
        }
        on_complete(Ok(()));
    }
}

fn parse_article_number_from_id(id: &MessageId) -> Option<u64> {
    let s = id.as_str();
    let rest = s.strip_prefix("nntp://")?;
    // Format: user@host/group/number
    let parts: Vec<&str> = rest.splitn(3, '/').collect();
    parts.get(2).and_then(|u| u.parse().ok())
}

// ======================================================================
// NntpTransport
// ======================================================================

pub struct NntpTransport {
    state: Arc<NntpStoreState>,
}

impl NntpTransport {
    pub fn from_store_state(state: Arc<NntpStoreState>) -> Self {
        Self { state }
    }

    pub fn with_runtime_handle(host: impl Into<String>, port: u16, handle: tokio::runtime::Handle) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 563;
        let state = NntpStoreState {
            host,
            port,
            use_implicit_tls: RwLock::new(use_implicit_tls),
            use_starttls: RwLock::new(true),
            auth: RwLock::new(None),
            username: RwLock::new(String::new()),
            runtime_handle: handle,
            connection: Mutex::new(None),
            read_state: RwLock::new(HashMap::new()),
        };
        Self { state: Arc::new(state) }
    }

    pub fn set_username(&mut self, user: impl Into<String>) -> &mut Self {
        *self.state.username.write().unwrap() = user.into();
        self
    }
}

impl Transport for NntpTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Nntp
    }

    fn send(
        &self,
        payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let conn = match self.state.ensure_connection() {
            Ok(c) => c,
            Err(e) => { on_complete(Err(e)); return; }
        };
        if !conn.posting_allowed() {
            on_complete(Err(StoreError::new("posting not allowed by this server")));
            return;
        }

        let newsgroups = payload.newsgroups.join(", ");
        if newsgroups.is_empty() {
            on_complete(Err(StoreError::new("no newsgroups specified")));
            return;
        }
        let from = payload.from.first().map(format_address).unwrap_or_default();
        let subject = payload.subject.as_deref().unwrap_or("");
        let body = payload.body_plain.as_deref().unwrap_or("");

        let date = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000").to_string();

        // Build the article
        let mut article = String::new();
        article.push_str(&format!("From: {}\r\n", from));
        article.push_str(&format!("Newsgroups: {}\r\n", newsgroups));
        article.push_str(&format!("Subject: {}\r\n", subject));
        article.push_str(&format!("Date: {}\r\n", date));
        article.push_str("\r\n");

        // Dot-stuff body lines and append
        for line in body.lines() {
            if line.starts_with('.') {
                article.push('.');
            }
            article.push_str(line);
            article.push_str("\r\n");
        }
        article.push_str(".\r\n");

        conn.post(&article, move |result| {
            on_complete(result.map_err(|e| StoreError::new(e.to_string())));
        });
    }
}

// ======================================================================
// Envelope helpers
// ======================================================================

fn envelope_from_overview(entry: &OverviewEntry) -> Envelope {
    Envelope {
        from: vec![parse_address_string(&entry.from)],
        to: Vec::new(),
        cc: Vec::new(),
        date: parse_date(&entry.date),
        subject: if entry.subject.is_empty() { None } else { Some(entry.subject.clone()) },
        message_id: if entry.message_id.is_empty() { None } else { Some(entry.message_id.clone()) },
    }
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

fn parse_address_string(s: &str) -> Address {
    // Simple "Name <local@domain>" or "local@domain" parser
    let s = s.trim();
    if let Some(open) = s.find('<') {
        if let Some(close) = s.find('>') {
            let display = s[..open].trim().trim_matches('"').to_string();
            let addr = &s[open + 1..close];
            let (local, domain) = split_email(addr);
            return Address {
                display_name: if display.is_empty() { None } else { Some(display) },
                local_part: local,
                domain: Some(domain),
            };
        }
    }
    let (local, domain) = split_email(s);
    Address {
        display_name: None,
        local_part: local,
        domain: Some(domain),
    }
}

fn split_email(addr: &str) -> (String, String) {
    if let Some(at) = addr.rfind('@') {
        (addr[..at].to_string(), addr[at + 1..].to_string())
    } else {
        (addr.to_string(), String::new())
    }
}

fn format_address(addr: &Address) -> String {
    let email = if let Some(ref d) = addr.domain {
        if d.is_empty() { addr.local_part.clone() } else { format!("{}@{}", addr.local_part, d) }
    } else {
        addr.local_part.clone()
    };
    if let Some(ref name) = addr.display_name {
        format!("{} <{}>", name, email)
    } else {
        email
    }
}

fn parse_date(s: &str) -> Option<DateTime> {
    // Try to parse RFC 2822 date
    chrono::DateTime::parse_from_rfc2822(s).ok().map(|dt| DateTime {
        timestamp: dt.timestamp(),
        tz_offset_secs: Some(dt.offset().local_minus_utc()),
    })
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
