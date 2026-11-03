//! Accounts, transports, credentials, OAuth sign-in and Nostr identities.
//!
//! Each command touches one entity. An account arrives as one JSON object
//! (the same shape `configAccount` events use) and is parsed with the app's
//! own streaming parser; a transport is a flat object. The result of a change
//! is the configuration again as events, so the page never holds a copy it
//! has to keep in step. Anything that waits on a person or a network (OAuth,
//! SMTP verification, Nostr relays) returns at once and reports with an event.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tagliacarte_app::frb_api::frb_json::parse_frb_account_json;
use tagliacarte_app::frb_api::{
    FrbAccount, FrbConfig, FrbTransport, frb_gmail_oauth_sign_in, frb_nostr_generate_keypair,
    frb_nostr_get_public_key_from_secret, frb_nostr_hex_to_npub, frb_nostr_publish_profile,
    frb_nostr_secret_key_to_hex, frb_nostr_sync_remote_profile, frb_save_store_credential,
    frb_save_transport_credential, frb_smtp_google_oauth_sign_in, frb_store_has_saved_password,
    frb_verify_smtp_transport,
};
use tagliacarte_app::session;
use tagliacarte_core::json::{JsonWriter, writer_into_string};

use crate::args::Args;
use crate::commands::Ctx;
use crate::events;
use crate::settings::{edit_config, load_for_edit};

fn new_id(prefix: char) -> String {
    let micros = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_micros()).unwrap_or(0);
    format!("{prefix}{micros}")
}

fn object(pairs: &[(&str, &str)]) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    for (k, v) in pairs {
        w.write_key(k);
        w.write_string(v);
    }
    w.write_end_object();
    writer_into_string(w)
}

/// Replaces the account with the same id where it stands, or appends it. The
/// last folder and message the user was viewing are kept unless the new
/// account names them.
fn upsert_account(cfg: &mut FrbConfig, mut acc: FrbAccount) {
    match cfg.accounts.iter_mut().find(|a| a.id == acc.id) {
        Some(slot) => {
            if acc.last_folder.is_none() {
                acc.last_folder = slot.last_folder.take();
            }
            if acc.last_message_id.is_none() {
                acc.last_message_id = slot.last_message_id.take();
            }
            *slot = acc;
        }
        None => cfg.accounts.push(acc),
    }
}

/// Drops a transport and removes it from the accounts that listed it. An
/// account that did not list it is left exactly as it was.
fn remove_transport(cfg: &mut FrbConfig, id: &str) -> bool {
    let before = cfg.transports.len();
    cfg.transports.retain(|t| t.id != id);
    for a in &mut cfg.accounts {
        if let Some(list) = a.lists.get_mut("transportIds") {
            list.retain(|t| t != id);
        }
    }
    cfg.transports.len() != before
}

fn transport_from_args(a: &Args) -> Result<FrbTransport, String> {
    let display_name = a.str("displayName")?.trim().to_owned();
    let host = a.str("host")?.trim().to_owned();
    if display_name.is_empty() || host.is_empty() {
        return Err("display name and host are required".to_owned());
    }
    let port = a.u64("port")?;
    if !(1..=65535).contains(&port) {
        return Err("port must be between 1 and 65535".to_owned());
    }
    let id = a.opt_str("id").map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    Ok(FrbTransport {
        id: id.unwrap_or_else(|| new_id('t')),
        transport_type: "smtp".to_owned(),
        display_name,
        host,
        port: port as u16,
        security: a.opt_str("security").unwrap_or("tls").to_owned(),
        default_from: a.opt_str("defaultFrom").unwrap_or("").trim().to_owned(),
        dsn_notify: a.opt_str("dsnNotify").unwrap_or("failure").to_owned(),
        oauth_provider: a.opt_str("oauthProvider").unwrap_or("").trim().to_ascii_lowercase(),
    })
}

