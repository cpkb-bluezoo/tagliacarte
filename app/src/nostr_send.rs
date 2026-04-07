/*
 * nostr_send.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
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
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Send Nostr direct messages via [`NostrTransport`], paired with the session [`NostrStore`].
//!
//! **Stores** load and list messages through the folder abstraction; **transports** send.
//! The Nostr transport shares relays, keys, and cache paths with its store—callers only need
//! this module for outbound DMs. Routing by account type lives in [`crate::session`].

use std::sync::mpsc;

use tagliacarte_core::protocol::nostr::NostrStore;
use tagliacarte_core::store::{Address, SendPayload, StoreError, Transport};

use crate::frb_api::FrbAccount;
use crate::mail_kind::is_nostr_store;
use crate::mail_store::open_cached_store;

/// Send a plaintext direct message to the peer identified by `recipient_pubkey` (folder name in the UI).
///
/// Uses the cached [`NostrStore`] so [`NostrTransport`] shares the same identity and relay set.
pub fn send_nostr_direct_message(
    acc: &FrbAccount,
    recipient_pubkey: &str,
    text: &str,
    use_keychain: bool,
) -> Result<(), String> {
    if !is_nostr_store(acc.backend_type.as_str()) {
        return Err("send_nostr_direct_message: not a Nostr account".to_owned());
    }
    let store = open_cached_store(acc, use_keychain)?;
    let nostr: &NostrStore = store.as_any().downcast_ref::<NostrStore>().ok_or_else(|| {
        "send_nostr_direct_message: store is not NostrStore (internal error)".to_owned()
    })?;
    let transport = nostr.paired_transport().map_err(|e| e.to_string())?;
    let mut payload = SendPayload::default();
    payload.to.push(Address {
        display_name: None,
        local_part: recipient_pubkey.trim().to_string(),
        domain: None,
    });
    payload.body_plain = Some(text.to_string());
    let (tx, rx) = mpsc::channel::<Result<(), StoreError>>();
    transport.send(
        &payload,
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    rx.recv()
        .map_err(|_| "Nostr send: internal channel closed".to_string())?
        .map_err(|e| e.to_string())
}
