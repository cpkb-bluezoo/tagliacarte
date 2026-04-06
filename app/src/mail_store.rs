/*
 * mail_store.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * App-layer mail store: open/cache [Store] from [FrbAccount] (type + attrs). UI-agnostic.
 */

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use tagliacarte_core::config::{
    CredentialEntry, default_config_dir, load_credentials, resolve_credentials_file_path,
    save_credential, set_credentials_backend,
};
use tagliacarte_core::oauth::flow::refresh_access_token;
use tagliacarte_core::oauth::{GoogleOAuthProvider, OAuthProvider, OAuthTokenEntry};
use tagliacarte_core::localstorage::maildir::MaildirStore;
use tagliacarte_core::localstorage::mbox::MboxStore;
use tagliacarte_core::message_id::MessageId;
use tagliacarte_core::protocol::imap::connect_and_authenticate;
use tagliacarte_core::protocol::imap::ImapStore;
use tagliacarte_core::protocol::matrix::MatrixStore;
use tagliacarte_core::protocol::nntp::NntpStore;
use tagliacarte_core::protocol::nostr::keys as nostr_keys;
use tagliacarte_core::protocol::nostr::NostrStore;
use tagliacarte_core::protocol::pop3::Pop3Store;
use tagliacarte_core::sasl::SaslMechanism;
use tagliacarte_core::store::{
    Address, Folder, FolderInfo, OpenFolderEvent, SendPayload, Store, StoreError, Transport,
};
use tokio::runtime::{Builder, Runtime};

use crate::frb_api::FrbAccount;
use crate::mail_kind::{
    is_imap_like_store, is_maildir_store, is_mbox_store, is_nostr_store, normalize_store_type,
};

static APP_MAIL_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    let n = std::thread::available_parallelism()
        .map(|p| p.get().clamp(4, 32))
        .unwrap_or(8);
    Builder::new_multi_thread()
        .worker_threads(n)
        .enable_all()
        .build()
        .expect("app mail tokio runtime")
});

/// Shared async runtime for IMAP, Nostr, SMTP helpers (Flutter and future TUI use the same app layer).
pub fn mail_runtime_handle() -> tokio::runtime::Handle {
    APP_MAIL_RUNTIME.handle().clone()
}

pub type DynStore = Arc<dyn Store + Send + Sync>;

static MAIL_STORE_CACHE: Lazy<Mutex<HashMap<(String, bool), DynStore>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn invalidate_mail_store_cache(account_id: &str, use_keychain: bool) {
    let key = (account_id.trim().to_string(), use_keychain);
    let mut g = MAIL_STORE_CACHE.lock().expect("mail store cache");
    g.remove(&key);
}

/// Drop every cached store (e.g. after toggling keychain vs file credentials).
pub fn invalidate_all_mail_store_caches() {
    let mut g = MAIL_STORE_CACHE.lock().expect("mail store cache");
    g.clear();
}

/// Load vault entry for `account_id` (store or transport id).
pub fn load_mail_credential(account_key: &str, use_keychain: bool) -> Result<CredentialEntry, String> {
    let cred_path = resolve_credentials_file_path().ok_or_else(|| {
        "could not resolve credentials path".to_owned()
    })?;
    let creds = load_credentials(
        &cred_path,
        if use_keychain {
            Some(account_key)
        } else {
            None
        },
    )
    .map_err(|e| format!("credentials: {e}"))?;
    creds
        .get(account_key)
        .cloned()
        .ok_or_else(|| format!("no saved credential for this account ({account_key})"))
}

/// Google OAuth desktop client (env). Used for Gmail token refresh and sign-in.
pub fn google_oauth_provider_from_env() -> Result<GoogleOAuthProvider, String> {
    let id = std::env::var("TAGLIACARTE_GOOGLE_CLIENT_ID").map_err(|_| {
        "TAGLIACARTE_GOOGLE_CLIENT_ID is not set (required for Gmail OAuth sign-in and token refresh)"
            .to_string()
    })?;
    let sec = std::env::var("TAGLIACARTE_GOOGLE_CLIENT_SECRET").unwrap_or_default();
    Ok(GoogleOAuthProvider::new(id, sec))
}

