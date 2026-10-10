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

//! Nostr backend (Store, Folder, Transport). DM-only: NIP-04 (kind 4) and NIP-17/59 (kind 1059).
//! Each Folder = one DM conversation (one contact pubkey).
//! Secret keys (nsec) stored in credential store (keychain / encrypted file), never logged.

pub mod cache;
pub mod crypto;
pub mod keys;
pub mod media;
mod relay;
mod types;

pub use crypto::{
    compute_event_id, create_blossom_auth_event, create_nip17_dm, create_nip98_auth_event,
    create_signed_dm, generate_keypair, get_public_key_from_secret, nip04_decrypt, nip04_encrypt,
    nip44_conversation_key, nostr_auth_header, sha256_hex, sign_event, unwrap_gift_wrap,
    verify_event_signature,
};
pub use keys::{
    hex_to_npub, hex_to_nsec, is_npub, is_nsec, is_valid_hex_key, npub_to_hex, nsec_to_hex,
    public_key_to_hex, secret_key_to_hex,
};
pub use relay::{
    fetch_contacts_relay_list_from_relays, fetch_dm_relay_list_from_relays,
    fetch_dm_relay_list_from_relays_blocking, fetch_notes_from_relay, fetch_profile_from_relay,
    fetch_profile_from_relays, fetch_profile_from_relays_blocking, fetch_relay_list_from_relay,
    fetch_relay_list_from_relays, fetch_relay_list_from_relays_blocking, OnFanOut,
};
pub use relay::{
    parse_relay_message, publish_event, publish_event_blocking, run_relay_dm_stream,
    run_relay_dm_stream_nip17, run_relay_feed_stream, Done as RelayDone, RelayMessage,
    StreamMessage, StreamSink,
};
pub use types::{
    event_to_json, event_to_json_compact, filter_dm_relay_list_by_author, filter_dms_received,
    filter_dms_sent, filter_gift_wraps_received, filter_to_json, other_pubkey_in_dm,
    parse_dm_relay_list, parse_event, Event, Filter, KIND_BLOSSOM_AUTH, KIND_CHAT_MESSAGE, KIND_DM,
    KIND_DM_RELAY_LIST, KIND_GIFT_WRAP, KIND_HTTP_AUTH, KIND_SEAL,
};
pub use types::{
    filter_contacts_by_author, filter_profile_by_author, filter_relay_list_by_author,
    parse_contacts_relay_list, parse_profile, parse_relay_list, ProfileMetadata, KIND_CONTACTS,
    KIND_METADATA, KIND_RELAY_LIST,
};

pub const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
    "wss://nostr.land",
    "wss://relay.primal.net",
];

/// Relays that index profiles (kind 0) and relay lists (kinds 10002, 3) for the whole network. Most
/// general-purpose relays drop old metadata, so a profile that is gone from the relays a user posts to is
/// usually still here.
pub const PROFILE_INDEXER_RELAYS: &[&str] = &["wss://purplepag.es"];

/// `relays` plus the profile indexers not already in it.
pub fn with_profile_indexers(mut relays: Vec<String>) -> Vec<String> {
    let norm = |u: &str| u.trim_end_matches('/').to_lowercase();
    let mut seen: HashSet<String> = relays.iter().map(|u| norm(u)).collect();
    for url in PROFILE_INDEXER_RELAYS {
        if seen.insert(norm(url)) {
            relays.push((*url).to_string());
        }
    }
    relays
}

