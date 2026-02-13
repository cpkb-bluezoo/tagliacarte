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

/// NIP-04: Encrypted direct message.
pub const KIND_DM: u32 = 4;

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
        authors: None,
        kinds: Some(vec![KIND_DM]),
        since,
        until: None,
        limit: Some(limit),
        p_tags: Some(vec![our_pubkey_hex.to_string()]),
        e_tags: None,
        ids: None,
    }
}

/// DMs we sent: kind 4 with authors = our pubkey.
pub fn filter_dms_sent(our_pubkey_hex: &str, limit: u32, since: Option<u64>) -> Filter {
    Filter {
        authors: Some(vec![our_pubkey_hex.to_string()]),
        kinds: Some(vec![KIND_DM]),
        since,
        until: None,
        limit: Some(limit),
        p_tags: None,
        e_tags: None,
        ids: None,
    }
}
