/*
 * frb_api/frb_mail.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

use std::collections::HashSet;
use std::ops::Range;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use base64::Engine;
use tagliacarte_core::json::{JsonNumber, JsonWriter, writer_into_string};
use tagliacarte_core::config::{
    CredentialEntry, load_credentials, resolve_credentials_file_path, save_credential,
    set_credentials_backend,
};
use tagliacarte_core::message_id::MessageId;
use tagliacarte_core::mime::{
    extract_structured_body, parse_envelope, utf8_body_after_rfc822_headers,
};
use tagliacarte_core::protocol::imap::ImapStore;
use tagliacarte_core::protocol::nntp::{NntpStore, NntpTransport};
use tagliacarte_core::protocol::imap::trace as imap_trace;
use tagliacarte_core::protocol::smtp::{build_rfc822_from_payload, send_message_async, SmtpClientError};
use tagliacarte_core::protocol::nostr::keys as nostr_keys;
use tagliacarte_core::protocol::nostr::{
    crypto as nostr_crypto,
    event_to_json_compact,
    fetch_dm_relay_list_from_relays,
    fetch_profile_from_relays,
    fetch_relay_list_from_relays,
    publish_event,
    Event, KIND_METADATA as NOSTR_KIND_METADATA,
};
use tagliacarte_core::sasl::SaslMechanism;
use tagliacarte_core::store::{
    sort_conversation_summaries_for_window, Address, Attachment, ConversationSummary, Envelope,
    Flag, Folder, MessageForDisplay, SendPayload, Transport,
};

use crate::mail_kind::{
    is_imap_like_store, is_nostr_store, normalize_store_type, uses_long_imap_fetch_timeout,
};
use crate::mail_store::{load_mail_credential, resolve_gmail_xoauth_secret};
use super::resolve_mail_account;
use crate::mail_store::{
    self, blocking_get_message_raw as mail_blocking_get_message_raw, blocking_imap_append,
    invalidate_mail_store_cache, list_range_for_page_backend, mail_runtime_handle,
    open_cached_store, same_mail_store as accounts_same_mail_store, wait_open_folder,
};
use super::FrbAccount;

pub(crate) fn frb_runtime_handle() -> tokio::runtime::Handle {
    mail_runtime_handle()
}

pub(crate) fn list_mail_folders_json(acc: FrbAccount, use_keychain: bool) -> Result<String, String> {
    let snap = mail_store::list_mail_folders_snapshot_with_progress(&acc, use_keychain, |_, _| {})
        .map_err(|e| {
        eprintln!("[mail] list_mail_folders: {e}");
        e
    })?;
    let folder_unread_counts: Vec<FolderUnreadCountJson> = snap
        .folders
        .iter()
        .map(|name| FolderUnreadCountJson {
            folder_name: name.clone(),
            unread: snap.unread_by_folder.get(name).copied().unwrap_or(0) as u64,
        })
        .collect();
    let payload = ListMailFoldersResponse {
        folders: snap.folders,
        hierarchy_delimiter: snap.hierarchy_delimiter,
        folder_unread_counts,
    };
    Ok(format_list_mail_folders_response(&payload))
}

struct FolderUnreadCountJson {
    folder_name: String,
    unread: u64,
}

struct ListMailFoldersResponse {
    folders: Vec<String>,
    hierarchy_delimiter: Option<String>,
    folder_unread_counts: Vec<FolderUnreadCountJson>,
}

fn format_list_mail_folders_response(r: &ListMailFoldersResponse) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("folders");
    w.write_start_array();
    for f in &r.folders {
        w.write_string(f);
    }
    w.write_end_array();
    if let Some(ref d) = r.hierarchy_delimiter {
        w.write_key("hierarchyDelimiter");
        w.write_string(d);
    }
    if !r.folder_unread_counts.is_empty() {
        w.write_key("folderUnreadCounts");
        w.write_start_array();
        for c in &r.folder_unread_counts {
            w.write_start_object();
            w.write_key("folderName");
            w.write_string(&c.folder_name);
            w.write_key("unread");
            w.write_number(u64_json(c.unread));
            w.write_end_object();
        }
        w.write_end_array();
    }
    w.write_end_object();
    writer_into_string(w)
}

pub(crate) fn create_mail_folder(
    acc: FrbAccount,
    folder_path: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let name = folder_path.trim();
    if name.is_empty() {
        return Err("empty folder path".to_owned());
    }
    let store = open_cached_store(&acc, use_keychain)?;
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    store.create_folder(
        name,
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timeout creating folder (120s)".to_owned()),
    }
}

pub(crate) fn rename_mail_folder(
    acc: FrbAccount,
    old_name: String,
    new_name: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let old_n = old_name.trim();
    let new_n = new_name.trim();
    if old_n.is_empty() || new_n.is_empty() {
        return Err("empty folder name".to_owned());
    }
    let store = open_cached_store(&acc, use_keychain)?;
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    store.rename_folder(
        old_n,
        new_n,
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timeout renaming folder (120s)".to_owned()),
    }
}

pub(crate) fn delete_mail_folder(
    acc: FrbAccount,
    folder_name: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let name = folder_name.trim();
    if name.is_empty() {
        return Err("empty folder name".to_owned());
    }
    let store = open_cached_store(&acc, use_keychain)?;
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    store.delete_folder(
        name,
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timeout deleting folder (120s)".to_owned()),
    }
}

/// Half-open range of **oldest-first** indices: `0` = oldest message, `total - 1` = newest.
fn folder_range_for_indices(total: u64, start_index: u64, limit: u64) -> Option<Range<u64>> {
    if total == 0 || limit == 0 || start_index >= total {
        return None;
    }
    let end = (start_index + limit).min(total);
    Some(start_index..end)
}

/// Paged folder listing: fetch summaries for `[start_index, start_index + limit)` in **ascending**
/// order for [message_list_sort] (UI reverses visually when the user chose descending).
pub(crate) fn list_folder_messages_window_response(
    acc: FrbAccount,
    folder_name: String,
    start_index: u64,
    limit: u64,
    message_list_sort: String,
    use_keychain: bool,
) -> Result<ListFolderMessagesWindowResponse, String> {
    set_credentials_backend(use_keychain);
    let folder_name = folder_name.trim();
    if folder_name.is_empty() {
        return Err("empty folder name".to_owned());
    }
    let limit = limit.max(1).min(10_000);
    let sort_eff = {
        let t = message_list_sort.trim();
        if t.is_empty() {
            "date_desc"
        } else {
            t
        }
    };
    let is_imap_fast = is_imap_like_store(acc.backend_type.as_str());
    let store = open_cached_store(&acc, use_keychain)?;
    let backend = acc.backend_type.as_str();

    if is_imap_fast {
        if let Some(imap) = store.as_ref().as_any().downcast_ref::<ImapStore>() {
            let (total, si, summaries, strat) = imap
                .list_folder_messages_window_blocking(folder_name, start_index, limit, sort_eff)
                .map_err(|e| e.to_string())?;
            let messages: Vec<MessageSummaryJson> = summaries
                .into_iter()
                .map(|s| conversation_to_message_summary(backend, s))
                .collect();
            return Ok(ListFolderMessagesWindowResponse {
                total,
                start_index: si,
                messages,
                list_strategy: strat.to_string(),
            });
        }
    }

    let folder = wait_open_folder(store, folder_name)?;
    let total = wait_message_count(folder.as_ref())?;

    let Some(_range) = folder_range_for_indices(total, start_index, limit) else {
        return Ok(ListFolderMessagesWindowResponse {
            total,
            start_index,
            messages: vec![],
            list_strategy: "fullScan".to_owned(),
        });
    };

    let collected = Arc::new(Mutex::new(Vec::<ConversationSummary>::new()));
    let c2 = Arc::clone(&collected);
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);

    folder.list_conversations(
        0..total,
        Box::new(move |s| {
            c2.lock().expect("summary lock").push(s);
        }),
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );

    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!(
                "[mail] list_folder_messages_window non-IMAP folder={folder_name:?}: {e}"
            );
            return Err(e);
        }
        Err(_) => {
            eprintln!(
                "[mail] list_folder_messages_window non-IMAP folder={folder_name:?}: timeout listing messages (120s)"
            );
            return Err("timeout listing messages (120s)".to_owned());
        }
    }

    let mut all = std::mem::take(&mut *collected.lock().expect("summary lock"));
    sort_conversation_summaries_for_window(&mut all, sort_eff);
    let slice_end = (start_index + limit).min(total) as usize;
    let messages: Vec<MessageSummaryJson> = all[start_index as usize..slice_end]
        .iter()
        .cloned()
        .map(|s| conversation_to_message_summary(backend, s))
        .collect();
    Ok(ListFolderMessagesWindowResponse {
        total,
        start_index,
        messages,
        list_strategy: "fullScan".to_owned(),
    })
}

pub(crate) fn list_folder_messages_window_json(
    acc: FrbAccount,
    folder_name: String,
    start_index: u64,
    limit: u64,
    message_list_sort: String,
    use_keychain: bool,
) -> Result<String, String> {
    let r = list_folder_messages_window_response(
        acc,
        folder_name,
        start_index,
        limit,
        message_list_sort,
        use_keychain,
    )?;
    Ok(format_list_folder_messages_window_response(&r))
}

pub(crate) fn list_folder_messages_json(
    acc: FrbAccount,
    folder_name: String,
    skip: u64,
    limit: u64,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let folder_name = folder_name.trim();
    if folder_name.is_empty() {
        return Err("empty folder name".to_owned());
    }
    let backend = acc.backend_type.clone();
    let store = open_cached_store(&acc, use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;
    let total = wait_message_count(folder.as_ref())?;
    let Some(range) = list_range_for_page_backend(backend.as_str(), total, skip, limit) else {
        return Ok("[]".to_owned());
    };

    let rows = Arc::new(Mutex::new(Vec::<MessageSummaryJson>::new()));
    let r2 = Arc::clone(&rows);
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);

    folder.list_conversations(
        range,
        Box::new(move |s| {
            r2.lock()
                .expect("summary lock")
                .push(conversation_to_message_summary(backend.as_str(), s));
        }),
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );

    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err("timeout listing messages (120s)".to_owned()),
    }

    let mut out: Vec<MessageSummaryJson> = std::mem::take(&mut *rows.lock().expect("summary lock"));
    out.reverse();
    Ok(format_message_summary_array(&out))
}

pub(crate) fn get_folder_message_json(
    acc: FrbAccount,
    folder_name: String,
    message_id: String,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let folder_name = folder_name.trim();
    let message_id = message_id.trim();
    if folder_name.is_empty() || message_id.is_empty() {
        return Err("empty folder name or message id".to_owned());
    }
    let is_imap = uses_long_imap_fetch_timeout(acc.backend_type.as_str());
    let load_secs = if is_imap { 300u64 } else { 120u64 };
    let is_nostr = is_nostr_store(acc.backend_type.as_str());
    let store = open_cached_store(&acc, use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;

    let (tx, rx) = mpsc::sync_channel::<Result<MessageForDisplay, String>>(1);
    let mid = message_id.to_owned();
    folder.get_message_display(
        &MessageId::new(mid),
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );

    match rx.recv_timeout(Duration::from_secs(load_secs)) {
        Ok(Ok(display)) => {
            if imap_trace::mail_body_debug_enabled() {
                eprintln!(
                    "[mail body trace] get_folder_message_json: display path attachments={}",
                    display.attachments.len(),
                );
            }
            Ok(format_message_detail(&detail_from_display(is_nostr, display)))
        }
        Ok(Err(e)) if e.contains("get_message_display not supported") => {
            get_folder_message_json_full_raw(&*folder, message_id, load_secs, is_nostr)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(format!("timeout loading message ({load_secs}s)")),
    }
}

/// Sets `\Seen` on the message (IMAP UID STORE, Maildir rename, Graph PATCH, …).
pub(crate) fn mark_folder_message_read(
    acc: FrbAccount,
    folder_name: String,
    message_id: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let folder_name = folder_name.trim();
    let message_id = message_id.trim();
    if folder_name.is_empty() || message_id.is_empty() {
        return Err("empty folder name or message id".to_owned());
    }
    let store = open_cached_store(&acc, use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;
    let mid = message_id.to_owned();
    let ids = [mid.as_str()];
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    folder.store_flags(
        &ids,
        &[Flag::Seen],
        &[],
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timeout marking message read (120s)".to_owned()),
    }
}

/// Fetch one IMAP `BODY.PEEK[section]` part (decoded using `transfer_encoding` from `BODYSTRUCTURE`).
pub(crate) fn fetch_folder_message_part_json(
    acc: FrbAccount,
    folder_name: String,
    message_id: String,
    imap_section: String,
    transfer_encoding: String,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let folder_name = folder_name.trim();
    let message_id = message_id.trim();
    let imap_section = imap_section.trim();
    if folder_name.is_empty() || message_id.is_empty() || imap_section.is_empty() {
        return Err("empty folder, message id, or section".to_owned());
    }
    let store = open_cached_store(&acc, use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;

    let (tx, rx) = mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
    let mid = message_id.to_owned();
    let enc = transfer_encoding.trim().to_string();
    folder.fetch_message_part(
        &MessageId::new(mid),
        imap_section,
        enc.as_str(),
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );

    let part_secs = if uses_long_imap_fetch_timeout(acc.backend_type.as_str()) {
        300u64
    } else {
        120u64
    };
    match rx.recv_timeout(Duration::from_secs(part_secs)) {
        Ok(Ok(bytes)) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok(format_bytes_base64(&b64))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(format!("timeout fetching attachment ({part_secs}s)")),
    }
}

fn get_folder_message_json_full_raw(
    folder: &dyn Folder,
    message_id: &str,
    timeout_secs: u64,
    is_nostr: bool,
) -> Result<String, String> {
    let meta_slot: Arc<Mutex<Option<Envelope>>> = Arc::new(Mutex::new(None));
    let raw_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let m2 = Arc::clone(&meta_slot);
    let b2 = Arc::clone(&raw_buf);
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);

    folder.get_message(
        &MessageId::new(message_id.to_owned()),
        Box::new(move |e| {
            *m2.lock().expect("meta lock") = Some(e);
        }),
        Box::new(move |chunk| {
            b2.lock().expect("raw lock").extend_from_slice(chunk);
        }),
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );

    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(format!("timeout loading message ({timeout_secs}s)")),
    }

    let env = meta_slot
        .lock()
        .expect("meta lock")
        .take()
        .unwrap_or_default();
    let raw = std::mem::take(&mut *raw_buf.lock().expect("raw lock"));

    let (plain, html, _) = match extract_structured_body(&raw) {
        Ok(parts) => parts,
        Err(e) => {
            if imap_trace::mail_body_debug_enabled() {
                eprintln!(
                    "[mail body trace] extract_structured_body failed ({} bytes): {e}",
                    raw.len()
                );
            }
            (None, None, vec![])
        }
    };
    let body_plain = if plain.is_none() && html.is_none() {
        let after_headers = utf8_body_after_rfc822_headers(&raw);
        let rfc822_header_strip_used = after_headers.is_some();
        let fb = after_headers.or_else(|| {
            if raw.len() < 2_000_000 {
                Some(String::from_utf8_lossy(&raw).into_owned())
            } else {
                None
            }
        });
        if imap_trace::mail_body_debug_enabled() {
            let no_display = fb.is_none() || fb.as_ref().is_some_and(|s| s.trim().is_empty());
            eprintln!(
                "[mail body trace] get_folder_message_json: raw_len={} structured_plain={} structured_html={} rfc822_header_strip_used={} full_raw_fallback_used={} no_displayable_text={}",
                raw.len(),
                false,
                false,
                rfc822_header_strip_used,
                fb.is_some() && !rfc822_header_strip_used,
                no_display,
            );
        }
        fb
    } else {
        if imap_trace::mail_body_debug_enabled() {
            eprintln!(
                "[mail body trace] get_folder_message_json: raw_len={} plain_len={:?} html_len={:?}",
                raw.len(),
                plain.as_ref().map(|s| s.len()),
                html.as_ref().map(|s| s.len()),
            );
        }
        plain
    };

    let detail = detail_from_env_and_body(is_nostr, &env, body_plain, html);
    Ok(format_message_detail(&detail))
}

fn load_credential_entry(
    credential_lookup_key: &str,
    use_keychain: bool,
) -> Result<CredentialEntry, String> {
    let cred_path = resolve_credentials_file_path().ok_or_else(|| {
        "could not resolve credentials path".to_owned()
    })?;
    let creds = load_credentials(
        &cred_path,
        if use_keychain {
            Some(credential_lookup_key)
        } else {
            None
        },
    )
    .map_err(|e| format!("credentials: {e}"))?;
    creds
        .get(credential_lookup_key)
        .cloned()
        .ok_or_else(|| {
            format!("no saved credential for this account ({credential_lookup_key})")
        })
}

/// Key in [FrbAccount::attrs]: `created_at` of the last kind 0 profile we merged from relays (unix secs).
const NOSTR_KIND0_CREATED_AT_ATTR: &str = "nostrKind0CreatedAt";

fn nostr_relay_url_key(url: &str) -> String {
    url.trim().trim_end_matches('/').to_lowercase()
}

/// Union relay URLs: keep order (`base`, then `nip65`, then `dm10050`), dedupe by normalized URL.
fn merge_nostr_relay_lists(base: &[String], nip65: &[String], dm10050: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for s in base.iter().chain(nip65.iter()).chain(dm10050.iter()) {
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        let k = nostr_relay_url_key(t);
        if seen.insert(k) {
            out.push(t.to_string());
        }
    }
    out
}

fn nostr_relay_sets_differ(a: &[String], b: &[String]) -> bool {
    let sa: HashSet<String> = a.iter().map(|s| nostr_relay_url_key(s)).collect();
    let sb: HashSet<String> = b.iter().map(|s| nostr_relay_url_key(s)).collect();
    sa != sb
}

/// Fetch kind 0, NIP-65 (10002), and NIP-17 DM relay list (10050); merge profile and union relay URLs into config.
pub(crate) fn nostr_sync_remote_profile_and_relays(
    config_path: &str,
    account_id: &str,
) -> Result<(), String> {
    let mut cfg = super::load_frb_config_struct(config_path);
    let idx = cfg
        .accounts
        .iter()
        .position(|a| a.id == account_id)
        .ok_or_else(|| "account not found".to_string())?;
    if !cfg.accounts[idx]
        .backend_type
        .eq_ignore_ascii_case("nostr")
    {
        return Ok(());
    }

    let use_keychain = cfg.use_keychain;

    let relays: Vec<String> = cfg.accounts[idx]
        .lists
        .get("relayUrls")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if relays.is_empty() {
        return Ok(());
    }
    let npub_or_hex = cfg.accounts[idx]
        .attrs
        .get("npub")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if npub_or_hex.is_empty() {
        return Ok(());
    }
    let pubkey_hex = nostr_keys::public_key_to_hex(&npub_or_hex).map_err(|e| e.to_string())?;
    let pubkey_hex_lc = pubkey_hex.to_lowercase();
    let secret_hex = load_credential_entry(account_id, cfg.use_keychain)
        .ok()
        .and_then(|e| nostr_keys::secret_key_to_hex(e.password_or_token.trim()).ok());
    let sk = secret_hex.clone();

    let (maybe_prof, nip65_relays, dm_relays) = mail_runtime_handle().block_on(async {
        let ((prof_res, _), (nip65_res, _), (dm_res, _)) = tokio::join!(
            fetch_profile_from_relays(&relays, &pubkey_hex_lc, 12, sk.clone()),
            fetch_relay_list_from_relays(&relays, &pubkey_hex_lc, 12, sk.clone()),
            fetch_dm_relay_list_from_relays(&relays, &pubkey_hex_lc, 12, sk),
        );
        let maybe_prof = prof_res.ok().flatten();
        let nip65_relays = nip65_res.unwrap_or_default();
        let dm_relays = dm_res.unwrap_or_default();
        (maybe_prof, nip65_relays, dm_relays)
    });

    let merged_relays = merge_nostr_relay_lists(&relays, &nip65_relays, &dm_relays);
    let mut changed = nostr_relay_sets_differ(&relays, &merged_relays);
    if changed {
        cfg.accounts[idx]
            .lists
            .insert("relayUrls".to_string(), merged_relays);
    }

    if let Some(prof) = maybe_prof {
        let acc = &mut cfg.accounts[idx];
        let remote_ts = prof.created_at.unwrap_or(0);
        let local_k0_ts: u64 = acc
            .attrs
            .get(NOSTR_KIND0_CREATED_AT_ATTR)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let remote_is_newer = remote_ts > local_k0_ts;

        if let Some(n) = prof
            .name
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            if acc.label.trim().is_empty() || remote_is_newer {
                if acc.label != n {
                    acc.label = n;
                    changed = true;
                }
            }
        }
        if let Some(n5) = prof
            .nip05
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            let local_empty = acc
                .attrs
                .get("nip05")
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if local_empty || remote_is_newer {
                if acc.attrs.get("nip05") != Some(&n5) {
                    acc.attrs.insert("nip05".to_string(), n5);
                    changed = true;
                }
            }
        }
        if let Some(pic) = prof
            .picture
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            let local_empty = acc
                .avatar_url
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if local_empty || remote_is_newer {
                if acc.avatar_url.as_deref() != Some(pic.as_str()) {
                    acc.avatar_url = Some(pic);
                    changed = true;
                }
            }
        }
        if remote_ts > 0 && (remote_is_newer || local_k0_ts == 0) {
            acc.attrs
                .insert(NOSTR_KIND0_CREATED_AT_ATTR.to_string(), remote_ts.to_string());
            changed = true;
        }
    }

    if changed {
        super::persist_frb_config(config_path, &cfg)?;
        invalidate_mail_store_cache(account_id, use_keychain);
    }
    Ok(())
}

pub(crate) fn nostr_publish_profile_metadata(
    config_path: &str,
    account_id: &str,
) -> Result<(), String> {
    let cfg = super::load_frb_config_struct(config_path);
    let acc = cfg
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| "account not found".to_string())?;
    if !acc.backend_type.eq_ignore_ascii_case("nostr") {
        return Err("not a Nostr account".to_string());
    }
    let relays: Vec<String> = acc
        .lists
        .get("relayUrls")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if relays.is_empty() {
        return Err("Nostr account has no relays".to_string());
    }
    let entry = load_credential_entry(account_id, cfg.use_keychain)?;
    let secret_hex = nostr_keys::secret_key_to_hex(entry.password_or_token.trim())?;
    let pk = nostr_crypto::get_public_key_from_secret(&secret_hex)?;
    let mut content = serde_json::Map::new();
    if !acc.label.trim().is_empty() {
        content.insert(
            "name".to_string(),
            serde_json::Value::String(acc.label.trim().to_string()),
        );
    }
    if let Some(n5) = acc.attrs.get("nip05") {
        let t = n5.trim();
        if !t.is_empty() {
            content.insert("nip05".to_string(), serde_json::Value::String(t.to_string()));
        }
    }
    if let Some(ref u) = acc.avatar_url {
        let t = u.trim();
        if !t.is_empty() {
            content.insert(
                "picture".to_string(),
                serde_json::Value::String(t.to_string()),
            );
        }
    }
    let content_str = serde_json::Value::Object(content).to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut event = Event {
        id: String::new(),
        pubkey: pk,
        created_at: now,
        kind: NOSTR_KIND_METADATA,
        tags: Vec::new(),
        content: content_str,
        sig: String::new(),
    };
    nostr_crypto::sign_event(&mut event, &secret_hex)?;
    let event_json = event_to_json_compact(&event);
    let mut ok_any = false;
    let mut last_err: Option<String> = None;
    for r in &relays {
        match mail_runtime_handle().block_on(publish_event(r, &event_json)) {
            Ok(()) => ok_any = true,
            Err(e) => last_err = Some(e),
        }
    }
    if !ok_any {
        return Err(last_err.unwrap_or_else(|| "failed to publish profile".to_string()));
    }
    Ok(())
}

/// Load full raw RFC 822 bytes for a message (blocking channel wait). Used by the mail body HTTP server.
pub(crate) fn blocking_get_message_raw(
    acc: &FrbAccount,
    use_keychain: bool,
    folder_name: &str,
    message_id: &str,
) -> Result<Vec<u8>, String> {
    mail_blocking_get_message_raw(acc, use_keychain, folder_name, message_id)
}

fn wait_message_count(folder: &dyn Folder) -> Result<u64, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    folder.message_count(Box::new(move |res| {
        let _ = tx.send(res.map_err(|e| e.to_string()));
    }));
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(Ok(n)) => Ok(n),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timeout reading message count (60s)".to_owned()),
    }
}

pub(crate) struct MessageSummaryJson {
    id: String,
    from: String,
    subject: String,
    date_ms: Option<i64>,
    /// `\Seen` / Graph isRead.
    is_read: bool,
    /// IMAP \\Deleted (and equivalent); list UI shows subject struck through.
    marked_for_deletion: bool,
    /// Nostr: sender pubkey (hex, lowercase) for async profile refresh in the UI.
    pub(crate) nostr_sender_pubkey_hex: Option<String>,
}

fn u64_json(n: u64) -> JsonNumber {
    if n <= i64::MAX as u64 {
        JsonNumber::I64(n as i64)
    } else {
        JsonNumber::F64(n as f64)
    }
}

fn write_message_summary(w: &mut JsonWriter, m: &MessageSummaryJson) {
    w.write_start_object();
    w.write_key("id");
    w.write_string(&m.id);
    w.write_key("from");
    w.write_string(&m.from);
    w.write_key("subject");
    w.write_string(&m.subject);
    if let Some(ms) = m.date_ms {
        w.write_key("dateMs");
        w.write_number(JsonNumber::I64(ms));
    }
    w.write_key("isRead");
    w.write_bool(m.is_read);
    if m.marked_for_deletion {
        w.write_key("markedForDeletion");
        w.write_bool(true);
    }
    if let Some(ref pk) = m.nostr_sender_pubkey_hex {
        w.write_key("nostrSenderPubkeyHex");
        w.write_string(pk);
    }
    w.write_end_object();
}

fn format_message_summary_array(rows: &[MessageSummaryJson]) -> String {
    let mut w = JsonWriter::new();
    w.write_start_array();
    for m in rows {
        write_message_summary(&mut w, m);
    }
    w.write_end_array();
    writer_into_string(w)
}

/// Response for [`list_folder_messages_window_json`]: folder total plus one window in **ascending**
/// sort order for the requested `messageListSort` (`startIndex` .. `startIndex + messages.len()`).
pub(crate) struct ListFolderMessagesWindowResponse {
    total: u64,
    start_index: u64,
    pub(crate) messages: Vec<MessageSummaryJson>,
    /// `imapSort` (UID SORT + UID FETCH) or `fullScan` (sequence FETCH / local scan + Rust sort).
    list_strategy: String,
}

impl ListFolderMessagesWindowResponse {
    pub(crate) fn total(&self) -> u64 {
        self.total
    }

    pub(crate) fn start_index(&self) -> u64 {
        self.start_index
    }

    pub(crate) fn list_strategy(&self) -> &str {
        self.list_strategy.as_str()
    }

    pub(crate) fn row_count(&self) -> u32 {
        self.messages.len() as u32
    }

    pub(crate) fn for_each_row(&self, mut on_row: impl FnMut(u64, serde_json::Value)) {
        for (i, m) in self.messages.iter().enumerate() {
            let rank = self.start_index.saturating_add(i as u64);
            on_row(rank, message_summary_json_value(m));
        }
    }
}

fn message_summary_json_value(m: &MessageSummaryJson) -> serde_json::Value {
    let mut o = serde_json::Map::new();
    o.insert("id".to_owned(), serde_json::Value::String(m.id.clone()));
    o.insert("from".to_owned(), serde_json::Value::String(m.from.clone()));
    o.insert(
        "subject".to_owned(),
        serde_json::Value::String(m.subject.clone()),
    );
    if let Some(ms) = m.date_ms {
        o.insert(
            "dateMs".to_owned(),
            serde_json::Value::Number(ms.into()),
        );
    }
    o.insert("isRead".to_owned(), serde_json::Value::Bool(m.is_read));
    if m.marked_for_deletion {
        o.insert(
            "markedForDeletion".to_owned(),
            serde_json::Value::Bool(true),
        );
    }
    if let Some(pk) = &m.nostr_sender_pubkey_hex {
        o.insert(
            "nostrSenderPubkeyHex".to_owned(),
            serde_json::Value::String(pk.clone()),
        );
    }
    serde_json::Value::Object(o)
}

fn format_list_folder_messages_window_response(r: &ListFolderMessagesWindowResponse) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("total");
    w.write_number(u64_json(r.total));
    w.write_key("startIndex");
    w.write_number(u64_json(r.start_index));
    w.write_key("messages");
    w.write_start_array();
    for m in &r.messages {
        write_message_summary(&mut w, m);
    }
    w.write_end_array();
    w.write_key("listStrategy");
    w.write_string(&r.list_strategy);
    w.write_end_object();
    writer_into_string(w)
}

fn conversation_to_message_summary(backend_type: &str, s: ConversationSummary) -> MessageSummaryJson {
    let is_nostr = is_nostr_store(backend_type);
    let from_addr = s.envelope.from.first();
    let (from, nostr_sender_pubkey_hex) = if is_nostr {
        if let Some(a) = from_addr {
            let lp = a.local_part.trim().to_lowercase();
            let dom_empty = a.domain.as_deref().unwrap_or("").is_empty();
            if dom_empty && nostr_keys::is_valid_hex_key(&lp) {
                let label = crate::nostr_profile_cache::display_label_for_pubkey_hex(&lp);
                (label, Some(lp))
            } else {
                (format_address(a), None)
            }
        } else {
            (String::new(), None)
        }
    } else {
        (
            from_addr.map(format_address).unwrap_or_default(),
            None,
        )
    };
    let subject = s.envelope.subject.unwrap_or_default();
    let date_ms = s.envelope.date.map(|d| d.timestamp.saturating_mul(1000));
    MessageSummaryJson {
        id: s.id.to_string(),
        from,
        subject,
        date_ms,
        is_read: s.flags.contains(&Flag::Seen),
        marked_for_deletion: s.flags.contains(&Flag::Deleted),
        nostr_sender_pubkey_hex,
    }
}

struct AttachmentDetailJson {
    filename: Option<String>,
    content_type: String,
    size_bytes: u64,
    transfer_encoding: String,
    imap_section: Option<String>,
    content_id: Option<String>,
    data_base64: Option<String>,
}

struct MessageDetailJson {
    subject: String,
    from: String,
    to: String,
    cc: Option<String>,
    date_ms: Option<i64>,
    message_id: Option<String>,
    body_plain: Option<String>,
    body_html: Option<String>,
    attachments: Vec<AttachmentDetailJson>,
}

fn write_attachment_detail(w: &mut JsonWriter, a: &AttachmentDetailJson) {
    w.write_start_object();
    if let Some(ref f) = a.filename {
        w.write_key("filename");
        w.write_string(f);
    }
    w.write_key("contentType");
    w.write_string(&a.content_type);
    w.write_key("sizeBytes");
    w.write_number(u64_json(a.size_bytes));
    w.write_key("transferEncoding");
    w.write_string(&a.transfer_encoding);
    if let Some(ref s) = a.imap_section {
        w.write_key("imapSection");
        w.write_string(s);
    }
    if let Some(ref s) = a.content_id {
        w.write_key("contentId");
        w.write_string(s);
    }
    if let Some(ref s) = a.data_base64 {
        w.write_key("dataBase64");
        w.write_string(s);
    }
    w.write_end_object();
}

fn format_message_detail(d: &MessageDetailJson) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("subject");
    w.write_string(&d.subject);
    w.write_key("from");
    w.write_string(&d.from);
    w.write_key("to");
    w.write_string(&d.to);
    if let Some(ref s) = d.cc {
        w.write_key("cc");
        w.write_string(s);
    }
    if let Some(ms) = d.date_ms {
        w.write_key("dateMs");
        w.write_number(JsonNumber::I64(ms));
    }
    if let Some(ref mid) = d.message_id {
        w.write_key("messageId");
        w.write_string(mid);
    }
    if let Some(ref s) = d.body_plain {
        w.write_key("bodyPlain");
        w.write_string(s);
    }
    if let Some(ref s) = d.body_html {
        w.write_key("bodyHtml");
        w.write_string(s);
    }
    if !d.attachments.is_empty() {
        w.write_key("attachments");
        w.write_start_array();
        for a in &d.attachments {
            write_attachment_detail(&mut w, a);
        }
        w.write_end_array();
    }
    w.write_end_object();
    writer_into_string(w)
}

fn format_bytes_base64(b64: &str) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("bytesBase64");
    w.write_string(b64);
    w.write_end_object();
    writer_into_string(w)
}

fn detail_from_display(is_nostr: bool, m: MessageForDisplay) -> MessageDetailJson {
    let env = &m.envelope;
    let attachments: Vec<AttachmentDetailJson> = m
        .attachments
        .into_iter()
        .map(|a| AttachmentDetailJson {
            filename: a.filename,
            content_type: a.content_type,
            size_bytes: a.size_bytes,
            transfer_encoding: a.transfer_encoding,
            imap_section: a.imap_section,
            content_id: a.content_id,
            data_base64: a
                .data
                .as_ref()
                .map(|b| base64::engine::general_purpose::STANDARD.encode(b)),
        })
        .collect();
    MessageDetailJson {
        subject: env.subject.clone().unwrap_or_default(),
        from: env
            .from
            .first()
            .map(|a| format_address_maybe_nostr(is_nostr, a))
            .unwrap_or_default(),
        to: format_address_list_maybe_nostr(is_nostr, &env.to),
        cc: if env.cc.is_empty() {
            None
        } else {
            Some(format_address_list_maybe_nostr(is_nostr, &env.cc))
        },
        date_ms: env.date.as_ref().map(|d| d.timestamp.saturating_mul(1000)),
        message_id: env.message_id.clone(),
        body_plain: m.body_plain,
        body_html: m.body_html,
        attachments,
    }
}

fn detail_from_env_and_body(
    is_nostr: bool,
    env: &Envelope,
    body_plain: Option<String>,
    body_html: Option<String>,
) -> MessageDetailJson {
    MessageDetailJson {
        subject: env.subject.clone().unwrap_or_default(),
        from: env
            .from
            .first()
            .map(|a| format_address_maybe_nostr(is_nostr, a))
            .unwrap_or_default(),
        to: format_address_list_maybe_nostr(is_nostr, &env.to),
        cc: if env.cc.is_empty() {
            None
        } else {
            Some(format_address_list_maybe_nostr(is_nostr, &env.cc))
        },
        date_ms: env.date.as_ref().map(|d| d.timestamp.saturating_mul(1000)),
        message_id: env.message_id.clone(),
        body_plain,
        body_html,
        attachments: vec![],
    }
}

fn format_address_maybe_nostr(is_nostr: bool, a: &Address) -> String {
    if is_nostr {
        let lp = a.local_part.trim().to_lowercase();
        if a.domain.as_deref().unwrap_or("").is_empty() && nostr_keys::is_valid_hex_key(&lp) {
            return crate::nostr_profile_cache::display_label_for_pubkey_hex(&lp);
        }
    }
    format_address(a)
}

fn format_address_list_maybe_nostr(is_nostr: bool, v: &[Address]) -> String {
    v.iter()
        .map(|a| format_address_maybe_nostr(is_nostr, a))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_address(a: &Address) -> String {
    let mailbox = match &a.domain {
        Some(d) if !d.is_empty() => format!("{}@{}", a.local_part, d),
        _ => a.local_part.clone(),
    };
    match &a.display_name {
        Some(n) if !n.is_empty() => format!("{n} <{mailbox}>"),
        _ => mailbox,
    }
}

// --- Move / copy messages (same-store and cross-store) -------------------------------------------

struct TransferOneResult {
    id: String,
    ok: bool,
    error: Option<String>,
}

struct TransferMailMessagesResponse {
    results: Vec<TransferOneResult>,
    ok_count: usize,
    failed_count: usize,
}

fn format_transfer_mail_messages_response(r: &TransferMailMessagesResponse) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("results");
    w.write_start_array();
    for e in &r.results {
        w.write_start_object();
        w.write_key("id");
        w.write_string(&e.id);
        w.write_key("ok");
        w.write_bool(e.ok);
        if let Some(ref err) = e.error {
            w.write_key("error");
            w.write_string(err);
        }
        w.write_end_object();
    }
    w.write_end_array();
    w.write_key("okCount");
    w.write_number(JsonNumber::I64(r.ok_count as i64));
    w.write_key("failedCount");
    w.write_number(JsonNumber::I64(r.failed_count as i64));
    w.write_end_object();
    writer_into_string(w)
}

fn wait_folder_copy_one(
    folder: &dyn Folder,
    id: &str,
    dest_folder: &str,
) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    folder.copy_messages_to(
        &[id],
        dest_folder,
        Box::new(move |r| {
            let _ = tx.send(r.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(r) => r,
        Err(_) => Err("timeout copying message (120s)".to_owned()),
    }
}

fn wait_folder_move_one(
    folder: &dyn Folder,
    id: &str,
    dest_folder: &str,
) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    folder.move_messages_to(
        &[id],
        dest_folder,
        Box::new(move |r| {
            let _ = tx.send(r.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(r) => r,
        Err(_) => Err("timeout moving message (120s)".to_owned()),
    }
}

fn wait_folder_append(
    folder: &dyn Folder,
    data: &[u8],
) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    let owned = data.to_vec();
    folder.append_message(
        &owned,
        Box::new(move |r| {
            let _ = tx.send(r.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(r) => r,
        Err(_) => Err("timeout appending message (120s)".to_owned()),
    }
}

fn wait_folder_delete(folder: &dyn Folder, id: &MessageId) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    folder.delete_message(
        id,
        Box::new(move |r| {
            let _ = tx.send(r.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(r) => r,
        Err(_) => Err("timeout deleting message (120s)".to_owned()),
    }
}

fn append_to_mail_folder(
    dest: &FrbAccount,
    dest_folder: &str,
    raw: &[u8],
    use_keychain: bool,
) -> Result<(), String> {
    let t = normalize_store_type(&dest.backend_type);
    if matches!(t.as_str(), "imap" | "imaps") {
        return blocking_imap_append(dest, dest_folder, raw, use_keychain);
    }
    let store = open_cached_store(dest, use_keychain)?;
    let folder = wait_open_folder(store, dest_folder)?;
    wait_folder_append(folder.as_ref(), raw)
}

/// Copy or move messages. Per-message results; on cross-store **move**, only successful appends are deleted from source.
pub(crate) fn transfer_mail_messages_json(
    source: FrbAccount,
    source_folder: String,
    dest: FrbAccount,
    dest_folder: String,
    message_ids: Vec<String>,
    is_move: bool,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let src_folder = source_folder.trim();
    let dst_folder = dest_folder.trim();
    if src_folder.is_empty() || dst_folder.is_empty() {
        return Err("empty folder".to_owned());
    }
    if message_ids.is_empty() {
        return Err("no message ids".to_owned());
    }
    if accounts_same_mail_store(&source, &dest) && src_folder == dst_folder {
        return Err("source and destination folder are the same".to_owned());
    }

    let mut results: Vec<TransferOneResult> = Vec::with_capacity(message_ids.len());

    if accounts_same_mail_store(&source, &dest) {
        let store = open_cached_store(&source, use_keychain)?;
        let folder = wait_open_folder(store, src_folder)?;
        for id in message_ids {
            let r = if is_move {
                wait_folder_move_one(folder.as_ref(), id.as_str(), dst_folder)
            } else {
                wait_folder_copy_one(folder.as_ref(), id.as_str(), dst_folder)
            };
            match r {
                Ok(()) => results.push(TransferOneResult {
                    id: id.clone(),
                    ok: true,
                    error: None,
                }),
                Err(e) => results.push(TransferOneResult {
                    id: id.clone(),
                    ok: false,
                    error: Some(e),
                }),
            }
        }
    } else {
        for id in message_ids {
            let raw = match blocking_get_message_raw(
                &source,
                use_keychain,
                src_folder,
                id.as_str(),
            ) {
                Ok(b) if !b.is_empty() => b,
                Ok(_) => {
                    results.push(TransferOneResult {
                        id: id.clone(),
                        ok: false,
                        error: Some("empty message body".to_owned()),
                    });
                    continue;
                }
                Err(e) => {
                    results.push(TransferOneResult {
                        id: id.clone(),
                        ok: false,
                        error: Some(e),
                    });
                    continue;
                }
            };
            match append_to_mail_folder(&dest, dst_folder, &raw, use_keychain) {
                Ok(()) => {
                    if is_move {
                        let store = open_cached_store(&source, use_keychain)?;
                        let folder = wait_open_folder(store, src_folder)?;
                        let mid = MessageId::new(id.clone());
                        match wait_folder_delete(folder.as_ref(), &mid) {
                            Ok(()) => results.push(TransferOneResult {
                                id: id.clone(),
                                ok: true,
                                error: None,
                            }),
                            Err(e) => results.push(TransferOneResult {
                                id: id.clone(),
                                ok: false,
                                error: Some(format!("appended to destination but source delete failed: {e}")),
                            }),
                        }
                    } else {
                        results.push(TransferOneResult {
                            id: id.clone(),
                            ok: true,
                            error: None,
                        });
                    }
                }
                Err(e) => results.push(TransferOneResult {
                    id: id.clone(),
                    ok: false,
                    error: Some(e),
                }),
            }
        }
    }

    let ok_count = results.iter().filter(|r| r.ok).count();
    let failed_count = results.len() - ok_count;
    let out = TransferMailMessagesResponse {
        results,
        ok_count,
        failed_count,
    };
    Ok(format_transfer_mail_messages_response(&out))
}

pub(crate) fn expunge_mail_folder(
    acc: FrbAccount,
    folder_name: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let folder_name = folder_name.trim();
    if folder_name.is_empty() {
        return Err("empty folder name".to_owned());
    }
    if !is_imap_like_store(acc.backend_type.as_str()) {
        return Err("expunge is only supported for IMAP folders".to_owned());
    }
    let store = open_cached_store(&acc, use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    folder.expunge(Box::new(move |r| {
        let _ = tx.send(r.map_err(|e| e.to_string()));
    }));
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(r) => r,
        Err(_) => Err("timeout expunging folder (120s)".to_owned()),
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrbComposeAttachment {
    filename: Option<String>,
    mime_type: String,
    bytes_base64: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrbSmtpComposeJson {
    from: String,
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    subject: String,
    body_plain: String,
    body_html: Option<String>,
    #[serde(default)]
    attachments: Vec<FrbComposeAttachment>,
    #[serde(default)]
    dsn_notify: Option<String>,
    /// When set, Gmail SMTP may reuse this store's OAuth vault entry and persist it to the transport.
    #[serde(default)]
    store_account_id: Option<String>,
}

fn dsn_setting_to_notify_param(setting: &str) -> Option<String> {
    let s = setting.trim().to_ascii_lowercase();
    if s.is_empty() || s == "never" {
        return None;
    }
    let mut parts: Vec<&'static str> = Vec::new();
    for tok in s.split(',') {
        match tok.trim() {
            "failure" => parts.push("FAILURE"),
            "success" => parts.push("SUCCESS"),
            "delay" => parts.push("DELAY"),
            _ => {}
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

fn smtp_parse_addrs(raw: impl AsRef<str>) -> Vec<Address> {
    raw.as_ref()
        .split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.split('@');
            let local = parts.next()?.trim().to_owned();
            let domain = parts.next().map(|v| v.trim().to_owned());
            Some(Address {
                display_name: None,
                local_part: local,
                domain,
            })
        })
        .collect()
}

fn smtp_tls_mode(security: &str) -> (bool, bool) {
    match security.trim().to_ascii_lowercase().as_str() {
        "tls" | "smtps" => (true, false),
        "plain" | "none" | "insecure" => (false, false),
        _ => (false, true),
    }
}

/// SMTP send for one `<transport id="…">` row; [transport] comes from config (host/port/security).
pub(crate) fn send_smtp_json(
    transport: &super::FrbTransport,
    use_keychain: bool,
    compose_json: &str,
) -> Result<(), String> {
    let tt = transport.transport_type.trim();
    let is_gmail = tt.eq_ignore_ascii_case("gmail");
    if !tt.eq_ignore_ascii_case("smtp") && !is_gmail {
        return Err(format!(
            "transport {:?} has type {:?}, expected smtp or gmail",
            transport.id, transport.transport_type
        ));
    }

    let mut host = transport.host.trim().to_string();
    if host.is_empty() {
        if is_gmail {
            host = "smtp.gmail.com".to_owned();
        } else {
            return Err(format!(
                "transport {:?} has empty host in config",
                transport.id
            ));
        }
    }

    let mut port = transport.port;
    if is_gmail && port == 0 {
        port = 587;
    }

    let (use_implicit_tls, use_starttls) = if is_gmail && port == 465 {
        (true, false)
    } else {
        smtp_tls_mode(transport.security.as_str())
    };

    let draft: FrbSmtpComposeJson =
        serde_json::from_str(compose_json).map_err(|e| format!("compose JSON: {e}"))?;

    let mut attachments: Vec<Attachment> = Vec::with_capacity(draft.attachments.len());
    for a in draft.attachments {
        let raw = a.bytes_base64.trim();
        if raw.is_empty() {
            continue;
        }
        let content = base64::engine::general_purpose::STANDARD
            .decode(raw.as_bytes())
            .map_err(|e| format!("attachment base64: {e}"))?;
        let mime_type = a.mime_type.trim();
        attachments.push(Attachment {
            filename: a.filename,
            mime_type: if mime_type.is_empty() {
                "application/octet-stream".to_owned()
            } else {
                mime_type.to_owned()
            },
            content,
        });
    }

    let transport_dsn = transport.dsn_notify.trim();
    let eff_dsn = if transport_dsn.is_empty() {
        "failure"
    } else {
        transport_dsn
    };
    let dsn_src = draft.dsn_notify.as_deref().unwrap_or(eff_dsn);
    let smtp_notify = dsn_setting_to_notify_param(dsn_src);

    let payload = SendPayload {
        from: smtp_parse_addrs(draft.from),
        to: draft.to.into_iter().flat_map(smtp_parse_addrs).collect(),
        cc: draft.cc.into_iter().flat_map(smtp_parse_addrs).collect(),
        bcc: draft.bcc.into_iter().flat_map(smtp_parse_addrs).collect(),
        subject: Some(draft.subject),
        body_plain: Some(draft.body_plain),
        body_html: draft.body_html,
        attachments,
        newsgroups: vec![],
        nntp_in_reply_to: None,
        nntp_references: None,
        smtp_notify,
    };

    if payload.from.len() != 1 {
        return Err("compose \"from\" must be exactly one address".to_owned());
    }
    if payload.to.is_empty() {
        return Err("compose \"to\" must include at least one address".to_owned());
    }

    set_credentials_backend(use_keychain);
    let tid = transport.id.trim();
    let cred_path = resolve_credentials_file_path().ok_or_else(|| {
        "could not resolve credentials path".to_owned()
    })?;

    let store_sid = draft
        .store_account_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let (smtp_user, smtp_token, copy_store_to_transport) = if is_gmail {
        let from_transport = load_credential_entry(tid, use_keychain).is_ok();
        let cred_key: String = if from_transport {
            tid.to_string()
        } else if let Some(sid) = store_sid {
            let (acc, _) = resolve_mail_account(sid).map_err(|e| {
                format!(
                    "no saved credential for Gmail transport {tid}; could not load linked store ({e})"
                )
            })?;
            if normalize_store_type(acc.backend_type.as_str()) != "gmail" {
                return Err(format!(
                    "storeAccountId must reference a Gmail store (got {:?})",
                    acc.backend_type
                ));
            }
            sid.to_string()
        } else {
            return Err(
                match load_credential_entry(tid, use_keychain) {
                    Err(e) if e.contains("no saved credential for this account") => format!(
                        "no saved credential for Gmail transport {tid}; sign in with Google on the Gmail store first, or pass storeAccountId when sending"
                    ),
                    Err(e) => e,
                    Ok(_) => {
                        "internal: expected missing Gmail transport credential".to_owned()
                    }
                },
            );
        };
        let (u, tok) = resolve_gmail_xoauth_secret(cred_key.as_str(), use_keychain)?;
        let copy = !from_transport && store_sid.is_some();
        (u, tok, copy)
    } else {
        let entry = load_credential_entry(tid, use_keychain).map_err(|e| {
            if e.contains("no saved credential for this account") {
                format!(
                    "no saved SMTP credential for transport {tid}; enter username and password when prompted"
                )
            } else {
                e
            }
        })?;
        let user = entry.username.trim();
        if user.is_empty() {
            return Err(format!(
                "no SMTP username in saved credentials for transport {}; enter username and password when prompted",
                tid
            ));
        }
        (
            user.to_string(),
            entry.password_or_token.clone(),
            false,
        )
    };

    if smtp_user.trim().is_empty() {
        return Err(if is_gmail {
            format!(
                "Gmail transport {tid}: saved credential has no email address; sign in again with your Google account"
            )
        } else {
            format!(
                "no SMTP username in saved credentials for transport {}; enter username and password when prompted",
                tid
            )
        });
    }

    let (message, mut envelope) = build_rfc822_from_payload(&payload);
    envelope.cc.extend(payload.bcc.iter().cloned());

    let notify_arg = payload.smtp_notify.as_deref();

    let mechanism = if is_gmail {
        SaslMechanism::XOAuth2
    } else {
        SaslMechanism::Plain
    };
    let auth = Some((
        smtp_user.clone(),
        smtp_token.clone(),
        mechanism,
    ));

    frb_runtime_handle()
        .block_on(async {
            send_message_async(
                host.as_str(),
                port,
                use_implicit_tls,
                use_starttls,
                auth.as_ref().map(|(u, p, m)| (u.as_str(), p.as_str(), *m)),
                "localhost",
                message.as_slice(),
                &envelope,
                notify_arg,
            )
            .await
        })
        .map_err(|e: SmtpClientError| e.to_string())?;

    if copy_store_to_transport {
        if let Some(sid) = store_sid {
            let store_entry = load_mail_credential(sid, use_keychain)?;
            save_credential(
                cred_path.as_path(),
                tid,
                store_entry.username.trim(),
                store_entry.password_or_token.as_str(),
            )?;
        }
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrbNntpComposeJson {
    from: String,
    #[serde(default)]
    newsgroups: Vec<String>,
    subject: String,
    body_plain: String,
    #[serde(default)]
    attachments: Vec<FrbComposeAttachment>,
    #[serde(default)]
    in_reply_to: Option<String>,
    #[serde(default)]
    references: Option<String>,
}

fn addresses_from_from_field(raw: &str) -> Vec<Address> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let wrapped = format!("From: {trimmed}\r\nSubject: tag\r\n\r\n");
    match parse_envelope(wrapped.as_bytes()) {
        Ok(h) => h
            .from
            .into_iter()
            .map(|e| Address {
                display_name: e.display_name,
                local_part: e.local_part,
                domain: Some(e.domain),
            })
            .collect(),
        Err(_) => smtp_parse_addrs(trimmed),
    }
}

/// POST via the NNTP store session for `<store id="…" type="nntp">` (no separate transport row).
pub(crate) fn send_nntp_json(
    acc: &FrbAccount,
    use_keychain: bool,
    compose_json: &str,
) -> Result<(), String> {
    if normalize_store_type(acc.backend_type.as_str()) != "nntp" {
        return Err(format!(
            "account {:?} has type {:?}, expected nntp",
            acc.id, acc.backend_type
        ));
    }
    let draft: FrbNntpComposeJson =
        serde_json::from_str(compose_json).map_err(|e| format!("compose JSON: {e}"))?;

    let groups: Vec<String> = draft
        .newsgroups
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if groups.is_empty() {
        return Err("compose \"newsgroups\" must name at least one group".to_owned());
    }

    set_credentials_backend(use_keychain);
    let store = open_cached_store(acc, use_keychain)?;
    let nntp = store
        .as_any()
        .downcast_ref::<NntpStore>()
        .ok_or_else(|| "internal: NNTP store expected".to_owned())?;
    let transport = NntpTransport::from_store_state(nntp.shared_state());

    let mut attachments: Vec<Attachment> = Vec::with_capacity(draft.attachments.len());
    for a in draft.attachments {
        let raw = a.bytes_base64.trim();
        if raw.is_empty() {
            continue;
        }
        let content = base64::engine::general_purpose::STANDARD
            .decode(raw.as_bytes())
            .map_err(|e| format!("attachment base64: {e}"))?;
        let mime_type = a.mime_type.trim();
        attachments.push(Attachment {
            filename: a.filename,
            mime_type: if mime_type.is_empty() {
                "application/octet-stream".to_owned()
            } else {
                mime_type.to_owned()
            },
            content,
        });
    }

    let from_addrs = addresses_from_from_field(&draft.from);
    if from_addrs.len() != 1 {
        return Err("compose \"from\" must be exactly one address".to_owned());
    }

    let payload = SendPayload {
        from: from_addrs,
        to: vec![],
        cc: vec![],
        bcc: vec![],
        subject: Some(draft.subject),
        body_plain: Some(draft.body_plain),
        body_html: None,
        attachments,
        newsgroups: groups,
        nntp_in_reply_to: draft.in_reply_to,
        nntp_references: draft.references,
        smtp_notify: None,
    };

    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    transport.send(&payload, Box::new(move |r| {
        let _ = tx.send(r.map_err(|e| e.to_string()));
    }));
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(r) => r,
        Err(_) => Err("timeout waiting for NNTP POST (120s)".to_owned()),
    }
}

#[cfg(test)]
mod transfer_response_tests {
    use super::{
        TransferMailMessagesResponse, TransferOneResult, format_transfer_mail_messages_response,
    };

    #[test]
    fn transfer_json_uses_camel_case_counts_and_results() {
        let out = TransferMailMessagesResponse {
            results: vec![
                TransferOneResult {
                    id: "1".into(),
                    ok: true,
                    error: None,
                },
                TransferOneResult {
                    id: "2".into(),
                    ok: false,
                    error: Some("boom".into()),
                },
            ],
            ok_count: 1,
            failed_count: 1,
        };
        let s = format_transfer_mail_messages_response(&out);
        assert_eq!(
            s,
            r#"{"results":[{"id":"1","ok":true},{"id":"2","ok":false,"error":"boom"}],"okCount":1,"failedCount":1}"#
        );
    }
}
