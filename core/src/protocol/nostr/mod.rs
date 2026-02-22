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

pub use relay::{parse_relay_message, run_relay_feed_stream, run_relay_dm_stream,
                run_relay_dm_stream_nip17, publish_event, RelayMessage, StreamMessage};
pub use types::{event_to_json, event_to_json_compact, filter_dms_received, filter_dms_sent,
                filter_gift_wraps_received, filter_dm_relay_list_by_author, filter_to_json,
                other_pubkey_in_dm, parse_event, parse_dm_relay_list,
                Event, Filter, KIND_DM, KIND_SEAL, KIND_CHAT_MESSAGE, KIND_GIFT_WRAP, KIND_DM_RELAY_LIST,
                KIND_HTTP_AUTH, KIND_BLOSSOM_AUTH};
pub use crypto::{get_public_key_from_secret, create_signed_dm, create_nip17_dm, unwrap_gift_wrap,
                 nip04_decrypt, nip04_encrypt, nip44_conversation_key, sign_event, compute_event_id,
                 verify_event_signature, generate_keypair,
                 sha256_hex, create_blossom_auth_event, create_nip98_auth_event, nostr_auth_header};
pub use keys::{secret_key_to_hex, public_key_to_hex, hex_to_npub, hex_to_nsec,
               nsec_to_hex, npub_to_hex, is_nsec, is_npub, is_valid_hex_key};
pub use types::{ProfileMetadata, parse_profile, filter_profile_by_author,
                filter_relay_list_by_author, parse_relay_list, KIND_METADATA, KIND_RELAY_LIST,
                KIND_CONTACTS, filter_contacts_by_author, parse_contacts_relay_list};
pub use relay::{fetch_notes_from_relay, fetch_profile_from_relay, fetch_profile_from_relays,
                fetch_relay_list_from_relay, fetch_relay_list_from_relays,
                fetch_contacts_relay_list_from_relays};

pub const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
    "wss://nostr.land",
    "wss://relay.primal.net",
];

use crate::message_id::MessageId;
use crate::store::{ConversationSummary, Envelope, Address, DateTime};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::{SendPayload, Transport, TransportKind};
use std::collections::HashSet;
use std::ops::Range;
use std::sync::{Arc, RwLock};

/// Nostr store: identity (pubkey) + relay list. list_folders = one folder per DM contact.
/// The nsec is loaded from the credential store via `set_credential`.
pub struct NostrStore {
    uri: String,
    pub pubkey_hex: String,
    relays: Vec<String>,
    secret_key_hex: Arc<RwLock<Option<String>>>,
    config_dir: Option<String>,
    runtime_handle: tokio::runtime::Handle,
}

