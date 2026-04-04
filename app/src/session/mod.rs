/*
 * session/mod.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Rust-owned app session: eager per-account connections, folder model, event broadcast.
 */

mod commands;
mod events;

pub use events::AppEvent;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::frb_generated::StreamSink;
use tokio::sync::broadcast;

use crate::frb_api::frb_mail::{
    self, credential_lookup, get_folder_message_json, imap_configure_idle_threshold,
    imap_take_folder_list_stale, list_folder_messages_window_json,
    list_folder_messages_window_response, list_mail_folders_snapshot_with_progress,
    mark_folder_message_read, nostr_folder_list_from_cache_snapshot,
    transfer_mail_messages_json,
};
use crate::frb_api::{load_frb_config_struct, FrbAccount};
use crate::mail_body_server;

use commands::AppCommand;

fn store_kind_label(backend_type: &str, store_uri: &str) -> String {
    let b = backend_type.trim();
    if b.eq_ignore_ascii_case("nostr") || b == "Nostr" {
        return "nostr".to_string();
    }
    if b.eq_ignore_ascii_case("matrix") || b == "Matrix" {
        return "matrix".to_string();
    }
    if !b.is_empty() {
        return "email".to_string();
    }
    if store_uri.starts_with("nostr:") {
        "nostr".to_string()
    } else if store_uri.starts_with("matrix:store:") {
        "matrix".to_string()
    } else {
        "email".to_string()
    }
}

fn session_supported_store_uri(uri: &str) -> bool {
    uri.starts_with("maildir:")
        || uri.starts_with("mbox:")
        || uri.starts_with("imap://")
        || uri.starts_with("imaps://")
        || uri.starts_with("nostr:store:")
        || uri.starts_with("nostr:npub")
        || (uri.starts_with("nostr:") && uri.len() > "nostr:".len())
        || uri.starts_with("matrix:store:")
}

/// One configured account the session tracks (email or conversation store).
#[derive(Debug, Clone)]
struct AccountRow {
    id: String,
    store_uri: String,
    credential_key: String,
    imap_min_idle_secs: u32,
    store_kind: String,
}

impl AccountRow {
    fn from_frb(a: &FrbAccount) -> Option<Self> {
        let uri = a.store_uri.trim();
        if uri.is_empty() {
            return None;
        }
        if !session_supported_store_uri(uri) {
            return None;
        }
        let ck = a.id.trim();
        let credential_key = if ck.is_empty() {
            uri.to_string()
        } else {
            ck.to_string()
        };
        Some(Self {
            id: a.id.clone(),
            store_uri: uri.to_string(),
            credential_key,
            imap_min_idle_secs: a
                .attrs
                .get("imapIdleMinIdleSeconds")
                .and_then(|s| s.parse().ok())
                .unwrap_or(120),
            store_kind: store_kind_label(&a.backend_type, uri),
        })
    }
}

#[derive(Clone)]
struct SessionShared {
    accounts: Arc<HashMap<String, AccountRow>>,
    use_keychain: bool,
    event_tx: broadcast::Sender<AppEvent>,
}

fn emit_json_event(tx: &broadcast::Sender<AppEvent>, ev: AppEvent) {
    let _ = tx.send(ev);
}

fn folder_list_event(account_id: &str, snap: &frb_mail::MailFoldersSnapshot) -> AppEvent {
    AppEvent::FolderListUpdated {
        account_id: account_id.to_string(),
        folders: snap.folders.clone(),
        hierarchy_delimiter: snap.hierarchy_delimiter.clone(),
        unread_by_folder: snap.unread_by_folder.clone(),
    }
}

/// Lists folders on a worker thread: emits `folderFound` per folder then `folderListUpdated`
/// (authoritative reconcile). Does not block the FRB caller.
fn folder_list_refresh_job(
    account_id: &str,
    acc: &AccountRow,
    use_keychain: bool,
    tx: &broadcast::Sender<AppEvent>,
) -> Result<(), String> {
    let aid = account_id.to_string();
    let snap = list_mail_folders_snapshot_with_progress(
        acc.store_uri.as_str(),
        acc.credential_key.as_str(),
        use_keychain,
        |name, unread| {
            let _ = tx.send(AppEvent::FolderFound {
                account_id: aid.clone(),
                folder_name: name.to_string(),
                unread,
            });
        },
    )?;
    emit_json_event(tx, folder_list_event(account_id, &snap));
    Ok(())
}

