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

#![allow(dead_code)]
// Legacy JsonWriter helpers retained for occasional debugging; primary API is typed FRB structs.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use base64::Engine;
use tagliacarte_core::json::{JsonNumber, JsonWriter, writer_into_string};
use tagliacarte_core::oauth::OAuthProvider;
use tagliacarte_core::config::{
    tagliacarte_data_dir, CredentialEntry, load_credentials, resolve_credentials_file_path,
    save_credential, set_credentials_backend,
};
use tagliacarte_core::message_id::MessageId;
use tagliacarte_core::mime::{
    extract_structured_body, parse_envelope, utf8_body_after_rfc822_headers,
};
use tagliacarte_core::protocol::imap::ImapStore;
use tagliacarte_core::protocol::gmail::GmailTransport;
use tagliacarte_core::protocol::nntp::{NntpStore, NntpTransport};
use tagliacarte_core::protocol::imap::trace as imap_trace;
use tagliacarte_core::protocol::smtp::{
    build_rfc822_from_payload, generate_smtp_message_id_angle_from_from_field,
    normalize_smtp_message_id_angle, probe_smtp_ehlo_auth_methods, send_message_async,
    verify_smtp_async, SmtpClientError,
};
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
use tagliacarte_core::sasl::{
    pick_first_smtp_auth_for_credentials, smtp_auth_advertised, SaslMechanism,
};
use tagliacarte_core::store::{
    sort_conversation_summaries_for_window, Address, Attachment, ConversationSummary, Envelope,
    Flag, Folder, MessageForDisplay, SendPayload, Transport,
};

use crate::contacts_store;
use crate::mail_kind::{
    is_imap_like_store, is_matrix_store, is_nostr_store, normalize_store_type,
    uses_long_imap_fetch_timeout,
};
use crate::mail_store::{load_mail_credential, resolve_xoauth_secret};
use super::resolve_mail_account;
use super::FrbTransport;
use crate::mail_store::{
    self, blocking_get_message_raw as mail_blocking_get_message_raw, blocking_imap_append,
    invalidate_mail_store_cache, list_range_for_page_backend, mail_runtime_handle,
    open_cached_store, same_mail_store as accounts_same_mail_store, wait_open_folder,
    AvailableFolderRow,
};
use super::FrbAccount;

pub(crate) fn frb_runtime_handle() -> tokio::runtime::Handle {
    mail_runtime_handle()
}

#[derive(Debug, Clone)]
pub struct FrbFolderUnreadCount {
    pub folder_name: String,
    pub unread: u64,
}

#[derive(Debug, Clone)]
pub struct FrbMailSubscriptionAvailableRow {
    pub id: String,
    pub is_subscribed: bool,
    pub display_name: Option<String>,
    pub unread: Option<u64>,
    pub allow_unsubscribe: bool,
}

#[derive(Debug, Clone)]
pub struct ListMailFoldersResult {
    pub folders: Vec<String>,
    pub hierarchy_delimiter: Option<String>,
    pub folder_unread_counts: Vec<FrbFolderUnreadCount>,
    pub folder_display_names: std::collections::HashMap<String, String>,
    pub subscription_available: Option<Vec<FrbMailSubscriptionAvailableRow>>,
}

pub fn list_mail_folders_result(
    acc: FrbAccount,
    use_keychain: bool,
) -> Result<ListMailFoldersResult, String> {
    let snap = mail_store::list_mail_folders_snapshot_with_progress(&acc, use_keychain, |_, _| {})
        .map_err(|e| {
        eprintln!("[mail] list_mail_folders: {e}");
        e
    })?;
    let folder_unread_counts: Vec<FrbFolderUnreadCount> = snap
        .folders
        .iter()
        .map(|name| FrbFolderUnreadCount {
            folder_name: name.clone(),
            unread: snap.unread_by_folder.get(name).copied().unwrap_or(0) as u64,
        })
        .collect();
    let subscription_available = snap.subscription_pane.as_ref().map(|p| {
        p.available
            .iter()
            .map(|r| FrbMailSubscriptionAvailableRow {
                id: r.id.clone(),
                is_subscribed: r.is_subscribed,
                display_name: r.display_name.clone(),
                unread: r.unread.map(|u| u as u64),
                allow_unsubscribe: r.allow_unsubscribe,
            })
            .collect()
    });
    Ok(ListMailFoldersResult {
        folders: snap.folders,
        hierarchy_delimiter: snap.hierarchy_delimiter,
        folder_unread_counts,
        folder_display_names: snap.folder_display_names,
        subscription_available,
    })
}