use crate::message_id::MessageId;
use crate::store::{Address, ConversationSummary, DateTime, Envelope};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::{SendPayload, Transport, TransportKind};
use std::collections::HashSet;
use std::ops::Range;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Nostr store: identity (pubkey) + relay list. list_folders = one folder per DM contact.
/// The nsec is loaded from the credential store via `set_credential`.
pub struct NostrStore {
    uri: String,
    pub pubkey_hex: String,
    relays: Vec<String>,
    secret_key_hex: Arc<RwLock<Option<String>>>,
    config_dir: Option<String>,
    /// Optional hook after background relay/DM sync (e.g. push updated folder list to the UI).
    folder_sync_idle_cb: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl NostrStore {
    pub fn new(
        relays: Vec<String>,
        pubkey_hex: String,
        config_dir: Option<String>,
        folder_sync_idle_cb: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, StoreError> {
        let uri = match keys::hex_to_npub(&pubkey_hex) {
            Ok(npub) => crate::uri::nostr_account_uri(&npub),
            Err(_) => crate::uri::nostr_account_uri(&pubkey_hex),
        };
        Ok(Self {
            uri,
            pubkey_hex,
            relays,
            secret_key_hex: Arc::new(RwLock::new(None)),
            config_dir,
            folder_sync_idle_cb,
        })
    }

    /// Conversation folder names (peer pubkeys) from on-disk cache only — no network.
    pub fn list_cached_conversation_pubkeys(&self) -> Result<Vec<String>, StoreError> {
        let config_dir = self.get_config_dir()?;
        let pubkey_hex = self.pubkey_hex.to_lowercase();
        cache::ensure_cache_dir(&config_dir, &pubkey_hex)
            .map_err(|e| StoreError::new(format!("Cache dir: {e}")))?;
        let convos = cache::list_conversations_with_timestamps(&config_dir, &pubkey_hex)
            .map_err(|e| StoreError::new(e.to_string()))?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (pk, _) in convos {
            if seen.insert(pk.clone()) {
                out.push(pk);
            }
        }
        Ok(out)
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Build a [`NostrTransport`] with the same relays, identity, runtime, config dir, and secret as this store.
    ///
    /// Outgoing direct messages should use this transport (see the app’s `nostr_send` module)
    /// rather than treating send as a generic store operation.
    pub fn paired_transport(&self) -> Result<NostrTransport, StoreError> {
        let t = NostrTransport::new(self.relays.clone(), self.pubkey_hex.clone(), self.config_dir.clone())?;
        if let Ok(guard) = self.secret_key_hex.read() {
            if let Some(h) = guard.as_ref() {
                t.set_secret_key(h);
            }
        }
        Ok(t)
    }

    fn get_secret(&self) -> Result<String, StoreError> {
        self.secret_key_hex
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?
            .clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.pubkey_hex.clone(),
                is_plaintext: false,
                advertised_capabilities: None,
            })
    }

    fn get_config_dir(&self) -> Result<String, StoreError> {
        self.config_dir
            .clone()
            .ok_or_else(|| StoreError::new("Config directory not set"))
    }
}

impl Store for NostrStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Nostr
    }

    fn set_credential(&self, _username: Option<&str>, password: &str) {
        let hex = match keys::secret_key_to_hex(password) {
            Ok(h) => h,
            Err(_) => return,
        };
        if let Ok(mut guard) = self.secret_key_hex.write() {
            *guard = Some(hex);
        }
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // Always stderr: confirms this path ran. Detailed protocol lines need TAGLIACARTE_TRACE=nostr
        // (or all); many GUI/IDE launches do not inherit variables from ~/.zshrc etc.
        eprintln!(
            "[nostr] list_folders; protocol trace {}",
            if crate::trace::enabled("nostr") {
                "on"
            } else {
                "off"
            }
        );
        static TRACE_OFF_HINT: OnceLock<()> = OnceLock::new();
        if !crate::trace::enabled("nostr") {
            TRACE_OFF_HINT.get_or_init(|| {
                let raw = std::env::var("TAGLIACARTE_TRACE").unwrap_or_default();
                let shown = if raw.is_empty() {
                    "(unset)".to_string()
                } else {
                    raw
                };
                eprintln!(
                    "[nostr] hint: set TAGLIACARTE_TRACE=nostr (or all) in the **process environment** before start. \
                     TAGLIACARTE_TRACE={shown:?}. IDE Run / macOS .app bundles often ignore shell exports — use launch.json env, Xcode scheme, or `TAGLIACARTE_TRACE=nostr flutter run`."
                );
            });
        }

        let secret_hex = match self.get_secret() {
            Ok(s) => {
                crate::trace_log!("nostr", "list_folders: secret key available");
                s
            }
            Err(e) => {
                crate::trace_log!("nostr", "list_folders: no secret key: {}", e);
                on_complete(Err(e));
                return;
            }
        };
        let config_dir = match self.get_config_dir() {
            Ok(d) => d,
            Err(e) => {
                crate::trace_log!("nostr", "list_folders: no config dir: {}", e);
                on_complete(Err(e));
                return;
            }
        };
        let pubkey_hex = self.pubkey_hex.clone();
        let bootstrap_relays = self.relays.clone();
        let folder_sync_idle_cb = self.folder_sync_idle_cb.clone();
        crate::trace_log!(
            "nostr",
            "list_folders: pubkey={}, bootstrap_relays={:?}, config_dir={}",
            pubkey_hex,
            bootstrap_relays,
            config_dir
        );

        // Relays and filters expect lowercase hex; matches event tags and cache paths.
        let pubkey_lower = pubkey_hex.to_lowercase();

        if let Err(e) = cache::ensure_cache_dir(&config_dir, &pubkey_lower) {
            crate::trace_log!("nostr", "list_folders: cache dir error: {}", e);
            on_complete(Err(StoreError::new(format!("Cache dir: {}", e))));
            return;
        }

        let mut seen_pubkeys: HashSet<String> = HashSet::new();

        // Emit folders from local cache immediately so list_mail_folders / session return without
        // waiting for relay discovery + DM sync (can exceed UI timeouts and leave an empty pane).
        match cache::list_conversations_with_timestamps(&config_dir, &pubkey_lower) {
            Ok(convos) => {
                crate::trace_log!(
                    "nostr",
                    "list_folders: {} cached conversations",
                    convos.len()
                );
                for (pk, _ts) in &convos {
                    if seen_pubkeys.insert(pk.clone()) {
                        on_folder(FolderInfo {
                            name: pk.clone(),
                            delimiter: None,
                            attributes: Vec::new(),
                        });
                    }
                }
            }
            Err(e) => {
                crate::trace_log!("nostr", "list_folders: cache list error: {}", e);
            }
        }

        on_complete(Ok(()));

        discover_and_sync(DiscoveryCtx {
            pubkey_hex: pubkey_lower,
            secret_hex,
            config_dir,
            bootstrap_relays,
            folder_sync_idle_cb,
            seen_pubkeys,
        });
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        let secret_hex = match self.get_secret() {
            Ok(s) => s,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let config_dir = match self.get_config_dir() {
            Ok(d) => d,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };

        let folder = NostrFolder {
            our_secret_hex: secret_hex,
            our_pubkey_hex: self.pubkey_hex.clone(),
            other_pubkey_hex: name.to_lowercase(),
            config_dir,
        };
        on_complete(Ok(Box::new(folder)));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Folder = one DM conversation with a contact. Messages = kind 4 and/or kind 1059 events.
struct NostrFolder {
    our_secret_hex: String,
    our_pubkey_hex: String,
    other_pubkey_hex: String,
    config_dir: String,
}

impl Folder for NostrFolder {
    fn list_conversations(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let messages = match cache::get_messages(
            &self.config_dir,
            &self.our_secret_hex,
            &self.our_pubkey_hex,
            &self.other_pubkey_hex,
        ) {
            Ok(m) => m,
            Err(e) => {
                on_complete(Err(StoreError::new(e)));
                return;
            }
        };

        let start = range.start as usize;
        let end = (range.end as usize).min(messages.len());

        if start < messages.len() {
            for msg in &messages[start..end] {
                let from_addr = Address {
                    display_name: None,
                    local_part: msg.pubkey.clone(),
                    domain: None,
                };
                let to_addr = Address {
                    display_name: None,
                    local_part: if msg.is_outgoing {
                        self.other_pubkey_hex.clone()
                    } else {
                        self.our_pubkey_hex.clone()
                    },
                    domain: None,
                };

                let envelope = Envelope {
                    from: vec![from_addr],
                    to: vec![to_addr],
                    cc: Vec::new(),
                    date: Some(DateTime {
                        timestamp: msg.created_at as i64,
                        tz_offset_secs: Some(0),
                    }),
                    subject: Some(msg.content.clone()),
                    message_id: Some(msg.id.clone()),
                    in_reply_to: None,
                    references: None,
                };

                let message_id = crate::message_id::nostr_dm_message_id(&msg.id);
                on_summary(ConversationSummary {
                    id: message_id,
                    envelope,
                    flags: std::collections::HashSet::new(),
                    size: msg.content.len() as u64,
                });
            }
        }

        on_complete(Ok(()));
    }

    fn message_count(&self, on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>) {
        let count = match cache::get_messages(
            &self.config_dir,
            &self.our_secret_hex,
            &self.our_pubkey_hex,
            &self.other_pubkey_hex,
        ) {
            Ok(m) => m.len() as u64,
            Err(_) => 0,
        };
        on_complete(Ok(count));
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let id_str = id.as_str();
        // Strip nostr:dm: prefix if present
        let event_id = id_str.strip_prefix("nostr:dm:").unwrap_or(id_str);

        let messages = match cache::get_messages(
            &self.config_dir,
            &self.our_secret_hex,
            &self.our_pubkey_hex,
            &self.other_pubkey_hex,
        ) {
            Ok(m) => m,
            Err(e) => {
                on_complete(Err(StoreError::new(e)));
                return;
            }
        };

        let msg = match messages.iter().find(|m| m.id == event_id) {
            Some(m) => m,
            None => {
                on_complete(Err(StoreError::new(format!(
                    "Message {} not found",
                    event_id
                ))));
                return;
            }
        };

        let from_addr = Address {
            display_name: None,
            local_part: msg.pubkey.clone(),
            domain: None,
        };
        let to_addr = Address {
            display_name: None,
            local_part: if msg.is_outgoing {
                self.other_pubkey_hex.clone()
            } else {
                self.our_pubkey_hex.clone()
            },
            domain: None,
        };

        let envelope = Envelope {
            from: vec![from_addr],
            to: vec![to_addr],
            cc: Vec::new(),
            date: Some(DateTime {
                timestamp: msg.created_at as i64,
                tz_offset_secs: Some(0),
            }),
            subject: Some(truncate_content(&msg.content, 80)),
            message_id: Some(msg.id.clone()),
            in_reply_to: None,
            references: None,
        };

        on_metadata(envelope);
        on_content_chunk(msg.content.as_bytes());
        on_complete(Ok(()));
    }
}

/// Truncate content for subject preview.
fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        let mut s = content.chars().take(max_len).collect::<String>();
        s.push_str("...");
        s
    }
}