fn run_account_loop(
    acc: AccountRow,
    use_keychain: bool,
    event_tx: broadcast::Sender<AppEvent>,
    config_xml_path: String,
) {
    let id = acc.id.clone();
    let sk = acc.store_kind.clone();
    emit_json_event(
        &event_tx,
        AppEvent::AccountConnectionChanged {
            account_id: id.clone(),
            store_kind: sk.clone(),
            connection_state: "connecting".to_string(),
            message: None,
        },
    );

    let is_imap = acc.store_uri.starts_with("imap://") || acc.store_uri.starts_with("imaps://");
    let acc_for_thread = acc.clone();
    let id_for_thread = id.clone();
    let sk_for_thread = sk.clone();
    let tx_for_thread = event_tx.clone();
    let uk = use_keychain;
    let cfg_path = config_xml_path.clone();

    std::thread::spawn(move || {
        match folder_list_refresh_job(&id_for_thread, &acc_for_thread, uk, &tx_for_thread) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("[session] initial folder list account_id={id_for_thread}: {e}");
                emit_json_event(
                    &tx_for_thread,
                    AppEvent::AccountConnectionChanged {
                        account_id: id_for_thread.clone(),
                        store_kind: sk_for_thread.clone(),
                        connection_state: "error".to_string(),
                        message: Some(e),
                    },
                );
                return;
            }
        }

        if is_imap {
            let _ = imap_configure_idle_threshold(
                acc_for_thread.store_uri.clone(),
                acc_for_thread.credential_key.clone(),
                uk,
                acc_for_thread.imap_min_idle_secs,
            );
        }

        if acc_for_thread.store_uri.starts_with("nostr:") {
            let path = cfg_path.clone();
            let aid = id_for_thread.clone();
            std::thread::spawn(move || {
                let _ = frb_mail::nostr_sync_remote_profile_and_relays(&path, &aid);
            });
        }

        if !is_imap {
            return;
        }

        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if imap_take_folder_list_stale(
                acc_for_thread.store_uri.clone(),
                acc_for_thread.credential_key.clone(),
                uk,
            ) && folder_list_refresh_job(&id_for_thread, &acc_for_thread, uk, &tx_for_thread)
                .is_err()
            {
                emit_json_event(
                    &tx_for_thread,
                    AppEvent::AccountConnectionChanged {
                        account_id: id_for_thread.clone(),
                        store_kind: sk_for_thread.clone(),
                        connection_state: "error".to_string(),
                        message: Some("folder list refresh failed".to_string()),
                    },
                );
            }
        }
    });

    emit_json_event(
        &event_tx,
        AppEvent::AccountConnectionChanged {
            account_id: id,
            store_kind: sk,
            connection_state: "connected".to_string(),
            message: None,
        },
    );
}

static SESSION: OnceLock<Mutex<Option<Arc<SessionShared>>>> = OnceLock::new();

fn session_cell() -> &'static Mutex<Option<Arc<SessionShared>>> {
    SESSION.get_or_init(|| Mutex::new(None))
}

