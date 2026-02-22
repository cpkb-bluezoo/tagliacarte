/*
 * types.rs
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

//! Nostr event and filter types (NIP-01). Serialization for relay REQ/EVENT messages.
//! Kind constants for DM protocols (NIP-04, NIP-17/59).

use bytes::BytesMut;
use crate::json::{JsonContentHandler, JsonNumber, JsonParser};

/// NIP-01 event: the fundamental data structure in Nostr.
#[derive(Debug, Clone)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

/// NIP-01: User profile metadata.
pub const KIND_METADATA: u32 = 0;
/// NIP-02: Contacts / follow list.
pub const KIND_CONTACTS: u32 = 3;
/// NIP-04: Encrypted direct message.
pub const KIND_DM: u32 = 4;
/// NIP-59: Seal (encrypted rumor, signed by sender).
pub const KIND_SEAL: u32 = 13;
/// NIP-17: Private chat message (rumor kind inside seal/gift wrap).
pub const KIND_CHAT_MESSAGE: u32 = 14;
/// NIP-59: Gift wrap (encrypted seal, signed by ephemeral key).
pub const KIND_GIFT_WRAP: u32 = 1059;
/// NIP-65: Relay list metadata (tags: ["r", "wss://..."]).
pub const KIND_RELAY_LIST: u32 = 10002;
/// NIP-17: DM relay list (tags: ["relay", "wss://..."]).
pub const KIND_DM_RELAY_LIST: u32 = 10050;
/// NIP-98: HTTP auth event for NIP-96 media uploads.
pub const KIND_HTTP_AUTH: u32 = 27235;
/// Blossom: auth event for BUD-02 upload / BUD-04 delete.
pub const KIND_BLOSSOM_AUTH: u32 = 24242;

/// Filter for REQ subscription (NIP-01).
#[derive(Clone, Default)]
pub struct Filter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u32>>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub limit: Option<u32>,
    pub p_tags: Option<Vec<String>>,
    pub e_tags: Option<Vec<String>>,
}

fn escape_json_string(input: &str) -> String {
    let mut output = String::new();
    for c in input.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            _ => output.push(c),
        }
    }
    output
}

/// Serialize filter to JSON object for REQ message.
pub fn filter_to_json(filter: &Filter) -> String {
    let mut json = String::new();
    let mut first = true;

    if let Some(ref ids) = filter.ids {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str("\"ids\":[");
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json_string(id));
            json.push('"');
        }
        json.push(']');
    }
    if let Some(ref authors) = filter.authors {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str("\"authors\":[");
        for (i, a) in authors.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json_string(a));
            json.push('"');
        }
        json.push(']');
    }
    if let Some(ref kinds) = filter.kinds {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str("\"kinds\":[");
        for (i, k) in kinds.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&k.to_string());
        }
        json.push(']');
    }
    if let Some(since) = filter.since {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str("\"since\":");
        json.push_str(&since.to_string());
    }
    if let Some(until) = filter.until {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str("\"until\":");
        json.push_str(&until.to_string());
    }
    if let Some(limit) = filter.limit {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str("\"limit\":");
        json.push_str(&limit.to_string());
    }
    if let Some(ref p) = filter.p_tags {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str("\"#p\":[");
        for (i, pk) in p.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json_string(pk));
            json.push('"');
        }
        json.push(']');
    }
    if let Some(ref e) = filter.e_tags {
        if !first {
            json.push(',');
        }
        json.push_str("\"#e\":[");
        for (i, id) in e.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json_string(id));
            json.push('"');
        }
        json.push(']');
    }
    json.insert(0, '{');
    json.push('}');
    json
}

/// Serialize event to JSON for EVENT publish.
pub fn event_to_json(event: &Event) -> String {
    let mut json = String::new();
    json.push_str("{\"id\":\"");
    json.push_str(&escape_json_string(&event.id));
    json.push_str("\",\"pubkey\":\"");
    json.push_str(&escape_json_string(&event.pubkey));
    json.push_str("\",\"created_at\":");
    json.push_str(&event.created_at.to_string());
    json.push_str(",\"kind\":");
    json.push_str(&event.kind.to_string());
    json.push_str(",\"tags\":[");
    for (i, tag) in event.tags.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('[');
        for (j, s) in tag.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json_string(s));
            json.push('"');
        }
        json.push(']');
    }
    json.push_str("],\"content\":\"");
    json.push_str(&escape_json_string(&event.content));
    json.push_str("\",\"sig\":\"");
    json.push_str(&escape_json_string(&event.sig));
    json.push_str("\"}");
    json
}

/// DMs we received: kind 4 with #p = our pubkey.
pub fn filter_dms_received(our_pubkey_hex: &str, limit: u32, since: Option<u64>) -> Filter {
    Filter {
        kinds: Some(vec![KIND_DM]),
        since,
        limit: Some(limit),
        p_tags: Some(vec![our_pubkey_hex.to_string()]),
        ..Default::default()
    }
}

/// DMs we sent: kind 4 with authors = our pubkey.
pub fn filter_dms_sent(our_pubkey_hex: &str, limit: u32, since: Option<u64>) -> Filter {
    Filter {
        authors: Some(vec![our_pubkey_hex.to_string()]),
        kinds: Some(vec![KIND_DM]),
        since,
        limit: Some(limit),
        ..Default::default()
    }
}

/// NIP-17 gift wraps addressed to us: kind 1059, #p = our pubkey.
pub fn filter_gift_wraps_received(our_pubkey_hex: &str, limit: u32, since: Option<u64>) -> Filter {
    Filter {
        kinds: Some(vec![KIND_GIFT_WRAP]),
        since,
        limit: Some(limit),
        p_tags: Some(vec![our_pubkey_hex.to_string()]),
        ..Default::default()
    }
}

/// Kind 10050 DM relay list by author.
pub fn filter_dm_relay_list_by_author(author_pubkey: &str) -> Filter {
    Filter {
        authors: Some(vec![author_pubkey.to_string()]),
        kinds: Some(vec![KIND_DM_RELAY_LIST]),
        limit: Some(1),
        ..Default::default()
    }
}

/// Get the recipient pubkey (hex) from a kind 4 event's "p" tag.
pub fn get_recipient_pubkey_from_kind4(event: &Event) -> Option<String> {
    if event.kind != KIND_DM {
        return None;
    }
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "p" {
            return Some(tag[1].clone());
        }
    }
    None
}

/// For a kind 4 DM, return the "other" pubkey (conversation partner that is not us).
pub fn other_pubkey_in_dm(event: &Event, our_pubkey_hex: &str) -> Option<String> {
    let our = our_pubkey_hex.to_lowercase();
    let sender = event.pubkey.to_lowercase();
    let recipient = get_recipient_pubkey_from_kind4(event)?.to_lowercase();
    if sender == our {
        Some(recipient)
    } else if recipient == our {
        Some(sender)
    } else {
        None
    }
}

/// Compact event JSON (no whitespace) for embedding inside encrypted payloads (NIP-44/59).
pub fn event_to_json_compact(event: &Event) -> String {
    let mut json = String::new();
    json.push_str("{\"id\":\"");
    json.push_str(&escape_json_string(&event.id));
    json.push_str("\",\"pubkey\":\"");
    json.push_str(&escape_json_string(&event.pubkey));
    json.push_str("\",\"created_at\":");
    json.push_str(&event.created_at.to_string());
    json.push_str(",\"kind\":");
    json.push_str(&event.kind.to_string());
    json.push_str(",\"tags\":[");
    for (i, tag) in event.tags.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('[');
        for (j, item) in tag.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json_string(item));
            json.push('"');
        }
        json.push(']');
    }
    json.push_str("],\"content\":\"");
    json.push_str(&escape_json_string(&event.content));
    json.push_str("\",\"sig\":\"");
    json.push_str(&escape_json_string(&event.sig));
    json.push_str("\"}");
    json
}

/// Parse kind 10050 DM relay list: extract relay URLs from `["relay", "wss://..."]` tags.
pub fn parse_dm_relay_list(event: &Event) -> Result<Vec<String>, String> {
    if event.kind != KIND_DM_RELAY_LIST {
        return Err(format!("Expected kind 10050 event, got kind {}", event.kind));
    }
    let mut urls: Vec<String> = Vec::new();
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "relay" && !tag[1].is_empty() {
            let url = tag[1].trim();
            if !url.is_empty() && !urls.contains(&url.to_string()) {
                urls.push(url.to_string());
            }
        }
    }
    Ok(urls)
}

/// Kind 0 profile metadata by author.
pub fn filter_profile_by_author(author_pubkey: &str) -> Filter {
    Filter {
        authors: Some(vec![author_pubkey.to_string()]),
        kinds: Some(vec![KIND_METADATA]),
        limit: Some(1),
        ..Default::default()
    }
}

/// Kind 10002 relay list by author.
pub fn filter_relay_list_by_author(author_pubkey: &str) -> Filter {
    Filter {
        authors: Some(vec![author_pubkey.to_string()]),
        kinds: Some(vec![KIND_RELAY_LIST]),
        limit: Some(1),
        ..Default::default()
    }
}

/// Kind 3 contacts list by author (may contain relay preferences in content).
pub fn filter_contacts_by_author(author_pubkey: &str) -> Filter {
    Filter {
        authors: Some(vec![author_pubkey.to_string()]),
        kinds: Some(vec![KIND_CONTACTS]),
        limit: Some(1),
        ..Default::default()
    }
}

/// Parse relay URLs from a kind 3 contacts event's content field.
/// The content is JSON like: {"wss://relay.damus.io": {"read": true, "write": true}, ...}
pub fn parse_contacts_relay_list(content: &str) -> Vec<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return Vec::new();
    }
    let mut handler = ContactsRelayHandler::new();
    let mut parser = JsonParser::new();
    let mut buf = BytesMut::from(trimmed.as_bytes());
    let _ = parser.receive(&mut buf, &mut handler);
    let _ = parser.close(&mut handler);
    handler.urls
}

struct ContactsRelayHandler {
    depth: i32,
    urls: Vec<String>,
}

impl ContactsRelayHandler {
    fn new() -> Self {
        Self { depth: 0, urls: Vec::new() }
    }
}

impl JsonContentHandler for ContactsRelayHandler {
    fn start_object(&mut self) { self.depth += 1; }
    fn end_object(&mut self) { self.depth -= 1; }
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, name: &str) {
        if self.depth == 1 && (name.starts_with("wss://") || name.starts_with("ws://")) {
            if !self.urls.contains(&name.to_string()) {
                self.urls.push(name.to_string());
            }
        }
    }
    fn string_value(&mut self, _value: &str) {}
    fn number_value(&mut self, _number: JsonNumber) {}
    fn boolean_value(&mut self, _value: bool) {}
    fn null_value(&mut self) {}
}

/// Parse kind 10002 relay list: extract relay URLs from `["r", "wss://..."]` tags.
pub fn parse_relay_list(event: &Event) -> Result<Vec<String>, String> {
    if event.kind != KIND_RELAY_LIST {
        return Err(format!("Expected kind 10002 event, got kind {}", event.kind));
    }
    let mut urls: Vec<String> = Vec::new();
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "r" && !tag[1].is_empty() {
            let url = tag[1].trim();
            if !url.is_empty() && !urls.contains(&url.to_string()) {
                urls.push(url.to_string());
            }
        }
    }
    Ok(urls)
}

/// Profile metadata from a kind 0 event (NIP-01).
#[derive(Debug, Clone)]
pub struct ProfileMetadata {
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
    pub banner: Option<String>,
    pub website: Option<String>,
    pub lud16: Option<String>,
    pub created_at: Option<u64>,
}

/// Parse profile metadata from a kind 0 event's content JSON.
pub fn parse_profile(content: &str) -> Result<ProfileMetadata, String> {
    let mut handler = ProfileHandler::new();
    let mut parser = JsonParser::new();
    let mut buf = BytesMut::from(content.as_bytes());
    parser.receive(&mut buf, &mut handler)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    parser.close(&mut handler)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    Ok(handler.take_profile())
}

struct ProfileHandler {
    current_field: Option<String>,
    name: Option<String>,
    about: Option<String>,
    picture: Option<String>,
    nip05: Option<String>,
    banner: Option<String>,
    website: Option<String>,
    lud16: Option<String>,
}

impl ProfileHandler {
    fn new() -> Self {
        Self {
            current_field: None,
            name: None,
            about: None,
            picture: None,
            nip05: None,
            banner: None,
            website: None,
            lud16: None,
        }
    }

    fn take_profile(&self) -> ProfileMetadata {
        ProfileMetadata {
            name: self.name.clone(),
            about: self.about.clone(),
            picture: self.picture.clone(),
            nip05: self.nip05.clone(),
            banner: self.banner.clone(),
            website: self.website.clone(),
            lud16: self.lud16.clone(),
            created_at: None,
        }
    }
}

impl JsonContentHandler for ProfileHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_field = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if let Some(ref f) = self.current_field {
            match f.as_str() {
                "name" | "display_name" => {
                    if self.name.is_none() {
                        self.name = Some(value.to_string());
                    }
                }
                "about" => self.about = Some(value.to_string()),
                "picture" => self.picture = Some(value.to_string()),
                "nip05" => self.nip05 = Some(value.to_string()),
                "banner" => self.banner = Some(value.to_string()),
                "website" => self.website = Some(value.to_string()),
                "lud16" => self.lud16 = Some(value.to_string()),
                _ => {}
            }
        }
    }

    fn number_value(&mut self, _number: JsonNumber) {}
    fn boolean_value(&mut self, _value: bool) {}
    fn null_value(&mut self) {}
}

// ============================================================
// Event JSON Push-Parser
// ============================================================

struct EventHandler {
    depth: i32,
    current_field: Option<String>,
    id: Option<String>,
    pubkey: Option<String>,
    created_at: u64,
    kind: u32,
    content: String,
    sig: Option<String>,
    tags: Vec<Vec<String>>,
    current_tag: Vec<String>,
    tags_depth: i32,
}

impl EventHandler {
    fn new() -> Self {
        Self {
            depth: 0,
            current_field: None,
            id: None,
            pubkey: None,
            created_at: 0,
            kind: 0,
            content: String::new(),
            sig: None,
            tags: Vec::new(),
            current_tag: Vec::new(),
            tags_depth: 0,
        }
    }

    fn take_event(&self) -> Result<Event, String> {
        Ok(Event {
            id: self.id.clone().ok_or("Missing 'id' field")?,
            pubkey: self.pubkey.clone().ok_or("Missing 'pubkey' field")?,
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags.clone(),
            content: self.content.clone(),
            sig: self.sig.clone().unwrap_or_default(),
        })
    }
}

impl JsonContentHandler for EventHandler {
    fn start_object(&mut self) { self.depth += 1; }

    fn end_object(&mut self) { self.depth -= 1; }

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
        if self.tags_depth == 2 && self.depth == 3 {
            if !self.current_tag.is_empty() {
                self.tags.push(self.current_tag.clone());
            }
            self.current_tag.clear();
        } else if self.tags_depth == 2 && self.depth == 2 {
            self.tags_depth = 0;
        } else if self.tags_depth == 1 && self.depth == 2 {
            self.tags_depth = 0;
        }
        self.depth -= 1;
    }

    fn key(&mut self, key: &str) {
        self.current_field = Some(key.to_string());
        if self.depth == 1 && key == "tags" {
            self.tags_depth = 1;
        }
    }

    fn string_value(&mut self, value: &str) {
        if self.tags_depth == 2 {
            self.current_tag.push(value.to_string());
        } else if self.depth == 1 {
            if let Some(ref f) = self.current_field {
                match f.as_str() {
                    "id" => self.id = Some(value.to_string()),
                    "pubkey" => self.pubkey = Some(value.to_string()),
                    "content" => self.content = value.to_string(),
                    "sig" => self.sig = Some(value.to_string()),
                    _ => {}
                }
            }
        }
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.depth == 1 {
            if let Some(ref f) = self.current_field {
                if f == "created_at" {
                    self.created_at = number.as_f64().max(0.0) as u64;
                } else if f == "kind" {
                    self.kind = number.as_f64().max(0.0) as u32;
                }
            }
        }
    }

    fn boolean_value(&mut self, _value: bool) {}
    fn null_value(&mut self) {}
}

/// Parse a single Nostr event from a JSON string.
pub fn parse_event(json_str: &str) -> Result<Event, String> {
    let mut handler = EventHandler::new();
    let mut parser = JsonParser::new();
    let mut buf = BytesMut::from(json_str.as_bytes());
    parser.receive(&mut buf, &mut handler)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    parser.close(&mut handler)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    handler.take_event()
}
