/*
 * nostr_profile_cache.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Disk-backed cache of Nostr kind-0 metadata keyed by lowercase hex pubkey.
 */

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use tagliacarte_core::config::default_config_dir;
use tagliacarte_core::json::{
    parse_bytes_complete, IndentConfig, JsonContentHandler, JsonNumber, JsonWriter, writer_into_string,
};
use tagliacarte_core::protocol::nostr::keys::{hex_to_npub, is_valid_hex_key};
use tagliacarte_core::protocol::nostr::ProfileMetadata;

const CACHE_VERSION: u32 = 1;
/// Refresh from relays after this many seconds (even if we have a cached row).
const PROFILE_CACHE_TTL_SECS: u64 = 86_400;
/// When kind 0 was missing, wait this long before trying again.
const NEGATIVE_CACHE_TTL_SECS: u64 = 3_600;

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_path() -> Option<PathBuf> {
    let mut p = default_config_dir()?;
    p.push("nostr_profile_cache.json");
    Some(p)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NostrProfileCacheEntry {
    pub name: Option<String>,
    pub nip05: Option<String>,
    pub picture: Option<String>,
    /// Wall clock when we last wrote this row (fetch or merge).
    pub fetched_at: u64,
    pub kind0_created_at: Option<u64>,
    /// True when last fetch found no kind 0 (avoid hammering relays).
    pub negative: bool,
}

struct FilePayload {
    version: u32,
    profiles: HashMap<String, NostrProfileCacheEntry>,
}

static CACHE: Lazy<Mutex<FilePayload>> = Lazy::new(|| {
    Mutex::new(load_from_disk().unwrap_or_else(|_| FilePayload {
        version: CACHE_VERSION,
        profiles: HashMap::new(),
    }))
});

/// Push-parser for `{"version":n,"profiles":{ "<pk>": { ... } }}` — no DOM.
struct NostrCacheFileLoadHandler {
    depth: usize,
    key_root: Option<String>,
    expect_profiles_map: bool,
    /// Depth of the JSON object that is the value of `"profiles"` (closes when `depth == profiles_map_depth` at `end_object`).
    profiles_map_depth: usize,
    in_profiles_map: bool,
    profile_pk: Option<String>,
    in_profile_entry: bool,
    key_field: Option<String>,
    version: Option<u32>,
    profiles: HashMap<String, NostrProfileCacheEntry>,
    entry: NostrProfileCacheEntry,
}

impl JsonContentHandler for NostrCacheFileLoadHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.expect_profiles_map {
            self.expect_profiles_map = false;
            self.in_profiles_map = true;
            self.profiles_map_depth = self.depth;
            return;
        }
        if self.in_profiles_map && self.profile_pk.is_some() {
            self.in_profile_entry = true;
            self.entry = NostrProfileCacheEntry::default();
        }
    }

    fn end_object(&mut self) {
        if self.in_profile_entry {
            if let Some(pk) = self.profile_pk.take() {
                let ent = std::mem::replace(
                    &mut self.entry,
                    NostrProfileCacheEntry::default(),
                );
                self.profiles.insert(pk, ent);
            }
            self.in_profile_entry = false;
            self.depth -= 1;
            return;
        }
        if self.in_profiles_map && self.depth == self.profiles_map_depth {
            self.in_profiles_map = false;
            self.profiles_map_depth = 0;
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {}

    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        if key == "profiles" {
            self.expect_profiles_map = true;
        }
        if !self.in_profiles_map {
            self.key_root = Some(key.to_string());
        } else if !self.in_profile_entry {
            self.profile_pk = Some(key.to_string());
        } else {
            self.key_field = Some(key.to_string());
        }
    }

    fn string_value(&mut self, value: &str) {
        if self.in_profile_entry {
            match self.key_field.as_deref() {
                Some("name") => self.entry.name = Some(value.to_string()),
                Some("nip05") => self.entry.nip05 = Some(value.to_string()),
                Some("picture") => self.entry.picture = Some(value.to_string()),
                _ => {}
            }
        }
        self.key_field = None;
    }

    fn number_value(&mut self, n: JsonNumber) {
        if !self.in_profiles_map && self.key_root.as_deref() == Some("version") {
            self.version = n.as_u64().map(|u| u as u32);
        } else if self.in_profile_entry {
            match self.key_field.as_deref() {
                Some("fetched_at") => self.entry.fetched_at = n.as_u64().unwrap_or(0),
                Some("kind0_created_at") => self.entry.kind0_created_at = n.as_u64(),
                _ => {}
            }
        }
        self.key_field = None;
        self.key_root = None;
    }

    fn boolean_value(&mut self, value: bool) {
        if self.in_profile_entry && self.key_field.as_deref() == Some("negative") {
            self.entry.negative = value;
        }
        self.key_field = None;
    }

    fn null_value(&mut self) {
        self.key_field = None;
    }
}