/// Start session: load config, spawn per-account threads, forward events to [sink].
pub fn start_session(sink: StreamSink<String>, config_xml_path: String) -> Result<(), String> {
    let mut g = session_cell()
        .lock()
        .map_err(|_| "session mutex poisoned")?;
    if g.is_some() {
        return Err("session already started".to_string());
    }

    let path_trim = config_xml_path.trim().to_string();
    let cfg = load_frb_config_struct(path_trim.as_str());
    let use_keychain = cfg.use_keychain;

    let (event_tx, _) = broadcast::channel::<AppEvent>(4096);

    let mut map = HashMap::new();
    for a in &cfg.accounts {
        if let Some(row) = AccountRow::from_frb(a) {
            map.insert(row.id.clone(), row);
        }
    }
    let accounts = Arc::new(map);

    let shared = Arc::new(SessionShared {
        accounts: Arc::clone(&accounts),
        use_keychain,
        event_tx: event_tx.clone(),
    });
    *g = Some(Arc::clone(&shared));

    for acc in accounts.values() {
        let acc = acc.clone();
        let tx = event_tx.clone();
        let uk = use_keychain;
        let cfgp = path_trim.clone();
        std::thread::spawn(move || run_account_loop(acc, uk, tx, cfgp));
    }

    let mut sub = event_tx.subscribe();
    frb_mail::frb_runtime_handle().spawn(async move {
        loop {
            match sub.recv().await {
                Ok(ev) => {
                    if let Ok(s) = serde_json::to_string(&ev) {
                        let _ = sink.add(s);
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    Ok(())
}

fn lookup(shared: &SessionShared, account_id: &str) -> Result<AccountRow, String> {
    shared
        .accounts
        .get(account_id)
        .cloned()
        .ok_or_else(|| format!("unknown account_id {account_id}"))
}

/// Match session row when the Nostr store callback passes a credential key that may differ from
/// `AccountRow.id` (legacy / empty id / URI-as-key cases).
fn resolve_account_row_for_nostr_refresh(
    shared: &SessionShared,
    hint: &str,
) -> Option<AccountRow> {
    let hint = hint.trim();
    if hint.is_empty() {
        return None;
    }
    if let Ok(row) = lookup(shared, hint) {
        return Some(row);
    }
    for row in shared.accounts.values() {
        if row.id == hint || row.credential_key == hint {
            return Some(row.clone());
        }
        let resolved = credential_lookup(row.store_uri.as_str(), row.credential_key.as_str());
        if resolved == hint {
            return Some(row.clone());
        }
    }
    None
}

async fn dispatch_command(shared: Arc<SessionShared>, cmd: AppCommand) {
    let tx = &shared.event_tx;
    let uk = shared.use_keychain;
    match cmd {
        AppCommand::MarkRead {
            account_id,
            folder,
            message_id,
            request_id,
        } => {
            let shared2 = Arc::clone(&shared);
            std::thread::spawn(move || {
                let tx2 = &shared2.event_tx;
                let uk2 = shared2.use_keychain;
                let folder_for_flags = folder.clone();
                let mid_for_flags = message_id.clone();
                let res = (|| {
                    let acc = lookup(&shared2, &account_id)?;
                    mark_folder_message_read(
                        acc.store_uri.clone(),
                        acc.credential_key.clone(),
                        folder,
                        message_id,
                        uk2,
                    )?;
                    folder_list_refresh_job(&account_id, &acc, uk2, tx2)?;
                    emit_json_event(
                        tx2,
                        AppEvent::MessageFlagsChanged {
                            account_id: account_id.clone(),
                            folder: folder_for_flags,
                            message_id: mid_for_flags,
                            is_read: true,
                        },
                    );
                    Ok::<(), String>(())
                })();
                let (ok, err) = match res {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e)),
                };
                emit_json_event(
                    tx2,
                    AppEvent::CommandResult {
                        request_id,
                        ok,
                        error: err,
                    },
                );
            });
        }
        AppCommand::RefreshFolders { account_id } => {
            let shared2 = Arc::clone(&shared);
            let aid = account_id.clone();
            // Blocking store work must not run on the FRB tokio pool (§2–3 ARCHITECTURE.md).
            std::thread::spawn(move || {
                let tx2 = &shared2.event_tx;
                let uk2 = shared2.use_keychain;
                let res = (|| {
                    let acc = lookup(&shared2, &aid)?;
                    folder_list_refresh_job(&aid, &acc, uk2, tx2)
                })();
                let (ok, err) = match res {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e)),
                };
                emit_json_event(
                    tx2,
                    AppEvent::CommandResult {
                        request_id: None,
                        ok,
                        error: err,
                    },
                );
            });
        }
        AppCommand::ListMessagesWindow {
            account_id,
            folder_name,
            start_index,
            limit,
            message_list_sort,
            request_id,
            list_ready,
        } => {
            let shared2 = Arc::clone(&shared);
            std::thread::spawn(move || {
                let tx2 = &shared2.event_tx;
                let uk2 = shared2.use_keychain;
                let acc = match lookup(&shared2, &account_id) {
                    Ok(a) => a,
                    Err(e) => {
                        emit_json_event(
                            tx2,
                            AppEvent::MessageListWindowComplete {
                                request_id: request_id.clone(),
                                account_id: account_id.clone(),
                                folder_name: folder_name.clone(),
                                message_list_sort: message_list_sort.clone(),
                                error: Some(e),
                            },
                        );
                        return;
                    }
                };
                match list_folder_messages_window_response(
                    acc.store_uri,
                    acc.credential_key,
                    folder_name.clone(),
                    start_index,
                    limit,
                    message_list_sort.clone(),
                    uk2,
                ) {
                    Ok(resp) => {
                        let row_count = resp.row_count();
                        emit_json_event(
                            tx2,
                            AppEvent::MessageListWindowStarted {
                                request_id: request_id.clone(),
                                account_id: account_id.clone(),
                                folder_name: folder_name.clone(),
                                message_list_sort: message_list_sort.clone(),
                                total: resp.total(),
                                start_index: resp.start_index(),
                                list_strategy: resp.list_strategy().to_string(),
                                row_count,
                                list_ready,
                            },
                        );
                        resp.for_each_row(|rank, summary| {
                            emit_json_event(
                                tx2,
                                AppEvent::MessageListRowFound {
                                    request_id: request_id.clone(),
                                    account_id: account_id.clone(),
                                    folder_name: folder_name.clone(),
                                    message_list_sort: message_list_sort.clone(),
                                    rank,
                                    summary,
                                },
                            );
                        });
                        emit_json_event(
                            tx2,
                            AppEvent::MessageListWindowComplete {
                                request_id,
                                account_id,
                                folder_name,
                                message_list_sort,
                                error: None,
                            },
                        );
                    }
                    Err(e) => {
                        emit_json_event(
                            tx2,
                            AppEvent::MessageListWindowComplete {
                                request_id,
                                account_id,
                                folder_name,
                                message_list_sort,
                                error: Some(e),
                            },
                        );
                    }
                }
            });
        }
        AppCommand::SendChatMessage {
            account_id,
            folder,
            text,
            request_id,
        } => {
            let res = (|| {
                let acc = lookup(&shared, &account_id)?;
                if !acc.store_uri.starts_with("nostr:") {
                    return Err("sendChatMessage is only supported for Nostr".to_string());
                }
                frb_mail::nostr_send_chat_message(
                    acc.store_uri.as_str(),
                    acc.credential_key.as_str(),
                    folder.as_str(),
                    text.as_str(),
                    uk,
                )
            })();
            let (ok, err) = match res {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e)),
            };
            emit_json_event(
                tx,
                AppEvent::CommandResult {
                    request_id,
                    ok,
                    error: err,
                },
            );
        }
        AppCommand::TransferMessages {
            source_account_id,
            source_folder,
            dest_account_id,
            dest_folder,
            message_ids,
            is_move,
            request_id,
        } => {
            let shared2 = Arc::clone(&shared);
            std::thread::spawn(move || {
                let tx2 = &shared2.event_tx;
                let uk2 = shared2.use_keychain;
                let src_id = source_account_id.clone();
                let dst_id = dest_account_id.clone();
                let res = (|| {
                    let src = lookup(&shared2, &source_account_id)?;
                    let dst = lookup(&shared2, &dest_account_id)?;
                    transfer_mail_messages_json(
                        src.store_uri.clone(),
                        src.credential_key.clone(),
                        source_folder.clone(),
                        dst.store_uri.clone(),
                        dst.credential_key.clone(),
                        dest_folder.clone(),
                        message_ids,
                        is_move,
                        uk2,
                    )?;
                    for aid in [src_id.as_str(), dst_id.as_str()] {
                        if let Ok(acc) = lookup(&shared2, aid) {
                            let _ = folder_list_refresh_job(aid, &acc, uk2, tx2);
                        }
                    }
                    Ok::<(), String>(())
                })();
                let (ok, err) = match res {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e)),
                };
                emit_json_event(
                    tx2,
                    AppEvent::CommandResult {
                        request_id,
                        ok,
                        error: err,
                    },
                );
            });
        }
    }
}