/// Nostr transport: send kind 4 DMs (NIP-04) or kind 1059 gift wraps (NIP-17).
pub struct NostrTransport {
    uri: String,
    pub pubkey_hex: String,
    relays: Vec<String>,
    secret_key_hex: Arc<RwLock<Option<String>>>,
    config_dir: Option<String>,
}

impl NostrTransport {
    pub fn new(
        relays: Vec<String>,
        pubkey_hex: String,
        config_dir: Option<String>,
    ) -> Result<Self, StoreError> {
        let uri = crate::uri::nostr_transport_uri(&pubkey_hex);
        Ok(Self {
            uri,
            pubkey_hex,
            relays,
            secret_key_hex: Arc::new(RwLock::new(None)),
            config_dir,
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Set the secret key (accepts nsec bech32 or hex).
    pub fn set_secret_key(&self, key: &str) {
        let hex = match keys::secret_key_to_hex(key) {
            Ok(h) => h,
            Err(_) => return,
        };
        if let Ok(mut guard) = self.secret_key_hex.write() {
            *guard = Some(hex);
        }
    }

    fn get_secret(&self) -> Result<String, StoreError> {
        self.secret_key_hex
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?
            .clone()
            .ok_or_else(|| StoreError::new("Nostr secret key not set"))
    }

    fn get_config_dir(&self) -> Result<String, StoreError> {
        self.config_dir
            .clone()
            .ok_or_else(|| StoreError::new("Config directory not set"))
    }
}

impl Transport for NostrTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Nostr
    }

    fn send(
        &self,
        payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let secret_hex = match self.get_secret() {
            Ok(s) => s,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };
        let config_dir = match self.get_config_dir() {
            Ok(d) => d,
            Err(e) => {
                on_complete(Err(e));
                return;
            }
        };

        let recipient_pubkey = if let Some(addr) = payload.to.first() {
            match keys::public_key_to_hex(&addr.local_part) {
                Ok(pk) => pk,
                Err(e) => {
                    on_complete(Err(StoreError::new(format!(
                        "Invalid recipient pubkey: {}",
                        e
                    ))));
                    return;
                }
            }
        } else {
            on_complete(Err(StoreError::new("No recipient specified")));
            return;
        };

        let plaintext = payload.body_plain.clone().unwrap_or_default();
        if plaintext.is_empty() {
            on_complete(Err(StoreError::new("Message body is empty")));
            return;
        }

        let relays = self.relays.clone();
        // Try to discover the recipient's NIP-17 DM relay list (kind 10050),
        // then send a gift wrap there, or a kind 4 DM when it has none.
        let (secret, recipient, our_relays) = (secret_hex.clone(), recipient_pubkey.clone(), relays.clone());
        query_dm_relay_list(
            &relays,
            &recipient_pubkey,
            Some(secret_hex),
            Box::new(move |dm_relays| {
                let done: Box<dyn FnOnce(Result<(), String>) + Send> =
                    Box::new(move |r| on_complete(r.map_err(StoreError::new)));
                if !dm_relays.is_empty() {
                    send_nip17(&secret, &recipient, &plaintext, our_relays, dm_relays, &config_dir, done)
                } else {
                    send_nip04(&secret, &recipient, &plaintext, our_relays, &config_dir, done)
                }
            }),
        );
    }
}

/// What the background relay discovery and DM sync need.
struct DiscoveryCtx {
    pubkey_hex: String,
    secret_hex: String,
    config_dir: String,
    bootstrap_relays: Vec<String>,
    folder_sync_idle_cb: Option<Arc<dyn Fn() + Send + Sync>>,
    seen_pubkeys: HashSet<String>,
}

/// Relay discovery (kind 10002, then kind 3, then a profile check), then the
/// DM sync on every relay found, one hopf connection each.
fn discover_and_sync(ctx: DiscoveryCtx) {
    // Combine bootstrap + defaults for maximum coverage during discovery.
    let mut discovery_relays: Vec<String> = Vec::new();
    let mut discovery_seen: HashSet<String> = HashSet::new();
    for url in ctx
        .bootstrap_relays
        .iter()
        .map(|s| s.as_str())
        .chain(DEFAULT_RELAYS.iter().copied())
        .chain(PROFILE_INDEXER_RELAYS.iter().copied())
    {
        let normalized = url.trim_end_matches('/').to_lowercase();
        if discovery_seen.insert(normalized) {
            discovery_relays.push(url.to_string());
        }
    }
    let state = Arc::new(Discovery {
        ctx,
        discovery_relays,
        // Relays that reject NIP-42 auth, so we stop contacting them.
        dead_relays: Mutex::new(HashSet::new()),
    });
    // Try kind 10002 (NIP-65 relay list metadata) first.
    crate::trace_log!(
        "nostr",
        "list_folders: fetching kind 10002 relay list from {} relays",
        state.discovery_relays.len()
    );
    let s2 = Arc::clone(&state);
    relay::fetch_relay_list_from_relays(
        &state.discovery_relays,
        &state.ctx.pubkey_hex,
        8,
        Some(state.ctx.secret_hex.clone()),
        Box::new(move |(relay_list_result, auth_failed)| {
            s2.mark_dead(auth_failed);
            let discovered = relay_list_result.unwrap_or_default();
            if !discovered.is_empty() {
                return s2.with_relays(discovered);
            }
            // Fall back to kind 3 contacts event (older clients store relays in content).
            crate::trace_log!("nostr", "list_folders: no kind 10002 found, trying kind 3 contacts...");
            let s3 = Arc::clone(&s2);
            relay::fetch_contacts_relay_list_from_relays(
                &s2.alive(),
                &s2.ctx.pubkey_hex,
                8,
                Some(s2.ctx.secret_hex.clone()),
                Box::new(move |(contacts_result, auth_failed)| {
                    s3.mark_dead(auth_failed);
                    let discovered = contacts_result.unwrap_or_default();
                    if !discovered.is_empty() {
                        return s3.with_relays(discovered);
                    }
                    // No published relay list: verify the user exists, then use bootstrap + defaults.
                    crate::trace_log!("nostr", "list_folders: no published relay list, checking if profile exists...");
                    let s4 = Arc::clone(&s3);
                    relay::fetch_profile_from_relays(
                        &s3.alive(),
                        &s3.ctx.pubkey_hex,
                        8,
                        Some(s3.ctx.secret_hex.clone()),
                        Box::new(move |(profile_result, auth_failed)| {
                            s4.mark_dead(auth_failed);
                            let profile = profile_result.unwrap_or(None);
                            if profile.is_none() {
                                let dead = s4.dead_relays.lock().unwrap().len();
                                let msg = format!(
                                    "Could not find profile for this Nostr identity on any relay. \
                                     Tried {} relays ({} required auth, {} actually queried). \
                                     Check that the secret key is correct and that \
                                     the account has been published to at least one relay.",
                                    s4.discovery_relays.len(),
                                    dead,
                                    s4.discovery_relays.len() - dead
                                );
                                crate::trace_log!(
                                    "nostr",
                                    "list_folders: {} (background sync stops; cached folders already returned)",
                                    msg
                                );
                                s4.idle();
                                return;
                            }
                            crate::trace_log!(
                                "nostr",
                                "list_folders: profile found but no published relay list, using bootstrap + defaults"
                            );
                            let relays = s4.alive();
                            s4.with_relays(relays)
                        }),
                    );
                }),
            );
        }),
    );
}

struct Discovery {
    ctx: DiscoveryCtx,
    discovery_relays: Vec<String>,
    dead_relays: Mutex<HashSet<String>>,
}

impl Discovery {
    fn mark_dead(&self, urls: Vec<String>) {
        self.dead_relays.lock().unwrap().extend(urls);
    }