/// Saves the configuration change `edit` makes, re-reads the session's
/// accounts, and sends the new configuration to the page as events.
fn change<T>(
    ctx: &Ctx,
    edit: impl FnOnce(&mut FrbConfig) -> Result<T, String>,
) -> Result<T, String> {
    let (out, cfg) = edit_config(&ctx.config_path, edit)?;
    session::reload_session_accounts(&ctx.config_path)?;
    ctx.hub.sequence(|| events::config_events(&cfg, |e| ctx.hub.push(e)));
    Ok(out)
}

/// Runs `work` on its own thread and reports on the event stream when it
/// ends: `kind` events carry the request id and, on failure, the error.
fn background(ctx: &Ctx, kind: &'static str, request_id: String, work: impl FnOnce() -> Result<(), String> + Send + 'static) {
    let hub = Arc::clone(&ctx.hub);
    std::thread::spawn(move || {
        let r = work();
        hub.push(events::done_json(kind, &request_id, r.err().as_deref()));
    });
}

/// `Some(result)` if `name` is one of this module's commands.
pub fn run(ctx: &Ctx, name: &str, body: &str) -> Option<Result<Option<String>, String>> {
    let r = match name {
        "account.save" => account_save(ctx, body),
        "account.remove" => account_remove(ctx, body),
        "transport.save" => transport_save(ctx, body),
        "transport.remove" => transport_remove(ctx, body),
        "transport.verify" => transport_verify(ctx, body),
        "credential.saveStore" => credential_save(body, false),
        "credential.saveTransport" => credential_save(body, true),
        "credential.hasStorePassword" => has_password(body),
        "oauth.google" => oauth_google(ctx, body),
        "nostr.generateKeypair" => nostr_generate(),
        "nostr.linkSecret" => nostr_link(body),
        "nostr.syncProfile" => nostr_profile(ctx, body, true),
        "nostr.publishProfile" => nostr_profile(ctx, body, false),
        _ => return None,
    };
    Some(r)
}

/// What the page sends is tidied before it is saved: names and the picture are
/// trimmed, and an empty picture is no picture.
fn tidy_account(acc: &mut FrbAccount) {
    acc.label = acc.label.trim().to_owned();
    acc.avatar_url = acc.avatar_url.take().map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
}

fn account_save(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let mut acc = parse_frb_account_json(body)?;
    tidy_account(&mut acc);
    if acc.label.is_empty() {
        return Err("account name is required".to_owned());
    }
    if acc.id.trim().is_empty() {
        acc.id = new_id('s');
    }
    let id = acc.id.clone();
    change(ctx, |cfg| {
        upsert_account(cfg, acc);
        Ok(())
    })?;
    Ok(Some(object(&[("id", &id)])))
}

fn account_remove(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let id = Args::parse(body)?.str("id")?;
    change(ctx, |cfg| {
        let before = cfg.accounts.len();
        cfg.accounts.retain(|a| a.id != id);
        if cfg.accounts.len() == before {
            return Err(format!("no such account '{id}'"));
        }
        Ok(())
    })?;
    Ok(None)
}

fn transport_save(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let t = transport_from_args(&Args::parse(body)?)?;
    let id = t.id.clone();
    change(ctx, |cfg| {
        match cfg.transports.iter_mut().find(|x| x.id == t.id) {
            Some(slot) => *slot = t,
            None => cfg.transports.push(t),
        }
        Ok(())
    })?;
    Ok(Some(object(&[("id", &id)])))
}

fn transport_remove(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let id = Args::parse(body)?.str("id")?;
    change(ctx, |cfg| {
        if remove_transport(cfg, &id) { Ok(()) } else { Err(format!("no such transport '{id}'")) }
    })?;
    Ok(None)
}

fn transport_verify(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    let (id, request_id) = (a.str("id")?, a.str("requestId")?);
    background(ctx, "transportVerified", request_id, move || frb_verify_smtp_transport(id));
    Ok(None)
}

