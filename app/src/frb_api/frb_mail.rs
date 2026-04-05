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

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use base64::Engine;
use once_cell::sync::Lazy;
use percent_encoding::percent_decode_str;
use tagliacarte_core::json::{JsonNumber, JsonWriter, writer_into_string};
use tagliacarte_core::config::{
    CredentialEntry, default_config_dir, load_credentials, resolve_credentials_file_path,
    set_credentials_backend,
};
use tagliacarte_core::localstorage::maildir::MaildirStore;
use tagliacarte_core::localstorage::mbox::MboxStore;
use tagliacarte_core::message_id::MessageId;
use tagliacarte_core::mime::{extract_structured_body, utf8_body_after_rfc822_headers};
use tagliacarte_core::protocol::imap::connect_and_authenticate;
use tagliacarte_core::protocol::imap::ImapStore;
use tagliacarte_core::protocol::imap::trace as imap_trace;
use tagliacarte_core::protocol::smtp::{build_rfc822_from_payload, send_message_async, SmtpClientError};
use tagliacarte_core::protocol::matrix::MatrixStore;
use tagliacarte_core::protocol::nostr::keys as nostr_keys;
use tagliacarte_core::protocol::nostr::{
    crypto as nostr_crypto,
    event_to_json_compact,
    fetch_dm_relay_list_from_relays,
    fetch_profile_from_relays,
    fetch_relay_list_from_relays,
    publish_event,
    Event, KIND_METADATA as NOSTR_KIND_METADATA, NostrStore,
};
use tagliacarte_core::sasl::SaslMechanism;
use tagliacarte_core::store::{
    sort_conversation_summaries_for_window, Address, ConversationSummary, Envelope, Flag, Folder,
    FolderInfo, MessageForDisplay, OpenFolderEvent, SendPayload, Store, StoreError, Transport,
};
use tokio::runtime::{Builder, Runtime};
use url::Url;

static FRB_TOKIO: Lazy<Runtime> = Lazy::new(|| {
    // IMAP pipeline + Nostr async `list_folders` share this runtime; too few workers can stall
    // callbacks while other code uses `Handle::block_on`, leaving the UI waiting on folder/message
    // list channels until recv_timeout (looks like an infinite spinner).
    let n = std::thread::available_parallelism()
        .map(|p| p.get().clamp(4, 32))
        .unwrap_or(8);
    Builder::new_multi_thread()
        .worker_threads(n)
        .enable_all()
        .build()
        .expect("frb tokio runtime")
});

pub(crate) fn frb_runtime_handle() -> tokio::runtime::Handle {
    FRB_TOKIO.handle().clone()
}

type DynStore = Arc<dyn Store + Send + Sync>;

/// Reuse one [`Store`] per `(uri, credential_lookup_key, use_keychain)` so the Flutter bridge does
/// not open a new IMAP connection on every call. The credential key is the vault id (`s1`, …) or
/// the store URI when empty (legacy).
static FRB_STORE_CACHE: Lazy<Mutex<HashMap<(String, String, bool), DynStore>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Drop cached store so the next open reloads credentials (after password change, etc.).
pub(crate) fn invalidate_frb_store_cache(
    store_uri: &str,
    credential_key: &str,
    use_keychain: bool,
) {
    let cred_key = credential_lookup(store_uri, credential_key).to_string();
    let key = (store_uri.to_string(), cred_key, use_keychain);
    let mut g = FRB_STORE_CACHE.lock().expect("frb store cache");
    g.remove(&key);
}

pub(crate) fn credential_lookup<'a>(store_uri: &'a str, credential_key: &'a str) -> &'a str {
    let ck = credential_key.trim();
    if ck.is_empty() { store_uri.trim() } else { ck }
}

/// In-memory folder list + per-folder unread counts (same semantics as list-mail-folders JSON).
#[derive(Debug, Clone)]
pub(crate) struct MailFoldersSnapshot {
    pub folders: Vec<String>,
    pub hierarchy_delimiter: Option<String>,
    pub unread_by_folder: HashMap<String, u32>,
}

