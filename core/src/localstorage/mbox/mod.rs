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

use crate::message_id::{mbox_message_id, MessageId};
use crate::mime::{extract_structured_body, parse_envelope, parse_thread_headers, EmailAddress, EnvelopeHeaders};
use crate::store::{Address, Attachment, ConversationSummary, DateTime, Envelope, Message};
use crate::store::{ThreadId, ThreadSummary};
use crate::store::{Folder, FolderInfo, Store, StoreError, StoreKind};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
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

    fn list_folders(&self) -> Result<Vec<FolderInfo>, StoreError> {
        Ok(vec![FolderInfo {
            name: "INBOX".to_string(),
            delimiter: None,
            attributes: vec![],
        }])
    }

    fn open_folder(&self, name: &str) -> Result<Box<dyn Folder>, StoreError> {
        if !name.eq_ignore_ascii_case("INBOX") {
            return Err(StoreError::new("Only INBOX is supported for mbox"));
        }
        Ok(Box::new(MboxFolder {
            path: self.path.clone(),
        }))
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        Some("INBOX")
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
            let from_line_start = if line.starts_with(b"From ") { pos } else { pos + 1 };
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
    fn list_conversations(&self, range: std::ops::Range<u64>) -> Result<Vec<ConversationSummary>, StoreError> {
        let offsets = scan_offsets(&self.path)?;
        let total = offsets.len() as u64;
        let start = range.start.min(total);
        let end = range.end.min(total);
        if start >= end {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for i in start..end {
            let (s, e) = offsets[i as usize];
            let raw = self.read_range(s, e)?;
            let id = mbox_message_id(&self.path_str(), &format!("#{}", s));
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            out.push(ConversationSummary {
                id,
                envelope,
                flags: HashSet::new(),
                size: (e - s) as u64,
            });
        }
        Ok(out)
    }

    fn message_count(&self) -> Result<u64, StoreError> {
        let offsets = scan_offsets(&self.path)?;
        Ok(offsets.len() as u64)
    }

    fn get_message(&self, id: &MessageId) -> Result<Option<Message>, StoreError> {
        let s = id.as_str();
        let prefix = "mbox://";
        if !s.starts_with(prefix) {
            return Ok(None);
        }
        let rest = s.strip_prefix(prefix).unwrap();
        let hash_idx = rest.find('#');
        let offset_str = hash_idx.and_then(|i| rest.get(i + 1..)).and_then(|s| s.split('/').next());
        let start: u64 = match offset_str.and_then(|x| x.parse().ok()) {
            Some(n) => n,
            None => return Ok(None),
        };
        let offsets = scan_offsets(&self.path)?;
        let idx = offsets.iter().position(|(s, _)| *s == start);
        let (s, e) = match idx.and_then(|i| offsets.get(i)) {
            Some(&(a, b)) => (a, b),
            None => return Ok(None),
        };
        let raw = self.read_range(s, e)?;
        let envelope = parse_envelope(&raw)
            .map(|h| envelope_headers_to_store(&h))
            .unwrap_or_else(|_| Envelope::default());
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
            flags: HashSet::new(),
            size: (e - s) as u64,
            body_plain,
            body_html,
            attachments,
            raw: Some(raw),
        }))
    }

    fn list_threads(&self, range: std::ops::Range<u64>) -> Result<Vec<ThreadSummary>, StoreError> {
        let offsets = scan_offsets(&self.path)?;
        let mut thread_groups: std::collections::HashMap<
            String,
            (Option<String>, Vec<ConversationSummary>),
        > = std::collections::HashMap::new();
        for &(start, end) in offsets.iter() {
            let raw = self.read_range(start, end)?;
            let th = parse_thread_headers(&raw).unwrap_or_default();
            let root = th
                .references
                .first()
                .cloned()
                .or(th.message_id.clone())
                .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
            let id = mbox_message_id(&self.path_str(), &format!("#{}", start));
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            let summary = ConversationSummary {
                id,
                envelope,
                flags: HashSet::new(),
                size: (end - start) as u64,
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
        let start_idx = range.start.min(threads.len() as u64) as usize;
        let end_idx = range.end.min(threads.len() as u64) as usize;
        if start_idx >= end_idx {
            return Ok(Vec::new());
        }
        Ok(threads[start_idx..end_idx].to_vec())
    }

    fn list_messages_in_thread(
        &self,
        thread_id: &ThreadId,
        range: std::ops::Range<u64>,
    ) -> Result<Vec<ConversationSummary>, StoreError> {
        let offsets = scan_offsets(&self.path)?;
        let mut in_thread = Vec::new();
        for &(start, end) in &offsets {
            let raw = self.read_range(start, end)?;
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
        if start_idx >= end_idx {
            return Ok(Vec::new());
        }
        Ok(in_thread[start_idx..end_idx].to_vec())
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
