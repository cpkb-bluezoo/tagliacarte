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

//! mbox Store/Folder (single file, From_ boundaries). Port from gumdrop.
//!
//! All trait methods are callback-driven. Since mbox is file-based, callbacks
//! fire inline before the method returns.

use crate::message_id::{mbox_message_id, MessageId};
use crate::mime::{parse_envelope, parse_thread_headers, EmailAddress, EnvelopeHeaders};
use crate::store::{Address, ConversationSummary, DateTime, Envelope};
use crate::store::{ThreadId, ThreadSummary};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use chrono::Utc;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Local Store over a single mbox file (one folder: INBOX).
pub struct MboxStore {
    path: PathBuf,
}

impl MboxStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        Ok(Self { path })
    }
}

impl Store for MboxStore {
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
        if !name.eq_ignore_ascii_case("INBOX") {
            on_complete(Err(StoreError::new("Only INBOX is supported for mbox")));
            return;
        }
        on_complete(Ok(Box::new(MboxFolder {
            path: self.path.clone(),
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

/// Folder over a single mbox file.
struct MboxFolder {
    path: PathBuf,
}

/// Offsets of each message in the mbox (start inclusive, end exclusive; start is after "From " line).
fn scan_offsets(path: &Path) -> Result<Vec<(u64, u64)>, StoreError> {
    let f = File::open(path).map_err(|e| StoreError::new(e.to_string()))?;
    let mut r = BufReader::new(f);
    let mut offsets = Vec::new();
    let mut line = Vec::new();
    let mut current_start: Option<u64> = None;
    let mut pos: u64 = 0;

    loop {
        line.clear();
        let n = r.read_until(b'\n', &mut line).map_err(|e| StoreError::new(e.to_string()))?;
        if n == 0 {
            if let Some(start) = current_start {
                offsets.push((start, pos));
            }
            break;
        }
        let is_from_line = line.starts_with(b"From ")
            || (line.len() >= 6 && line[0] == b'\n' && line.get(1..6) == Some(b"From " as &[u8]));

        if is_from_line {
            let from_line_start = if line.starts_with(b"From ") {
                pos
            } else {
                pos + 1
            };
            if let Some(start) = current_start {
                offsets.push((start, from_line_start));
            }
            current_start = Some(from_line_start + n as u64);
        }
        pos += n as u64;
    }

    Ok(offsets)
}

impl MboxFolder {
    fn path_str(&self) -> String {
        self.path.to_string_lossy().to_string()
    }

    fn read_range(&self, start: u64, end: u64) -> Result<Vec<u8>, StoreError> {
        let mut f = File::open(&self.path).map_err(|e| StoreError::new(e.to_string()))?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        f.seek(SeekFrom::Start(start)).map_err(|e| StoreError::new(e.to_string()))?;
        f.read_exact(&mut buf).map_err(|e| StoreError::new(e.to_string()))?;
        Ok(buf)
    }
}

impl Folder for MboxFolder {
    fn list_conversations(
        &self,
        range: std::ops::Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let offsets = match scan_offsets(&self.path) {
            Ok(o) => o,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let total = offsets.len() as u64;
        let start = range.start.min(total);
        let end = range.end.min(total);
        if start >= end {
            on_complete(Ok(()));
            return;
        }
        for i in start..end {
            let (s, e) = offsets[i as usize];
            let raw = match self.read_range(s, e) {
                Ok(r) => r,
                Err(err) => {
                    on_complete(Err(err));
                    return;
                }
            };
            let id = mbox_message_id(&self.path_str(), &format!("#{}", s));
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            on_summary(ConversationSummary {
                id,
                envelope,
                flags: HashSet::new(),
                size: (e - s) as u64,
            });
        }
        on_complete(Ok(()));
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        match scan_offsets(&self.path) {
            Ok(offsets) => on_complete(Ok(offsets.len() as u64)),
            Err(e) => on_complete(Err(e)),
        }
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let s = id.as_str();
        let prefix = "mbox://";
        if !s.starts_with(prefix) {
            on_complete(Err(StoreError::new("invalid mbox message id")));
            return;
        }
        let rest = s.strip_prefix(prefix).unwrap();
        let hash_idx = rest.find('#');
        let offset_str = hash_idx.and_then(|i| rest.get(i + 1..)).and_then(|s| s.split('/').next());
        let start: u64 = match offset_str.and_then(|x| x.parse().ok()) {
            Some(n) => n,
            None => {
                on_complete(Err(StoreError::new("invalid mbox message id")));
                return;
            }
        };
        let offsets = match scan_offsets(&self.path) {
            Ok(o) => o,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let idx = offsets.iter().position(|(s, _)| *s == start);
        let (s, e) = match idx.and_then(|i| offsets.get(i)) {
            Some(&(a, b)) => (a, b),
            None => {
                on_complete(Err(StoreError::new("message not found")));
                return;
            }
        };
        let raw = match self.read_range(s, e) {
            Ok(r) => r,
            Err(err) => {
                on_complete(Err(err));
                return;
            }
        };
        let envelope = parse_envelope(&raw)
            .map(|h| envelope_headers_to_store(&h))
            .unwrap_or_else(|_| Envelope::default());
        on_metadata(envelope);
        on_content_chunk(&raw);
        on_complete(Ok(()));
    }

    fn list_threads(
        &self,
        range: std::ops::Range<u64>,
        on_thread: Box<dyn Fn(ThreadSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let offsets = match scan_offsets(&self.path) {
            Ok(o) => o,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let mut thread_groups: std::collections::HashMap<
            String,
            (Option<String>, u64),
        > = std::collections::HashMap::new();
        for &(start, end) in offsets.iter() {
            let raw = match self.read_range(start, end) {
                Ok(r) => r,
                Err(e) => {
                    on_complete(Err(e));
                    return;
                }
            };
            let th = parse_thread_headers(&raw).unwrap_or_default();
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
        let start_idx = range.start.min(threads.len() as u64) as usize;
        let end_idx = range.end.min(threads.len() as u64) as usize;
        for t in threads.into_iter().skip(start_idx).take(end_idx.saturating_sub(start_idx)) {
            on_thread(ThreadSummary {
                id: ThreadId(t.0),
                subject: t.1,
                message_count: t.2,
            });
        }
        on_complete(Ok(()));
    }

    fn list_messages_in_thread(
        &self,
        thread_id: &ThreadId,
        range: std::ops::Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let offsets = match scan_offsets(&self.path) {
            Ok(o) => o,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let mut in_thread = Vec::new();
        for &(start, end) in &offsets {
            let raw = match self.read_range(start, end) {
                Ok(r) => r,
                Err(e) => {
                    on_complete(Err(e));
                    return;
                }
            };
            let th = parse_thread_headers(&raw).unwrap_or_default();
            let root = th
                .references
                .first()
                .cloned()
                .or(th.message_id.clone())
                .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
            if root != thread_id.as_str() {
                continue;
            }
            let id = mbox_message_id(&self.path_str(), &format!("#{}", start));
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            in_thread.push(ConversationSummary {
                id,
                envelope,
                flags: HashSet::new(),
                size: (end - start) as u64,
            });
        }
        in_thread.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        let start_idx = range.start.min(in_thread.len() as u64) as usize;
        let end_idx = range.end.min(in_thread.len() as u64) as usize;
        for s in in_thread.into_iter().skip(start_idx).take(end_idx.saturating_sub(start_idx)) {
            on_summary(s);
        }
        on_complete(Ok(()));
    }

    fn append_message(
        &self,
        data: &[u8],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let result = (|| -> Result<(), StoreError> {
            // mbox format: each message starts with "From " line then raw RFC 822 message.
            // Lines in the body that start with "From " must be escaped as ">From ".
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|e| StoreError::new(e.to_string()))?;
            let from_line = format!("From MAILER-DAEMON {}  \n", Utc::now().format("%a %b %e %T %Y"));
            file.write_all(from_line.as_bytes())
                .map_err(|e| StoreError::new(e.to_string()))?;
            // Escape lines that start with "From " -> ">From "
            let mut i = 0;
            let mut at_line_start = true;
            while i < data.len() {
                if at_line_start && data[i..].starts_with(b"From ") {
                    file.write_all(b">From ").map_err(|e| StoreError::new(e.to_string()))?;
                    i += 5;
                    at_line_start = false;
                    continue;
                }
                let b = data[i];
                if b == b'\n' {
                    at_line_start = true;
                } else if b != b'\r' {
                    at_line_start = false;
                }
                file.write_all(&[b]).map_err(|e| StoreError::new(e.to_string()))?;
                i += 1;
            }
            if !data.ends_with(b"\n") {
                file.write_all(b"\n").map_err(|e| StoreError::new(e.to_string()))?;
            }
            file.flush().map_err(|e| StoreError::new(e.to_string()))?;
            Ok(())
        })();
        on_complete(result);
    }
}

fn envelope_headers_to_store(h: &EnvelopeHeaders) -> Envelope {
    Envelope {
        from: h.from.iter().map(email_to_address).collect(),
        to: h.to.iter().map(email_to_address).collect(),
        cc: h.cc.iter().map(email_to_address).collect(),
        date: h.date.map(|dt| DateTime {
            timestamp: dt.timestamp(),
            tz_offset_secs: Some(dt.offset().local_minus_utc()),
        }),
        subject: h.subject.clone(),
        message_id: h.message_id.as_ref().map(|c| c.to_string()),
    }
}

fn email_to_address(e: &EmailAddress) -> Address {
    Address {
        display_name: e.display_name.clone(),
        local_part: e.local_part.clone(),
        domain: Some(e.domain.clone()),
    }
}

#[allow(dead_code)]
trait DefaultEnvelope {
    fn default_envelope() -> Envelope;
}
impl DefaultEnvelope for Envelope {
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
}