/// Build folder snapshot; for each folder calls `on_each(name, unread)` in list order
/// (after inbox-first), then returns the snapshot. Session emits `folderFound` then `folderListUpdated`.
pub(crate) fn list_mail_folders_snapshot_with_progress(
    store_uri: &str,
    credential_key: &str,
    use_keychain: bool,
    mut on_each: impl FnMut(&str, u32),
) -> Result<MailFoldersSnapshot, String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    if uri.is_empty() {
        return Err("empty store URI".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, credential_key), use_keychain)?;
    let is_imap_uri = uri.starts_with("imap://") || uri.starts_with("imaps://");
    let (folders, hierarchy_delimiter, unread_by_folder) =
        match store.as_any().downcast_ref::<ImapStore>() {
            Some(imap) if is_imap_uri => {
                let (names, delim_char, unread_map) = imap
                    .list_folders_and_unread_blocking()
                    .map_err(|e| e.to_string())?;
                let mut out = names;
                inbox_first_preserve_order(&mut out);
                let hierarchy_delimiter = delim_char.map(|c| c.to_string());
                let mut m = HashMap::new();
                for name in &out {
                    m.insert(
                        name.clone(),
                        unread_map.get(name).copied().unwrap_or(0),
                    );
                }
                (out, hierarchy_delimiter, m)
            }
            _ => {
                let names = Arc::new(Mutex::new(Vec::<String>::new()));
                let n2 = Arc::clone(&names);
                let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
                store.list_folders(
                    Box::new(move |fi: FolderInfo| {
                        n2.lock().expect("folder names lock").push(fi.name);
                    }),
                    Box::new(move |res: Result<(), StoreError>| {
                        let _ = tx.send(res.map_err(|e| e.to_string()));
                    }),
                );
                match rx.recv_timeout(Duration::from_secs(120)) {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => return Err(e),
                    Err(_) => return Err("timeout listing folders (120s)".to_owned()),
                }
                let mut out = names.lock().expect("folder names lock").clone();
                inbox_first_preserve_order(&mut out);
                let hierarchy_delimiter = store.hierarchy_delimiter().map(|c| c.to_string());
                let counts = folder_unread_counts_for_store(uri, &store, &out);
                let mut m = HashMap::new();
                for c in counts {
                    m.insert(c.folder_name, c.unread as u32);
                }
                (out, hierarchy_delimiter, m)
            }
        };
    for name in &folders {
        let u = unread_by_folder.get(name).copied().unwrap_or(0);
        on_each(name.as_str(), u);
    }
    Ok(MailFoldersSnapshot {
        folders,
        hierarchy_delimiter,
        unread_by_folder,
    })
}

pub(crate) fn list_mail_folders_snapshot(
    store_uri: &str,
    credential_key: &str,
    use_keychain: bool,
) -> Result<MailFoldersSnapshot, String> {
    list_mail_folders_snapshot_with_progress(store_uri, credential_key, use_keychain, |_, _| {})
}

/// Folder list from Nostr on-disk DM cache only (no `list_folders` / no network). Used after
/// background relay sync to emit `folderListUpdated` without re-entering a long-running list call.
pub(crate) fn nostr_folder_list_from_cache_snapshot(
    store_uri: &str,
    credential_key: &str,
    use_keychain: bool,
) -> Result<MailFoldersSnapshot, String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    if uri.is_empty() {
        return Err("empty store URI".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, credential_key), use_keychain)?;
    let ns = store
        .as_any()
        .downcast_ref::<NostrStore>()
        .ok_or_else(|| "internal: not a Nostr store".to_string())?;
    let mut out = ns
        .list_cached_conversation_pubkeys()
        .map_err(|e| e.to_string())?;
    inbox_first_preserve_order(&mut out);
    let hierarchy_delimiter = store.hierarchy_delimiter().map(|c| c.to_string());
    let counts = folder_unread_counts_for_store(uri, &store, &out);
    let mut m = HashMap::new();
    for c in counts {
        m.insert(c.folder_name, c.unread as u32);
    }
    Ok(MailFoldersSnapshot {
        folders: out,
        hierarchy_delimiter,
        unread_by_folder: m,
    })
}