    /// Discovery relays not found dead.
    fn alive(&self) -> Vec<String> {
        let dead = self.dead_relays.lock().unwrap();
        self.discovery_relays.iter().filter(|r| !dead.contains(r.as_str())).cloned().collect()
    }

    fn idle(&self) {
        if let Some(ref cb) = self.ctx.folder_sync_idle_cb {
            cb();
        }
    }

    /// The relay set is known: add the NIP-17 DM relays (kind 10050), then sync.
    fn with_relays(self: Arc<Self>, discovered: Vec<String>) {
        crate::trace_log!(
            "nostr",
            "list_folders: using {} discovered relays: {:?}",
            discovered.len(),
            discovered
        );
        let alive_discovery = self.alive();
        let me = Arc::clone(&self);
        relay::fetch_dm_relay_list_from_relays(
            &alive_discovery,
            &self.ctx.pubkey_hex,
            8,
            Some(self.ctx.secret_hex.clone()),
            Box::new(move |(dm_relays_res, dm_auth_failed)| {
                me.mark_dead(dm_auth_failed);
                let dead = me.dead_relays.lock().unwrap().clone();
                if !dead.is_empty() {
                    crate::trace_log!(
                        "nostr",
                        "list_folders: {} relays removed (auth-required): {:?}",
                        dead.len(),
                        dead
                    );
                }
                // Subscribe to DMs on the full relay set (excluding dead relays).
                let mut relays: Vec<String> = discovered.into_iter().filter(|r| !dead.contains(r.as_str())).collect();
                if let Ok(dm_urls) = dm_relays_res {
                    if !dm_urls.is_empty() {
                        crate::trace_log!(
                            "nostr",
                            "list_folders: merging {} NIP-17 DM relay URL(s) from kind 10050",
                            dm_urls.len()
                        );
                        let mut seen: HashSet<String> =
                            relays.iter().map(|r| r.trim_end_matches('/').to_lowercase()).collect();
                        for u in dm_urls {
                            let t = u.trim();
                            if t.is_empty() {
                                continue;
                            }
                            let key = t.trim_end_matches('/').to_lowercase();
                            if seen.insert(key) {
                                relays.push(t.to_string());
                            }
                        }
                    }
                }
                me.sync_dms(relays)
            }),
        );
    }

