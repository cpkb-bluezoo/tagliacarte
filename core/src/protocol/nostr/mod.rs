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

//! Nostr backend (Store, Folder, Transport). Folder = one DM conversation (one contact).
//! Connection reuse for relays; semantic send (SendPayload → kind-4); event-driven folder/message events.
//! MessageIds: nostr:nevent:..., nostr:dm:... (ARCHITECTURE §7). Keys from file/env only; no logging of keys.
//!
//! Relay connection: our WebSocket client + our JSON push parser. Each WSS text frame is parsed
//! and we emit StreamMessage in real time.
//!
//! All trait methods are callback-driven and return immediately.

mod relay;
mod types;

pub use relay::{parse_relay_message, run_relay_feed_stream, run_relay_dm_stream, RelayMessage, StreamMessage};
pub use types::{event_to_json, filter_dms_received, filter_dms_sent, filter_to_json, Event, Filter, KIND_DM};

use crate::message_id::MessageId;
use crate::store::{ConversationSummary, Envelope};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::{SendPayload, Transport, TransportKind};
use std::ops::Range;

/// Nostr store: identity (key) + relay list. list_folders = one folder per DM contact.
pub struct NostrStore {
    /// Store URI (e.g. nostr:store:<pubkey_hex>).
    uri: String,
    /// Relay URLs (e.g. wss://relay.damus.io). Used when connecting for list_folders / open_folder.
    _relays: Vec<String>,
    /// Path to secret key file (never logged).
    _key_path: Option<String>,
}

impl NostrStore {
    /// Create a new Nostr store. Keys loaded from key_path or env; relays used for connection.
    /// Returns error if key_path is missing and env not set (stub: always succeeds with empty relays).
    pub fn new(relays: Vec<String>, key_path: Option<String>) -> Result<Self, StoreError> {
        Ok(Self {
            uri: crate::uri::nostr_store_uri(
                key_path.as_deref().unwrap_or("default"),
            ),
            _relays: relays,
            _key_path: key_path,
        })
    }

    /// Store URI for registry and FFI.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

impl Store for NostrStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Nostr
    }

    fn list_folders(
        &self,
        _on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // TODO: connect to relays, query kind-4 DMs, collect unique pubkeys, call on_folder per contact
        on_complete(Ok(()));
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        // name = other pubkey or dm id (nostr:dm:...)
        // TODO: return NostrFolder for this DM conversation
        let _ = name;
        on_complete(Err(StoreError::new("Nostr open_folder not yet implemented")));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        None
    }
}

/// Folder = one DM conversation with a contact. Messages = kind-4 events.
#[allow(dead_code)]
struct NostrFolder {
    /// Our pubkey (from store).
    _our_pubkey: String,
    /// Other contact pubkey.
    _other_pubkey: String,
    /// Folder/conversation id (e.g. nostr:dm:our:other).
    _folder_id: String,
}

impl Folder for NostrFolder {
    fn list_conversations(
        &self,
        _range: Range<u64>,
        _on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // TODO: query kind-4 with filter p=[other_pubkey], call on_summary for each
        on_complete(Ok(()));
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        on_complete(Ok(0));
    }

    fn get_message(
        &self,
        id: &MessageId,
        _on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        _on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let _ = id;
        // TODO: fetch event by id, decrypt, call on_metadata + on_content_chunk
        on_complete(Err(StoreError::new("Nostr get_message not yet implemented")));
    }
}

/// Nostr transport: send kind-4 DMs. Same identity as store.
pub struct NostrTransport {
    uri: String,
    _relays: Vec<String>,
    _key_path: Option<String>,
}

impl NostrTransport {
    pub fn new(relays: Vec<String>, key_path: Option<String>) -> Result<Self, StoreError> {
        Ok(Self {
            uri: crate::uri::nostr_transport_uri(
                key_path.as_deref().unwrap_or("default"),
            ),
            _relays: relays,
            _key_path: key_path,
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }
}

impl Transport for NostrTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Nostr
    }

    fn send(
        &self,
        _payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // TODO: build kind-4 from payload (to = one pubkey), encrypt (NIP-04/44), publish to relays
        on_complete(Err(StoreError::new("Nostr send not yet implemented")));
    }
}
