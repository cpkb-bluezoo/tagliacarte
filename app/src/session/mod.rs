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
    self, get_folder_message_json, imap_configure_idle_threshold, imap_take_folder_list_stale,
    list_folder_messages_window_json, list_mail_folders_snapshot, mark_folder_message_read,
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
    if store_uri.starts_with("nostr:store:") {
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
            imap_min_idle_secs: a.imap_idle_min_idle_seconds.unwrap_or(120),
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

fn run_account_loop(acc: AccountRow, use_keychain: bool, event_tx: broadcast::Sender<AppEvent>) {
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

    match list_mail_folders_snapshot(&acc.store_uri, &acc.credential_key, use_keychain) {
        Ok(snap) => {
            if is_imap {
                let _ = imap_configure_idle_threshold(
                    acc.store_uri.clone(),
                    acc.credential_key.clone(),
                    use_keychain,
                    acc.imap_min_idle_secs,
                );
            }
            emit_json_event(&event_tx, folder_list_event(&id, &snap));
            emit_json_event(
                &event_tx,
                AppEvent::AccountConnectionChanged {
                    account_id: id.clone(),
                    store_kind: sk.clone(),
                    connection_state: "connected".to_string(),
                    message: None,
                },
            );

            if !is_imap {
                return;
            }

            loop {
                std::thread::sleep(std::time::Duration::from_secs(3));
                if imap_take_folder_list_stale(
                    acc.store_uri.clone(),
                    acc.credential_key.clone(),
                    use_keychain,
                ) {
                    match list_mail_folders_snapshot(
                        &acc.store_uri,
                        &acc.credential_key,
                        use_keychain,
                    ) {
                        Ok(s) => emit_json_event(&event_tx, folder_list_event(&id, &s)),
                        Err(_) => emit_json_event(
                            &event_tx,
                            AppEvent::AccountConnectionChanged {
                                account_id: id.clone(),
                                store_kind: sk.clone(),
                                connection_state: "error".to_string(),
                                message: Some("folder list refresh failed".to_string()),
                            },
                        ),
                    }
                }
            }
        }
        Err(e) => {
            emit_json_event(
                &event_tx,
                AppEvent::AccountConnectionChanged {
                    account_id: id,
                    store_kind: sk,
                    connection_state: "error".to_string(),
                    message: Some(e),
                },
            );
        }
    }
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

    let cfg = load_frb_config_struct(config_xml_path.trim());
    let use_keychain = cfg.use_keychain;

    let (event_tx, _) = broadcast::channel::<AppEvent>(256);

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
        std::thread::spawn(move || run_account_loop(acc, uk, tx));
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
            let folder_for_flags = folder.clone();
            let mid_for_flags = message_id.clone();
            let res = (|| {
                let acc = lookup(&shared, &account_id)?;
                mark_folder_message_read(
                    acc.store_uri.clone(),
                    acc.credential_key.clone(),
                    folder,
                    message_id,
                    uk,
                )?;
                let snap = list_mail_folders_snapshot(
                    acc.store_uri.as_str(),
                    acc.credential_key.as_str(),
                    uk,
                )?;
                emit_json_event(tx, folder_list_event(&account_id, &snap));
                emit_json_event(
                    tx,
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
                tx,
                AppEvent::CommandResult {
                    request_id,
                    ok,
                    error: err,
                },
            );
        }
        AppCommand::RefreshFolders { account_id } => {
            let res = (|| {
                let acc = lookup(&shared, &account_id)?;
                let snap = list_mail_folders_snapshot(
                    acc.store_uri.as_str(),
                    acc.credential_key.as_str(),
                    uk,
                )?;
                emit_json_event(tx, folder_list_event(&account_id, &snap));
                Ok::<(), String>(())
            })();
            let (ok, err) = match res {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e)),
            };
            emit_json_event(
                tx,
                AppEvent::CommandResult {
                    request_id: None,
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
            let src_id = source_account_id.clone();
            let dst_id = dest_account_id.clone();
            let res = (|| {
                let src = lookup(&shared, &source_account_id)?;
                let dst = lookup(&shared, &dest_account_id)?;
                transfer_mail_messages_json(
                    src.store_uri.clone(),
                    src.credential_key.clone(),
                    source_folder.clone(),
                    dst.store_uri.clone(),
                    dst.credential_key.clone(),
                    dest_folder.clone(),
                    message_ids,
                    is_move,
                    uk,
                )?;
                for aid in [src_id.as_str(), dst_id.as_str()] {
                    if let Ok(acc) = lookup(&shared, aid) {
                        if let Ok(snap) = list_mail_folders_snapshot(
                            acc.store_uri.as_str(),
                            acc.credential_key.as_str(),
                            uk,
                        ) {
                            emit_json_event(tx, folder_list_event(aid, &snap));
                        }
                    }
                }
                Ok::<(), String>(())
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
