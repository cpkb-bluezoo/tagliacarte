/*
 * session/mod.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Rust-owned app session: eager per-account connections, folder model, event broadcast.
 */

mod commands;
mod events;
mod nostr_profile_jobs;

pub use events::AppEvent;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use tagliacarte_core::config::{set_active_config_xml_path, set_credentials_backend};

use crate::frb_generated::StreamSink;
use tokio::sync::broadcast;

use crate::frb_api::frb_mail::{
    get_folder_message_json, list_folder_messages_window_json,
    list_folder_messages_window_response, mark_folder_message_read,
    nostr_sync_remote_profile_and_relays, transfer_mail_messages_json,
};
use crate::frb_api::{load_frb_config_struct, FrbAccount};
use crate::mail_body_server;
use crate::mail_kind::{is_imap_like_store, is_nostr_store, normalize_store_type};
use crate::mail_store::{
    imap_configure_idle_threshold, imap_take_folder_list_stale, mail_runtime_handle,
    list_mail_folders_snapshot_with_progress, nostr_folder_list_from_cache_snapshot,
    nostr_send_chat_message, MailFoldersSnapshot,
};

use commands::AppCommand;

fn store_kind_label(backend_type: &str) -> String {
    match normalize_store_type(backend_type).as_str() {
        "nostr" => "nostr".to_string(),
        "matrix" => "matrix".to_string(),
        _ => "email".to_string(),
    }
}

fn session_supported_account(a: &FrbAccount) -> bool {
    let t = normalize_store_type(&a.backend_type);
    if t.is_empty() {
        return false;
    }
    matches!(
        t.as_str(),
        "maildir"
            | "mbox"
            | "imap"
            | "imaps"
            | "gmail"
            | "pop3"
            | "pop3s"
            | "nntp"
            | "nntps"
            | "nostr"
            | "matrix"
    )
}

/// One configured account the session tracks (email or conversation store).
#[derive(Debug, Clone)]
struct AccountRow {
    id: String,
    account: FrbAccount,
    imap_min_idle_secs: u32,
    store_kind: String,
}

impl AccountRow {
    fn from_frb(a: &FrbAccount) -> Option<Self> {
        if a.id.trim().is_empty() {
            return None;
        }
        if !session_supported_account(a) {
            return None;
        }
        Some(Self {
            id: a.id.clone(),
            imap_min_idle_secs: a
                .attrs
                .get("imapIdleMinIdleSeconds")
                .and_then(|s| s.parse().ok())
                .unwrap_or(120),
            store_kind: store_kind_label(&a.backend_type),
            account: a.clone(),
        })
    }
}

#[derive(Clone)]
struct SessionShared {
    accounts: Arc<RwLock<HashMap<String, AccountRow>>>,
    /// Live flag: reload from config updates this so background IMAP loops honor keychain toggles.
    use_keychain: Arc<AtomicBool>,
    event_tx: broadcast::Sender<AppEvent>,
}

fn emit_json_event(tx: &broadcast::Sender<AppEvent>, ev: AppEvent) {
    let _ = tx.send(ev);
}

fn folder_list_event(account_id: &str, snap: &MailFoldersSnapshot) -> AppEvent {
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
        &acc.account,
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
    if is_nostr_store(acc.account.backend_type.as_str()) {
        nostr_profile_jobs::schedule_nostr_profile_fetches_for_folder_list(
            account_id.to_string(),
            acc.account.clone(),
            use_keychain,
            &snap.folders,
            (*tx).clone(),
        );
    }
    Ok(())
}