fn credential_save(body: &str, transport: bool) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    let (username, password) = (a.str("username")?, a.str("password")?);
    if username.trim().is_empty() || password.is_empty() {
        return Err("a username and a password are required".to_owned());
    }
    if transport {
        frb_save_transport_credential(a.str("transportId")?, username, password)?;
    } else {
        frb_save_store_credential(a.str("accountId")?, username, password)?;
    }
    Ok(None)
}

fn has_password(body: &str) -> Result<Option<String>, String> {
    let yes = frb_store_has_saved_password(Args::parse(body)?.str("accountId")?).unwrap_or(false);
    Ok(Some(if yes { "true" } else { "false" }.to_owned()))
}

/// Browser sign-in with Google, for a mail account or an SMTP transport. The
/// flow waits for the person at the browser, so it runs on its own thread.
fn oauth_google(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    let (kind, id, request_id) = (a.str("kind")?, a.str("id")?, a.str("requestId")?);
    match kind.as_str() {
        "store" => background(ctx, "oauthFinished", request_id, move || frb_gmail_oauth_sign_in(id)),
        "transport" => background(ctx, "oauthFinished", request_id, move || frb_smtp_google_oauth_sign_in(id)),
        other => return Err(format!("unknown OAuth target '{other}'")),
    }
    Ok(None)
}

fn nostr_generate() -> Result<Option<String>, String> {
    let kp = frb_nostr_generate_keypair()?;
    let npub = frb_nostr_hex_to_npub(kp.pubkey_hex.clone())?;
    Ok(Some(object(&[("secretHex", &kp.secret_hex), ("pubkeyHex", &kp.pubkey_hex), ("npub", &npub)])))
}

/// Takes an nsec or 64-digit hex secret, stores it in the vault for the
/// account, and returns the public key as an npub. The secret goes no further.
fn nostr_link(body: &str) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    let account_id = a.str("accountId")?;
    let secret = a.str("secret")?;
    let hex = frb_nostr_secret_key_to_hex(secret)?;
    let pubkey = frb_nostr_get_public_key_from_secret(hex.clone())?;
    let npub = frb_nostr_hex_to_npub(pubkey)?;
    frb_save_store_credential(account_id, npub.clone(), hex)?;
    Ok(Some(object(&[("npub", &npub)])))
}

