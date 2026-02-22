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
//!
//! All trait methods are callback-driven. Since Maildir is file-based, callbacks
//! fire inline (synchronously) before the method returns.

mod filename;
mod uidlist;

use crate::localstorage::mailbox_name_codec;
use crate::message_id::{maildir_message_id, MessageId};
use crate::mime::{parse_envelope, parse_thread_headers, EmailAddress, EnvelopeHeaders};
use crate::store::{Address, ConversationSummary, DateTime, Envelope};
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

    fn open_folder_sync(&self, name: &str) -> Result<Box<dyn Folder>, StoreError> {
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
}

impl Store for MaildirStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        match self.list_mailboxes() {
            Ok(names) => {
                let delim = Some(HIERARCHY_DELIMITER);
                for name in names {
                    on_folder(FolderInfo {
                        name,
                        delimiter: delim,
                        attributes: Vec::new(),
                    });
                }
                on_complete(Ok(()));
            }
            Err(e) => on_complete(Err(e)),
        }
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        on_complete(self.open_folder_sync(name));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        Some(HIERARCHY_DELIMITER)
    }

    fn default_folder(&self) -> Option<&str> {
        Some(INBOX)
    }

    fn create_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        if name.eq_ignore_ascii_case(INBOX) {
            on_complete(Err(StoreError::new("cannot create INBOX")));
            return;
        }
        let dir = self.mailbox_to_dir(name);
        let folder_path = self.root.join(&dir);
        match (|| -> Result<(), StoreError> {
            for sub in ["cur", "new", "tmp"] {
                fs::create_dir_all(folder_path.join(sub)).map_err(|e| StoreError::new(e.to_string()))?;
            }
            Ok(())
        })() {
            Ok(()) => on_complete(Ok(())),
            Err(e) => on_complete(Err(e)),
        }
    }

    fn rename_folder(
        &self,
        old_name: &str,
        new_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        if old_name.eq_ignore_ascii_case(INBOX) {
            on_complete(Err(StoreError::new("cannot rename INBOX")));
            return;
        }
        let old_dir = self.mailbox_to_dir(old_name);
        let new_dir = self.mailbox_to_dir(new_name);
        let old_path = self.root.join(&old_dir);
        let new_path = self.root.join(&new_dir);
        match fs::rename(&old_path, &new_path) {
            Ok(()) => on_complete(Ok(())),
            Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
        }
    }

    fn delete_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        if name.eq_ignore_ascii_case(INBOX) {
            on_complete(Err(StoreError::new("cannot delete INBOX")));
            return;
        }
        let dir = self.mailbox_to_dir(name);
        let folder_path = self.root.join(&dir);
        match fs::remove_dir_all(&folder_path) {
            Ok(()) => on_complete(Ok(())),
            Err(e) => on_complete(Err(StoreError::new(e.to_string()))),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
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

    fn find_message_file(&self, filename: &str) -> Result<PathBuf, StoreError> {
        let path_cur = self.cur_path().join(filename);
        if path_cur.exists() {
            return Ok(path_cur);
        }
        let path_new = self.new_path().join(filename);
        if path_new.exists() {
            return Ok(path_new);
        }
        Err(StoreError::new(format!("message file not found: {}", filename)))
    }
}

