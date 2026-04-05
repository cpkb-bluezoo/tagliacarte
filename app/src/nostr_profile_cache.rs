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
use serde::{Deserialize, Serialize};
use tagliacarte_core::config::default_config_dir;
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct NostrProfileCacheEntry {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub nip05: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    /// Wall clock when we last wrote this row (fetch or merge).
    pub fetched_at: u64,
    #[serde(default)]
    pub kind0_created_at: Option<u64>,
    /// True when last fetch found no kind 0 (avoid hammering relays).
    #[serde(default)]
    pub negative: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct FilePayload {
    version: u32,
    #[serde(default)]
    profiles: HashMap<String, NostrProfileCacheEntry>,
}

static CACHE: Lazy<Mutex<FilePayload>> = Lazy::new(|| {
    Mutex::new(load_from_disk().unwrap_or_else(|_| FilePayload {
        version: CACHE_VERSION,
        profiles: HashMap::new(),
    }))
});

fn load_from_disk() -> Result<FilePayload, String> {
    let path = cache_path().ok_or_else(|| "no config dir".to_string())?;
    if !path.is_file() {
        return Ok(FilePayload {
            version: CACHE_VERSION,
            profiles: HashMap::new(),
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut p: FilePayload = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if p.version != CACHE_VERSION {
        p.version = CACHE_VERSION;
    }
    Ok(p)
}

fn save_locked(data: &FilePayload) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, s);
    }
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