fn refresh_gmail_oauth_json_in_place(
    cred_key: &str,
    use_keychain: bool,
    email: &str,
    json: &str,
) -> Result<String, String> {
    let mut oauth_entry = OAuthTokenEntry::from_json(json).ok_or_else(|| {
        "Gmail: stored credential is not valid OAuth JSON (sign in again)".to_owned()
    })?;
    if !oauth_entry.is_expired() {
        return Ok(oauth_entry.access_token.clone());
    }
    if oauth_entry.refresh_token.is_empty() {
        return Err(
            "Gmail OAuth access token expired and no refresh token is stored; sign in again with Google"
                .to_owned(),
        );
    }
    let provider = google_oauth_provider_from_env()?;
    let tokens = mail_runtime_handle()
        .block_on(refresh_access_token(&provider, oauth_entry.refresh_token.as_str()))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    oauth_entry.access_token = tokens.access_token.clone();
    oauth_entry.expires_at = now + tokens.expires_in.unwrap_or(3600) as i64;
    if let Some(rt) = tokens.refresh_token {
        oauth_entry.refresh_token = rt;
    }
    let path = resolve_credentials_file_path()
        .ok_or_else(|| "could not resolve credentials path".to_owned())?;
    set_credentials_backend(use_keychain);
    let scopes = provider.scopes().join(" ");
    // Keep provider id in sync with [OAuthTokenEntry::from_tokens].
    oauth_entry.provider = "google".to_string();
    oauth_entry.scopes = scopes;
    save_credential(
        path.as_path(),
        cred_key,
        email.trim(),
        &oauth_entry.to_json(),
    )
    .map_err(|e| format!("credentials: {e}"))?;
    Ok(oauth_entry.access_token)
}

/// Email + secret for Gmail XOAUTH2: `secret` is either a raw access token or OAuth JSON (refresh).
pub fn resolve_gmail_xoauth_secret(
    cred_key: &str,
    use_keychain: bool,
) -> Result<(String, String), String> {
    let entry = load_mail_credential(cred_key, use_keychain).map_err(|e| {
        if e.contains("no saved credential") {
            format!(
                "no saved password for IMAP account ({cred_key}). Add credentials in Tagliacarte."
            )
        } else {
            e
        }
    })?;
    let user = entry.username.trim();
    if user.is_empty() {
        return Err(
            "Gmail: add credentials with your email address and OAuth token for this account"
                .to_owned(),
        );
    }
    let user = user.to_string();
    let secret = entry.password_or_token.trim();
    let token = if secret.starts_with('{') {
        refresh_gmail_oauth_json_in_place(cred_key, use_keychain, user.as_str(), secret)?
    } else {
        secret.to_string()
    };
    Ok((user, token))
}