impl NostrStore {
    pub fn new(
        relays: Vec<String>,
        pubkey_hex: String,
        config_dir: Option<String>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Result<Self, StoreError> {
        let uri = crate::uri::nostr_store_uri(&pubkey_hex);
        Ok(Self {
            uri,
            pubkey_hex,
            relays,
            secret_key_hex: Arc::new(RwLock::new(None)),
            config_dir,
            runtime_handle,
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn get_secret(&self) -> Result<String, StoreError> {
        self.secret_key_hex
            .read()
            .map_err(|e| StoreError::new(e.to_string()))?
            .clone()
            .ok_or_else(|| StoreError::NeedsCredential {
                username: self.pubkey_hex.clone(),
                is_plaintext: false,
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
        let secret_hex = match self.get_secret() {
            Ok(s) => {
                eprintln!("[nostr] list_folders: secret key available");
                s
            }
            Err(e) => {
                eprintln!("[nostr] list_folders: no secret key: {}", e);
                on_complete(Err(e));
                return;
            }
        };
        let config_dir = match self.get_config_dir() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[nostr] list_folders: no config dir: {}", e);
                on_complete(Err(e));
                return;
            }
        };
        let pubkey_hex = self.pubkey_hex.clone();
        let bootstrap_relays = self.relays.clone();
        eprintln!("[nostr] list_folders: pubkey={}, bootstrap_relays={:?}, config_dir={}", pubkey_hex, bootstrap_relays, config_dir);

        self.runtime_handle.spawn(async move {
            if let Err(e) = cache::ensure_cache_dir(&config_dir, &pubkey_hex) {
                eprintln!("[nostr] list_folders: cache dir error: {}", e);
                on_complete(Err(StoreError::new(format!("Cache dir: {}", e))));
                return;
            }

            let mut seen_pubkeys: HashSet<String> = HashSet::new();

            // First emit folders from local cache
            match cache::list_conversations_with_timestamps(&config_dir, &pubkey_hex) {
                Ok(convos) => {
                    eprintln!("[nostr] list_folders: {} cached conversations", convos.len());
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
                    eprintln!("[nostr] list_folders: cache list error: {}", e);
                }
            }

            // Step 1: Discover the user's actual relays.
            // Combine bootstrap + defaults for maximum coverage during discovery.
            let mut discovery_relays: Vec<String> = Vec::new();
            let mut discovery_seen: HashSet<String> = HashSet::new();
            for url in bootstrap_relays.iter().map(|s| s.as_str()).chain(DEFAULT_RELAYS.iter().copied()) {
                let normalized = url.trim_end_matches('/').to_lowercase();
                if discovery_seen.insert(normalized) {
                    discovery_relays.push(url.to_string());
                }
            }

            // Track relays that reject NIP-42 auth so we stop contacting them.
            let mut dead_relays: HashSet<String> = HashSet::new();
            let sk = Some(secret_hex.clone());

            // Try kind 10002 (NIP-65 relay list metadata) first
            eprintln!("[nostr] list_folders: fetching kind 10002 relay list from {} relays", discovery_relays.len());
            let (relay_list_result, auth_failed) = relay::fetch_relay_list_from_relays(
                &discovery_relays, &pubkey_hex, 8, sk.clone(),
            ).await;
            dead_relays.extend(auth_failed);
            let mut discovered_relays = relay_list_result.unwrap_or_default();

            // Fall back to kind 3 contacts event (older clients store relays in content)
            if discovered_relays.is_empty() {
                eprintln!("[nostr] list_folders: no kind 10002 found, trying kind 3 contacts...");
                let alive: Vec<String> = discovery_relays.iter()
                    .filter(|r| !dead_relays.contains(r.as_str()))
                    .cloned().collect();
                let (contacts_result, auth_failed) = relay::fetch_contacts_relay_list_from_relays(
                    &alive, &pubkey_hex, 8, sk.clone(),
                ).await;
                dead_relays.extend(auth_failed);
                discovered_relays = contacts_result.unwrap_or_default();
            }

            // If no published relay list, verify the user exists then use bootstrap + defaults
            let relays: Vec<String> = if !discovered_relays.is_empty() {
                eprintln!("[nostr] list_folders: using {} discovered relays: {:?}", discovered_relays.len(), discovered_relays);
                discovered_relays
            } else {
                // Check if the user exists at all (kind 0 profile)
                eprintln!("[nostr] list_folders: no published relay list, checking if profile exists...");
                let alive: Vec<String> = discovery_relays.iter()
                    .filter(|r| !dead_relays.contains(r.as_str()))
                    .cloned().collect();
                let (profile_result, auth_failed) = relay::fetch_profile_from_relays(
                    &alive, &pubkey_hex, 8, sk.clone(),
                ).await;
                dead_relays.extend(auth_failed);
                let profile = profile_result.unwrap_or(None);
                if profile.is_none() {
                    let alive_count = discovery_relays.len() - dead_relays.len();
                    let msg = format!(
                        "Could not find profile for this Nostr identity on any relay. \
                         Tried {} relays ({} required auth, {} actually queried). \
                         Check that the secret key is correct and that \
                         the account has been published to at least one relay.",
                        discovery_relays.len(), dead_relays.len(), alive_count
                    );
                    eprintln!("[nostr] list_folders: {}", msg);
                    on_complete(Err(StoreError::new(msg)));
                    return;
                }
                eprintln!("[nostr] list_folders: profile found but no published relay list, using bootstrap + defaults");
                discovery_relays.iter()
                    .filter(|r| !dead_relays.contains(r.as_str()))
                    .cloned().collect()
            };

            if !dead_relays.is_empty() {
                eprintln!("[nostr] list_folders: {} relays removed (auth-required): {:?}", dead_relays.len(), dead_relays);
            }

            // Step 2: Subscribe to DMs on the full relay set (excluding dead relays)
            let relays: Vec<String> = relays.into_iter()
                .filter(|r| !dead_relays.contains(r.as_str()))
                .collect();
            eprintln!("[nostr] list_folders: starting DM sync with {} relays: {:?}", relays.len(), relays);
            let filter_recv = types::filter_dms_received(&pubkey_hex, 500, None);
            let filter_sent = types::filter_dms_sent(&pubkey_hex, 500, None);
            let filter_gw = types::filter_gift_wraps_received(&pubkey_hex, 500, None);

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            for relay_url in &relays {
                eprintln!("[nostr] list_folders: spawning DM stream for {}", relay_url);
                let url = relay_url.clone();
                let fr = filter_recv.clone();
                let fs = filter_sent.clone();
                let fg = filter_gw.clone();
                let tx_clone = tx.clone();
                let sk = sk.clone();
                tokio::spawn(async move {
                    relay::run_relay_dm_stream_nip17(url, fr, fs, fg, true, tx_clone, sk).await;
                });
            }
            drop(tx);

            let mut event_count = 0u64;
            while let Some(msg) = rx.recv().await {
                match msg {
                    StreamMessage::Event(event) => {
                        event_count += 1;
                        let event_json = types::event_to_json_compact(&event);

                        let other_pk = match event.kind {
                            types::KIND_DM => {
                                types::other_pubkey_in_dm(&event, &pubkey_hex)
                            }
                            types::KIND_GIFT_WRAP => {
                                match crypto::unwrap_gift_wrap(&event, &secret_hex) {
                                    Ok((_seal, rumor)) => {
                                        let rumor_pk = rumor.pubkey.to_lowercase();
                                        if rumor_pk == pubkey_hex.to_lowercase() {
                                            rumor.tags.iter()
                                                .find(|t| t.len() >= 2 && t[0] == "p")
                                                .map(|t| t[1].to_lowercase())
                                        } else {
                                            Some(rumor_pk)
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[nostr] list_folders: unwrap gift wrap failed: {}", e);
                                        None
                                    }
                                }
                            }
                            _ => None,
                        };

                        if let Some(other) = other_pk {
                            let _ = cache::append_raw_event(
                                &config_dir, &pubkey_hex, &other, &event_json,
                            );
                            if seen_pubkeys.insert(other.clone()) {
                                eprintln!("[nostr] list_folders: new conversation partner: {}", other);
                                on_folder(FolderInfo {
                                    name: other,
                                    delimiter: None,
                                    attributes: Vec::new(),
                                });
                            }
                        }
                    }
                    StreamMessage::Eose => {
                        eprintln!("[nostr] list_folders: EOSE received");
                    }
                    StreamMessage::Notice(n) => {
                        eprintln!("[nostr] list_folders: NOTICE: {}", n);
                    }
                    StreamMessage::AuthRequired(url) => {
                        eprintln!("[nostr] list_folders: relay {} removed (auth-required during DM sync)", url);
                    }
                }
            }

            eprintln!("[nostr] list_folders: relay sync complete, {} events received, {} total conversations", event_count, seen_pubkeys.len());
            on_complete(Ok(()));
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

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
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
                on_complete(Err(StoreError::new(format!("Message {} not found", event_id))));
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
    runtime_handle: tokio::runtime::Handle,
}

impl NostrTransport {
    pub fn new(
        relays: Vec<String>,
        pubkey_hex: String,
        config_dir: Option<String>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Result<Self, StoreError> {
        let uri = crate::uri::nostr_transport_uri(&pubkey_hex);
        Ok(Self {
            uri,
            pubkey_hex,
            relays,
            secret_key_hex: Arc::new(RwLock::new(None)),
            config_dir,
            runtime_handle,
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
                    on_complete(Err(StoreError::new(format!("Invalid recipient pubkey: {}", e))));
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
        let pubkey_hex = self.pubkey_hex.clone();

        self.runtime_handle.spawn(async move {
            // Try to discover recipient's NIP-17 DM relay list (kind 10050)
            let dm_relays = query_dm_relay_list(&relays, &recipient_pubkey, Some(secret_hex.clone())).await;

            let result = if !dm_relays.is_empty() {
                // NIP-17: send via gift wrap
                send_nip17(&secret_hex, &pubkey_hex, &recipient_pubkey, &plaintext,
                           &relays, &dm_relays, &config_dir).await
            } else {
                // NIP-04: send kind 4
                send_nip04(&secret_hex, &pubkey_hex, &recipient_pubkey, &plaintext,
                           &relays, &config_dir).await
            };

            on_complete(result.map_err(StoreError::new));
        });
    }
}

/// Query relays for a recipient's kind 10050 DM relay list. Returns the relay URLs or empty vec.
async fn query_dm_relay_list(our_relays: &[String], recipient_pubkey: &str, secret_key: Option<String>) -> Vec<String> {
    let filter = types::filter_dm_relay_list_by_author(recipient_pubkey);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    for relay_url in our_relays {
        let url = relay_url.clone();
        let f = filter.clone();
        let tx_clone = tx.clone();
        let sk = secret_key.clone();
        tokio::spawn(async move {
            relay::run_relay_feed_stream(url, f, 10, tx_clone, sk).await;
        });
    }
    drop(tx);

    while let Some(msg) = rx.recv().await {
        if let StreamMessage::Event(event) = msg {
            if event.kind == types::KIND_DM_RELAY_LIST {
                if let Ok(urls) = types::parse_dm_relay_list(&event) {
                    if !urls.is_empty() {
                        return urls;
                    }
                }
            }
        }
    }

    Vec::new()
}

/// Send a NIP-04 kind 4 DM to relays.
async fn send_nip04(
    secret_hex: &str,
    _our_pubkey: &str,
    recipient_pubkey: &str,
    plaintext: &str,
    relays: &[String],
    _config_dir: &str,
) -> Result<(), String> {
    let event = crypto::create_signed_dm(recipient_pubkey, plaintext, secret_hex)?;
    let event_json = types::event_to_json_compact(&event);

    let mut last_err = None;
    for relay_url in relays {
        match relay::publish_event(relay_url, &event_json).await {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }

    Err(last_err.unwrap_or_else(|| String::from("No relays configured")))
}

/// Send a NIP-17 gift-wrapped DM to relays.
async fn send_nip17(
    secret_hex: &str,
    _our_pubkey: &str,
    recipient_pubkey: &str,
    plaintext: &str,
    our_relays: &[String],
    dm_relays: &[String],
    _config_dir: &str,
) -> Result<(), String> {
    let (wrap_for_recipient, wrap_for_self) =
        crypto::create_nip17_dm(plaintext, secret_hex, recipient_pubkey)?;

    let recipient_json = types::event_to_json_compact(&wrap_for_recipient);
    let self_json = types::event_to_json_compact(&wrap_for_self);

    // Publish recipient's gift wrap to their DM relays
    let mut published = false;
    for relay_url in dm_relays {
        if relay::publish_event(relay_url, &recipient_json).await.is_ok() {
            published = true;
        }
    }
    // Also publish to our relays as fallback
    for relay_url in our_relays {
        if relay::publish_event(relay_url, &recipient_json).await.is_ok() {
            published = true;
        }
    }

    if !published {
        return Err(String::from("Failed to publish gift wrap to any relay"));
    }

    // Publish self-copy to our relays
    for relay_url in our_relays {
        let _ = relay::publish_event(relay_url, &self_json).await;
    }

    Ok(())
}