/// Pulls the profile and relay list from the relays (and then shows the
/// configuration again), or publishes this account's profile.
fn nostr_profile(ctx: &Ctx, body: &str, pull: bool) -> Result<Option<String>, String> {
    let account_id = Args::parse(body)?.str("accountId")?;
    let path = ctx.config_path.clone();
    let hub = Arc::clone(&ctx.hub);
    background(ctx, if pull { "nostrProfileSynced" } else { "nostrProfilePublished" }, account_id.clone(), move || {
        if pull {
            frb_nostr_sync_remote_profile(path.clone(), account_id)?;
            let cfg = load_for_edit(&path)?;
            hub.sequence(|| events::config_events(&cfg, |e| hub.push(e)));
            Ok(())
        } else {
            frb_nostr_publish_profile(path, account_id)
        }
    });
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use std::collections::HashMap;

    fn account(id: &str, label: &str) -> FrbAccount {
        FrbAccount { id: id.into(), label: label.into(), ..FrbAccount::default() }
    }

    #[test]
    fn editing_an_account_keeps_its_place_and_its_last_viewed_folder() {
        let mut cfg = FrbConfig::default();
        let mut one = account("s1", "One");
        one.last_folder = Some("INBOX".into());
        one.last_message_id = Some("42".into());
        cfg.accounts = vec![one, account("s2", "Two"), account("s3", "Three")];

        upsert_account(&mut cfg, account("s1", "One renamed"));
        let ids: Vec<&str> = cfg.accounts.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["s1", "s2", "s3"], "an edit does not reorder");
        assert_eq!(cfg.accounts[0].label, "One renamed");
        assert_eq!(cfg.accounts[0].last_folder.as_deref(), Some("INBOX"));
        assert_eq!(cfg.accounts[0].last_message_id.as_deref(), Some("42"));

        upsert_account(&mut cfg, account("s4", "Four"));
        assert_eq!(cfg.accounts.len(), 4);
        assert_eq!(cfg.accounts[3].id, "s4");
    }

    #[test]
    fn a_saved_accounts_picture_is_trimmed_and_an_empty_one_is_none() {
        let parse = |body: &str| {
            let mut a = parse_frb_account_json(body).unwrap();
            tidy_account(&mut a);
            a
        };
        let kept = parse(r#"{"id":"s1","label":"  Work  ","backendType":"imap","avatarUrl":"  https://example.com/me.png "}"#);
        assert_eq!(kept.label, "Work");
        assert_eq!(kept.avatar_url.as_deref(), Some("https://example.com/me.png"));
        assert_eq!(parse(r#"{"id":"s1","label":"W","backendType":"imap","avatarUrl":"   "}"#).avatar_url, None);
        assert_eq!(parse(r#"{"id":"s1","label":"W","backendType":"imap"}"#).avatar_url, None);
    }

    #[test]
    fn removing_a_transport_touches_only_the_accounts_that_used_it() {
        let mut cfg = FrbConfig::default();
        cfg.transports = vec![
            FrbTransport { id: "t1".into(), ..transport_for_test() },
            FrbTransport { id: "t2".into(), ..transport_for_test() },
        ];
        let mut a = account("s1", "A");
        a.lists = HashMap::from([("transportIds".to_owned(), vec!["t1".to_owned(), "t2".to_owned()])]);
        let b = account("s2", "B");
        cfg.accounts = vec![a, b];

        assert!(remove_transport(&mut cfg, "t1"));
        assert_eq!(cfg.transports.len(), 1);
        assert_eq!(cfg.accounts[0].lists["transportIds"], ["t2"]);
        assert!(cfg.accounts[1].lists.is_empty(), "an account that never listed it stays empty");
        assert!(!remove_transport(&mut cfg, "t1"));
    }

    fn transport_for_test() -> FrbTransport {
        FrbTransport {
            id: String::new(),
            transport_type: "smtp".into(),
            display_name: "x".into(),
            host: "h".into(),
            port: 587,
            security: "starttls".into(),
            default_from: String::new(),
            dsn_notify: "failure".into(),
            oauth_provider: String::new(),
        }
    }

    #[test]
    fn transport_arguments_are_validated() {
        let ok = |j: &str| transport_from_args(&Args::parse(j).unwrap());
        let t = ok(r#"{"displayName":" Work ","host":"smtp.example.org","port":587,"oauthProvider":"Google"}"#).unwrap();
        assert_eq!((t.display_name.as_str(), t.port, t.oauth_provider.as_str()), ("Work", 587, "google"));
        assert!(t.id.starts_with('t'));
        assert_eq!(t.security, "tls");
        assert!(ok(r#"{"displayName":"","host":"h","port":25}"#).is_err());
        assert!(ok(r#"{"displayName":"n","host":"h","port":0}"#).is_err());
        assert!(ok(r#"{"displayName":"n","host":"h","port":70000}"#).is_err());
        assert_eq!(ok(r#"{"id":"t9","displayName":"n","host":"h","port":25}"#).unwrap().id, "t9");
    }

    #[test]
    fn an_account_from_the_page_parses_in_the_config_event_shape() {
        let acc = parse_frb_account_json(
            r#"{"id":"s7","label":"Work","backendType":"imap",
                "attrs":{"host":"imap.example.org","port":"993","security":"tls"},
                "lists":{"transportIds":["t1","t2"]}}"#,
        )
        .unwrap();
        assert_eq!(acc.attrs["host"], "imap.example.org");
        assert_eq!(acc.lists["transportIds"], ["t1", "t2"]);
        let r = Value::parse(&object(&[("id", "s7")])).unwrap();
        assert_eq!(r.opt_str("id"), Some("s7"));
    }
}