impl Folder for MaildirFolder {
    fn list_conversations(
        &self,
        range: std::ops::Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let entries = match self.scan_messages() {
            Ok(e) => e,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let total = entries.len() as u64;
        let start = range.start.min(total);
        let end = range.end.min(total);
        if start >= end {
            on_complete(Ok(()));
            return;
        }
        for i in start..end {
            let (uid, ref path, ref parsed, size) = entries[i as usize];
            let filename = path.file_name().unwrap().to_string_lossy();
            let id = self.message_id(uid, &filename);
            let raw = match fs::read(path) {
                Ok(r) => r,
                Err(e) => {
                    on_complete(Err(StoreError::new(e.to_string())));
                    return;
                }
            };
            let envelope = parse_envelope(&raw)
                .map(|h| envelope_headers_to_store(&h))
                .unwrap_or_else(|_| Envelope::default());
            let flags = parsed.flags.clone();
            on_summary(ConversationSummary {
                id,
                envelope,
                flags,
                size,
            });
        }
        on_complete(Ok(()));
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        match self.scan_messages() {
            Ok(entries) => on_complete(Ok(entries.len() as u64)),
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
        let prefix = "maildir://";
        if !s.starts_with(prefix) {
            on_complete(Err(StoreError::new("invalid maildir message id")));
            return;
        }
        let rest = s.strip_prefix(prefix).unwrap();
        let filename = rest.rsplit('/').next().unwrap_or_default();
        if filename.is_empty() {
            on_complete(Err(StoreError::new("invalid maildir message id")));
            return;
        }
        let path_cur = self.cur_path().join(filename);
        let path_new = self.new_path().join(filename);
        let path = if path_cur.exists() {
            path_cur
        } else if path_new.exists() {
            path_new
        } else {
            on_complete(Err(StoreError::new("message not found")));
            return;
        };
        let raw = match fs::read(&path) {
            Ok(r) => r,
            Err(e) => {
                on_complete(Err(StoreError::new(e.to_string())));
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

    fn delete_message(
        &self,
        id: &MessageId,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let s = id.as_str();
        let prefix = "maildir://";
        if !s.starts_with(prefix) {
            on_complete(Err(StoreError::new("invalid maildir message id")));
            return;
        }
        let rest = s.strip_prefix(prefix).unwrap();
        let filename = rest.rsplit('/').next().unwrap_or_default();
        if filename.is_empty() {
            on_complete(Err(StoreError::new("invalid maildir message id")));
            return;
        }
        let path_cur = self.cur_path().join(filename);
        let path_new = self.new_path().join(filename);
        let path = if path_cur.exists() {
            path_cur
        } else if path_new.exists() {
            path_new
        } else {
            on_complete(Err(StoreError::new("message file not found")));
            return;
        };
        let result = fs::remove_file(&path).map_err(|e| StoreError::new(e.to_string()));
        if let Ok(()) = &result {
            let base = MaildirFilename::parse(filename)
                .map(|p| p.base_filename())
                .unwrap_or_else(|| filename.to_string());
            let mut uid_list = UidList::new(&self.path);
            let _ = uid_list.load();
            uid_list.remove_uid(&base);
            let _ = uid_list.save();
        }
        on_complete(result);
    }

    fn list_threads(
        &self,
        range: std::ops::Range<u64>,
        on_thread: Box<dyn Fn(ThreadSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let entries = match self.scan_messages() {
            Ok(e) => e,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let mut thread_groups: std::collections::HashMap<
            String,
            (Option<String>, u64),
        > = std::collections::HashMap::new();
        for (_uid, ref path, _parsed, _size) in &entries {
            let raw = match fs::read(path) {
                Ok(r) => r,
                Err(e) => {
                    on_complete(Err(StoreError::new(e.to_string())));
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

    fn list_messages_in_thread(
        &self,
        thread_id: &ThreadId,
        range: std::ops::Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let entries = match self.scan_messages() {
            Ok(e) => e,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let mut in_thread = Vec::new();
        for (uid, ref path, ref parsed, size) in &entries {
            let raw = match fs::read(path) {
                Ok(r) => r,
                Err(e) => {
                    on_complete(Err(StoreError::new(e.to_string())));
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
        for s in in_thread.into_iter().skip(start).take(end.saturating_sub(start)) {
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
        })();
        on_complete(result);
    }

    fn copy_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let result = (|| -> Result<(), StoreError> {
            // Resolve destination folder path from root
            let root = PathBuf::from(&self.root_path);
            let dest_dir_name = if dest_folder_name.eq_ignore_ascii_case(INBOX) {
                String::new()
            } else {
                let mut out = String::from(MAILDIR_FOLDER_PREFIX);
                for (i, part) in dest_folder_name.split(HIERARCHY_DELIMITER).enumerate() {
                    if i > 0 {
                        out.push(MAILDIR_FOLDER_PREFIX);
                    }
                    out.push_str(&mailbox_name_codec::encode(part));
                }
                out
            };
            let dest_path = if dest_dir_name.is_empty() {
                root.clone()
            } else {
                root.join(&dest_dir_name)
            };
            let dest_new = dest_path.join("new");
            fs::create_dir_all(&dest_new).map_err(|e| StoreError::new(e.to_string()))?;

            for id_str in ids {
                let filename = extract_maildir_filename(id_str);
                if filename.is_empty() {
                    continue;
                }
                let src = self.find_message_file(&filename)?;
                let data = fs::read(&src).map_err(|e| StoreError::new(e.to_string()))?;
                let new_parsed = MaildirFilename::generate(data.len() as u64, &HashSet::new());
                let dest_file = dest_new.join(new_parsed.to_string());
                fs::write(&dest_file, &data).map_err(|e| StoreError::new(e.to_string()))?;
            }
            Ok(())
        })();
        on_complete(result);
    }

    fn move_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let result = (|| -> Result<(), StoreError> {
            let root = PathBuf::from(&self.root_path);
            let dest_dir_name = if dest_folder_name.eq_ignore_ascii_case(INBOX) {
                String::new()
            } else {
                let mut out = String::from(MAILDIR_FOLDER_PREFIX);
                for (i, part) in dest_folder_name.split(HIERARCHY_DELIMITER).enumerate() {
                    if i > 0 {
                        out.push(MAILDIR_FOLDER_PREFIX);
                    }
                    out.push_str(&mailbox_name_codec::encode(part));
                }
                out
            };
            let dest_path = if dest_dir_name.is_empty() {
                root.clone()
            } else {
                root.join(&dest_dir_name)
            };
            let dest_new = dest_path.join("new");
            fs::create_dir_all(&dest_new).map_err(|e| StoreError::new(e.to_string()))?;

            for id_str in ids {
                let filename = extract_maildir_filename(id_str);
                if filename.is_empty() {
                    continue;
                }
                let src = self.find_message_file(&filename)?;
                let data = fs::read(&src).map_err(|e| StoreError::new(e.to_string()))?;
                let new_parsed = MaildirFilename::generate(data.len() as u64, &HashSet::new());
                let dest_file = dest_new.join(new_parsed.to_string());
                fs::write(&dest_file, &data).map_err(|e| StoreError::new(e.to_string()))?;
                // Remove source after successful write
                fs::remove_file(&src).map_err(|e| StoreError::new(e.to_string()))?;
            }
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

/// Extract the filename component from a maildir message ID URI.
fn extract_maildir_filename(id: &str) -> String {
    let prefix = "maildir://";
    if !id.starts_with(prefix) {
        return String::new();
    }
    let rest = id.strip_prefix(prefix).unwrap();
    rest.rsplit('/').next().unwrap_or_default().to_string()
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