/// JSON array of `{id, isSubscribed, displayName?, unread?, allowUnsubscribe}`.
pub(crate) fn subscription_available_array_json(rows: &[AvailableFolderRow]) -> String {
    let mut w = JsonWriter::new();
    w.write_start_array();
    for row in rows {
        w.write_start_object();
        w.write_key("id");
        w.write_string(&row.id);
        w.write_key("isSubscribed");
        w.write_bool(row.is_subscribed);
        if let Some(ref d) = row.display_name {
            w.write_key("displayName");
            w.write_string(d);
        }
        if let Some(u) = row.unread {
            w.write_key("unread");
            w.write_number(JsonNumber::I64(i64::from(u)));
        }
        w.write_key("allowUnsubscribe");
        w.write_bool(row.allow_unsubscribe);
        w.write_end_object();
    }
    w.write_end_array();
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
    visible_first_rank: Option<u64>,
    visible_last_rank: Option<u64>,
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
    let is_gmail_rest = normalize_store_type(acc.backend_type.as_str()) == "gmail";
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

    // Matrix (and similar) backends may report message_count = 0 while still having timeline
    // events; the window path must still call list_conversations.
    let matrix_style = is_matrix_store(backend) && total == 0;

    let window_range_oldest_first =
        (!matrix_style).then(|| folder_range_for_indices(total, start_index, limit)).flatten();

    if !matrix_style && window_range_oldest_first.is_none() {
        return Ok(ListFolderMessagesWindowResponse {
            total,
            start_index,
            messages: vec![],
            list_strategy: "fullScan".to_owned(),
        });
    }

    let fetch_range = if matrix_style {
        let n = (start_index.saturating_add(limit))
            .max(limit)
            .min(500)
            .max(1);
        0..n
    } else if is_gmail_rest {
        window_range_oldest_first
            .expect("gmail window: range validated above")
    } else {
        0..total
    };

    let collected = Arc::new(Mutex::new(Vec::<ConversationSummary>::new()));
    let c2 = Arc::clone(&collected);
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);

    let visible_ranks_inclusive = match (visible_first_rank, visible_last_rank) {
        (Some(lo), Some(hi)) if lo <= hi => Some((lo, hi)),
        _ => None,
    };

    if is_gmail_rest {
        folder.list_conversations_with_visible_ranks(
            fetch_range.clone(),
            visible_ranks_inclusive,
            Box::new(move |s| {
                c2.lock().expect("summary lock").push(s);
            }),
            Box::new(move |res| {
                let _ = tx.send(res.map_err(|e| e.to_string()));
            }),
        );
    } else {
        folder.list_conversations(
            fetch_range,
            Box::new(move |s| {
                c2.lock().expect("summary lock").push(s);
            }),
            Box::new(move |res| {
                let _ = tx.send(res.map_err(|e| e.to_string()));
            }),
        );
    }

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
    let loaded = all.len() as u64;
    let total_out = if matrix_style { loaded } else { total };
    // Windowed fetch (Gmail): `all` is only `[start_index .. start_index+limit)`; slice from 0.
    let start = if is_gmail_rest {
        0usize
    } else {
        start_index as usize
    };
    if start >= all.len() {
        return Ok(ListFolderMessagesWindowResponse {
            total: total_out,
            start_index,
            messages: vec![],
            list_strategy: "fullScan".to_owned(),
        });
    }
    let slice_end = (start + limit as usize).min(all.len());
    let messages: Vec<MessageSummaryJson> = all[start..slice_end]
        .iter()
        .cloned()
        .map(|s| conversation_to_message_summary(backend, s))
        .collect();
    Ok(ListFolderMessagesWindowResponse {
        total: total_out,
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
        None,
        None,
    )?;
    Ok(format_list_folder_messages_window_response(&r))
}