    /// One DM subscription per relay; the idle hook fires once all are done.
    fn sync_dms(self: Arc<Self>, relays: Vec<String>) {
        crate::trace_log!(
            "nostr",
            "list_folders: starting DM sync with {} relays: {:?}",
            relays.len(),
            relays
        );
        let pubkey_hex = &self.ctx.pubkey_hex;
        let filter_recv = types::filter_dms_received(pubkey_hex, 500, None);
        let filter_sent = types::filter_dms_sent(pubkey_hex, 500, None);
        let filter_gw = types::filter_gift_wraps_received(pubkey_hex, 500, None);
        let shared = Arc::new(Mutex::new(DmSync {
            seen_pubkeys: self.ctx.seen_pubkeys.clone(),
            event_count: 0,
            pending: relays.len(),
        }));
        if relays.is_empty() {
            self.sync_done(&shared);
            return;
        }
        for relay_url in relays {
            crate::trace_log!("nostr", "list_folders: spawning DM stream for {}", relay_url);
            relay::run_relay_dm_stream_nip17(
                relay_url,
                filter_recv.clone(),
                filter_sent.clone(),
                filter_gw.clone(),
                true,
                Some(self.ctx.secret_hex.clone()),
                Box::new(DmSink { discovery: Arc::clone(&self), shared: Arc::clone(&shared) }),
            );
        }
    }