fn session_shared_arc() -> Result<Arc<SessionShared>, String> {
    let g = session_cell()
        .lock()
        .map_err(|_| "session mutex poisoned".to_string())?;
    g.as_ref()
        .cloned()
        .ok_or_else(|| "session not started".to_string())
}

/// Push updated Nostr conversation folders from on-disk cache (after background DM sync).
pub(crate) fn refresh_nostr_folders_for_account(account_id_hint: &str) {
    let Ok(shared) = session_shared_arc() else {
        return;
    };
    let Some(acc) = resolve_account_row_for_nostr_refresh(&shared, account_id_hint) else {
        eprintln!(
            "[session] refresh_nostr_folders: no matching account for hint={account_id_hint:?}"
        );
        return;
    };
    let session_account_id = acc.id.clone();
    let u = acc.store_uri.trim();
    if !u.starts_with("nostr:") {
        return;
    }
    match nostr_folder_list_from_cache_snapshot(
        u,
        acc.credential_key.as_str(),
        shared.use_keychain,
    ) {
        Ok(snap) => {
            // One authoritative [FolderListUpdated] only: a burst of [FolderFound] before it can
            // overflow the tokio broadcast buffer and drop the final update, leaving the UI stale.
            emit_json_event(
                &shared.event_tx,
                folder_list_event(session_account_id.as_str(), &snap),
            );
        }
        Err(e) => {
            eprintln!(
                "[session] refresh_nostr_folders: cache snapshot failed for {}: {e}",
                session_account_id
            );
        }
    }
}

