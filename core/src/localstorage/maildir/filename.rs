/*
 * filename.rs
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

//! Maildir filename parse/generate (gumdrop MaildirFilename).
//! Format: <timestamp>.<unique>,S=<size>:2,<flags>  e.g. 1733356800000.12345.1,S=4523:2,SF

use crate::store::Flag;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
static mut MAILDIR_COUNTER: u64 = 0;

#[allow(dead_code)]
fn next_unique_part() -> String {
    let pid = std::process::id();
    let c = unsafe {
        MAILDIR_COUNTER += 1;
        MAILDIR_COUNTER
    };
    format!("{}.{}", pid, c)
}

/// Parsed Maildir filename (timestamp, unique part, size, flags).
#[derive(Debug, Clone)]
pub struct MaildirFilename {
    pub timestamp: u64,
    pub unique_part: String,
    pub size: Option<u64>,
    pub flags: HashSet<Flag>,
}

impl MaildirFilename {
    /// Parse a Maildir filename from cur/ or new/.
    pub fn parse(filename: &str) -> Option<Self> {
        let (base, flags_part) = if let Some(i) = filename.find(":2,") {
            (filename[..i].to_string(), filename[i + 3..].to_string())
        } else {
            (filename.to_string(), String::new())
        };

        let (base_no_size, size) = if let Some(i) = base.find(",S=") {
            let s = base[i + 3..].parse().ok()?;
            (base[..i].to_string(), Some(s))
        } else {
            (base, None)
        };

        let dot = base_no_size.find('.')?;
        let timestamp: u64 = base_no_size[..dot].parse().ok()?;
        let unique_part = base_no_size[dot + 1..].to_string();

        let mut flags = HashSet::new();
        for c in flags_part.chars() {
            match c {
                'D' => {
                    flags.insert(Flag::Draft);
                }
                'F' => {
                    flags.insert(Flag::Flagged);
                }
                'R' => {
                    flags.insert(Flag::Answered);
                }
                'S' => {
                    flags.insert(Flag::Seen);
                }
                'T' => {
                    flags.insert(Flag::Deleted);
                }
                'a'..='z' => {
                    flags.insert(Flag::Custom(c.to_string()));
                }
                _ => {}
            }
        }

        Some(Self {
            timestamp,
            unique_part,
            size,
            flags,
        })
    }

    /// Base filename (without flags) for UidList matching.
    pub fn base_filename(&self) -> String {
        let mut s = format!("{}.{}", self.timestamp, self.unique_part);
        if let Some(sz) = self.size {
            s.push_str(&format!(",S={}", sz));
        }
        s
    }

    /// Full filename including :2,<flags>.
    pub fn to_string(&self) -> String {
        let mut s = self.base_filename();
        s.push_str(":2,");
        let mut fl: Vec<char> = self
            .flags
            .iter()
            .filter_map(|f| match f {
                Flag::Draft => Some('D'),
                Flag::Flagged => Some('F'),
                Flag::Answered => Some('R'),
                Flag::Seen => Some('S'),
                Flag::Deleted => Some('T'),
                Flag::Custom(c) => c.chars().next().filter(|c| c.is_ascii_lowercase()),
            })
            .collect();
        fl.sort_unstable();
        s.extend(fl);
        s
    }

    /// Generate a new filename for delivery (timestamp.unique,S=size:2,).
    pub fn generate(size: u64, flags: &HashSet<Flag>) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self {
            timestamp: ts,
            unique_part: next_unique_part(),
            size: Some(size),
            flags: flags.clone(),
        }
    }

    /// New filename with updated flags (for rename).
    pub fn with_flags(&self, flags: HashSet<Flag>) -> Self {
        Self {
            timestamp: self.timestamp,
            unique_part: self.unique_part.clone(),
            size: self.size,
            flags,
        }
    }
}
