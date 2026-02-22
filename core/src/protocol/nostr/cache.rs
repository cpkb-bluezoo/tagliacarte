/*
 * cache.rs
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

//! Local DM event cache: one JSON file per conversation at
//! `<config_dir>/nostr/<our_pubkey>/<other_pubkey>.json`.
//! Raw events (still encrypted) are stored; decryption happens on read.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use bytes::BytesMut;
use crate::json::{JsonContentHandler, JsonNumber, JsonParser};

use super::crypto;
use super::types::{self, Event, KIND_DM, KIND_GIFT_WRAP};

/// Per-conversation file lock to prevent concurrent read-modify-write races.
fn conversation_locks() -> &'static Mutex<HashMap<String, std::sync::Arc<Mutex<()>>>> {
    static INSTANCE: OnceLock<Mutex<HashMap<String, std::sync::Arc<Mutex<()>>>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_conversation(path: &str) -> std::sync::Arc<Mutex<()>> {
    let mut map = conversation_locks().lock().unwrap();
    map.entry(path.to_string())
        .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
        .clone()
}

fn normalize_hex(s: &str) -> String {
    s.trim().to_lowercase()
}

fn nostr_dir(config_dir: &str, our_pubkey_hex: &str) -> String {
    Path::new(config_dir)
        .join("nostr")
        .join(normalize_hex(our_pubkey_hex))
        .to_string_lossy()
        .to_string()
}

fn conversation_file_path(config_dir: &str, our_pubkey_hex: &str, other_pubkey_hex: &str) -> String {
    Path::new(&nostr_dir(config_dir, our_pubkey_hex))
        .join(format!("{}.json", normalize_hex(other_pubkey_hex)))
        .to_string_lossy()
        .to_string()
}

/// Ensure the cache directory exists.
pub fn ensure_cache_dir(config_dir: &str, our_pubkey_hex: &str) -> Result<(), io::Error> {
    let dir = nostr_dir(config_dir, our_pubkey_hex);
    let path = Path::new(&dir);
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

/// List conversation partner pubkeys (hex) from files in the cache directory.
pub fn list_conversations(config_dir: &str, our_pubkey_hex: &str) -> Result<Vec<String>, String> {
    let dir = nostr_dir(config_dir, our_pubkey_hex);
    let path = Path::new(&dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut pubkeys: Vec<String> = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| format!("Read cache dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Read dir entry: {}", e))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("Invalid filename")?;
        if let Some(pk) = name.strip_suffix(".json") {
            if pk.len() == 64 && pk.chars().all(|c| c.is_ascii_hexdigit()) {
                pubkeys.push(pk.to_string());
            }
        }
    }
    Ok(pubkeys)
}

/// List conversations sorted by most recent activity (descending).
pub fn list_conversations_with_timestamps(
    config_dir: &str,
    our_pubkey_hex: &str,
) -> Result<Vec<(String, u64)>, String> {
    let convos = list_conversations(config_dir, our_pubkey_hex)?;
    let mut list: Vec<(String, u64)> = convos
        .into_iter()
        .map(|pk| {
            let ts = last_created_at(config_dir, our_pubkey_hex, &pk).unwrap_or(0);
            (pk, ts)
        })
        .collect();
    list.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(list)
}

pub struct DecryptedMessage {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub content: String,
    pub is_outgoing: bool,
}

/// Read a conversation file, decrypt each event, return messages sorted by created_at.
pub fn get_messages(
    config_dir: &str,
    our_secret_hex: &str,
    our_pubkey_hex: &str,
    other_pubkey_hex: &str,
) -> Result<Vec<DecryptedMessage>, String> {
    let path = conversation_file_path(config_dir, our_pubkey_hex, other_pubkey_hex);
    let our = normalize_hex(our_pubkey_hex);
    let other = normalize_hex(other_pubkey_hex);

    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Read conversation file: {}", e)),
    };

    let events = parse_event_array(&contents)?;
    let mut messages: Vec<DecryptedMessage> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for event in &events {
        match event.kind {
            KIND_DM => {
                let id_lower = event.id.to_lowercase();
                if !seen_ids.insert(id_lower) {
                    continue;
                }
                let is_outgoing = event.pubkey.to_lowercase() == our;
                let sender_pubkey = if is_outgoing { other.as_str() } else { event.pubkey.as_str() };
                let plaintext = crypto::nip04_decrypt(&event.content, our_secret_hex, sender_pubkey)
                    .unwrap_or_else(|_| String::from("[unable to decrypt]"));
                messages.push(DecryptedMessage {
                    id: event.id.clone(),
                    pubkey: event.pubkey.clone(),
                    created_at: event.created_at,
                    content: plaintext,
                    is_outgoing,
                });
            }
            KIND_GIFT_WRAP => {
                let id_lower = event.id.to_lowercase();
                if !seen_ids.insert(id_lower) {
                    continue;
                }
                match crypto::unwrap_gift_wrap(event, our_secret_hex) {
                    Ok((_seal, rumor)) => {
                        let rumor_id = rumor.id.to_lowercase();
                        if !seen_ids.insert(rumor_id) {
                            continue;
                        }
                        let is_outgoing = rumor.pubkey.to_lowercase() == our;
                        messages.push(DecryptedMessage {
                            id: rumor.id.clone(),
                            pubkey: rumor.pubkey.clone(),
                            created_at: rumor.created_at,
                            content: rumor.content.clone(),
                            is_outgoing,
                        });
                    }
                    Err(_) => {
                        messages.push(DecryptedMessage {
                            id: event.id.clone(),
                            pubkey: event.pubkey.clone(),
                            created_at: event.created_at,
                            content: String::from("[unable to decrypt]"),
                            is_outgoing: false,
                        });
                    }
                }
            }
            _ => continue,
        }
    }
    messages.sort_by_key(|m| m.created_at);
    Ok(messages)
}

/// Append a raw kind 4 or kind 1059 event to the conversation file (dedup by event id).
/// Returns `Ok(true)` if the event was appended, `Ok(false)` if duplicate.
pub fn append_raw_event(
    config_dir: &str,
    our_pubkey_hex: &str,
    other_pubkey_hex: &str,
    raw_event_json: &str,
) -> Result<bool, String> {
    let path = conversation_file_path(config_dir, our_pubkey_hex, other_pubkey_hex);
    let new_event = types::parse_event(raw_event_json)
        .map_err(|e| format!("Parse event: {}", e))?;
    if new_event.kind != KIND_DM && new_event.kind != KIND_GIFT_WRAP {
        return Err(format!("Event kind {} is not a DM event (expected 4 or 1059)", new_event.kind));
    }
    let new_id = new_event.id.to_lowercase();

    let lock = lock_conversation(&path);
    let _guard = lock.lock().unwrap();

    let dir = Path::new(&path).parent().ok_or("no parent dir")?;
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| format!("Create dir: {}", e))?;
    }

    if Path::new(&path).exists() {
        let contents = fs::read_to_string(&path).map_err(|e| format!("Read file: {}", e))?;
        let search_pattern = format!("\"id\":\"{}\"", new_id);
        if contents.to_lowercase().contains(&search_pattern) {
            return Ok(false);
        }
        let trimmed = contents.trim_end();
        if trimmed.ends_with(']') {
            let mut out = String::from(&trimmed[..trimmed.len() - 1]);
            if out.trim_end().ends_with('}') {
                out.push(',');
            }
            out.push_str(raw_event_json);
            out.push(']');
            fs::write(&path, out).map_err(|e| format!("Write file: {}", e))?;
        } else {
            let out = format!("[{}]", raw_event_json);
            fs::write(&path, out).map_err(|e| format!("Write file: {}", e))?;
        }
    } else {
        let out = format!("[{}]", raw_event_json);
        fs::write(&path, out).map_err(|e| format!("Write file: {}", e))?;
    }

    Ok(true)
}

/// Get last event's `created_at` from a conversation file.
fn last_created_at(config_dir: &str, our_pubkey_hex: &str, other_pubkey_hex: &str) -> Option<u64> {
    let path = conversation_file_path(config_dir, our_pubkey_hex, other_pubkey_hex);
    let contents = fs::read_to_string(&path).ok()?;
    let pattern = "\"created_at\":";
    let pos = contents.rfind(pattern)?;
    let after = contents[pos + pattern.len()..].trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u64>().ok()
}

// ============================================================
// Push-parser for JSON array of event objects
// ============================================================

struct EventArrayHandler {
    depth: i32,
    current_field: Option<String>,
    event_id: Option<String>,
    event_pubkey: Option<String>,
    event_created_at: u64,
    event_kind: u32,
    event_content: String,
    event_sig: Option<String>,
    event_tags: Vec<Vec<String>>,
    current_tag: Vec<String>,
    tags_depth: i32,
    events: Vec<Event>,
}

impl EventArrayHandler {
    fn new() -> Self {
        Self {
            depth: 0,
            current_field: None,
            event_id: None,
            event_pubkey: None,
            event_created_at: 0,
            event_kind: 0,
            event_content: String::new(),
            event_sig: None,
            event_tags: Vec::new(),
            current_tag: Vec::new(),
            tags_depth: 0,
            events: Vec::new(),
        }
    }

    fn reset_event(&mut self) {
        self.current_field = None;
        self.event_id = None;
        self.event_pubkey = None;
        self.event_created_at = 0;
        self.event_kind = 0;
        self.event_content.clear();
        self.event_sig = None;
        self.event_tags.clear();
        self.current_tag.clear();
        self.tags_depth = 0;
    }
}

impl JsonContentHandler for EventArrayHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 {
            self.reset_event();
        }
    }

    fn end_object(&mut self) {
        if self.depth == 2 {
            if let (Some(id), Some(pubkey)) = (self.event_id.clone(), self.event_pubkey.clone()) {
                self.events.push(Event {
                    id,
                    pubkey,
                    created_at: self.event_created_at,
                    kind: self.event_kind,
                    tags: self.event_tags.clone(),
                    content: self.event_content.clone(),
                    sig: self.event_sig.clone().unwrap_or_default(),
                });
            }
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        self.depth += 1;
        if self.tags_depth == 1 {
            self.tags_depth = 2;
            self.current_tag.clear();
        } else if self.tags_depth == 2 {
            self.current_tag.clear();
        }
    }

    fn end_array(&mut self) {
        if self.tags_depth == 2 && self.depth == 4 {
            if !self.current_tag.is_empty() {
                self.event_tags.push(self.current_tag.clone());
            }
            self.current_tag.clear();
        } else if self.tags_depth == 2 && self.depth == 3 {
            self.tags_depth = 0;
        } else if self.tags_depth == 1 && self.depth == 3 {
            self.tags_depth = 0;
        }
        self.depth -= 1;
    }

    fn key(&mut self, key: &str) {
        self.current_field = Some(key.to_string());
        if self.depth == 2 && key == "tags" {
            self.tags_depth = 1;
        }
    }

    fn string_value(&mut self, value: &str) {
        if self.tags_depth == 2 {
            self.current_tag.push(value.to_string());
        } else if self.depth == 2 {
            if let Some(ref f) = self.current_field {
                match f.as_str() {
                    "id" => self.event_id = Some(value.to_string()),
                    "pubkey" => self.event_pubkey = Some(value.to_string()),
                    "content" => self.event_content = value.to_string(),
                    "sig" => self.event_sig = Some(value.to_string()),
                    _ => {}
                }
            }
        }
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.depth == 2 {
            if let Some(ref f) = self.current_field {
                if f == "created_at" {
                    self.event_created_at = number.as_f64().max(0.0) as u64;
                } else if f == "kind" {
                    self.event_kind = number.as_f64().max(0.0) as u32;
                }
            }
        }
    }

    fn boolean_value(&mut self, _value: bool) {}
    fn null_value(&mut self) {}
}

fn parse_event_array(json_str: &str) -> Result<Vec<Event>, String> {
    let mut handler = EventArrayHandler::new();
    let mut parser = JsonParser::new();
    let mut buf = BytesMut::from(json_str.as_bytes());
    parser.receive(&mut buf, &mut handler)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    parser.close(&mut handler)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    Ok(handler.events)
}