fn list_folder_messages_summaries(
    acc: FrbAccount,
    folder_name: String,
    skip: u64,
    limit: u64,
    use_keychain: bool,
) -> Result<Vec<MessageSummaryJson>, String> {
    set_credentials_backend(use_keychain);
    let folder_name = folder_name.trim();
    if folder_name.is_empty() {
        return Err("empty folder name".to_owned());
    }
    let backend = acc.backend_type.clone();
    let store = open_cached_store(&acc, use_keychain)?;
    let folder = wait_open_folder(store, folder_name)?;
    let total = wait_message_count(folder.as_ref())?;
    let range = if is_matrix_store(backend.as_str()) && total == 0 {
        let n = skip.saturating_add(limit).max(limit).min(500).max(1);
        0..n
    } else {
        let Some(r) = list_range_for_page_backend(backend.as_str(), total, skip, limit) else {
            return Ok(vec![]);
        };
        r
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
    Ok(out)
}

pub(crate) fn list_folder_messages_json(
    acc: FrbAccount,
    folder_name: String,
    skip: u64,
    limit: u64,
    use_keychain: bool,
) -> Result<String, String> {
    let out = list_folder_messages_summaries(acc, folder_name, skip, limit, use_keychain)?;
    Ok(format_message_summary_array(&out))
}

pub fn list_folder_messages_result(
    acc: FrbAccount,
    folder_name: String,
    skip: u64,
    limit: u64,
    use_keychain: bool,
) -> Result<ListFolderMessagesResult, String> {
    let rows = list_folder_messages_summaries(acc, folder_name, skip, limit, use_keychain)?;
    Ok(ListFolderMessagesResult {
        messages: rows
            .iter()
            .map(MessageSummaryJson::to_frb_message_summary)
            .collect(),
    })
}

fn message_detail_json_to_frb(d: MessageDetailJson) -> FrbFolderMessageDetail {
    FrbFolderMessageDetail {
        subject: d.subject,
        from: d.from,
        to: d.to,
        cc: d.cc,
        date_ms: d.date_ms,
        message_id: d.message_id,
        references: d.references,
        body_plain: d.body_plain,
        body_html: d.body_html,
        attachments: d
            .attachments
            .into_iter()
            .map(|a| FrbMessageAttachmentDetail {
                filename: a.filename,
                content_type: a.content_type,
                size_bytes: a.size_bytes,
                transfer_encoding: a.transfer_encoding,
                imap_section: a.imap_section,
                content_id: a.content_id,
                data_base64: a.data_base64,
            })
            .collect(),
    }
}

pub(crate) fn get_folder_message_detail(
    acc: FrbAccount,
    folder_name: String,
    message_id: String,
    use_keychain: bool,
) -> Result<FrbFolderMessageDetail, String> {
    let d = load_folder_message_detail_json(acc, folder_name, message_id, use_keychain)?;
    Ok(message_detail_json_to_frb(d))
}

pub(crate) fn get_folder_message_json(
    acc: FrbAccount,
    folder_name: String,
    message_id: String,
    use_keychain: bool,
) -> Result<String, String> {
    let d = load_folder_message_detail_json(acc, folder_name, message_id, use_keychain)?;
    Ok(format_message_detail(&d))
}

fn load_folder_message_detail_json(
    acc: FrbAccount,
    folder_name: String,
    message_id: String,
    use_keychain: bool,
) -> Result<MessageDetailJson, String> {
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
            Ok(detail_from_display(is_nostr, display))
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
) -> Result<FrbFetchedMessagePart, String> {
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
            Ok(FrbFetchedMessagePart {
                bytes_base64: b64,
            })
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
) -> Result<MessageDetailJson, String> {
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

    let raw_for_extract = if let Some(path) = super::config_path_for_relay_lookup() {
        let cfg = super::load_frb_config_struct(path.as_str());
        crate::mail_crypto::prepare_incoming_rfc822_for_display(&raw, &cfg).unwrap_or_else(|_| raw.clone())
    } else {
        raw.clone()
    };

    let (plain, html, _) = match extract_structured_body(&raw_for_extract) {
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
    Ok(detail)
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

fn smtp_credential_required_message(tid: &str, auth_methods: &[String]) -> String {
    let tail = if auth_methods.is_empty() {
        "server did not advertise SMTP AUTH mechanisms in EHLO".to_string()
    } else {
        format!("server EHLO AUTH: {}", auth_methods.join(", "))
    };
    format!("credential required for SMTP transport {tid} ({tail})")
}

/// Picks SASL from EHLO `AUTH` list and vault; returns `(auth triple, copy_store_oauth_to_transport)`.
fn smtp_resolve_auth_from_ehlo(
    transport: &super::FrbTransport,
    use_keychain: bool,
    store_account_id: Option<&str>,
    auth_methods: &[String],
) -> Result<(Option<(String, String, SaslMechanism)>, bool), String> {
    let tid = transport.id.trim();
    let oauth_attr = transport.oauth_provider.trim();

    if auth_methods.is_empty() {
        return Ok((None, false));
    }

    let entry_t = load_credential_entry(tid, use_keychain).ok();
    let transport_has_oauth_json = entry_t.as_ref().is_some_and(|e| {
        e.password_or_token.trim().starts_with('{')
    });

    let mut xoauth: Option<(String, String)> = None;
    let mut oauth_from_store = false;

    if let Some(ref e) = entry_t {
        let secret = e.password_or_token.trim();
        if secret.starts_with('{') {
            if let Some(kind) = mail_store::oauth_service_for_smtp_credential(secret, oauth_attr) {
                if let Ok(pair) = resolve_xoauth_secret(kind, tid, use_keychain) {
                    xoauth = Some(pair);
                }
            }
        }
    }

    if xoauth.is_none() {
        if let Some(sid) = store_account_id {
            let sid = sid.trim();
            if !sid.is_empty() {
                let store_attr = resolve_mail_account(sid)
                    .ok()
                    .and_then(|(acc, _)| acc.attrs.get("oauthProvider").cloned())
                    .unwrap_or_default();
                if let Ok(e) = load_credential_entry(sid, use_keychain) {
                    let secret = e.password_or_token.trim();
                    if secret.starts_with('{') {
                        if let Some(kind) =
                            mail_store::oauth_service_for_smtp_credential(secret, store_attr.as_str())
                        {
                            if let Ok(pair) = resolve_xoauth_secret(kind, sid, use_keychain) {
                                xoauth = Some(pair);
                                oauth_from_store = true;
                            }
                        }
                    }
                }
            }
        }
    }

    let password_auth: Option<(String, String)> = entry_t.as_ref().and_then(|e| {
        let u = e.username.trim();
        let p = e.password_or_token.trim();
        if u.is_empty() || p.is_empty() || p.starts_with('{') {
            None
        } else {
            Some((u.to_string(), p.to_string()))
        }
    });

    let oauth_ready = xoauth.is_some();
    let have_password = password_auth.is_some();
    let wants_xoauth = smtp_auth_advertised(auth_methods, SaslMechanism::XOAuth2);
    let wants_password_mech = auth_methods.iter().any(|t| {
        SaslMechanism::from_name(t.as_str()).is_some_and(|m| m != SaslMechanism::XOAuth2)
    });

    if let Some(chosen) = pick_first_smtp_auth_for_credentials(
        auth_methods,
        oauth_ready,
        have_password,
    ) {
        return match chosen {
            SaslMechanism::XOAuth2 => {
                let (u, t) = xoauth.unwrap();
                let copy = oauth_from_store && !transport_has_oauth_json;
                Ok((Some((u, t, SaslMechanism::XOAuth2)), copy))
            }
            m => {
                let (u, p) = password_auth.as_ref().unwrap();
                Ok((Some((u.clone(), p.clone(), m)), false))
            }
        };
    }

    if oauth_ready && !wants_xoauth {
        return Err(format!(
            "SMTP transport {tid}: OAuth token is stored but the server EHLO does not list XOAUTH2 ({})",
            auth_methods.join(", ")
        ));
    }

    if have_password && !wants_password_mech {
        return Err(format!(
            "SMTP transport {tid}: password is stored but the server EHLO lists no LOGIN/PLAIN/SCRAM-SHA-256/CRAM-MD5 ({})",
            auth_methods.join(", ")
        ));
    }

    Err(smtp_credential_required_message(tid, auth_methods))
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
    let mut w = JsonWriter::new();
    w.write_start_object();
    if !acc.label.trim().is_empty() {
        w.write_key("name");
        w.write_string(acc.label.trim());
    }
    if let Some(n5) = acc.attrs.get("nip05") {
        let t = n5.trim();
        if !t.is_empty() {
            w.write_key("nip05");
            w.write_string(t);
        }
    }
    if let Some(ref u) = acc.avatar_url {
        let t = u.trim();
        if !t.is_empty() {
            w.write_key("picture");
            w.write_string(t);
        }
    }
    w.write_end_object();
    let content_str = writer_into_string(w);
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

/// Message row summary for list APIs (same data as [MessageSummaryJson]).
#[derive(Debug, Clone)]
pub struct FrbMessageSummary {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub date_ms: Option<i64>,
    pub is_read: bool,
    pub marked_for_deletion: bool,
    pub nostr_sender_pubkey_hex: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListFolderMessagesWindowResult {
    pub total: u64,
    pub start_index: u64,
    pub list_strategy: String,
    pub messages: Vec<FrbMessageSummary>,
}

/// Result of [list_folder_messages_json] / [list_folder_messages_result]: newest-first page.
#[derive(Debug, Clone)]
pub struct ListFolderMessagesResult {
    pub messages: Vec<FrbMessageSummary>,
}

#[derive(Debug, Clone)]
pub struct FrbMessageAttachmentDetail {
    pub filename: Option<String>,
    pub content_type: String,
    pub size_bytes: u64,
    pub transfer_encoding: String,
    pub imap_section: Option<String>,
    pub content_id: Option<String>,
    pub data_base64: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrbFolderMessageDetail {
    pub subject: String,
    pub from: String,
    pub to: String,
    pub cc: Option<String>,
    pub date_ms: Option<i64>,
    pub message_id: Option<String>,
    pub references: Option<String>,
    pub body_plain: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<FrbMessageAttachmentDetail>,
}

#[derive(Debug, Clone)]
pub struct FrbMailOperationItem {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrbBatchMailOperationResult {
    pub results: Vec<FrbMailOperationItem>,
    pub ok_count: u64,
    pub failed_count: u64,
}

#[derive(Debug, Clone)]
pub struct FrbFetchedMessagePart {
    pub bytes_base64: String,
}

/// Outgoing SMTP / Gmail REST / IMAP draft save (camelCase in Dart).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrbComposeMessage {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_plain: String,
    pub body_html: Option<String>,
    pub attachments: Vec<FrbComposeAttachment>,
    pub dsn_notify: Option<String>,
    pub store_account_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub message_id: Option<String>,
    /// `none` | `sign` | `encrypt` | `sign_encrypt` (camelCase `cryptoMode` in JSON); legacy
    /// `smime_sign` / `pgp_sign` / `*_sign_encrypt` are accepted and normalized.
    #[serde(default)]
    pub crypto_mode: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrbComposeAttachment {
    pub filename: Option<String>,
    pub mime_type: String,
    pub bytes_base64: String,
}

/// NNTP post payload (camelCase in Dart).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrbNntpComposeMessage {
    pub from: String,
    pub newsgroups: Vec<String>,
    pub subject: String,
    pub body_plain: String,
    pub attachments: Vec<FrbComposeAttachment>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
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

impl MessageSummaryJson {
    pub(crate) fn to_message_list_row_summary(&self) -> crate::session::MessageListRowSummary {
        crate::session::MessageListRowSummary {
            id: self.id.clone(),
            from: self.from.clone(),
            subject: self.subject.clone(),
            date_ms: self.date_ms,
            is_read: self.is_read,
            marked_for_deletion: self.marked_for_deletion,
            nostr_sender_pubkey_hex: self.nostr_sender_pubkey_hex.clone(),
        }
    }

    fn to_frb_message_summary(&self) -> FrbMessageSummary {
        FrbMessageSummary {
            id: self.id.clone(),
            from: self.from.clone(),
            subject: self.subject.clone(),
            date_ms: self.date_ms,
            is_read: self.is_read,
            marked_for_deletion: self.marked_for_deletion,
            nostr_sender_pubkey_hex: self.nostr_sender_pubkey_hex.clone(),
        }
    }
}

pub fn list_folder_messages_window_result(
    acc: FrbAccount,
    folder_name: String,
    start_index: u64,
    limit: u64,
    message_list_sort: String,
    use_keychain: bool,
) -> Result<ListFolderMessagesWindowResult, String> {
    let r = list_folder_messages_window_response(
        acc,
        folder_name,
        start_index,
        limit,
        message_list_sort,
        use_keychain,
        None,
        None,
    )?;
    Ok(ListFolderMessagesWindowResult {
        total: r.total(),
        start_index: r.start_index(),
        list_strategy: r.list_strategy().to_string(),
        messages: r
            .messages
            .iter()
            .map(MessageSummaryJson::to_frb_message_summary)
            .collect(),
    })
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

    pub(crate) fn for_each_row(
        &self,
        mut on_row: impl FnMut(u64, crate::session::MessageListRowSummary),
    ) {
        for (i, m) in self.messages.iter().enumerate() {
            let rank = self.start_index.saturating_add(i as u64);
            on_row(rank, m.to_message_list_row_summary());
        }
    }
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
    /// RFC 5322 References from the source message (for reply threading).
    references: Option<String>,
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
    if let Some(ref s) = d.references {
        w.write_key("references");
        w.write_string(s);
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
        references: env.references.clone(),
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
        references: env.references.clone(),
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

struct TransferMailMessagesResponse {
    results: Vec<FrbMailOperationItem>,
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

fn batch_mail_op_result_from_transfer(r: TransferMailMessagesResponse) -> FrbBatchMailOperationResult {
    FrbBatchMailOperationResult {
        results: r.results,
        ok_count: r.ok_count as u64,
        failed_count: r.failed_count as u64,
    }
}

/// Delete messages in one folder. IMAP uses [FrbAccount] attrs `imapDeleteMode` and
/// `imapTrashFolderName` (see [crate::mail_store::apply_imap_delete_config_from_account]).
pub(crate) fn delete_mail_messages_result(
    acc: FrbAccount,
    folder_name: String,
    message_ids: Vec<String>,
    use_keychain: bool,
) -> Result<FrbBatchMailOperationResult, String> {
    set_credentials_backend(use_keychain);
    let folder = folder_name.trim();
    if folder.is_empty() {
        return Err("empty folder".to_owned());
    }
    if message_ids.is_empty() {
        return Err("no message ids".to_owned());
    }
    let store = open_cached_store(&acc, use_keychain)?;
    crate::mail_store::apply_imap_delete_config_from_account(&acc, &store);
    crate::mail_store::apply_maildir_mailbox_config_from_account(&acc, &store);
    let folder_obj = wait_open_folder(store, folder)?;
    let mut results: Vec<FrbMailOperationItem> = Vec::with_capacity(message_ids.len());
    for id in message_ids {
        let mid = MessageId::new(id.clone());
        let r = wait_folder_delete(folder_obj.as_ref(), &mid);
        results.push(match r {
            Ok(()) => FrbMailOperationItem {
                id,
                ok: true,
                error: None,
            },
            Err(e) => FrbMailOperationItem {
                id,
                ok: false,
                error: Some(e),
            },
        });
    }
    let ok_count = results.iter().filter(|r| r.ok).count();
    let failed_count = results.len() - ok_count;
    Ok(batch_mail_op_result_from_transfer(TransferMailMessagesResponse {
        results,
        ok_count,
        failed_count,
    }))
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
pub(crate) fn transfer_mail_messages_result(
    source: FrbAccount,
    source_folder: String,
    dest: FrbAccount,
    dest_folder: String,
    message_ids: Vec<String>,
    is_move: bool,
    use_keychain: bool,
) -> Result<FrbBatchMailOperationResult, String> {
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

    let mut results: Vec<FrbMailOperationItem> = Vec::with_capacity(message_ids.len());

    if accounts_same_mail_store(&source, &dest) {
        let store = open_cached_store(&source, use_keychain)?;
        crate::mail_store::apply_imap_delete_config_from_account(&source, &store);
        crate::mail_store::apply_maildir_mailbox_config_from_account(&source, &store);
        let folder = wait_open_folder(store, src_folder)?;
        for id in message_ids {
            let r = if is_move {
                wait_folder_move_one(folder.as_ref(), id.as_str(), dst_folder)
            } else {
                wait_folder_copy_one(folder.as_ref(), id.as_str(), dst_folder)
            };
            match r {
                Ok(()) => results.push(FrbMailOperationItem {
                    id: id.clone(),
                    ok: true,
                    error: None,
                }),
                Err(e) => results.push(FrbMailOperationItem {
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
                    results.push(FrbMailOperationItem {
                        id: id.clone(),
                        ok: false,
                        error: Some("empty message body".to_owned()),
                    });
                    continue;
                }
                Err(e) => {
                    results.push(FrbMailOperationItem {
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
                            Ok(()) => results.push(FrbMailOperationItem {
                                id: id.clone(),
                                ok: true,
                                error: None,
                            }),
                            Err(e) => results.push(FrbMailOperationItem {
                                id: id.clone(),
                                ok: false,
                                error: Some(format!("appended to destination but source delete failed: {e}")),
                            }),
                        }
                    } else {
                        results.push(FrbMailOperationItem {
                            id: id.clone(),
                            ok: true,
                            error: None,
                        });
                    }
                }
                Err(e) => results.push(FrbMailOperationItem {
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
    Ok(batch_mail_op_result_from_transfer(out))
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

/// Build recipient list and optional contacts DB handle for mail crypto (encrypt / encrypt-to-self).
fn apply_mail_crypto_for_send(
    inner: &[u8],
    draft: &FrbComposeMessage,
    payload: &SendPayload,
    cfg: &super::FrbConfig,
) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let mut rcpt_addrs: Vec<Address> = Vec::with_capacity(
        payload.to.len() + payload.cc.len() + payload.bcc.len(),
    );
    rcpt_addrs.extend(payload.to.iter().cloned());
    rcpt_addrs.extend(payload.cc.iter().cloned());
    rcpt_addrs.extend(payload.bcc.iter().cloned());
    let emails = crate::mail_crypto::recipient_emails_from_addresses(&rcpt_addrs);
    let conn = tagliacarte_data_dir().and_then(|d| contacts_store::open_contacts_db(&d).ok());
    let from_norm = payload
        .from
        .first()
        .map(|a| crate::contacts_crypto::normalize_email_addr(a));
    let ctx = crate::mail_crypto::OutgoingCryptoCtx {
        contacts: conn.as_ref(),
        recipient_emails: &emails,
        from_email: from_norm.as_deref(),
    };
    crate::mail_crypto::apply_outgoing_mime_crypto(
        inner,
        draft.crypto_mode.as_deref(),
        cfg,
        &ctx,
    )
}

/// Gmail REST send using the account's embedded Gmail transport (no SMTP transport row).
pub(crate) fn send_gmail_json(
    acc: &FrbAccount,
    use_keychain: bool,
    compose: &FrbComposeMessage,
) -> Result<(), String> {
    if normalize_store_type(acc.backend_type.as_str()) != "gmail" {
        return Err(format!(
            "account {:?} has type {:?}, expected gmail",
            acc.id, acc.backend_type
        ));
    }
    let draft = compose;
    let mut attachments: Vec<Attachment> = Vec::with_capacity(draft.attachments.len());
    for a in &draft.attachments {
        let raw = a.bytes_base64.trim();
        if raw.is_empty() {
            continue;
        }
        let content = base64::engine::general_purpose::STANDARD
            .decode(raw.as_bytes())
            .map_err(|e| format!("attachment base64: {e}"))?;
        let mime_type = a.mime_type.trim();
        attachments.push(Attachment {
            filename: a.filename.clone(),
            mime_type: if mime_type.is_empty() {
                "application/octet-stream".to_owned()
            } else {
                mime_type.to_owned()
            },
            content,
        });
    }
    let payload = SendPayload {
        from: smtp_parse_addrs(draft.from.clone()),
        to: draft.to.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        cc: draft.cc.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        bcc: draft.bcc.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        subject: Some(draft.subject.clone()),
        body_plain: Some(draft.body_plain.clone()),
        body_html: draft.body_html.clone(),
        attachments,
        newsgroups: vec![],
        nntp_in_reply_to: None,
        nntp_references: None,
        smtp_notify: None,
        smtp_in_reply_to: draft
            .in_reply_to
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        smtp_references: draft
            .references
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        smtp_message_id: draft
            .message_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    };
    if payload.from.len() != 1 {
        return Err("compose \"from\" must be exactly one address".to_owned());
    }
    if payload.to.is_empty() {
        return Err("compose \"to\" must include at least one address".to_owned());
    }
    let email = acc
        .attrs
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| payload.from[0].local_part.clone());
    let google_oauth = crate::mail_store::google_oauth_provider();
    let tr = GmailTransport::new(
        email,
        acc.id.trim(),
        use_keychain,
        google_oauth.client_id().to_string(),
        google_oauth
            .client_secret()
            .unwrap_or("")
            .to_string(),
        frb_runtime_handle(),
    )
    .map_err(|e| e.to_string())?;
    let (mut raw, _) = build_rfc822_from_payload(&payload);
    let crypto_cfg = {
        let path = super::config_path_for_relay_lookup().ok_or_else(|| {
            "config path not registered; load settings before sending".to_owned()
        })?;
        super::load_frb_config_struct(path.as_str())
    };
    let (wire, _sent) =
        apply_mail_crypto_for_send(raw.as_slice(), draft, &payload, &crypto_cfg)?;
    raw = wire;
    let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
    tr.send_prebuilt_rfc822(raw.as_slice(), Box::new(move |r| {
        let _ = tx.send(r.map_err(|e| e.to_string()));
    }));
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(r) => r,
        Err(_) => Err("timeout waiting for Gmail REST send (120s)".to_owned()),
    }
}

/// SMTP send for one `<transport id="…">` row; [transport] comes from config (host/port/security).
pub(crate) fn send_smtp_json(
    transport: &super::FrbTransport,
    use_keychain: bool,
    compose: &FrbComposeMessage,
) -> Result<(), String> {
    let tt = transport.transport_type.trim();
    if !tt.eq_ignore_ascii_case("smtp") && !tt.eq_ignore_ascii_case("gmail") {
        return Err(format!(
            "transport {:?} has type {:?}, expected smtp or gmail",
            transport.id, transport.transport_type
        ));
    }

    let host = transport.host.trim().to_string();
    if host.is_empty() {
        return Err(format!(
            "transport {:?} has empty host in config",
            transport.id
        ));
    }

    let port = transport.port;

    let (use_implicit_tls, use_starttls) = if port == 465 {
        (true, false)
    } else {
        smtp_tls_mode(transport.security.as_str())
    };

    let draft = compose;

    let mut attachments: Vec<Attachment> = Vec::with_capacity(draft.attachments.len());
    for a in &draft.attachments {
        let raw = a.bytes_base64.trim();
        if raw.is_empty() {
            continue;
        }
        let content = base64::engine::general_purpose::STANDARD
            .decode(raw.as_bytes())
            .map_err(|e| format!("attachment base64: {e}"))?;
        let mime_type = a.mime_type.trim();
        attachments.push(Attachment {
            filename: a.filename.clone(),
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

    let smtp_in_reply_to = draft
        .in_reply_to
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let smtp_references = draft
        .references
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let payload = SendPayload {
        from: smtp_parse_addrs(draft.from.clone()),
        to: draft.to.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        cc: draft.cc.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        bcc: draft.bcc.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        subject: Some(draft.subject.clone()),
        body_plain: Some(draft.body_plain.clone()),
        body_html: draft.body_html.clone(),
        attachments,
        newsgroups: vec![],
        nntp_in_reply_to: None,
        nntp_references: None,
        smtp_notify,
        smtp_in_reply_to,
        smtp_references,
        smtp_message_id: draft
            .message_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
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

    let auth_methods = frb_runtime_handle()
        .block_on(async {
            probe_smtp_ehlo_auth_methods(
                host.as_str(),
                port,
                use_implicit_tls,
                use_starttls,
                None,
            )
            .await
        })
        .map_err(|e: SmtpClientError| e.to_string())?;

    let (auth, copy_store_to_transport) = smtp_resolve_auth_from_ehlo(
        transport,
        use_keychain,
        store_sid,
        auth_methods.as_slice(),
    )?;

    let (mut message, mut envelope) = build_rfc822_from_payload(&payload);
    envelope.cc.extend(payload.bcc.iter().cloned());

    let crypto_cfg = {
        let path = super::config_path_for_relay_lookup().ok_or_else(|| {
            "config path not registered; load settings before sending".to_owned()
        })?;
        super::load_frb_config_struct(path.as_str())
    };
    let (wire, sent_for_imap) =
        apply_mail_crypto_for_send(message.as_slice(), draft, &payload, &crypto_cfg)?;
    message = wire;

    let notify_arg = payload.smtp_notify.as_deref();

    frb_runtime_handle()
        .block_on(async {
            send_message_async(
                host.as_str(),
                port,
                use_implicit_tls,
                use_starttls,
                auth.as_ref().map(|(u, p, m)| (u.as_str(), p.as_str(), *m)),
                None,
                message.as_slice(),
                &envelope,
                notify_arg,
            )
            .await
        })
        .map_err(|e: SmtpClientError| e.to_string())?;

    if let Some(sid) = store_sid {
        if let Ok((store_acc, _kc)) = resolve_mail_account(sid) {
            let st = normalize_store_type(store_acc.backend_type.as_str());
            if matches!(st.as_str(), "imap" | "imaps")
                && imap_mirror_sent_after_smtp_enabled(&store_acc)
            {
                if let Some(ref inner) = envelope.message_id {
                    if let Some(angle) = normalize_smtp_message_id_angle(inner.as_str()) {
                        let sent_bytes = sent_for_imap
                            .as_deref()
                            .unwrap_or(message.as_slice());
                        let _ = crate::mail_store::blocking_imap_mirror_sent_if_missing(
                            &store_acc,
                            use_keychain,
                            angle.as_str(),
                            sent_bytes,
                        );
                    }
                }
            }
        }
    }

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

fn imap_mirror_sent_after_smtp_enabled(acc: &FrbAccount) -> bool {
    match acc.attrs.get("imapMirrorSentIfMissing").map(|s| s.trim()) {
        None => true,
        Some(s) if s.is_empty() => true,
        Some(s) => {
            let sl = s.to_ascii_lowercase();
            !matches!(sl.as_str(), "0" | "false" | "no" | "off")
        }
    }
}

/// Append a draft (`\\Draft`) to the account’s drafts mailbox (IMAP). [compose] matches SMTP compose (empty `to` allowed).
/// When [replace_draft_uid] is set, that UID is removed first (same drafts mailbox).
pub(crate) fn save_imap_draft_json(
    store_account_id: &str,
    compose: &FrbComposeMessage,
    replace_draft_uid: Option<u32>,
) -> Result<Option<u32>, String> {
    let (acc, use_keychain) = resolve_mail_account(store_account_id)?;
    let t = normalize_store_type(acc.backend_type.as_str());
    if !matches!(t.as_str(), "imap" | "imaps") {
        return Err("draft save is only supported for imap/imaps store accounts".to_owned());
    }
    let draft = compose;

    let mut attachments: Vec<Attachment> = Vec::with_capacity(draft.attachments.len());
    for a in &draft.attachments {
        let raw = a.bytes_base64.trim();
        if raw.is_empty() {
            continue;
        }
        let content = base64::engine::general_purpose::STANDARD
            .decode(raw.as_bytes())
            .map_err(|e| format!("attachment base64: {e}"))?;
        let mime_type = a.mime_type.trim();
        attachments.push(Attachment {
            filename: a.filename.clone(),
            mime_type: if mime_type.is_empty() {
                "application/octet-stream".to_owned()
            } else {
                mime_type.to_owned()
            },
            content,
        });
    }

    let payload = SendPayload {
        from: smtp_parse_addrs(draft.from.clone()),
        to: draft.to.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        cc: draft.cc.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        bcc: draft.bcc.iter().cloned().flat_map(smtp_parse_addrs).collect(),
        subject: Some(draft.subject.clone()),
        body_plain: Some(draft.body_plain.clone()),
        body_html: draft.body_html.clone(),
        attachments,
        newsgroups: vec![],
        nntp_in_reply_to: None,
        nntp_references: None,
        smtp_notify: None,
        smtp_in_reply_to: draft
            .in_reply_to
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        smtp_references: draft
            .references
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        smtp_message_id: draft
            .message_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    };

    if payload.from.len() != 1 {
        return Err("compose \"from\" must be exactly one address".to_owned());
    }

    let (raw, _) = build_rfc822_from_payload(&payload);
    set_credentials_backend(use_keychain);
    crate::mail_store::blocking_imap_save_draft(
        &acc,
        use_keychain,
        replace_draft_uid,
        raw.as_slice(),
    )
}

pub(crate) fn generate_smtp_compose_message_id_impl(from: String) -> Result<String, String> {
    let s = from.trim();
    if s.is_empty() {
        return Err("from address is required to generate Message-ID".to_owned());
    }
    Ok(generate_smtp_message_id_angle_from_from_field(s))
}

/// Connect, SMTP AUTH (if credentials exist), QUIT — no mail payload.
pub(crate) fn verify_smtp_transport(
    transport: &FrbTransport,
    use_keychain: bool,
) -> Result<(), String> {
    let tt = transport.transport_type.trim();
    if !tt.eq_ignore_ascii_case("smtp") && !tt.eq_ignore_ascii_case("gmail") {
        return Err(format!(
            "transport {:?} has type {:?}, expected smtp or gmail",
            transport.id, transport.transport_type
        ));
    }

    let host = transport.host.trim().to_string();
    if host.is_empty() {
        return Err(format!(
            "transport {:?} has empty host in config",
            transport.id
        ));
    }

    let port = transport.port;

    let (use_implicit_tls, use_starttls) = if port == 465 {
        (true, false)
    } else {
        smtp_tls_mode(transport.security.as_str())
    };

    set_credentials_backend(use_keychain);

    let auth_methods = frb_runtime_handle()
        .block_on(async {
            probe_smtp_ehlo_auth_methods(
                host.as_str(),
                port,
                use_implicit_tls,
                use_starttls,
                None,
            )
            .await
        })
        .map_err(|e: SmtpClientError| e.to_string())?;

    let (auth, _) = smtp_resolve_auth_from_ehlo(transport, use_keychain, None, auth_methods.as_slice())?;

    frb_runtime_handle()
        .block_on(async {
            verify_smtp_async(
                host.as_str(),
                port,
                use_implicit_tls,
                use_starttls,
                auth.as_ref().map(|(u, p, m)| (u.as_str(), p.as_str(), *m)),
                None,
            )
            .await
        })
        .map_err(|e: SmtpClientError| e.to_string())
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
    compose: &FrbNntpComposeMessage,
) -> Result<(), String> {
    if normalize_store_type(acc.backend_type.as_str()) != "nntp" {
        return Err(format!(
            "account {:?} has type {:?}, expected nntp",
            acc.id, acc.backend_type
        ));
    }
    let draft = compose;

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
    for a in &draft.attachments {
        let raw = a.bytes_base64.trim();
        if raw.is_empty() {
            continue;
        }
        let content = base64::engine::general_purpose::STANDARD
            .decode(raw.as_bytes())
            .map_err(|e| format!("attachment base64: {e}"))?;
        let mime_type = a.mime_type.trim();
        attachments.push(Attachment {
            filename: a.filename.clone(),
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
        subject: Some(draft.subject.clone()),
        body_plain: Some(draft.body_plain.clone()),
        body_html: None,
        attachments,
        newsgroups: groups,
        nntp_in_reply_to: draft.in_reply_to.clone(),
        nntp_references: draft.references.clone(),
        smtp_notify: None,
        smtp_in_reply_to: None,
        smtp_references: None,
        smtp_message_id: None,
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
    use super::{FrbMailOperationItem, TransferMailMessagesResponse};

    #[test]
    fn transfer_response_counts() {
        let out = TransferMailMessagesResponse {
            results: vec![
                FrbMailOperationItem {
                    id: "1".into(),
                    ok: true,
                    error: None,
                },
                FrbMailOperationItem {
                    id: "2".into(),
                    ok: false,
                    error: Some("boom".into()),
                },
            ],
            ok_count: 1,
            failed_count: 1,
        };
        assert_eq!(out.results.len(), 2);
        assert_eq!(out.ok_count, 1);
        assert_eq!(out.failed_count, 1);
    }
}
