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

//! Maildir+ Store/Folder (cur, new, tmp, subfolders). Port from gumdrop.

mod filename;
mod uidlist;

use crate::localstorage::mailbox_name_codec;
use crate::message_id::{maildir_message_id, MessageId};
use crate::mime::{extract_structured_body, parse_envelope, parse_thread_headers, EmailAddress, EnvelopeHeaders};
use crate::store::{Address, Attachment, ConversationSummary, DateTime, Envelope, Message};
use crate::store::{ThreadId, ThreadSummary};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use filename::MaildirFilename;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uidlist::UidList;

const HIERARCHY_DELIMITER: char = '/';
const MAILDIR_FOLDER_PREFIX: char = '.';
const INBOX: &str = "INBOX";

/// Local Store over a Maildir+ directory (root = user's maildir, contains cur/new/tmp and .Folder subdirs).
pub struct MaildirStore {
    root: PathBuf,
}

impl MaildirStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).map_err(|e| StoreError::new(e.to_string()))?;
        for sub in ["cur", "new", "tmp"] {
            fs::create_dir_all(root.join(sub)).map_err(|e| StoreError::new(e.to_string()))?;
        }
        Ok(Self { root })
    }

    fn mailbox_to_dir(&self, name: &str) -> String {
        if name.eq_ignore_ascii_case(INBOX) {
            return String::new();
        }
        let mut out = String::from(MAILDIR_FOLDER_PREFIX);
        for (i, part) in name.split(HIERARCHY_DELIMITER).enumerate() {
            if i > 0 {
                out.push(MAILDIR_FOLDER_PREFIX);
            }
            out.push_str(&mailbox_name_codec::encode(part));
        }
        out
    }

    fn dir_to_mailbox(&self, dir_name: &str) -> String {
        if dir_name.is_empty() {
            return INBOX.to_string();
        }
        let rest = dir_name.trim_start_matches(MAILDIR_FOLDER_PREFIX);
        rest.split(MAILDIR_FOLDER_PREFIX)
            .map(|s| mailbox_name_codec::decode(s))
            .collect::<Vec<_>>()
            .join(&HIERARCHY_DELIMITER.to_string())
    }

    fn resolve_mailbox_path(&self, name: &str) -> PathBuf {
        let dir = self.mailbox_to_dir(name);
        if dir.is_empty() {
            self.root.clone()
        } else {
            self.root.join(&dir)
        }
    }

    fn is_valid_maildir(&self, p: &Path) -> bool {
        p.is_dir()
            && p.join("cur").is_dir()
            && p.join("new").is_dir()
            && p.join("tmp").is_dir()
    }

    fn list_mailboxes(&self) -> Result<Vec<String>, StoreError> {
        let mut result = Vec::new();
        if self.is_valid_maildir(&self.root) {
            result.push(INBOX.to_string());
        }
        let entries = fs::read_dir(&self.root).map_err(|e| StoreError::new(e.to_string()))?;
        for e in entries {
            let e = e.map_err(|e| StoreError::new(e.to_string()))?;
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.')
                && name != "."
                && name != ".."
                && name != ".subscriptions"
                && name != ".uidlist"
                && !name.ends_with(".tmp")
            {
                let path = e.path();
                if self.is_valid_maildir(&path) {
                    result.push(self.dir_to_mailbox(&name));
                }
            }
        }
        Ok(result)
    }
}

impl Store for MaildirStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn list_folders(&self) -> Result<Vec<FolderInfo>, StoreError> {
        let names = self.list_mailboxes()?;
        let delim = Some(HIERARCHY_DELIMITER);
        Ok(names
            .into_iter()
            .map(|name| FolderInfo {
                name,
                delimiter: delim,
                attributes: Vec::new(),
            })
            .collect())
    }

    fn open_folder(&self, name: &str) -> Result<Box<dyn Folder>, StoreError> {
        let path = self.resolve_mailbox_path(name);
        if !self.is_valid_maildir(&path) {
            return Err(StoreError::new(format!("Mailbox does not exist: {}", name)));
        }
        let root_str = self.root.to_string_lossy().to_string();
        let folder_name = if name.eq_ignore_ascii_case(INBOX) {
            INBOX.to_string()
        } else {
            name.to_string()
        };
        Ok(Box::new(MaildirFolder {
            root_path: root_str,
            folder_name,
            path,
        }))
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        Some(HIERARCHY_DELIMITER)
    }

    fn default_folder(&self) -> Option<&str> {
        Some(INBOX)
    }

    /// Maildir open is synchronous; run it and invoke on_complete (FFI calls from background thread).
    fn start_open_folder_streaming(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        let result = self.open_folder(name);
        on_complete(result);
        Ok(())
    }
}