fn load_from_disk() -> Result<FilePayload, String> {
    let path = cache_path().ok_or_else(|| "no config dir".to_string())?;
    if !path.is_file() {
        return Ok(FilePayload {
            version: CACHE_VERSION,
            profiles: HashMap::new(),
        });
    }
    let raw = fs::read(&path).map_err(|e| e.to_string())?;
    let mut h = NostrCacheFileLoadHandler {
        depth: 0,
        key_root: None,
        expect_profiles_map: false,
        profiles_map_depth: 0,
        in_profiles_map: false,
        profile_pk: None,
        in_profile_entry: false,
        key_field: None,
        version: None,
        profiles: HashMap::new(),
        entry: NostrProfileCacheEntry::default(),
    };
    parse_bytes_complete(&raw, &mut h).map_err(|e| e.to_string())?;
    let mut version = h.version.unwrap_or(CACHE_VERSION);
    if version != CACHE_VERSION {
        version = CACHE_VERSION;
    }
    Ok(FilePayload {
        version,
        profiles: h.profiles,
    })
}

fn save_locked(data: &FilePayload) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let indent = IndentConfig::spaces2();
    let mut w = JsonWriter::with_indent(indent);
    w.write_start_object();
    w.write_key("version");
    w.write_number(JsonNumber::I64(i64::from(data.version)));
    w.write_key("profiles");
    w.write_start_object();
    for (pk, e) in &data.profiles {
        w.write_key(pk);
        w.write_start_object();
        if let Some(ref n) = e.name {
            w.write_key("name");
            w.write_string(n);
        }
        if let Some(ref n) = e.nip05 {
            w.write_key("nip05");
            w.write_string(n);
        }
        if let Some(ref n) = e.picture {
            w.write_key("picture");
            w.write_string(n);
        }
        w.write_key("fetched_at");
        w.write_number(JsonNumber::I64(i64::try_from(e.fetched_at).unwrap_or(i64::MAX)));
        if let Some(ts) = e.kind0_created_at {
            w.write_key("kind0_created_at");
            w.write_number(JsonNumber::I64(i64::try_from(ts).unwrap_or(i64::MAX)));
        }
        if e.negative {
            w.write_key("negative");
            w.write_bool(true);
        }
        w.write_end_object();
    }
    w.write_end_object();
    w.write_end_object();
    let s = writer_into_string(w);
    let _ = fs::write(path, s);
}

/// Display label: cached name → nip05 → npub bech32 → raw hex.
pub(crate) fn display_label_for_pubkey_hex(pubkey_hex: &str) -> String {
    let pk = pubkey_hex.trim().to_lowercase();
    if !is_valid_hex_key(&pk) {
        return pubkey_hex.trim().to_string();
    }
    {
        let g = CACHE.lock().expect("nostr profile cache");
        if let Some(e) = g.profiles.get(&pk) {
            if let Some(n) = e.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                return n.to_string();
            }
            if let Some(n5) = e.nip05.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                return n5.to_string();
            }
        }
    }
    hex_to_npub(&pk).unwrap_or_else(|_| pk.clone())
}

/// Cached kind-0 fields for immediate UI (folder list); empty when unknown or negative-cached.
pub(crate) fn cached_profile_fields_for_emit(
    pubkey_hex: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let pk = pubkey_hex.trim().to_lowercase();
    if !is_valid_hex_key(&pk) {
        return (None, None, None);
    }
    let g = CACHE.lock().expect("nostr profile cache");
    let Some(e) = g.profiles.get(&pk) else {
        return (None, None, None);
    };
    if e.negative {
        return (None, None, None);
    }
    let name = e
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let nip05 = e
        .nip05
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let picture = e
        .picture
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    (name, nip05, picture)
}

/// Whether we should hit the network for this pubkey (no row, stale TTL, or negative cache expired).
pub(crate) fn should_fetch_profile(pubkey_hex: &str) -> bool {
    let pk = pubkey_hex.trim().to_lowercase();
    if !is_valid_hex_key(&pk) {
        return false;
    }
    let g = CACHE.lock().expect("nostr profile cache");
    let now = now_unix_secs();
    match g.profiles.get(&pk) {
        None => true,
        Some(e) => {
            if e.negative {
                return now.saturating_sub(e.fetched_at) >= NEGATIVE_CACHE_TTL_SECS;
            }
            now.saturating_sub(e.fetched_at) >= PROFILE_CACHE_TTL_SECS
        }
    }
}

pub(crate) fn merge_profile(pubkey_hex: &str, meta: &ProfileMetadata) {
    let pk = pubkey_hex.trim().to_lowercase();
    if !is_valid_hex_key(&pk) {
        return;
    }
    let mut g = CACHE.lock().expect("nostr profile cache");
    let entry = NostrProfileCacheEntry {
        name: meta.name.clone(),
        nip05: meta.nip05.clone(),
        picture: meta.picture.clone(),
        fetched_at: now_unix_secs(),
        kind0_created_at: meta.created_at,
        negative: false,
    };
    g.profiles.insert(pk, entry);
    save_locked(&g);
}

pub(crate) fn merge_negative_fetch(pubkey_hex: &str) {
    let pk = pubkey_hex.trim().to_lowercase();
    if !is_valid_hex_key(&pk) {
        return;
    }
    let mut g = CACHE.lock().expect("nostr profile cache");
    let entry = NostrProfileCacheEntry {
        name: None,
        nip05: None,
        picture: None,
        fetched_at: now_unix_secs(),
        kind0_created_at: None,
        negative: true,
    };
    g.profiles.insert(pk, entry);
    save_locked(&g);
}