pub(crate) fn list_mail_folders_json(
    store_uri: String,
    credential_key: String,
    use_keychain: bool,
) -> Result<String, String> {
    let snap = list_mail_folders_snapshot(
        store_uri.trim(),
        credential_key.trim(),
        use_keychain,
    )
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

fn folder_unread_counts_for_store(
    uri: &str,
    store: &DynStore,
    folder_names: &[String],
) -> Vec<FolderUnreadCountJson> {
    let mut v = Vec::new();
    if uri.starts_with("maildir:") {
        if let Some(md) = store.as_any().downcast_ref::<MaildirStore>() {
            for name in folder_names {
                let u = md.unread_count_for_mailbox(name).unwrap_or(0);
                v.push(FolderUnreadCountJson {
                    folder_name: name.clone(),
                    unread: u,
                });
            }
        }
    } else if uri.starts_with("mbox:") {
        for name in folder_names {
            v.push(FolderUnreadCountJson {
                folder_name: name.clone(),
                unread: 0,
            });
        }
    }
    v
}

pub(crate) fn create_mail_folder(
    store_uri: String,
    credential_key: String,
    folder_path: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let name = folder_path.trim();
    if uri.is_empty() || name.is_empty() {
        return Err("empty store URI or folder path".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
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
    store_uri: String,
    credential_key: String,
    old_name: String,
    new_name: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let old_n = old_name.trim();
    let new_n = new_name.trim();
    if uri.is_empty() || old_n.is_empty() || new_n.is_empty() {
        return Err("empty store URI or folder name".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
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
    store_uri: String,
    credential_key: String,
    folder_name: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let name = folder_name.trim();
    if uri.is_empty() || name.is_empty() {
        return Err("empty store URI or folder name".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
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

/// Puts `INBOX` first (case-insensitive); leaves all other folders in list order from the store.
fn inbox_first_preserve_order(names: &mut Vec<String>) {
    if let Some(pos) = names.iter().position(|n| n.eq_ignore_ascii_case("INBOX")) {
        let inbox = names.remove(pos);
        names.insert(0, inbox);
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
    store_uri: String,
    credential_key: String,
    folder_name: String,
    start_index: u64,
    limit: u64,
    message_list_sort: String,
    use_keychain: bool,
) -> Result<ListFolderMessagesWindowResponse, String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let folder_name = folder_name.trim();
    if uri.is_empty() || folder_name.is_empty() {
        return Err("empty store URI or folder name".to_owned());
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
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;

    if uri.starts_with("imap://") || uri.starts_with("imaps://") {
        if let Some(imap) = store.as_ref().as_any().downcast_ref::<ImapStore>() {
            let (total, si, summaries, strat) = imap
                .list_folder_messages_window_blocking(folder_name, start_index, limit, sort_eff)
                .map_err(|e| e.to_string())?;
            let messages: Vec<MessageSummaryJson> = summaries
                .into_iter()
                .map(|s| conversation_to_message_summary(uri, s))
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
        .map(|s| conversation_to_message_summary(uri, s))
        .collect();
    Ok(ListFolderMessagesWindowResponse {
        total,
        start_index,
        messages,
        list_strategy: "fullScan".to_owned(),
    })
}

pub(crate) fn list_folder_messages_window_json(
    store_uri: String,
    credential_key: String,
    folder_name: String,
    start_index: u64,
    limit: u64,
    message_list_sort: String,
    use_keychain: bool,
) -> Result<String, String> {
    let r = list_folder_messages_window_response(
        store_uri,
        credential_key,
        folder_name,
        start_index,
        limit,
        message_list_sort,
        use_keychain,
    )?;
    Ok(format_list_folder_messages_window_response(&r))
}

pub(crate) fn list_folder_messages_json(
    store_uri: String,
    credential_key: String,
    folder_name: String,
    skip: u64,
    limit: u64,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let folder_name = folder_name.trim();
    if uri.is_empty() || folder_name.is_empty() {
        return Err("empty store URI or folder name".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;
    let total = wait_message_count(folder.as_ref())?;
    let Some(range) = list_range_for_page(uri, total, skip, limit) else {
        return Ok("[]".to_owned());
    };

    let uri_owned = uri.to_string();
    let rows = Arc::new(Mutex::new(Vec::<MessageSummaryJson>::new()));
    let r2 = Arc::clone(&rows);
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);

    folder.list_conversations(
        range,
        Box::new(move |s| {
            r2.lock()
                .expect("summary lock")
                .push(conversation_to_message_summary(uri_owned.as_str(), s));
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
    store_uri: String,
    credential_key: String,
    folder_name: String,
    message_id: String,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let folder_name = folder_name.trim();
    let message_id = message_id.trim();
    if uri.is_empty() || folder_name.is_empty() || message_id.is_empty() {
        return Err("empty store URI, folder name, or message id".to_owned());
    }
    let is_imap = uri.starts_with("imap://") || uri.starts_with("imaps://");
    let load_secs = if is_imap { 300u64 } else { 120u64 };
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
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
            Ok(format_message_detail(&detail_from_display(uri, display)))
        }
        Ok(Err(e)) if e.contains("get_message_display not supported") => {
            get_folder_message_json_full_raw(&*folder, message_id, load_secs, uri)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(format!("timeout loading message ({load_secs}s)")),
    }
}

/// Sets `\Seen` on the message (IMAP UID STORE, Maildir rename, Graph PATCH, …).
pub(crate) fn mark_folder_message_read(
    store_uri: String,
    credential_key: String,
    folder_name: String,
    message_id: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let folder_name = folder_name.trim();
    let message_id = message_id.trim();
    if uri.is_empty() || folder_name.is_empty() || message_id.is_empty() {
        return Err("empty store URI, folder name, or message id".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
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
    store_uri: String,
    credential_key: String,
    folder_name: String,
    message_id: String,
    imap_section: String,
    transfer_encoding: String,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let folder_name = folder_name.trim();
    let message_id = message_id.trim();
    let imap_section = imap_section.trim();
    if uri.is_empty() || folder_name.is_empty() || message_id.is_empty() || imap_section.is_empty()
    {
        return Err("empty store URI, folder, message id, or section".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
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

    let part_secs = if uri.starts_with("imap://") || uri.starts_with("imaps://") {
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
    store_uri: &str,
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

    let detail = detail_from_env_and_body(store_uri, &env, body_plain, html);
    Ok(format_message_detail(&detail))
}

pub(crate) fn open_store(
    uri: &str,
    credential_lookup_key: &str,
    use_keychain: bool,
) -> Result<DynStore, String> {
    let key = (
        uri.to_string(),
        credential_lookup_key.to_string(),
        use_keychain,
    );
    {
        let g = FRB_STORE_CACHE.lock().expect("frb store cache");
        if let Some(s) = g.get(&key) {
            return Ok(Arc::clone(s));
        }
    }
    let s = build_store(uri, credential_lookup_key, use_keychain)?;
    let mut g = FRB_STORE_CACHE.lock().expect("frb store cache");
    if let Some(existing) = g.get(&key) {
        return Ok(Arc::clone(existing));
    }
    g.insert(key, Arc::clone(&s));
    Ok(s)
}

/// If this store is a cached IMAP connection, atomically read and clear the
/// "folder list / mailbox changed" latch (EXISTS / RECENT / IDLE path).
/// Returns `false` when the store is not cached or not IMAP.
pub(crate) fn imap_take_folder_list_stale(
    store_uri: String,
    credential_key: String,
    use_keychain: bool,
) -> bool {
    let uri = store_uri.trim();
    if uri.is_empty() {
        return false;
    }
    if !(uri.starts_with("imap://") || uri.starts_with("imaps://")) {
        return false;
    }
    let ck = credential_lookup(uri, &credential_key).to_string();
    let key = (uri.to_string(), ck, use_keychain);
    let g = FRB_STORE_CACHE.lock().expect("frb store cache");
    let Some(store) = g.get(&key) else {
        return false;
    };
    store
        .as_any()
        .downcast_ref::<ImapStore>()
        .is_some_and(|imap| imap.take_folder_list_stale())
}

/// Per-account minimum quiet seconds on the wire before the pipeline may enter IMAP IDLE.
pub(crate) fn imap_configure_idle_threshold(
    store_uri: String,
    credential_key: String,
    use_keychain: bool,
    min_idle_seconds: u32,
) -> Result<(), String> {
    let uri = store_uri.trim();
    if uri.is_empty() {
        return Err("empty store URI".to_owned());
    }
    if !(uri.starts_with("imap://") || uri.starts_with("imaps://")) {
        return Ok(());
    }
    set_credentials_backend(use_keychain);
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
    if let Some(imap) = store.as_any().downcast_ref::<ImapStore>() {
        imap.set_imap_min_idle_secs(min_idle_seconds);
    }
    Ok(())
}

fn load_credential_entry(
    credential_lookup_key: &str,
    use_keychain: bool,
) -> Result<CredentialEntry, String> {
    let cred_path = resolve_credentials_file_path().ok_or_else(|| {
        "could not resolve credentials path (~/.tagliacarte/credentials)".to_owned()
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

fn parse_nostr_query_relays(query_opt: Option<&str>) -> Vec<String> {
    let mut relays: Vec<String> = Vec::new();
    if let Some(q) = query_opt {
        for (k, v) in url::form_urlencoded::parse(q.as_bytes()) {
            if k == "relays" {
                for part in v.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
                    let t = part.trim();
                    if !t.is_empty() {
                        relays.push(t.to_string());
                    }
                }
            }
        }
    }
    relays
}

fn decode_nostr_id_part(id_part: &str) -> Result<String, String> {
    let id_part = id_part.trim();
    if nostr_keys::is_valid_hex_key(id_part) {
        Ok(id_part.to_string())
    } else if nostr_keys::is_npub(id_part) {
        nostr_keys::npub_to_hex(id_part).map_err(|e| format!("nostr npub: {e}"))
    } else {
        Err(format!(
            "nostr id must be 64-char hex or npub1…, got {id_part:?}"
        ))
    }
}

/// Returns (pubkey_hex, relays_from_uri_query). Relays may be empty for `nostr:<npub>` — use
/// [nostr_relays_from_saved_config] with the account id.
fn parse_nostr_store_uri(uri: &str) -> Result<(String, Vec<String>), String> {
    let u = uri.trim();
    if let Some(rest) = u.strip_prefix("nostr:store:") {
        let (id_part, query_opt) = match rest.split_once('?') {
            Some((a, b)) => (a, Some(b)),
            None => (rest, None),
        };
        let relays = parse_nostr_query_relays(query_opt);
        let pubkey_hex = decode_nostr_id_part(id_part)?;
        return Ok((pubkey_hex, relays));
    }
    let rest = u
        .strip_prefix("nostr:")
        .ok_or_else(|| format!("not a nostr store URI: {u}"))?;
    if rest.starts_with("transport:") {
        return Err(format!("not a nostr store URI (transport): {u}"));
    }
    let id_part = rest
        .split(|c| c == '?' || c == '#')
        .next()
        .unwrap_or(rest)
        .trim();
    if id_part.is_empty() {
        return Err(format!("empty nostr identity in URI: {u}"));
    }
    let pubkey_hex = decode_nostr_id_part(id_part)?;
    Ok((pubkey_hex, Vec::new()))
}

fn nostr_relays_from_saved_config(account_id: &str) -> Vec<String> {
    let Some(p) = super::config_path_for_relay_lookup() else {
        return Vec::new();
    };
    let cfg = super::load_frb_config_struct(p.as_str());
    let urls: &[String] = cfg
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .and_then(|a| a.lists.get("relayUrls"))
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    urls
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Relay URLs + optional hex secret for `REQ` profile fetch (same rules as opening a Nostr store).
pub(crate) fn nostr_profile_fetch_context(
    store_uri: &str,
    credential_key: &str,
    use_keychain: bool,
) -> Result<(Vec<String>, Option<String>), String> {
    let (_our_pk, mut relays) = parse_nostr_store_uri(store_uri)?;
    if relays.is_empty() {
        relays = nostr_relays_from_saved_config(credential_key.trim());
    }
    if relays.is_empty() {
        return Err(
            "Nostr account has no relay URLs (add relays in account settings)".to_owned(),
        );
    }
    set_credentials_backend(use_keychain);
    let sk = load_credential_entry(credential_key.trim(), use_keychain)
        .ok()
        .and_then(|e| nostr_keys::secret_key_to_hex(e.password_or_token.trim()).ok());
    Ok((relays, sk))
}

fn parse_matrix_store_uri(uri: &str) -> Result<(String, String), String> {
    let u = uri.trim();
    let rest = u
        .strip_prefix("matrix:store:")
        .ok_or_else(|| format!("not a matrix store URI: {u}"))?;
    let colon = rest.find(':').ok_or_else(|| {
        "matrix store URI expected matrix:store:<homeserver>:<mxid>".to_string()
    })?;
    let homeserver = rest[..colon].trim().to_string();
    let user_id = rest[colon + 1..].trim().to_string();
    if homeserver.is_empty() || user_id.is_empty() {
        return Err("matrix store: empty homeserver or user id".to_string());
    }
    Ok((homeserver, user_id))
}

fn build_nostr_store(
    uri: &str,
    credential_lookup_key: &str,
    use_keychain: bool,
) -> Result<DynStore, String> {
    let (pubkey_hex, mut relays) = parse_nostr_store_uri(uri)?;
    if relays.is_empty() {
        relays = nostr_relays_from_saved_config(credential_lookup_key);
    }
    if relays.is_empty() {
        return Err(
            "Nostr account has no relay URLs (add relays in account settings)".to_owned(),
        );
    }
    let config_dir = default_config_dir().map(|p| p.to_string_lossy().into_owned());
    let account_id = credential_lookup_key.trim().to_string();
    let on_sync_done = Arc::new(move || {
        crate::session::refresh_nostr_folders_for_account(account_id.as_str());
    });
    let store = NostrStore::new(
        relays,
        pubkey_hex,
        config_dir,
        FRB_TOKIO.handle().clone(),
        Some(on_sync_done),
    )
    .map_err(|e| e.to_string())?;
    let arc_store: DynStore = Arc::new(store);
    if let Ok(entry) = load_credential_entry(credential_lookup_key, use_keychain) {
        arc_store.set_credential(None, entry.password_or_token.as_str());
    }
    Ok(arc_store)
}

fn build_matrix_store(
    uri: &str,
    credential_lookup_key: &str,
    use_keychain: bool,
) -> Result<DynStore, String> {
    let (homeserver, user_id) = parse_matrix_store_uri(uri)?;
    let store = MatrixStore::new(homeserver, user_id, None, FRB_TOKIO.handle().clone())
        .map_err(|e| e.to_string())?;
    let arc_store: DynStore = Arc::new(store);
    let entry = load_credential_entry(credential_lookup_key, use_keychain)?;
    arc_store.set_credential(None, entry.password_or_token.as_str());
    Ok(arc_store)
}

fn build_store(
    uri: &str,
    credential_lookup_key: &str,
    use_keychain: bool,
) -> Result<DynStore, String> {
    if uri.starts_with("maildir:") {
        build_maildir_store(uri)
    } else if uri.starts_with("mbox:") {
        build_mbox_store(uri)
    } else if uri.starts_with("imap://") || uri.starts_with("imaps://") {
        build_imap_store(uri, credential_lookup_key, use_keychain)
    } else if uri.starts_with("nostr:") {
        build_nostr_store(uri, credential_lookup_key, use_keychain)
    } else if uri.starts_with("matrix:store:") {
        build_matrix_store(uri, credential_lookup_key, use_keychain)
    } else {
        Err(format!(
            "store type not supported for mail operations (got scheme from {uri:?})"
        ))
    }
}

pub(crate) fn nostr_send_chat_message(
    store_uri: &str,
    credential_key: &str,
    folder_recipient: &str,
    text: &str,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let store = open_store(uri, credential_lookup(uri, credential_key), use_keychain)?;
    let nostr = store
        .as_any()
        .downcast_ref::<NostrStore>()
        .ok_or_else(|| "sendChatMessage is only supported for Nostr accounts".to_string())?;
    let transport = nostr.paired_transport().map_err(|e| e.to_string())?;
    let mut payload = SendPayload::default();
    payload.to.push(Address {
        display_name: None,
        local_part: folder_recipient.trim().to_string(),
        domain: None,
    });
    payload.body_plain = Some(text.to_string());
    let (tx, rx) = mpsc::channel::<Result<(), StoreError>>();
    transport.send(&payload, Box::new(move |r| {
        let _ = tx.send(r);
    }));
    rx.recv()
        .map_err(|_| "Nostr send: internal channel closed".to_string())?
        .map_err(|e| e.to_string())
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

    let store_uri = cfg.accounts[idx].store_uri.clone();
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

    let (maybe_prof, nip65_relays, dm_relays) = FRB_TOKIO.block_on(async {
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
        invalidate_frb_store_cache(&store_uri, account_id, use_keychain);
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
        match FRB_TOKIO.block_on(publish_event(r, &event_json)) {
            Ok(()) => ok_any = true,
            Err(e) => last_err = Some(e),
        }
    }
    if !ok_any {
        return Err(last_err.unwrap_or_else(|| "failed to publish profile".to_string()));
    }
    Ok(())
}

fn build_maildir_store(store_uri: &str) -> Result<DynStore, String> {
    let u = Url::parse(store_uri).map_err(|e| format!("bad maildir URL: {e}"))?;
    if u.scheme() != "maildir" {
        return Err("not a maildir URL".to_owned());
    }
    let path_str = u.path();
    if path_str.is_empty() {
        return Err("maildir URL has no path".to_owned());
    }
    let root = PathBuf::from(path_str);
    let store = MaildirStore::new(&root).map_err(|e| e.to_string())?;
    Ok(Arc::new(store))
}

fn build_mbox_store(store_uri: &str) -> Result<DynStore, String> {
    let u = Url::parse(store_uri).map_err(|e| format!("bad mbox URL: {e}"))?;
    if u.scheme() != "mbox" {
        return Err("not an mbox URL".to_owned());
    }
    let path_str = u.path();
    if path_str.is_empty() {
        return Err("mbox URL has no path".to_owned());
    }
    let path = PathBuf::from(path_str);
    let store = MboxStore::new(&path).map_err(|e| e.to_string())?;
    Ok(Arc::new(store))
}

fn build_imap_store(
    store_uri: &str,
    credential_lookup_key: &str,
    use_keychain: bool,
) -> Result<DynStore, String> {
    let u = Url::parse(store_uri).map_err(|e| format!("bad IMAP URL: {e}"))?;
    let scheme = u.scheme();
    let use_implicit_tls = scheme == "imaps";
    let host = u
        .host_str()
        .ok_or_else(|| "IMAP URL missing host".to_owned())?
        .to_owned();
    let port = u.port().unwrap_or(if use_implicit_tls { 993 } else { 143 });

    let entry: CredentialEntry = load_credential_entry(credential_lookup_key, use_keychain)
        .map_err(|e| {
            if e.contains("no saved credential") {
                format!(
                    "no saved password for this IMAP account ({credential_lookup_key}). Add credentials in Tagliacarte (keychain or credentials file)."
                )
            } else {
                e
            }
        })?;

    // `url::Url::username()` is percent-encoded (e.g. `alice%40example.com`); SASL PLAIN needs the
    // real login string (`alice@example.com`). Same encoding is used when building store URIs in core (`uri.rs`).
    let user_from_url = percent_decode_str(u.username())
        .decode_utf8_lossy()
        .into_owned();
    let user = if user_from_url.is_empty() {
        entry.username.clone()
    } else {
        user_from_url
    };
    if user.is_empty() {
        return Err("IMAP username missing in URL and credentials".to_owned());
    }

    let mut imap = ImapStore::with_runtime_handle(host, port, FRB_TOKIO.handle().clone());
    if use_implicit_tls || port == 993 {
        imap.set_implicit_tls(true);
    }
    // Prefer PLAIN on TLS: if the server advertises AUTH=PLAIN we use it; otherwise the
    // client falls back to IMAP LOGIN. SCRAM-SHA-256 works on many hosts but some
    // servers advertise it yet reject our client flow while accepting PLAIN/LOGIN.
    imap.set_auth(user, entry.password_or_token.as_str(), SaslMechanism::Plain);
    Ok(Arc::new(imap))
}

/// Load full raw RFC 822 bytes for a message (blocking channel wait). Used by the mail body HTTP server.
pub(crate) fn blocking_get_message_raw(
    store_uri: &str,
    credential_key: &str,
    use_keychain: bool,
    folder_name: &str,
    message_id: &str,
) -> Result<Vec<u8>, String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    if uri.is_empty() {
        return Err("empty store URI".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, credential_key), use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;
    let raw_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let b2 = Arc::clone(&raw_buf);
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    folder.get_message(
        &MessageId::new(message_id.to_owned()),
        Box::new(|_| {}),
        Box::new(move |chunk| {
            b2.lock().expect("raw lock").extend_from_slice(chunk);
        }),
        Box::new(move |res| {
            let _ = tx.send(res.map_err(|e| e.to_string()));
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(())) => Ok(std::mem::take(&mut *raw_buf.lock().expect("raw lock"))),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timeout loading message (120s)".to_owned()),
    }
}

fn wait_open_folder(store: DynStore, folder_name: &str) -> Result<Box<dyn Folder>, String> {
    let name = folder_name.to_string();
    let (tx, rx) = mpsc::sync_channel(1);
    store.open_folder(
        &name,
        Box::new(|_ev: OpenFolderEvent| {}),
        Box::new(move |res| {
            let _ = tx.send(res);
        }),
    );
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(folder)) => Ok(folder),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("timeout opening folder (120s)".to_owned()),
    }
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

/// Newest-first paging: `skip` skips that many newest messages; `limit` caps how many to return.
fn list_range_for_page(store_uri: &str, total: u64, skip: u64, limit: u64) -> Option<Range<u64>> {
    if total == 0 || limit == 0 {
        return None;
    }
    let skip = skip.min(total);
    let limit = limit.max(1).min(10_000);

    if store_uri.starts_with("maildir:") {
        let end = total.saturating_sub(skip);
        if end == 0 {
            return None;
        }
        let start = end.saturating_sub(limit);
        if start >= end {
            return None;
        }
        Some(start..end)
    } else {
        let exists = total;
        let end_seq = exists.saturating_sub(skip);
        if end_seq == 0 {
            return None;
        }
        let start_seq = end_seq.saturating_sub(limit).saturating_add(1).max(1);
        let range_start = start_seq.saturating_sub(1);
        Some(range_start..end_seq)
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

fn conversation_to_message_summary(store_uri: &str, s: ConversationSummary) -> MessageSummaryJson {
    let is_nostr = store_uri.trim().starts_with("nostr:");
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

fn detail_from_display(store_uri: &str, m: MessageForDisplay) -> MessageDetailJson {
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
            .map(|a| format_address_maybe_nostr(store_uri, a))
            .unwrap_or_default(),
        to: format_address_list_maybe_nostr(store_uri, &env.to),
        cc: if env.cc.is_empty() {
            None
        } else {
            Some(format_address_list_maybe_nostr(store_uri, &env.cc))
        },
        date_ms: env.date.as_ref().map(|d| d.timestamp.saturating_mul(1000)),
        body_plain: m.body_plain,
        body_html: m.body_html,
        attachments,
    }
}

fn detail_from_env_and_body(
    store_uri: &str,
    env: &Envelope,
    body_plain: Option<String>,
    body_html: Option<String>,
) -> MessageDetailJson {
    MessageDetailJson {
        subject: env.subject.clone().unwrap_or_default(),
        from: env
            .from
            .first()
            .map(|a| format_address_maybe_nostr(store_uri, a))
            .unwrap_or_default(),
        to: format_address_list_maybe_nostr(store_uri, &env.to),
        cc: if env.cc.is_empty() {
            None
        } else {
            Some(format_address_list_maybe_nostr(store_uri, &env.cc))
        },
        date_ms: env.date.as_ref().map(|d| d.timestamp.saturating_mul(1000)),
        body_plain,
        body_html,
        attachments: vec![],
    }
}

fn format_address_maybe_nostr(store_uri: &str, a: &Address) -> String {
    if store_uri.trim().starts_with("nostr:") {
        let lp = a.local_part.trim().to_lowercase();
        if a.domain.as_deref().unwrap_or("").is_empty() && nostr_keys::is_valid_hex_key(&lp) {
            return crate::nostr_profile_cache::display_label_for_pubkey_hex(&lp);
        }
    }
    format_address(a)
}

fn format_address_list_maybe_nostr(store_uri: &str, v: &[Address]) -> String {
    v.iter()
        .map(|a| format_address_maybe_nostr(store_uri, a))
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

fn mail_store_identity(
    store_uri: &str,
    credential_key: &str,
    use_keychain: bool,
) -> (String, String, bool) {
    let u = store_uri.trim();
    (
        u.to_string(),
        credential_lookup(u, credential_key).to_string(),
        use_keychain,
    )
}

fn same_mail_store(
    a_uri: &str,
    a_key: &str,
    b_uri: &str,
    b_key: &str,
    use_keychain: bool,
) -> bool {
    mail_store_identity(a_uri, a_key, use_keychain) == mail_store_identity(b_uri, b_key, use_keychain)
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

/// Second IMAP connection for APPEND (pipeline client does not support literals yet).
fn blocking_imap_append(
    store_uri: &str,
    credential_lookup_key: &str,
    mailbox: &str,
    data: &[u8],
    use_keychain: bool,
) -> Result<(), String> {
    let u = Url::parse(store_uri).map_err(|e| format!("bad IMAP URL: {e}"))?;
    let scheme = u.scheme();
    let use_implicit_tls = scheme == "imaps";
    let host = u
        .host_str()
        .ok_or_else(|| "IMAP URL missing host".to_owned())?;
    let port = u.port().unwrap_or(if use_implicit_tls { 993 } else { 143 });

    let cred_path = resolve_credentials_file_path().ok_or_else(|| {
        "could not resolve credentials path (~/.tagliacarte/credentials)".to_owned()
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
    let entry: CredentialEntry = creds
        .get(credential_lookup_key)
        .cloned()
        .ok_or_else(|| {
            format!(
                "no saved password for this IMAP account ({credential_lookup_key}). Add credentials in Tagliacarte."
            )
        })?;

    let user_from_url = percent_decode_str(u.username())
        .decode_utf8_lossy()
        .into_owned();
    let user = if user_from_url.is_empty() {
        entry.username.clone()
    } else {
        user_from_url
    };
    if user.is_empty() {
        return Err("IMAP username missing in URL and credentials".to_owned());
    }

    let data = data.to_vec();
    let mailbox = mailbox.to_string();
    let host = host.to_string();
    FRB_TOKIO.handle().block_on(async move {
        let mut sess = connect_and_authenticate(
            host.as_str(),
            port,
            use_implicit_tls,
            true,
            Some((
                user.as_str(),
                entry.password_or_token.as_str(),
                SaslMechanism::Plain,
            )),
        )
        .await
        .map_err(|e| e.to_string())?;
        sess.append(mailbox.as_str(), &data)
            .await
            .map_err(|e| e.to_string())
    })
}

fn append_to_mail_folder(
    dest_uri: &str,
    dest_cred: &str,
    dest_folder: &str,
    raw: &[u8],
    use_keychain: bool,
) -> Result<(), String> {
    if dest_uri.starts_with("imap://") || dest_uri.starts_with("imaps://") {
        return blocking_imap_append(
            dest_uri,
            credential_lookup(dest_uri.trim(), dest_cred),
            dest_folder,
            raw,
            use_keychain,
        );
    }
    let store = open_store(
        dest_uri.trim(),
        credential_lookup(dest_uri.trim(), dest_cred),
        use_keychain,
    )?;
    let folder = wait_open_folder(store, dest_folder)?;
    wait_folder_append(folder.as_ref(), raw)
}

/// Copy or move messages. Per-message results; on cross-store **move**, only successful appends are deleted from source.
pub(crate) fn transfer_mail_messages_json(
    source_store_uri: String,
    source_credential_key: String,
    source_folder: String,
    dest_store_uri: String,
    dest_credential_key: String,
    dest_folder: String,
    message_ids: Vec<String>,
    is_move: bool,
    use_keychain: bool,
) -> Result<String, String> {
    set_credentials_backend(use_keychain);
    let src_uri = source_store_uri.trim();
    let src_folder = source_folder.trim();
    let dst_uri = dest_store_uri.trim();
    let dst_folder = dest_folder.trim();
    if src_uri.is_empty() || src_folder.is_empty() || dst_uri.is_empty() || dst_folder.is_empty() {
        return Err("empty store URI or folder".to_owned());
    }
    if message_ids.is_empty() {
        return Err("no message ids".to_owned());
    }
    if same_mail_store(
        src_uri,
        &source_credential_key,
        dst_uri,
        &dest_credential_key,
        use_keychain,
    ) && src_folder == dst_folder
    {
        return Err("source and destination folder are the same".to_owned());
    }

    let mut results: Vec<TransferOneResult> = Vec::with_capacity(message_ids.len());

    if same_mail_store(
        src_uri,
        &source_credential_key,
        dst_uri,
        &dest_credential_key,
        use_keychain,
    ) {
        let store = open_store(
            src_uri,
            credential_lookup(src_uri, &source_credential_key),
            use_keychain,
        )?;
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
        let src_ck = credential_lookup(src_uri, &source_credential_key).to_string();
        let dst_ck = credential_lookup(dst_uri, &dest_credential_key).to_string();
        for id in message_ids {
            let raw = match blocking_get_message_raw(
                src_uri,
                &src_ck,
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
            match append_to_mail_folder(dst_uri, &dst_ck, dst_folder, &raw, use_keychain) {
                Ok(()) => {
                    if is_move {
                        let store = open_store(src_uri, src_ck.as_str(), use_keychain)?;
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
    store_uri: String,
    credential_key: String,
    folder_name: String,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let uri = store_uri.trim();
    let folder_name = folder_name.trim();
    if uri.is_empty() || folder_name.is_empty() {
        return Err("empty store URI or folder name".to_owned());
    }
    if !uri.starts_with("imap://") && !uri.starts_with("imaps://") {
        return Err("expunge is only supported for IMAP folders".to_owned());
    }
    let store = open_store(uri, credential_lookup(uri, &credential_key), use_keychain)?;
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
struct FrbSmtpComposeJson {
    from: String,
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    subject: String,
    body_plain: String,
    body_html: Option<String>,
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
    if !transport.transport_type.eq_ignore_ascii_case("smtp") {
        return Err(format!(
            "transport {:?} has type {:?}, not smtp",
            transport.id, transport.transport_type
        ));
    }
    let host = transport.host.trim();
    if host.is_empty() {
        return Err(format!(
            "transport {:?} has empty host in config",
            transport.id
        ));
    }
    let port = transport.port;
    let (use_implicit_tls, use_starttls) = smtp_tls_mode(transport.security.as_str());

    let draft: FrbSmtpComposeJson =
        serde_json::from_str(compose_json).map_err(|e| format!("compose JSON: {e}"))?;

    let payload = SendPayload {
        from: smtp_parse_addrs(draft.from),
        to: draft.to.into_iter().flat_map(smtp_parse_addrs).collect(),
        cc: draft.cc.into_iter().flat_map(smtp_parse_addrs).collect(),
        bcc: draft.bcc.into_iter().flat_map(smtp_parse_addrs).collect(),
        subject: Some(draft.subject),
        body_plain: Some(draft.body_plain),
        body_html: draft.body_html,
        attachments: vec![],
        newsgroups: vec![],
    };

    if payload.from.len() != 1 {
        return Err("compose \"from\" must be exactly one address".to_owned());
    }
    if payload.to.is_empty() {
        return Err("compose \"to\" must include at least one address".to_owned());
    }

    set_credentials_backend(use_keychain);
    let entry = load_credential_entry(transport.id.trim(), use_keychain)?;
    let user = entry.username.trim();
    if user.is_empty() {
        return Err(
            "no SMTP username in saved credentials for this transport; save username + password"
                .to_owned(),
        );
    }

    let (message, mut envelope) = build_rfc822_from_payload(&payload);
    envelope.cc.extend(payload.bcc.iter().cloned());

    let auth = Some((
        user.to_string(),
        entry.password_or_token.clone(),
        SaslMechanism::Plain,
    ));

    frb_runtime_handle()
        .block_on(async {
            send_message_async(
                host,
                port,
                use_implicit_tls,
                use_starttls,
                auth.as_ref().map(|(u, p, m)| (u.as_str(), p.as_str(), *m)),
                "localhost",
                message.as_slice(),
                &envelope,
            )
            .await
        })
        .map_err(|e: SmtpClientError| e.to_string())
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