/// Folder over a single Maildir (cur + new).
struct MaildirFolder {
    root_path: String,
    folder_name: String,
    path: PathBuf,
}

impl MaildirFolder {
    fn cur_path(&self) -> PathBuf {
        self.path.join("cur")
    }
    fn new_path(&self) -> PathBuf {
        self.path.join("new")
    }

    fn scan_messages(&self) -> Result<Vec<(u64, PathBuf, MaildirFilename, u64)>, StoreError> {
        let mut uid_list = UidList::new(&self.path);
        uid_list.load().map_err(|e| StoreError::new(e.to_string()))?;

        // Move new -> cur (simplified: just scan both)
        let mut entries: Vec<(u64, PathBuf, MaildirFilename, u64)> = Vec::new();
        for sub in ["cur", "new"] {
            let dir = self.path.join(sub);
            if !dir.is_dir() {
                continue;
            }
            let read_dir = fs::read_dir(&dir).map_err(|e| StoreError::new(e.to_string()))?;
            for e in read_dir {
                let e = e.map_err(|e| StoreError::new(e.to_string()))?;
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let path = e.path();
                if !path.is_file() {
                    continue;
                }
                let parsed = match MaildirFilename::parse(&name) {
                    Some(p) => p,
                    None => continue,
                };
                let base = parsed.base_filename();
                let uid = uid_list.get_uid(&base).unwrap_or_else(|| uid_list.assign_uid(&base));
                let size = parsed.size.unwrap_or_else(|| fs::metadata(&path).map(|m| m.len()).unwrap_or(0));
                entries.push((uid, path, parsed, size));
            }
        }
        entries.sort_by_key(|e| e.0);
        if uid_list.is_dirty() {
            uid_list.save().map_err(|e| StoreError::new(e.to_string()))?;
        }
        Ok(entries)
    }

    fn message_id(&self, _uid: u64, filename: &str) -> MessageId {
        maildir_message_id(&self.root_path, &self.folder_name, filename)
    }
}

