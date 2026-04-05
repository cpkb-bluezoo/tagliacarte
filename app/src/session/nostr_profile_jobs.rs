/*
 * session/nostr_profile_jobs.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Async Nostr kind-0 fetches after message windows; emits [AppEvent::NostrProfileUpdated].
 */

use std::collections::HashSet;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use tokio::sync::broadcast;

use tagliacarte_core::protocol::nostr::{fetch_profile_from_relays, hex_to_npub, keys::is_valid_hex_key};

use crate::frb_api::FrbAccount;
use crate::mail_kind::is_nostr_store;
use crate::mail_store::{mail_runtime_handle, nostr_profile_fetch_context};
use crate::nostr_profile_cache::{
    cached_profile_fields_for_emit, display_label_for_pubkey_hex, merge_negative_fetch,
    merge_profile, should_fetch_profile,
};

use super::events::AppEvent;

fn emit_profile_updated(
    tx: &broadcast::Sender<AppEvent>,
    account_id: &str,
    pubkey_hex: &str,
    display_name: Option<String>,
    nip05: Option<String>,
    picture: Option<String>,
) {
    let pk = pubkey_hex.trim().to_lowercase();
    let npub = hex_to_npub(&pk).unwrap_or_default();
    let _ = tx.send(AppEvent::NostrProfileUpdated {
        account_id: account_id.to_string(),
        pubkey_hex: pk,
        npub,
        display_name,
        nip05,
        picture,
    });
}

static IN_FLIGHT: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Fire-and-forget profile fetches for peer pubkeys seen in a message list window.
pub(super) fn schedule_nostr_profile_fetches_after_message_window(
    account_id: String,
    account: FrbAccount,
    use_keychain: bool,
    pubkeys: Vec<String>,
    event_tx: broadcast::Sender<AppEvent>,
) {
    if !is_nostr_store(account.backend_type.as_str()) {
        return;
    }
    let Ok((relays, sk)) = nostr_profile_fetch_context(&account, use_keychain) else {
        return;
    };

    let mut seen = HashSet::<String>::new();
    for raw in pubkeys {
        let pk = raw.trim().to_lowercase();
        if !is_valid_hex_key(&pk) || !seen.insert(pk.clone()) {
            continue;
        }
        if !should_fetch_profile(&pk) {
            continue;
        }
        {
            let mut g = IN_FLIGHT.lock().expect("nostr profile in-flight");
            if !g.insert(pk.clone()) {
                continue;
            }
        }

        let account_id = account_id.clone();
        let relays = relays.clone();
        let sk = sk.clone();
        let tx = event_tx.clone();
        std::thread::spawn(move || {
            let (res, _) = mail_runtime_handle().block_on(fetch_profile_from_relays(
                relays.as_slice(),
                pk.as_str(),
                12,
                sk,
            ));
            let _ = IN_FLIGHT.lock().expect("nostr profile in-flight").remove(&pk);
            match res {
                Ok(Some(meta)) => {
                    merge_profile(&pk, &meta);
                    emit_profile_updated(
                        &tx,
                        &account_id,
                        &pk,
                        meta.name.clone(),
                        meta.nip05.clone(),
                        meta.picture.clone(),
                    );
                }
                Ok(None) => {
                    merge_negative_fetch(&pk);
                    emit_profile_updated(&tx, &account_id, &pk, None, None, None);
                }
                Err(_) => {
                    // Do not negative-cache errors (network); try again next window.
                }
            }
        });
    }
}

/// Pre-fetch profiles for Nostr **folder** names (peer pubkeys) when listing folders.
pub(super) fn schedule_nostr_profile_fetches_for_folder_list(
    account_id: String,
    account: FrbAccount,
    use_keychain: bool,
    folder_names: &[String],
    event_tx: broadcast::Sender<AppEvent>,
) {
    let pks: Vec<String> = folder_names
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| is_valid_hex_key(s))
        .collect();
    if pks.is_empty() {
        return;
    }
    let aid = account_id.as_str();
    // Push cached metadata immediately so Flutter folder titles update even when
    // [should_fetch_profile] is false (fresh TTL); otherwise no event was sent.
    for pk in &pks {
        let (name, nip05, picture) = cached_profile_fields_for_emit(pk);
        if name.is_some() || nip05.is_some() || picture.is_some() {
            emit_profile_updated(&event_tx, aid, pk, name, nip05, picture);
        } else if !should_fetch_profile(pk) {
            emit_profile_updated(&event_tx, aid, pk, None, None, None);
        }
    }
    schedule_nostr_profile_fetches_after_message_window(
        account_id,
        account,
        use_keychain,
        pks,
        event_tx,
    );
}

/// Resolved label for UI (same rules as cache); used if we add Rust-side folder title rewriting later.
#[allow(dead_code)]
pub(super) fn label_for_pubkey_hex(pubkey_hex: &str) -> String {
    display_label_for_pubkey_hex(pubkey_hex)
}