fn run_account_loop(
    acc: AccountRow,
    use_keychain_flag: Arc<AtomicBool>,
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

    let is_imap = is_imap_like_store(acc.account.backend_type.as_str());
    let acc_for_thread = acc.clone();
    let id_for_thread = id.clone();
    let sk_for_thread = sk.clone();
    let tx_for_thread = event_tx.clone();
    let uk_flag = use_keychain_flag;
    let cfg_path = config_xml_path.clone();

    std::thread::spawn(move || {
        let uk = || uk_flag.load(Ordering::SeqCst);
        match folder_list_refresh_job(&id_for_thread, &acc_for_thread, uk(), &tx_for_thread) {
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
                &acc_for_thread.account,
                uk(),
                acc_for_thread.imap_min_idle_secs,
            );
        }

        if is_nostr_store(acc_for_thread.account.backend_type.as_str()) {
            let path = cfg_path.clone();
            let aid = id_for_thread.clone();
            std::thread::spawn(move || {
                let _ = nostr_sync_remote_profile_and_relays(&path, &aid);
            });
        }

        if !is_imap {
            return;
        }

        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if imap_take_folder_list_stale(&acc_for_thread.account, uk())
                && folder_list_refresh_job(&id_for_thread, &acc_for_thread, uk(), &tx_for_thread)
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
    start_session_with_push(config_xml_path, move |s| {
        let _ = sink.add(s);
    })
}

/// Same as [`start_session`] but forwards JSON event strings to a channel (native TUI / tests).
pub fn start_session_native(
    tx: tokio::sync::mpsc::UnboundedSender<String>,
    config_xml_path: String,
) -> Result<(), String> {
    start_session_with_push(config_xml_path, move |s| {
        let _ = tx.send(s);
    })
}

fn start_session_with_push<F>(config_xml_path: String, push: F) -> Result<(), String>
where
    F: Fn(String) + Send + 'static,
{
    let mut g = session_cell()
        .lock()
        .map_err(|_| "session mutex poisoned")?;
    if g.is_some() {
        return Err("session already started".to_string());
    }

    let path_trim = config_xml_path.trim().to_string();
    set_active_config_xml_path(std::path::Path::new(path_trim.as_str()));
    let cfg = load_frb_config_struct(path_trim.as_str());
    set_credentials_backend(cfg.use_keychain);
    let use_keychain_flag = Arc::new(AtomicBool::new(cfg.use_keychain));

    let (event_tx, _) = broadcast::channel::<AppEvent>(4096);

    let mut map = HashMap::new();
    let mut config_errors: Vec<String> = Vec::new();
    for a in &cfg.accounts {
        if normalize_store_type(&a.backend_type).is_empty() {
            config_errors.push(format!(
                "account id={:?} label={:?}: empty backend type (every account must join the session)",
                a.id, a.label
            ));
            continue;
        }
        if !session_supported_account(a) {
            config_errors.push(format!(
                "account id={:?} label={:?}: unsupported store type {:?}",
                a.id, a.label, a.backend_type
            ));
            continue;
        }
        match AccountRow::from_frb(a) {
            Some(row) => {
                map.insert(row.id.clone(), row);
            }
            None => {
                config_errors.push(format!(
                    "account id={:?} label={:?}: could not join session (internal)",
                    a.id, a.label
                ));
            }
        }
    }
    if !config_errors.is_empty() {
        return Err(config_errors.join("\n"));
    }

    let accounts = Arc::new(RwLock::new(map));

    let shared = Arc::new(SessionShared {
        accounts: Arc::clone(&accounts),
        use_keychain: Arc::clone(&use_keychain_flag),
        event_tx: event_tx.clone(),
    });
    *g = Some(Arc::clone(&shared));

    let boot_rows: Vec<AccountRow> = accounts
        .read()
        .expect("session accounts lock poisoned")
        .values()
        .cloned()
        .collect();
    for acc in boot_rows {
        let tx = event_tx.clone();
        let ukf = Arc::clone(&use_keychain_flag);
        let cfgp = path_trim.clone();
        std::thread::spawn(move || run_account_loop(acc, ukf, tx, cfgp));
    }

    let mut sub = event_tx.subscribe();
    mail_runtime_handle().spawn(async move {
        loop {
            match sub.recv().await {
                Ok(ev) => {
                    if let Ok(s) = serde_json::to_string(&ev) {
                        push(s);
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
    let map = shared
        .accounts
        .read()
        .map_err(|_| "session accounts lock poisoned".to_string())?;
    map.get(account_id)
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
    let map = shared.accounts.read().ok()?;
    for row in map.values() {
        if row.id == hint {
            return Some(row.clone());
        }
    }
    None
}

/// After Flutter saves new `<store>` rows, register them and start background loops (no restart).
pub fn reload_session_accounts(config_xml_path: &str) -> Result<(), String> {
    let shared = session_shared_arc()?;
    let path_trim = config_xml_path.trim().to_string();
    set_active_config_xml_path(std::path::Path::new(path_trim.as_str()));
    let cfg = load_frb_config_struct(path_trim.as_str());
    let old_uk = shared.use_keychain.load(Ordering::SeqCst);
    let new_uk = cfg.use_keychain;
    shared.use_keychain.store(new_uk, Ordering::SeqCst);
    set_credentials_backend(new_uk);
    if old_uk != new_uk {
        crate::mail_store::invalidate_all_mail_store_caches();
    }
    let mut to_spawn: Vec<AccountRow> = Vec::new();
    {
        let mut map = shared
            .accounts
            .write()
            .map_err(|_| "session accounts lock poisoned".to_string())?;
        for a in &cfg.accounts {
            let Some(row) = AccountRow::from_frb(a) else {
                continue;
            };
            if map.contains_key(&row.id) {
                continue;
            }
            map.insert(row.id.clone(), row.clone());
            to_spawn.push(row);
        }
    }
    let tx = shared.event_tx.clone();
    let ukf = Arc::clone(&shared.use_keychain);
    for acc in to_spawn {
        let tx2 = tx.clone();
        let cfgp = path_trim.clone();
        let uk2 = Arc::clone(&ukf);
        std::thread::spawn(move || run_account_loop(acc, uk2, tx2, cfgp));
    }
    Ok(())
}

async fn dispatch_command(shared: Arc<SessionShared>, cmd: AppCommand) {
    let tx = &shared.event_tx;
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
                let folder_for_flags = folder.clone();
                let mid_for_flags = message_id.clone();
                let res = (|| {
                    let uk2 = shared2.use_keychain.load(Ordering::SeqCst);
                    let acc = lookup(&shared2, &account_id)?;
                    mark_folder_message_read(
                        acc.account.clone(),
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
                let res = (|| {
                    let uk2 = shared2.use_keychain.load(Ordering::SeqCst);
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
                let uk2 = shared2.use_keychain.load(Ordering::SeqCst);
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
                let nostr_account = acc.account.clone();
                match list_folder_messages_window_response(
                    acc.account,
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
                        if is_nostr_store(nostr_account.backend_type.as_str()) {
                            let pks: Vec<String> = resp
                                .messages
                                .iter()
                                .filter_map(|m| m.nostr_sender_pubkey_hex.clone())
                                .collect();
                            nostr_profile_jobs::schedule_nostr_profile_fetches_after_message_window(
                                account_id.clone(),
                                nostr_account.clone(),
                                uk2,
                                pks,
                                tx2.clone(),
                            );
                        }
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
                if !is_nostr_store(acc.account.backend_type.as_str()) {
                    return Err("sendChatMessage is only supported for Nostr".to_string());
                }
                let uk = shared.use_keychain.load(Ordering::SeqCst);
                nostr_send_chat_message(
                    &acc.account,
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
                let uk2 = shared2.use_keychain.load(Ordering::SeqCst);
                let src_id = source_account_id.clone();
                let dst_id = dest_account_id.clone();
                let res = (|| {
                    let src = lookup(&shared2, &source_account_id)?;
                    let dst = lookup(&shared2, &dest_account_id)?;
                    transfer_mail_messages_json(
                        src.account.clone(),
                        source_folder.clone(),
                        dst.account.clone(),
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
    if !is_nostr_store(acc.account.backend_type.as_str()) {
        return;
    }
    let uk = shared.use_keychain.load(Ordering::SeqCst);
    match nostr_folder_list_from_cache_snapshot(&acc.account, uk) {
        Ok(snap) => {
            // One authoritative [FolderListUpdated] only: a burst of [FolderFound] before it can
            // overflow the tokio broadcast buffer and drop the final update, leaving the UI stale.
            emit_json_event(
                &shared.event_tx,
                folder_list_event(session_account_id.as_str(), &snap),
            );
            nostr_profile_jobs::schedule_nostr_profile_fetches_for_folder_list(
                session_account_id.clone(),
                acc.account.clone(),
                uk,
                &snap.folders,
                shared.event_tx.clone(),
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
        acc.account.clone(),
        folder_name.to_string(),
        start_index,
        limit,
        message_list_sort.to_string(),
        shared.use_keychain.load(Ordering::SeqCst),
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
        acc.account.clone(),
        folder_name.to_string(),
        message_id.to_string(),
        shared.use_keychain.load(Ordering::SeqCst),
    )
}

/// Register this account's store with the loopback mail-body HTTPS server; returns opaque `storeKey`.
pub fn session_register_mail_body_store(account_id: &str) -> Result<String, String> {
    let shared = session_shared_arc()?;
    let acc = lookup(&shared, account_id)?;
    mail_body_server::register_mail_body_store(
        acc.id.clone(),
        shared.use_keychain.load(Ordering::SeqCst),
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
    mail_runtime_handle().spawn(async move {
        dispatch_command(s2, cmd).await;
    });
    Ok(())
}