impl Folder for MaildirFolder {
    fn list_conversations(&self, range: std::ops::Range<u64>) -> Result<Vec<ConversationSummary>, StoreError> {
        let entries = self.scan_messages()?;
        let total = entries.len() as u64;
        let start = range.start.min(total);
        let end = range.end.min(total);
        if start >= end {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for i in start..end {
            let (uid, path, parsed, size) = &entries[i as usize];
            let filename = path.file_name().unwrap().to_string_lossy();
            let id = self.message_id(*uid, &filename);
            let raw = fs::read(&path).map_err(|e| StoreError::new(e.to_string()))?;
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            let flags = parsed.flags.clone();
            out.push(ConversationSummary {
                id,
                envelope,
                flags,
                size: *size,
            });
        }
        Ok(out)
    }

    fn message_count(&self) -> Result<u64, StoreError> {
        let entries = self.scan_messages()?;
        Ok(entries.len() as u64)
    }

    fn get_message(&self, id: &MessageId) -> Result<Option<Message>, StoreError> {
        let s = id.as_str();
        let prefix = "maildir://";
        if !s.starts_with(prefix) {
            return Ok(None);
        }
        let rest = s.strip_prefix(prefix).unwrap();
        let filename = rest.rsplit('/').next().unwrap_or_default();
        if filename.is_empty() {
            return Ok(None);
        }
        let path = self.cur_path().join(filename);
        if !path.exists() {
            let path_new = self.new_path().join(filename);
            if path_new.exists() {
                let raw = fs::read(&path_new).map_err(|e| StoreError::new(e.to_string()))?;
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
                return Ok(Some(Message {
                    id: id.clone(),
                    envelope,
                    flags: HashSet::new(),
                    size: raw.len() as u64,
                    body_plain,
                    body_html,
                    attachments,
                    raw: Some(raw),
                }));
            }
            return Ok(None);
        }
        let raw = fs::read(&path).map_err(|e| StoreError::new(e.to_string()))?;
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
        let parsed = MaildirFilename::parse(filename).unwrap_or_default();
        Ok(Some(Message {
            id: id.clone(),
            envelope,
            flags: parsed.flags,
            size: raw.len() as u64,
            body_plain,
            body_html,
            attachments,
            raw: Some(raw),
        }))
    }

    fn list_threads(&self, range: std::ops::Range<u64>) -> Result<Vec<ThreadSummary>, StoreError> {
        let entries = self.scan_messages()?;
        let mut thread_groups: std::collections::HashMap<
            String,
            (Option<String>, Vec<ConversationSummary>),
        > = std::collections::HashMap::new();
        for (uid, path, parsed, size) in &entries {
            let raw = fs::read(&path).map_err(|e| StoreError::new(e.to_string()))?;
            let th = parse_thread_headers(&raw).unwrap_or_default();
            let root = th
                .references
                .first()
                .cloned()
                .or(th.message_id.clone())
                .unwrap_or_else(|| format!("s:{}", th.subject.as_deref().unwrap_or("")));
            let filename = path.file_name().unwrap().to_string_lossy();
            let id = self.message_id(*uid, &filename);
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            let summary = ConversationSummary {
                id,
                envelope,
                flags: parsed.flags.clone(),
                size: *size,
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
        range: std::ops::Range<u64>,
    ) -> Result<Vec<ConversationSummary>, StoreError> {
        let entries = self.scan_messages()?;
        let mut in_thread = Vec::new();
        for (uid, path, parsed, size) in &entries {
            let raw = fs::read(&path).map_err(|e| StoreError::new(e.to_string()))?;
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
            let filename = path.file_name().unwrap().to_string_lossy();
            let id = self.message_id(*uid, &filename);
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            in_thread.push(ConversationSummary {
                id,
                envelope,
                flags: parsed.flags.clone(),
                size: *size,
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

    fn append_message(&self, data: &[u8]) -> Result<(), StoreError> {
        let flags = HashSet::new();
        let parsed = MaildirFilename::generate(data.len() as u64, &flags);
        let filename = parsed.to_string();
        let new_dir = self.new_path();
        fs::create_dir_all(&new_dir).map_err(|e| StoreError::new(e.to_string()))?;
        let path = new_dir.join(&filename);
        fs::write(&path, data).map_err(|e| StoreError::new(e.to_string()))?;
        let base = parsed.base_filename();
        let mut uid_list = UidList::new(&self.path);
        uid_list.load().map_err(|e| StoreError::new(e.to_string()))?;
        uid_list.assign_uid(&base);
        if uid_list.is_dirty() {
            uid_list.save().map_err(|e| StoreError::new(e.to_string()))?;
        }
        Ok(())
    }

    fn delete_message(&self, id: &MessageId) -> Result<(), StoreError> {
        let s = id.as_str();
        let prefix = "maildir://";
        if !s.starts_with(prefix) {
            return Err(StoreError::new("invalid maildir message id"));
        }
        let rest = s.strip_prefix(prefix).unwrap();
        let filename = rest.rsplit('/').next().unwrap_or_default();
        if filename.is_empty() {
            return Err(StoreError::new("invalid maildir message id"));
        }
        let path_cur = self.cur_path().join(filename);
        let path_new = self.new_path().join(filename);
        let path = if path_cur.exists() {
            path_cur
        } else if path_new.exists() {
            path_new
        } else {
            return Err(StoreError::new("message file not found"));
        };
        fs::remove_file(&path).map_err(|e| StoreError::new(e.to_string()))?;
        let base = MaildirFilename::parse(filename)
            .map(|p| p.base_filename())
            .unwrap_or_else(|| filename.to_string());
        let mut uid_list = UidList::new(&self.path);
        uid_list.load().map_err(|e| StoreError::new(e.to_string()))?;
        uid_list.remove_uid(&base);
        if uid_list.is_dirty() {
            uid_list.save().map_err(|e| StoreError::new(e.to_string()))?;
        }
        Ok(())
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

impl Default for MaildirFilename {
    fn default() -> Self {
        Self {
            timestamp: 0,
            unique_part: String::new(),
            size: None,
            flags: HashSet::new(),
        }
    }
}