    fn sync_done(&self, shared: &Arc<Mutex<DmSync>>) {
        let g = shared.lock().unwrap();
        crate::trace_log!(
            "nostr",
            "list_folders: relay sync complete, {} events received, {} total conversations",
            g.event_count,
            g.seen_pubkeys.len()
        );
        drop(g);
        self.idle();
    }
}

struct DmSync {
    seen_pubkeys: HashSet<String>,
    event_count: u64,
    pending: usize,
}

/// One relay's DM stream feeding the shared sync state.
struct DmSink {
    discovery: Arc<Discovery>,
    shared: Arc<Mutex<DmSync>>,
}

impl StreamSink for DmSink {
    fn message(&mut self, msg: StreamMessage) {
        let ctx = &self.discovery.ctx;
        match msg {
            StreamMessage::Event(event) => {
                let event_json = types::event_to_json_compact(&event);
                let other_pk = match event.kind {
                    types::KIND_DM => types::other_pubkey_in_dm(&event, &ctx.pubkey_hex),
                    types::KIND_GIFT_WRAP => match crypto::unwrap_gift_wrap(&event, &ctx.secret_hex) {
                        Ok((_seal, rumor)) => {
                            let rumor_pk = rumor.pubkey.to_lowercase();
                            if rumor_pk == ctx.pubkey_hex.to_lowercase() {
                                rumor
                                    .tags
                                    .iter()
                                    .find(|t| t.len() >= 2 && t[0] == "p")
                                    .map(|t| t[1].to_lowercase())
                            } else {
                                Some(rumor_pk)
                            }
                        }
                        Err(e) => {
                            crate::trace_log!("nostr", "list_folders: unwrap gift wrap failed: {}", e);
                            None
                        }
                    },
                    _ => None,
                };
                let mut g = self.shared.lock().unwrap();
                g.event_count += 1;
                if let Some(other) = other_pk {
                    let _ = cache::append_raw_event(&ctx.config_dir, &ctx.pubkey_hex, &other, &event_json);
                    if g.seen_pubkeys.insert(other.clone()) {
                        crate::trace_log!("nostr", "list_folders: new conversation partner (cached): {}", other);
                    }
                }
            }
            StreamMessage::Eose => {
                crate::trace_log!("nostr", "list_folders: EOSE received");
            }
            StreamMessage::Notice(n) => {
                crate::trace_log!("nostr", "list_folders: NOTICE: {}", n);
            }
            StreamMessage::AuthRequired(url) => {
                crate::trace_log!(
                    "nostr",
                    "list_folders: relay {} removed (auth-required during DM sync)",
                    url
                );
            }
        }
    }