fn attr(acc: &FrbAccount, key: &str) -> Option<String> {
    acc.attrs
        .get(key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn attr_required(acc: &FrbAccount, key: &str) -> Result<String, String> {
    attr(acc, key).ok_or_else(|| format!("store {}: missing attribute {:?}", acc.id, key))
}

/// Display / prompt hint when the vault has no password yet. Sign-in names often live **only** in
/// the vault (Flutter clears `username` / `email` from attrs on save), so this may be empty until
/// the user is prompted — the store must still be constructed so connect + capability probe can run.
fn account_identity_hint(acc: &FrbAccount) -> String {
    attr(acc, "username")
        .or_else(|| attr(acc, "email"))
        .or_else(|| attr(acc, "defaultFrom"))
        .or_else(|| {
            let l = acc.label.trim();
            if l.is_empty() {
                None
            } else {
                Some(l.to_string())
            }
        })
        .unwrap_or_default()
}

fn nostr_relays_from_saved_config(account_id: &str) -> Vec<String> {
    let Some(p) = crate::frb_api::config_path_for_relay_lookup() else {
        return Vec::new();
    };
    let cfg = crate::frb_api::load_frb_config_struct(p.as_str());
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

fn build_store_from_account(acc: &FrbAccount, use_keychain: bool) -> Result<DynStore, String> {
    let t = normalize_store_type(acc.backend_type.as_str());
    let cred_key = acc.id.trim();
    if cred_key.is_empty() {
        return Err("store account has empty id".to_owned());
    }
    match t.as_str() {
        "maildir" => {
            let path = attr_required(acc, "path")?;
            let store = MaildirStore::new(&PathBuf::from(path)).map_err(|e| e.to_string())?;
            Ok(Arc::new(store))
        }
        "mbox" => {
            let path = attr_required(acc, "path")?;
            let store = MboxStore::new(PathBuf::from(path)).map_err(|e| e.to_string())?;
            Ok(Arc::new(store))
        }
        "imap" | "imaps" => build_imap_like_from_account(acc, cred_key, use_keychain, false),
        "gmail" => build_imap_like_from_account(acc, cred_key, use_keychain, true),
        "pop3" | "pop3s" => {
            let host = attr_required(acc, "host")?;
            let port: u16 = attr(acc, "port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(995);
            let hint = account_identity_hint(acc);
            let mut pop = Pop3Store::with_runtime_handle(host, port, mail_runtime_handle());
            if port == 995 {
                pop.set_implicit_tls(true);
            } else {
                let sec = attr(acc, "security").unwrap_or_else(|| "starttls".to_string());
                if sec == "tls" {
                    pop.set_implicit_tls(true);
                } else {
                    pop.set_use_stls(sec != "plain");
                }
            }
            match load_mail_credential(cred_key, use_keychain) {
                Ok(entry) => {
                    let user = entry.username.trim();
                    let pass = entry.password_or_token.trim();
                    if !user.is_empty() && !pass.is_empty() {
                        pop.set_auth(user.to_string(), pass);
                    } else if !user.is_empty() {
                        pop.set_username(user.to_string());
                    } else {
                        pop.set_username(hint);
                    }
                }
                Err(_) => {
                    pop.set_username(hint);
                }
            }
            Ok(Arc::new(pop))
        }
        "nntp" | "nntps" => {
            let host = attr_required(acc, "host")?;
            let port: u16 = attr(acc, "port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(563);
            let hint = account_identity_hint(acc);
            let mut nntp = NntpStore::with_runtime_handle(host, port, mail_runtime_handle());
            if port == 563 {
                nntp.set_implicit_tls(true);
            }
            let sec = attr(acc, "security").unwrap_or_else(|| "tls".to_string());
            nntp.set_use_starttls(sec == "starttls");
            match load_mail_credential(cred_key, use_keychain) {
                Ok(entry) => {
                    let user = entry.username.trim();
                    let pass = entry.password_or_token.trim();
                    if !user.is_empty() && !pass.is_empty() {
                        let arc: DynStore = Arc::new(nntp);
                        arc.set_credential(Some(user), pass);
                        Ok(arc)
                    } else if !user.is_empty() {
                        nntp.set_username(user.to_string());
                        Ok(Arc::new(nntp))
                    } else {
                        nntp.set_username(hint);
                        Ok(Arc::new(nntp))
                    }
                }
                Err(_) => {
                    nntp.set_username(hint);
                    Ok(Arc::new(nntp))
                }
            }
        }
        "nostr" => {
            let npub_or_hex = attr(acc, "npub").ok_or_else(|| "nostr: missing npub".to_string())?;
            let pubkey_hex = decode_nostr_id_part(&npub_or_hex)?;
            let relays = nostr_relays_from_saved_config(cred_key);
            if relays.is_empty() {
                return Err(
                    "Nostr account has no relay URLs (add relays in account settings)".to_owned(),
                );
            }
            let config_dir = default_config_dir().map(|p| p.to_string_lossy().into_owned());
            let account_id = cred_key.to_string();
            let on_sync_done = Arc::new(move || {
                crate::session::refresh_nostr_folders_for_account(account_id.as_str());
            });
            let store = NostrStore::new(
                relays,
                pubkey_hex,
                config_dir,
                mail_runtime_handle(),
                Some(on_sync_done),
            )
            .map_err(|e| e.to_string())?;
            let arc_store: DynStore = Arc::new(store);
            if let Ok(entry) = load_mail_credential(cred_key, use_keychain) {
                arc_store.set_credential(None, entry.password_or_token.as_str());
            }
            Ok(arc_store)
        }
        "matrix" => {
            let homeserver = attr(acc, "host")
                .or_else(|| attr(acc, "homeserver"))
                .ok_or_else(|| "matrix: missing homeserver".to_string())?;
            let user_id = attr_required(acc, "username")?;
            let store = MatrixStore::new(homeserver, user_id, None, mail_runtime_handle())
                .map_err(|e| e.to_string())?;
            let arc_store: DynStore = Arc::new(store);
            if let Ok(entry) = load_mail_credential(cred_key, use_keychain) {
                let t = entry.password_or_token.trim();
                if !t.is_empty() {
                    arc_store.set_credential(None, t);
                }
            }
            Ok(arc_store)
        }
        "graph" => Err(
            "Microsoft Graph (Exchange) store is not opened via this path yet".to_owned(),
        ),
        _ => Err(format!(
            "unsupported store type {:?} for account {:?}",
            acc.backend_type, acc.id
        )),
    }
}

fn build_imap_like_from_account(
    acc: &FrbAccount,
    cred_key: &str,
    use_keychain: bool,
    is_gmail: bool,
) -> Result<DynStore, String> {
    let (host, port, use_implicit_tls, use_starttls) = if is_gmail {
        ("imap.gmail.com".to_owned(), 993u16, true, false)
    } else {
        let host = attr_required(acc, "host")?;
        let port: u16 = attr(acc, "port")
            .and_then(|p| p.parse().ok())
            .unwrap_or(993);
        let sec = attr(acc, "security").unwrap_or_else(|| "tls".to_string());
        let use_implicit_tls = sec == "tls" || port == 993;
        let use_starttls = sec == "starttls";
        (host, port, use_implicit_tls, use_starttls)
    };

    let username_hint = account_identity_hint(acc);

    let mut imap = ImapStore::with_runtime_handle(host, port, mail_runtime_handle());
    imap.set_implicit_tls(use_implicit_tls);
    imap.set_use_starttls(use_starttls);

    if is_gmail {
        match resolve_gmail_xoauth_secret(cred_key, use_keychain) {
            Ok((user, secret)) => {
                imap.set_oauth_token(user, secret.as_str());
            }
            Err(_) => {
                imap.set_username(username_hint);
            }
        }
    } else {
        match load_mail_credential(cred_key, use_keychain) {
            Ok(entry) => {
                let user = entry.username.trim();
                let pass = entry.password_or_token.trim();
                if !user.is_empty() && !pass.is_empty() {
                    imap.set_auth(user.to_string(), pass, SaslMechanism::Plain);
                } else if !user.is_empty() {
                    imap.set_username(user.to_string());
                } else {
                    imap.set_username(username_hint);
                }
            }
            Err(_) => {
                imap.set_username(username_hint);
            }
        }
    }

    if let Some(s) = attr(acc, "imapIdleMinIdleSeconds")
        .and_then(|v| v.parse::<u32>().ok())
    {
        imap.set_imap_min_idle_secs(s);
    }
    Ok(Arc::new(imap))
}

pub fn open_cached_store(acc: &FrbAccount, use_keychain: bool) -> Result<DynStore, String> {
    let key = (acc.id.clone(), use_keychain);
    {
        let g = MAIL_STORE_CACHE.lock().expect("mail store cache");
        if let Some(s) = g.get(&key) {
            return Ok(Arc::clone(s));
        }
    }
    let s = build_store_from_account(acc, use_keychain)?;
    let mut g = MAIL_STORE_CACHE.lock().expect("mail store cache");
    if let Some(existing) = g.get(&key) {
        return Ok(Arc::clone(existing));
    }
    g.insert(key, Arc::clone(&s));
    Ok(s)
}

#[derive(Debug, Clone)]
pub struct MailFoldersSnapshot {
    pub folders: Vec<String>,
    pub hierarchy_delimiter: Option<String>,
    pub unread_by_folder: HashMap<String, u32>,
}

fn inbox_first_preserve_order(names: &mut Vec<String>) {
    if let Some(pos) = names.iter().position(|n| n.eq_ignore_ascii_case("INBOX")) {
        let inbox = names.remove(pos);
        names.insert(0, inbox);
    }
}

fn folder_unread_counts_for_backend(
    backend_type: &str,
    store: &DynStore,
    folder_names: &[String],
) -> Vec<(String, u64)> {
    let mut v = Vec::new();
    if is_maildir_store(backend_type) {
        if let Some(md) = store.as_any().downcast_ref::<MaildirStore>() {
            for name in folder_names {
                let u = md.unread_count_for_mailbox(name).unwrap_or(0);
                v.push((name.clone(), u));
            }
        }
    } else if is_mbox_store(backend_type) {
        for name in folder_names {
            v.push((name.clone(), 0));
        }
    }
    v
}

pub fn list_mail_folders_snapshot_with_progress(
    acc: &FrbAccount,
    use_keychain: bool,
    mut on_each: impl FnMut(&str, u32),
) -> Result<MailFoldersSnapshot, String> {
    set_credentials_backend(use_keychain);
    if acc.id.trim().is_empty() {
        return Err("empty account id".to_owned());
    }
    let store = open_cached_store(acc, use_keychain)?;
    let is_imap_fast = is_imap_like_store(&acc.backend_type);
    let (folders, hierarchy_delimiter, unread_by_folder) =
        match store.as_any().downcast_ref::<ImapStore>() {
            Some(imap) if is_imap_fast => {
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
                let counts =
                    folder_unread_counts_for_backend(&acc.backend_type, &store, &out);
                let mut m = HashMap::new();
                for (name, u) in counts {
                    m.insert(name, u as u32);
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

pub fn nostr_folder_list_from_cache_snapshot(
    acc: &FrbAccount,
    use_keychain: bool,
) -> Result<MailFoldersSnapshot, String> {
    set_credentials_backend(use_keychain);
    let store = open_cached_store(acc, use_keychain)?;
    let ns = store
        .as_any()
        .downcast_ref::<NostrStore>()
        .ok_or_else(|| "internal: not a Nostr store".to_string())?;
    let mut out = ns
        .list_cached_conversation_pubkeys()
        .map_err(|e| e.to_string())?;
    inbox_first_preserve_order(&mut out);
    let hierarchy_delimiter = store.hierarchy_delimiter().map(|c| c.to_string());
    let counts = folder_unread_counts_for_backend(&acc.backend_type, &store, &out);
    let mut m = HashMap::new();
    for (name, u) in counts {
        m.insert(name, u as u32);
    }
    Ok(MailFoldersSnapshot {
        folders: out,
        hierarchy_delimiter,
        unread_by_folder: m,
    })
}

pub fn imap_take_folder_list_stale(acc: &FrbAccount, use_keychain: bool) -> bool {
    if !is_imap_like_store(&acc.backend_type) {
        return false;
    }
    let key = (acc.id.clone(), use_keychain);
    let g = MAIL_STORE_CACHE.lock().expect("mail store cache");
    let Some(store) = g.get(&key) else {
        return false;
    };
    store
        .as_any()
        .downcast_ref::<ImapStore>()
        .is_some_and(|imap| imap.take_folder_list_stale())
}

pub fn imap_configure_idle_threshold(
    acc: &FrbAccount,
    use_keychain: bool,
    min_idle_seconds: u32,
) -> Result<(), String> {
    if !is_imap_like_store(&acc.backend_type) {
        return Ok(());
    }
    set_credentials_backend(use_keychain);
    let store = open_cached_store(acc, use_keychain)?;
    if let Some(imap) = store.as_any().downcast_ref::<ImapStore>() {
        imap.set_imap_min_idle_secs(min_idle_seconds);
    }
    Ok(())
}

pub fn nostr_profile_fetch_context(
    acc: &FrbAccount,
    use_keychain: bool,
) -> Result<(Vec<String>, Option<String>), String> {
    if !is_nostr_store(&acc.backend_type) {
        return Err("not a Nostr account".to_owned());
    }
    let relays = nostr_relays_from_saved_config(acc.id.trim());
    if relays.is_empty() {
        return Err(
            "Nostr account has no relay URLs (add relays in account settings)".to_owned(),
        );
    }
    set_credentials_backend(use_keychain);
    let sk = load_mail_credential(acc.id.trim(), use_keychain)
        .ok()
        .and_then(|e| nostr_keys::secret_key_to_hex(e.password_or_token.trim()).ok());
    Ok((relays, sk))
}

pub fn nostr_send_chat_message(
    acc: &FrbAccount,
    folder_recipient: &str,
    text: &str,
    use_keychain: bool,
) -> Result<(), String> {
    set_credentials_backend(use_keychain);
    let store = open_cached_store(acc, use_keychain)?;
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

pub fn wait_open_folder(store: DynStore, folder_name: &str) -> Result<Box<dyn Folder>, String> {
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

pub fn blocking_get_message_raw(
    acc: &FrbAccount,
    use_keychain: bool,
    folder_name: &str,
    message_id: &str,
) -> Result<Vec<u8>, String> {
    set_credentials_backend(use_keychain);
    let store = open_cached_store(acc, use_keychain)?;
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

/// Second IMAP connection for APPEND (pipeline client does not support literals yet).
pub fn blocking_imap_append(
    dest: &FrbAccount,
    mailbox: &str,
    data: &[u8],
    use_keychain: bool,
) -> Result<(), String> {
    if !matches!(
        normalize_store_type(&dest.backend_type).as_str(),
        "imap" | "imaps"
    ) {
        return Err("append over secondary IMAP connection is only for imap type".to_owned());
    }
    let host = attr_required(dest, "host")?;
    let port: u16 = attr(dest, "port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(993);
    let sec = attr(dest, "security").unwrap_or_else(|| "tls".to_string());
    let use_implicit_tls = sec == "tls" || port == 993;

    let entry = load_mail_credential(dest.id.trim(), use_keychain).map_err(|e| {
        if e.contains("no saved credential") {
            format!(
                "no saved password for this IMAP account ({}). Add credentials in Tagliacarte.",
                dest.id
            )
        } else {
            e
        }
    })?;

    let mut user = entry.username.trim().to_string();
    if user.is_empty() {
        if let Some(u) = attr(dest, "username") {
            user = u;
        }
    }
    if user.is_empty() {
        return Err("IMAP username missing in credentials".to_owned());
    }

    let data = data.to_vec();
    let mailbox = mailbox.to_string();
    let host = host.to_string();
    APP_MAIL_RUNTIME.handle().block_on(async move {
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

pub fn same_mail_store(a: &FrbAccount, b: &FrbAccount) -> bool {
    !a.id.is_empty() && a.id == b.id
}

pub fn list_range_for_page_backend(
    backend_type: &str,
    total: u64,
    skip: u64,
    limit: u64,
) -> Option<std::ops::Range<u64>> {
    if total == 0 || limit == 0 {
        return None;
    }
    let skip = skip.min(total);
    let limit = limit.max(1).min(10_000);

    if is_maildir_store(backend_type) {
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
