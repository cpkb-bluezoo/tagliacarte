/*
 * frb_api/mod.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use base64::Engine;
use crate::frb_generated::StreamSink;
use crate::mail_kind::normalize_store_type;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tagliacarte_core::config::default_config_dir;
use tagliacarte_core::oauth::OAuthProvider;
#[cfg(target_os = "macos")]
use tagliacarte_core::config::macos_real_user_home_dir;
use tagliacarte_core::config::{
    resolve_credentials_file_path, save_credential, set_credentials_backend,
};

mod config_persist;
pub mod frb_json;
pub(crate) mod frb_mail;

/// Flutter’s active `config.xml` (same path as [frb_session_start]). Nostr relay URLs live in the
/// merged config from this file — not in auxiliary `~/.tagliacarte/config.xml` alone.
static PRIMARY_CONFIG_XML_PATH: Mutex<Option<String>> = Mutex::new(None);

pub(super) fn register_primary_config_xml_path(path: &str) {
    let t = path.trim();
    if t.is_empty() {
        return;
    }
    let mut g = PRIMARY_CONFIG_XML_PATH.lock().expect("primary config path lock");
    *g = Some(t.to_string());
}

/// Path passed to [load_frb_config_struct] for `FrbAccount.lists["relayUrls"]` (Nostr bootstrap).
pub(super) fn config_path_for_relay_lookup() -> Option<String> {
    let g = PRIMARY_CONFIG_XML_PATH.lock().expect("primary config path lock");
    if let Some(ref p) = *g {
        return Some(p.clone());
    }
    drop(g);
    config_xml_path().and_then(|pb| pb.to_str().map(|s| s.to_string()))
}

/// Resolve store account row and keychain mode from the active `config.xml` (`<store id="…">`).
pub(crate) fn resolve_mail_account(account_id: &str) -> Result<(FrbAccount, bool), String> {
    let path = {
        let g = PRIMARY_CONFIG_XML_PATH
            .lock()
            .map_err(|_| "primary config path lock poisoned".to_string())?;
        g.clone().ok_or_else(|| {
            "config path not registered; call load_config or start_session first".to_string()
        })?
    };
    let cfg = load_frb_config_struct(path.as_str());
    let id = account_id.trim();
    if id.is_empty() {
        return Err("empty account_id".to_string());
    }
    let acc = cfg
        .accounts
        .iter()
        .find(|a| a.id == id)
        .cloned()
        .ok_or_else(|| format!("unknown store account_id {:?}", id))?;
    if acc.backend_type.trim().is_empty() {
        return Err(format!("store {:?} has empty type in config", id));
    }
    Ok((acc, cfg.use_keychain))
}

/// Resolve `<transport id="…">` from the active config. The UI passes only [FrbTransport::id].
pub(crate) fn resolve_transport_in_primary_config(
    transport_id: &str,
) -> Result<(FrbTransport, bool), String> {
    let path = {
        let g = PRIMARY_CONFIG_XML_PATH
            .lock()
            .map_err(|_| "primary config path lock poisoned".to_string())?;
        g.clone().ok_or_else(|| {
            "config path not registered; call load_config or start_session first".to_string()
        })?
    };
    let id = transport_id.trim();
    if id.is_empty() {
        return Err("empty transport_id".to_string());
    }
    let cfg = load_frb_config_struct(path.as_str());
    let t = cfg
        .transports
        .iter()
        .find(|x| x.id == id)
        .cloned()
        .ok_or_else(|| format!("unknown transport_id {:?}", id))?;
    Ok((t, cfg.use_keychain))
}

#[derive(Debug, Clone)]
pub struct FrbTransport {
    pub id: String,
    pub transport_type: String,
    pub display_name: String,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub default_from: String,
    pub dsn_notify: String,
}

#[derive(Debug, Clone)]
pub struct FrbAccount {
    pub id: String,
    pub label: String,
    pub backend_type: String,
    pub avatar_url: Option<String>,
    pub last_folder: Option<String>,
    pub last_message_id: Option<String>,
    /// Backend-specific scalar settings (IMAP host/port, Nostr npub/nip05, Maildir path, …).
    pub attrs: std::collections::HashMap<String, String>,
    /// Backend-specific lists (`transportIds`, `relayUrls`, …).
    pub lists: std::collections::HashMap<String, Vec<String>>,
}

impl Default for FrbAccount {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            backend_type: String::new(),
            avatar_url: None,
            last_folder: None,
            last_message_id: None,
            attrs: std::collections::HashMap::new(),
            lists: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrbConfig {
    pub accounts: Vec<FrbAccount>,
    pub transports: Vec<FrbTransport>,
    pub selected_store_id: Option<String>,
    pub date_format: String,
    pub resource_policy: String,
    pub use_keychain: bool,
    pub load_remote_images: bool,
    pub threaded_view: bool,
    pub quote_original: bool,
    pub delete_mode: String,
    pub trash_folder_name: String,
    /// Symbolic message list sort, e.g. `date_desc`, `from_asc`, `subject_asc`.
    pub message_list_sort: String,
    /// In-app / OS new-mail notifications (toasts, local notifications).
    pub notify_new_messages: bool,
}

fn default_message_list_sort() -> String {
    "date_desc".to_owned()
}

impl Default for FrbConfig {
    fn default() -> Self {
        Self {
            accounts: vec![],
            transports: vec![],
            selected_store_id: None,
            date_format: "yyyy-MM-dd HH:mm".to_owned(),
            resource_policy: "block-remote".to_owned(),
            use_keychain: true,
            load_remote_images: false,
            threaded_view: true,
            quote_original: true,
            delete_mode: "Move to Trash".to_owned(),
            trash_folder_name: "Trash".to_owned(),
            message_list_sort: default_message_list_sort(),
            notify_new_messages: false,
        }
    }
}

pub fn frb_load_config_json(path: String) -> String {
    register_primary_config_xml_path(&path);
    #[cfg(debug_assertions)]
    eprintln!("tagliacarte: load_config_json path={path}");
    read_config(&path)
        .map(|cfg| frb_json::format_frb_config_json(&cfg))
        .unwrap_or_else(|| frb_json::format_frb_config_json(&FrbConfig::default()))
}

/// Same merged [FrbConfig] as JSON load, for Rust session boot.
pub(crate) fn load_frb_config_struct(path: &str) -> FrbConfig {
    read_config(path).unwrap_or_default()
}

pub fn frb_save_config_json(path: String, config_json: String) -> Result<(), String> {
    register_primary_config_xml_path(&path);
    #[cfg(debug_assertions)]
    eprintln!("tagliacarte: save_config_json path={path}");
    let parsed = frb_json::parse_frb_config_json(&config_json)?;
    write_config(&path, &parsed)
}

pub fn frb_upsert_account(path: String, account_json: String) -> Result<String, String> {
    register_primary_config_xml_path(&path);
    let mut cfg = read_config(&path).unwrap_or_default();
    let incoming = frb_json::parse_frb_account_json(&account_json)?;
    cfg.accounts.retain(|a| a.id != incoming.id);
    cfg.accounts.push(incoming);
    write_config(&path, &cfg)?;
    Ok(frb_json::format_frb_config_json(&cfg))
}

pub fn frb_remove_account(path: String, account_id: String) -> Result<String, String> {
    register_primary_config_xml_path(&path);
    let mut cfg = read_config(&path).unwrap_or_default();
    cfg.accounts.retain(|a| a.id != account_id);
    write_config(&path, &cfg)?;
    Ok(frb_json::format_frb_config_json(&cfg))
}

pub fn frb_list_mail_folders(account_id: String) -> Result<String, String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::list_mail_folders_json(acc, use_keychain)
}

pub fn frb_imap_take_folder_list_stale(account_id: String) -> bool {
    resolve_mail_account(account_id.trim())
        .map(|(acc, uk)| crate::mail_store::imap_take_folder_list_stale(&acc, uk))
        .unwrap_or(false)
}

pub fn frb_imap_configure_idle_threshold(
    account_id: String,
    min_idle_seconds: u32,
) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    crate::mail_store::imap_configure_idle_threshold(&acc, use_keychain, min_idle_seconds)
}

pub fn frb_create_mail_folder(
    account_id: String,
    folder_path: String,
) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::create_mail_folder(acc, folder_path, use_keychain)
}

pub fn frb_rename_mail_folder(
    account_id: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::rename_mail_folder(acc, old_name, new_name, use_keychain)
}

pub fn frb_delete_mail_folder(
    account_id: String,
    folder_name: String,
) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::delete_mail_folder(acc, folder_name, use_keychain)
}

pub fn frb_list_folder_messages(
    account_id: String,
    folder_name: String,
    skip: i32,
    limit: i32,
) -> Result<String, String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::list_folder_messages_json(
        acc,
        folder_name,
        skip.max(0) as u64,
        limit.max(1).min(10_000) as u64,
        use_keychain,
    )
}

/// List one page of message summaries in **ascending** order for [message_list_sort]. JSON includes
/// `total`, `startIndex`, `messages`, `listStrategy` (`imapSort` or `fullScan`).
pub fn frb_list_folder_messages_window(
    account_id: String,
    folder_name: String,
    start_index: i32,
    limit: i32,
    message_list_sort: String,
) -> Result<String, String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::list_folder_messages_window_json(
        acc,
        folder_name,
        start_index.max(0) as u64,
        limit.max(1).min(10_000) as u64,
        message_list_sort,
        use_keychain,
    )
}

pub fn frb_get_folder_message(
    account_id: String,
    folder_name: String,
    message_id: String,
) -> Result<String, String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::get_folder_message_json(acc, folder_name, message_id, use_keychain)
}

pub fn frb_mark_folder_message_read(
    account_id: String,
    folder_name: String,
    message_id: String,
) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::mark_folder_message_read(acc, folder_name, message_id, use_keychain)
}

/// JSON: `{ results: [{ id, ok, error? }], okCount, failedCount }`. Cross-store move deletes source only after successful append.
pub fn frb_transfer_mail_messages(
    source_account_id: String,
    source_folder: String,
    dest_account_id: String,
    dest_folder: String,
    message_ids: Vec<String>,
    is_move: bool,
) -> Result<String, String> {
    let (src_acc, uk) = resolve_mail_account(source_account_id.trim())?;
    let (dst_acc, _) = resolve_mail_account(dest_account_id.trim())?;
    frb_mail::transfer_mail_messages_json(
        src_acc,
        source_folder,
        dst_acc,
        dest_folder,
        message_ids,
        is_move,
        uk,
    )
}

pub fn frb_expunge_mail_folder(account_id: String, folder_name: String) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::expunge_mail_folder(acc, folder_name, use_keychain)
}

/// Set whether the mail-body HTTPS server requires a **client certificate** (mutual TLS).
/// Default is **true** (strict). Call with **false** before [`frb_mail_body_server_init`] when the
/// embedded WebView cannot present [`client_cert_pem`] yet (loopback TLS remains enabled).
pub fn frb_mail_body_set_tls_require_client_cert(require: bool) {
    tagliacarte_core::protocol::http::mail_view_server::set_mail_body_tls_require_client_cert(
        require,
    );
}

/// Fetch one IMAP MIME part by section (decoded). JSON: `{ "bytesBase64": "..." }`.
/// Start loopback mTLS server if needed. JSON includes `enforcesClientCert`, PEM material, `baseUrl`.
pub fn frb_mail_body_server_init() -> Result<String, String> {
    let init = crate::mail_body_server::ensure_mail_body_server()?;
    Ok(crate::mail_body_server::mail_body_server_init_json(&init))
}

/// Register store for `/view/{key}/...` URLs. Call after `frb_mail_body_server_init`. Returns opaque `storeKey`.
pub fn frb_mail_body_register_store(account_id: String) -> Result<String, String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    crate::mail_body_server::ensure_mail_body_server()?;
    crate::mail_body_server::register_mail_body_store(acc.id.clone(), use_keychain)
}

/// Path segment used under `/view/.../{msg}/body`. Accepts a numeric IMAP UID or an `imap(s)://…/{uid}` id (last segment).
fn mail_body_path_message_id_segment(message_id: &str) -> &str {
    let t = message_id.trim();
    if let Some(rest) = t
        .strip_prefix("imap://")
        .or_else(|| t.strip_prefix("imaps://"))
    {
        if let Some(pos) = rest.rfind('/') {
            let tail = rest[pos + 1..].trim();
            if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                return tail;
            }
        }
    }
    t
}

/// Build message body URL. `message_id` is IMAP UID as decimal digits, or any store-specific id (percent-encoded in path).
/// `folder_name` is raw mailbox name; `extra_query` like `fg=fff&bg=000`.
pub fn frb_mail_body_message_url(
    store_key: String,
    folder_name: String,
    message_id: String,
    extra_query: String,
) -> Result<String, String> {
    let base = crate::mail_body_server::mail_body_server_base_url()
        .ok_or_else(|| "mail body server not started".to_string())?;
    let msg_seg = utf8_percent_encode(
        mail_body_path_message_id_segment(message_id.trim()),
        NON_ALPHANUMERIC,
    );
    let folder_enc = utf8_percent_encode(folder_name.trim(), NON_ALPHANUMERIC);
    let mut url = format!(
        "{}/view/{}/{}/{}/body",
        base.trim_end_matches('/'),
        store_key,
        folder_enc,
        msg_seg
    );
    if !extra_query.trim().is_empty() {
        let q = extra_query.trim().trim_start_matches('?');
        url.push('?');
        url.push_str(q);
    }
    Ok(url)
}

pub fn frb_fetch_folder_message_part(
    account_id: String,
    folder_name: String,
    message_id: String,
    imap_section: String,
    transfer_encoding: String,
) -> Result<String, String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    frb_mail::fetch_folder_message_part_json(
        acc,
        folder_name,
        message_id,
        imap_section,
        transfer_encoding,
        use_keychain,
    )
}

/// Persists secrets for the store identified by `account_id` (XML `<store id="…">`). Invalidates IMAP cache.
///
/// If the account row is not in config yet (e.g. Nostr identity wizard before first save), writes the vault
/// entry keyed by `account_id` only — no store cache invalidation until the store exists in XML.
pub fn frb_save_store_credential(
    account_id: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let id = account_id.trim().to_string();
    if id.is_empty() {
        return Err("empty account_id".to_string());
    }
    match resolve_mail_account(id.as_str()) {
        Ok((_acc, use_keychain)) => {
            set_credentials_backend(use_keychain);
            let path = resolve_credentials_file_path().ok_or_else(|| {
                "could not resolve credentials path (~/.tagliacarte/credentials)".to_owned()
            })?;
            save_credential(&path, id.as_str(), username.trim(), password.as_str())?;
            crate::mail_store::invalidate_mail_store_cache(id.as_str(), use_keychain);
            Ok(())
        }
        Err(e) if e.contains("unknown store account_id") => {
            let cfg_path = {
                let g = PRIMARY_CONFIG_XML_PATH
                    .lock()
                    .map_err(|_| "primary config path lock poisoned".to_string())?;
                g.clone().ok_or_else(|| {
                    "config path not registered; load config or start session first".to_string()
                })?
            };
            let cfg = load_frb_config_struct(cfg_path.as_str());
            set_credentials_backend(cfg.use_keychain);
            let path = resolve_credentials_file_path().ok_or_else(|| {
                "could not resolve credentials path (~/.tagliacarte/credentials)".to_owned()
            })?;
            save_credential(&path, id.as_str(), username.trim(), password.as_str())?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Outbound transport credentials (e.g. SMTP), keyed by transport id (`t1`). Keychain vs file
/// follows [FrbConfig::use_keychain] from the active config (same as store credentials).
pub fn frb_save_transport_credential(
    transport_id: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let id = transport_id.trim().to_string();
    if id.is_empty() {
        return Err("empty transport_id".to_string());
    }
    match resolve_transport_in_primary_config(id.as_str()) {
        Ok((_, use_keychain)) => {
            set_credentials_backend(use_keychain);
            let path = resolve_credentials_file_path().ok_or_else(|| {
                "could not resolve credentials path (~/.tagliacarte/credentials)".to_owned()
            })?;
            save_credential(&path, id.as_str(), username.trim(), password.as_str())?;
            Ok(())
        }
        Err(e) if e.contains("unknown transport_id") => {
            let cfg_path = {
                let g = PRIMARY_CONFIG_XML_PATH
                    .lock()
                    .map_err(|_| "primary config path lock poisoned".to_string())?;
                g.clone().ok_or_else(|| {
                    "config path not registered; load config or start session first".to_string()
                })?
            };
            let cfg = load_frb_config_struct(cfg_path.as_str());
            set_credentials_backend(cfg.use_keychain);
            let path = resolve_credentials_file_path().ok_or_else(|| {
                "could not resolve credentials path (~/.tagliacarte/credentials)".to_owned()
            })?;
            save_credential(&path, id.as_str(), username.trim(), password.as_str())?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Send one message via SMTP using the transport row identified by `transport_id` (not URI).
/// [compose_json] is camelCase: `from`, `to`, `cc`, `bcc`, `subject`, `bodyPlain`, optional `bodyHtml`.
pub fn frb_send_smtp_message(transport_id: String, compose_json: String) -> Result<(), String> {
    let (t, use_keychain) = resolve_transport_in_primary_config(transport_id.trim())?;
    frb_mail::send_smtp_json(&t, use_keychain, compose_json.trim())
}

/// POST a Netnews article using the NNTP store account (`<store type="nntp">`), same server connection as reading.
/// [compose_json] is camelCase: `from`, `newsgroups` (array of strings), `subject`, `bodyPlain`, optional `attachments`,
/// optional `inReplyTo`, optional `references`.
pub fn frb_send_nntp_message(store_account_id: String, compose_json: String) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(store_account_id.trim())?;
    frb_mail::send_nntp_json(&acc, use_keychain, compose_json.trim())
}

pub fn frb_nostr_generate_keypair_json() -> Result<String, String> {
    let (sk, pk) = tagliacarte_core::protocol::nostr::generate_keypair()?;
    Ok(serde_json::json!({"secretHex": sk, "pubkeyHex": pk}).to_string())
}

pub fn frb_nostr_get_public_key_from_secret(secret: String) -> Result<String, String> {
    tagliacarte_core::protocol::nostr::get_public_key_from_secret(secret.trim())
}

pub fn frb_nostr_hex_to_npub(hex_pubkey: String) -> Result<String, String> {
    tagliacarte_core::protocol::nostr::hex_to_npub(hex_pubkey.trim())
}

pub fn frb_nostr_secret_key_to_hex(input: String) -> Result<String, String> {
    tagliacarte_core::protocol::nostr::secret_key_to_hex(input.trim())
}

/// Publish kind 0 metadata from current account fields (needs nsec in vault).
pub fn frb_nostr_publish_profile(path: String, account_id: String) -> Result<(), String> {
    register_primary_config_xml_path(&path);
    frb_mail::nostr_publish_profile_metadata(path.trim(), account_id.trim())
}

/// Fetch profile + NIP-65 relay list and merge into config (background use).
pub fn frb_nostr_sync_remote_profile(path: String, account_id: String) -> Result<(), String> {
    register_primary_config_xml_path(&path);
    frb_mail::nostr_sync_remote_profile_and_relays(path.trim(), account_id.trim())
}

/// Subscribe to app-level mail events (folder lists, connection status, command results).
/// Call once after [RustLib.init]. [config_xml_path] is the same `config.xml` path Flutter uses.
pub fn frb_session_start(
    sink: StreamSink<String>,
    config_xml_path: String,
) -> Result<(), String> {
    register_primary_config_xml_path(&config_xml_path);
    crate::session::start_session(sink, config_xml_path)
}

/// Register new `<store>` accounts with the running session (spawn folder loops) after Flutter saves config.
pub fn frb_session_reload_accounts(config_xml_path: String) -> Result<(), String> {
    let p = config_xml_path.trim().to_string();
    register_primary_config_xml_path(&p);
    crate::session::reload_session_accounts(p.as_str())
}

fn email_from_google_id_token_jwt(id_token: &str) -> Result<String, String> {
    let mid = id_token
        .split('.')
        .nth(1)
        .ok_or_else(|| "id_token: missing JWT payload segment".to_string())?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(mid.as_bytes())
        .or_else(|_| {
            let pad = (4 - mid.len() % 4) % 4;
            let padded = format!("{}{}", mid, "=".repeat(pad));
            base64::engine::general_purpose::URL_SAFE.decode(padded.as_bytes())
        })
        .map_err(|e| format!("id_token payload base64: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&decoded)
        .map_err(|e| format!("id_token JSON: {e}"))?;
    v.get("email")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "id_token has no email claim".to_string())
}

/// Browser-based Google OAuth (PKCE); saves refreshable token JSON for the Gmail store vault key.
pub fn frb_gmail_oauth_sign_in(account_id: String) -> Result<(), String> {
    let (acc, use_keychain) = resolve_mail_account(account_id.trim())?;
    if normalize_store_type(acc.backend_type.as_str()) != "gmail" {
        return Err("OAuth sign-in is only for Gmail store accounts".to_string());
    }
    let provider = crate::mail_store::google_oauth_provider_from_env()?;
    let tokens = crate::mail_store::mail_runtime_handle().block_on(
        tagliacarte_core::oauth::flow::start_oauth_flow(&provider, |url| {
            let _ = webbrowser::open(url);
        }),
    )?;
    let id_tok = tokens.id_token.as_deref().ok_or_else(|| {
        "Google token response had no id_token; ensure openid and email scopes are granted"
            .to_string()
    })?;
    let email = email_from_google_id_token_jwt(id_tok)?;
    let scopes = provider.scopes().join(" ");
    let entry = tagliacarte_core::oauth::OAuthTokenEntry::from_tokens("google", &tokens, &scopes);
    set_credentials_backend(use_keychain);
    let path = resolve_credentials_file_path().ok_or_else(|| {
        "could not resolve credentials path (~/.tagliacarte/credentials)".to_owned()
    })?;
    save_credential(
        path.as_path(),
        acc.id.trim(),
        email.trim(),
        &entry.to_json(),
    )?;
    crate::mail_store::invalidate_mail_store_cache(acc.id.trim(), use_keychain);
    Ok(())
}

/// Fire-and-forget session command (JSON with `type`: markRead, refreshFolders, transferMessages).
pub fn frb_session_command(command_json: String) -> Result<(), String> {
    crate::session::session_command(command_json)
}

/// Paged message summaries for [account_id]; session maps to store URI and vault key.
pub fn frb_session_list_messages_window(
    account_id: String,
    folder_name: String,
    start_index: i32,
    limit: i32,
    message_list_sort: String,
) -> Result<String, String> {
    crate::session::session_list_messages_window(
        account_id.trim(),
        folder_name.trim(),
        start_index.max(0) as u64,
        limit.max(1).min(10_000) as u64,
        message_list_sort.trim(),
    )
}

/// Full message JSON for [account_id] (same shape as [frb_get_folder_message]).
pub fn frb_session_get_folder_message(
    account_id: String,
    folder_name: String,
    message_id: String,
) -> Result<String, String> {
    crate::session::session_get_folder_message(
        account_id.trim(),
        folder_name.trim(),
        message_id.trim(),
    )
}

/// Register store for mail-body HTTPS URLs; returns opaque key (after [frb_mail_body_server_init]).
pub fn frb_session_register_mail_body_store(account_id: String) -> Result<String, String> {
    crate::mail_body_server::ensure_mail_body_server()?;
    crate::session::session_register_mail_body_store(account_id.trim())
}

/// Load [FrbConfig] from `config.xml` at `xml_config_path`. Dart still receives JSON over FRB;
/// only `config.xml` is stored on disk (legacy `config.json` is migrated once then removed).
fn read_config(xml_config_path: &str) -> Option<FrbConfig> {
    let xml_path = Path::new(xml_config_path);
    let legacy_json = xml_path.parent().map(|p| p.join("config.json"));

    if xml_path.is_file() {
        match tagliacarte_core::tagliacarte_config_xml::load_tagliacarte_config(xml_path) {
            Ok(file) => {
                return Some(config_persist::frb_config_from_tagliacarte_file(&file));
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("tagliacarte: failed to parse config.xml: {e}");
                #[cfg(not(debug_assertions))]
                let _ = e;
            }
        }
    }

    if let Some(ref jp) = legacy_json {
        if jp.is_file() {
            let mut cfg = fs::read_to_string(jp)
                .ok()
                .and_then(|content| frb_json::parse_frb_config_json(&content).ok())
                .unwrap_or_default();
            merge_accounts_from_tagliacarte_xml(&mut cfg, xml_config_path);
            match write_config(xml_config_path, &cfg) {
                Ok(()) => {
                    if let Err(e) = fs::remove_file(jp) {
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "tagliacarte: wrote config.xml but could not remove legacy config.json: {e}"
                        );
                        #[cfg(not(debug_assertions))]
                        let _ = e;
                    } else {
                        #[cfg(debug_assertions)]
                        eprintln!("tagliacarte: migrated config.json → config.xml");
                    }
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("tagliacarte: config.xml migration write failed: {e}");
                    #[cfg(not(debug_assertions))]
                    let _ = e;
                }
            }
            return Some(cfg);
        }
    }

    let mut cfg = FrbConfig::default();
    merge_accounts_from_tagliacarte_xml(&mut cfg, xml_config_path);
    Some(cfg)
}

pub(super) fn config_xml_path() -> Option<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("TAGLIACARTE_CONFIG_DIR") {
        let p = std::path::PathBuf::from(dir).join("config.xml");
        if p.is_file() {
            return Some(p);
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = macos_real_user_home_dir() {
        let p = home.join(".tagliacarte").join("config.xml");
        if p.is_file() {
            return Some(p);
        }
    }
    let dir = default_config_dir()?;
    let p = dir.join("config.xml");
    p.is_file().then_some(p)
}

/// When a separate `config.xml` exists (beside the app `config.xml` path or under the global config dir), it overrides accounts and transports only (not UI prefs).
fn merge_accounts_from_tagliacarte_xml(cfg: &mut FrbConfig, primary_config_path: &str) {
    let Some(file) =
        config_persist::try_load_tagliacarte_xml(primary_config_path, config_xml_path())
    else {
        #[cfg(debug_assertions)]
        eprintln!(
            "tagliacarte: no auxiliary config.xml with stores (beside primary path or ~/.tagliacarte)"
        );
        return;
    };
    if let Err(e) = config_persist::apply_tagliacarte_file(cfg, &file) {
        #[cfg(debug_assertions)]
        eprintln!("tagliacarte: failed to apply config.xml: {e}");
        #[cfg(not(debug_assertions))]
        let _ = e;
        return;
    }
    #[cfg(debug_assertions)]
    eprintln!(
        "tagliacarte: merged {} store(s), {} transport(s) from XML",
        cfg.accounts.len(),
        cfg.transports.len()
    );
}

pub(super) fn persist_frb_config(path: &str, cfg: &FrbConfig) -> Result<(), String> {
    write_config(path, cfg)
}

fn validate_frb_config_for_save(cfg: &FrbConfig) -> Result<(), String> {
    for a in &cfg.accounts {
        if a.backend_type.eq_ignore_ascii_case("nostr") {
            let relays = a.lists.get("relayUrls").map(|v| v.as_slice()).unwrap_or(&[]);
            let count = relays.iter().filter(|s| !s.trim().is_empty()).count();
            if count == 0 {
                return Err(format!(
                    "Nostr account \"{}\" needs at least one relay URL",
                    a.label
                ));
            }
            let npub = a.attrs.get("npub").map(|s| s.trim()).unwrap_or("");
            if npub.is_empty() {
                return Err(format!(
                    "Nostr account \"{}\" needs an npub (create or link identity first)",
                    a.label
                ));
            }
        }
    }
    Ok(())
}

/// Writes only `config.xml` (merges unknown attributes on `<security>` / `<viewing>` / `<composing>` when the file already exists).
fn write_config(xml_config_path: &str, cfg: &FrbConfig) -> Result<(), String> {
    validate_frb_config_for_save(cfg)?;
    let xml_path = Path::new(xml_config_path);
    let mut file = config_persist::tagliacarte_file_from_frb(cfg)?;
    if xml_path.is_file() {
        if let Ok(existing) =
            tagliacarte_core::tagliacarte_config_xml::load_tagliacarte_config(xml_path)
        {
            file.security.attrs = config_persist::merge_pref_attr_maps(
                existing.security.attrs,
                file.security.attrs,
            );
            file.viewing.attrs = config_persist::merge_pref_attr_maps(
                existing.viewing.attrs,
                file.viewing.attrs,
            );
            file.composing.attrs = config_persist::merge_pref_attr_maps(
                existing.composing.attrs,
                file.composing.attrs,
            );
        }
    }
    if let Some(parent) = xml_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    file.write(xml_path).map_err(|e| e.to_string())
}