    fn finished(&mut self) {
        let last = {
            let mut g = self.shared.lock().unwrap();
            g.pending -= 1;
            g.pending == 0
        };
        if last {
            self.discovery.sync_done(&self.shared);
        }
    }
}

/// Hears a list of DM relay URLs (empty when none was found).
type OnRelays = Box<dyn FnOnce(Vec<String>) + Send>;

/// Query relays for a recipient's kind 10050 DM relay list: the first
/// non-empty list wins, or none once every relay has answered.
fn query_dm_relay_list(our_relays: &[String], recipient_pubkey: &str, secret_key: Option<String>, done: OnRelays) {
    if our_relays.is_empty() {
        return done(Vec::new());
    }
    let filter = types::filter_dm_relay_list_by_author(recipient_pubkey);
    let shared = Arc::new(Mutex::new(DmRelayQuery { pending: our_relays.len(), done: Some(done) }));
    for relay_url in our_relays {
        relay::run_relay_feed_stream(
            relay_url.clone(),
            filter.clone(),
            10,
            secret_key.clone(),
            Box::new(DmRelayQuerySink { shared: Arc::clone(&shared) }),
        );
    }
}

struct DmRelayQuery {
    pending: usize,
    done: Option<OnRelays>,
}

struct DmRelayQuerySink {
    shared: Arc<Mutex<DmRelayQuery>>,
}

impl StreamSink for DmRelayQuerySink {
    fn message(&mut self, msg: StreamMessage) {
        if let StreamMessage::Event(event) = msg {
            if event.kind == types::KIND_DM_RELAY_LIST {
                if let Ok(urls) = types::parse_dm_relay_list(&event) {
                    if !urls.is_empty() {
                        let done = self.shared.lock().unwrap().done.take();
                        if let Some(done) = done {
                            done(urls);
                        }
                    }
                }
            }
        }
    }

