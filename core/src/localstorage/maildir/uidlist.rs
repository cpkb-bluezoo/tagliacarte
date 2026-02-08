/*
 * uidlist.rs
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

//! Maildir UID list (.uidlist) - maps base filename to persistent UID (gumdrop MaildirUidList).

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

const HEADER: &str = "# gumdrop-uidlist v1";

#[derive(Debug)]
pub struct UidList {
    path: std::path::PathBuf,
    pub uid_validity: u64,
    pub uid_next: u64,
    filename_to_uid: HashMap<String, u64>,
    uid_to_filename: HashMap<u64, String>,
    dirty: bool,
}

impl UidList {
    pub fn new(maildir_path: &Path) -> Self {
        Self {
            path: maildir_path.join(".uidlist"),
            uid_validity: 0,
            uid_next: 1,
            filename_to_uid: HashMap::new(),
            uid_to_filename: HashMap::new(),
            dirty: false,
        }
    }

    /// Load from disk or initialize new.
    pub fn load(&mut self) -> std::io::Result<()> {
        self.filename_to_uid.clear();
        self.uid_to_filename.clear();

        if !self.path.exists() {
            self.uid_validity = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            self.uid_next = 1;
            self.dirty = true;
            return Ok(());
        }

        let f = File::open(&self.path)?;
        let r = BufReader::new(f);
        let mut lines = r.lines();
        let first = lines.next().transpose()?.unwrap_or_default();
        if first != HEADER {
            self.uid_validity = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            self.uid_next = 1;
            self.dirty = true;
            return Ok(());
        }

        for line in lines {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("uidvalidity ") {
                if let Ok(n) = line[12..].trim().parse::<u64>() {
                    self.uid_validity = n;
                }
            } else if line.starts_with("uidnext ") {
                if let Ok(n) = line[8..].trim().parse::<u64>() {
                    self.uid_next = n;
                }
            } else {
                let sp = line.find(' ').unwrap_or(0);
                if sp > 0 {
                    if let Ok(uid) = line[..sp].parse::<u64>() {
                        let base = line[sp + 1..].to_string();
                        self.filename_to_uid.insert(base.clone(), uid);
                        self.uid_to_filename.insert(uid, base);
                    }
                }
            }
        }
        self.dirty = false;
        Ok(())
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let tmp = self.path.with_extension("tmp");
        let f = File::create(&tmp)?;
        let mut w = BufWriter::new(f);
        writeln!(w, "{}", HEADER)?;
        writeln!(w, "uidvalidity {}", self.uid_validity)?;
        writeln!(w, "uidnext {}", self.uid_next)?;
        let mut uids: Vec<u64> = self.uid_to_filename.keys().copied().collect();
        uids.sort_unstable();
        for uid in uids {
            let base = self.uid_to_filename.get(&uid).unwrap();
            writeln!(w, "{} {}", uid, base)?;
        }
        w.flush()?;
        drop(w);
        std::fs::rename(tmp, &self.path)?;
        self.dirty = false;
        Ok(())
    }

    pub fn get_uid(&self, base_filename: &str) -> Option<u64> {
        self.filename_to_uid.get(base_filename).copied()
    }

    pub fn assign_uid(&mut self, base_filename: &str) -> u64 {
        if let Some(uid) = self.filename_to_uid.get(base_filename) {
            return *uid;
        }
        let uid = self.uid_next;
        self.uid_next += 1;
        self.filename_to_uid
            .insert(base_filename.to_string(), uid);
        self.uid_to_filename
            .insert(uid, base_filename.to_string());
        self.dirty = true;
        uid
    }

    #[allow(dead_code)]
    pub fn remove_uid(&mut self, base_filename: &str) {
        if let Some(uid) = self.filename_to_uid.remove(base_filename) {
            self.uid_to_filename.remove(&uid);
            self.dirty = true;
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}