/// Paged message summaries for [account_id] (resolves store URI / vault key from session config).
pub fn session_list_messages_window(
    account_id: &str,
    folder_name: &str,
    start_index: u64,
    limit: u64,
    message_list_sort: &str,
) -> Result<String, String> {
    let shared = session_shared_arc()?;
    let acc = lookup(&shared, account_id)?;
    list_folder_messages_window_json(
        acc.store_uri,
        acc.credential_key,
        folder_name.to_string(),
        start_index,
        limit,
        message_list_sort.to_string(),
        shared.use_keychain,
    )
}

/// Structured message body JSON for [account_id].
pub fn session_get_folder_message(
    account_id: &str,
    folder_name: &str,
    message_id: &str,
) -> Result<String, String> {
    let shared = session_shared_arc()?;
    let acc = lookup(&shared, account_id)?;
    get_folder_message_json(
        acc.store_uri,
        acc.credential_key,
        folder_name.to_string(),
        message_id.to_string(),
        shared.use_keychain,
    )
}

/// Register this account's store with the loopback mail-body HTTPS server; returns opaque `storeKey`.
pub fn session_register_mail_body_store(account_id: &str) -> Result<String, String> {
    let shared = session_shared_arc()?;
    let acc = lookup(&shared, account_id)?;
    mail_body_server::register_mail_body_store(
        acc.store_uri,
        acc.credential_key,
        shared.use_keychain,
    )
}

/// Parse JSON command and dispatch on the session runtime (non-blocking).
pub fn session_command(command_json: String) -> Result<(), String> {
    let cmd: AppCommand =
        serde_json::from_str(&command_json).map_err(|e| format!("bad command json: {e}"))?;
    let g = session_cell()
        .lock()
        .map_err(|_| "session mutex poisoned")?;
    let Some(shared) = g.as_ref().map(Arc::clone) else {
        return Err("session not started".to_string());
    };
    drop(g);
    let s2 = Arc::clone(&shared);
    frb_mail::frb_runtime_handle().spawn(async move {
        dispatch_command(s2, cmd).await;
    });
    Ok(())
}