    fn finished(&mut self) {
        let done = {
            let mut g = self.shared.lock().unwrap();
            g.pending -= 1;
            if g.pending == 0 {
                g.done.take()
            } else {
                None
            }
        };
        if let Some(done) = done {
            done(Vec::new());
        }
    }
}

/// Publish `event_json` to the relays in turn until one accepts it.
fn publish_to_first(mut relays: VecDeque<String>, event_json: Arc<String>, last_err: Option<String>, done: RelayDone<()>) {
    let Some(relay_url) = relays.pop_front() else {
        return done(Err(last_err.unwrap_or_else(|| String::from("No relays configured"))));
    };
    let json = Arc::clone(&event_json);
    relay::publish_event(
        &relay_url,
        &json,
        Box::new(move |r| match r {
            Ok(()) => done(Ok(())),
            Err(e) => publish_to_first(relays, event_json, Some(e), done),
        }),
    );
}

/// Publish `event_json` to every relay in turn; `then` hears whether any accepted it.
fn publish_to_all(mut relays: VecDeque<String>, event_json: Arc<String>, published: bool, then: Box<dyn FnOnce(bool) + Send>) {
    let Some(relay_url) = relays.pop_front() else {
        return then(published);
    };
    let json = Arc::clone(&event_json);
    relay::publish_event(
        &relay_url,
        &json,
        Box::new(move |r| publish_to_all(relays, event_json, published || r.is_ok(), then)),
    );
}

/// Send a NIP-04 kind 4 DM to the first relay that takes it.
fn send_nip04(secret_hex: &str, recipient_pubkey: &str, plaintext: &str, relays: Vec<String>, _config_dir: &str, done: RelayDone<()>) {
    let event = match crypto::create_signed_dm(recipient_pubkey, plaintext, secret_hex) {
        Ok(e) => e,
        Err(e) => return done(Err(e)),
    };
    let event_json = Arc::new(types::event_to_json_compact(&event));
    publish_to_first(relays.into(), event_json, None, done)
}

/// Send a NIP-17 gift-wrapped DM: the recipient's wrap to their DM relays
/// and ours, then our own copy to our relays.
fn send_nip17(
    secret_hex: &str,
    recipient_pubkey: &str,
    plaintext: &str,
    our_relays: Vec<String>,
    dm_relays: Vec<String>,
    _config_dir: &str,
    done: RelayDone<()>,
) {
    let (wrap_for_recipient, wrap_for_self) = match crypto::create_nip17_dm(plaintext, secret_hex, recipient_pubkey) {
        Ok(w) => w,
        Err(e) => return done(Err(e)),
    };
    let recipient_json = Arc::new(types::event_to_json_compact(&wrap_for_recipient));
    let self_json = Arc::new(types::event_to_json_compact(&wrap_for_self));
    let targets: VecDeque<String> = dm_relays.into_iter().chain(our_relays.iter().cloned()).collect();
    publish_to_all(
        targets,
        recipient_json,
        false,
        Box::new(move |published| {
            if !published {
                return done(Err(String::from("Failed to publish gift wrap to any relay")));
            }
            publish_to_all(our_relays.into(), self_json, false, Box::new(move |_| done(Ok(()))))
        }),
    );
}

#[cfg(test)]
mod relay_list_tests {
    use super::*;

    #[test]
    fn profile_lookups_add_the_indexer_relays_once() {
        let r = with_profile_indexers(vec!["wss://relay.damus.io".to_string(), "wss://PurplePag.es/".to_string()]);
        assert_eq!(r, vec!["wss://relay.damus.io".to_string(), "wss://PurplePag.es/".to_string()]);
        let r = with_profile_indexers(vec!["wss://relay.damus.io".to_string()]);
        assert!(r.iter().any(|u| u == "wss://purplepag.es"), "{r:?}");
        assert_eq!(r.len(), 1 + PROFILE_INDEXER_RELAYS.len());
    }
}
