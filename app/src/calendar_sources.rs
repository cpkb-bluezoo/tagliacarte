/*
 * calendar_sources.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Calendars the user has added by address: a CalDAV collection, and how to sign
 * in to it. Each source is one local calendar (`caldav:<id>`), refreshed by
 * pulling the collection. A password lives in the credential vault, never in the
 * calendar database; a source that signs in with one of the user's accounts
 * borrows that account's OAuth token when it syncs.
 */

use rusqlite::{Connection, params};
use url::Url;

use tagliacarte_core::config::{credentials_use_keychain, resolve_credentials_file_path, save_credential, set_credentials_backend};
use tagliacarte_core::oauth::OAuthTokenEntry;

use crate::calendar_caldav_sync::{CaldavAuth, pull_calendar_collection};
use crate::calendar_store::delete_calendar;
use crate::calendar_view::SyncReport;
use crate::mail_store::{OAuthService, load_mail_credential, oauth_provider_for_store, resolve_xoauth_secret};

/// What Google asks to be granted before it serves a calendar (by CalDAV or its API).
pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar";

/// How a source is signed in to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceAuth {
    None,
    /// A user name; the password is in the vault under [`credential_key`].
    Basic { username: String },
    /// An account of the user's that signs in with OAuth.
    Account { account_id: String },
}

impl SourceAuth {
    pub fn kind(&self) -> &'static str {
        match self {
            SourceAuth::None => "none",
            SourceAuth::Basic { .. } => "basic",
            SourceAuth::Account { .. } => "account",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarSource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub auth: SourceAuth,
    pub enabled: bool,
}

/// The calendar a source's events are kept in.
pub fn calendar_id(source_id: &str) -> String {
    format!("caldav:{source_id}")
}

/// Where a source's password is kept in the vault.
pub fn credential_key(source_id: &str) -> String {
    format!("caldav:{source_id}")
}

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarSource> {
    let kind: String = r.get(3)?;
    let username: String = r.get(4)?;
    let account_id: String = r.get(5)?;
    Ok(CalendarSource {
        id: r.get(0)?,
        name: r.get(1)?,
        url: r.get(2)?,
        auth: match kind.as_str() {
            "basic" => SourceAuth::Basic { username },
            "account" => SourceAuth::Account { account_id },
            _ => SourceAuth::None,
        },
        enabled: r.get::<_, i64>(6)? != 0,
    })
}

const COLUMNS: &str = "source_id, name, url, auth, username, account_id, enabled";

/// The sources, by name.
pub fn list_sources(conn: &Connection) -> Result<Vec<CalendarSource>, String> {
    let mut st = conn
        .prepare(&format!("SELECT {COLUMNS} FROM calendar_sources ORDER BY name COLLATE NOCASE, source_id"))
        .map_err(|e| e.to_string())?;
    let rows = st.query_map([], from_row).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn get_source(conn: &Connection, id: &str) -> Result<Option<CalendarSource>, String> {
    let mut st = conn.prepare(&format!("SELECT {COLUMNS} FROM calendar_sources WHERE source_id = ?1")).map_err(|e| e.to_string())?;
    let mut rows = st.query_map(params![id], from_row).map_err(|e| e.to_string())?;
    rows.next().transpose().map_err(|e| e.to_string())
}

/// Makes the source, or changes the one with this id. Only web addresses are taken.
pub fn save_source(conn: &Connection, s: &CalendarSource) -> Result<(), String> {
    let id = s.id.trim();
    if id.is_empty() {
        return Err("a calendar source needs an id".to_owned());
    }
    if s.name.trim().is_empty() {
        return Err("a calendar needs a name".to_owned());
    }
    let url = Url::parse(s.url.trim()).map_err(|e| format!("not a web address: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("a calendar address must start with http:// or https://".to_owned());
    }
    let (username, account_id) = match &s.auth {
        SourceAuth::None => (String::new(), String::new()),
        SourceAuth::Basic { username } => (username.trim().to_owned(), String::new()),
        SourceAuth::Account { account_id } => {
            if account_id.trim().is_empty() {
                return Err("choose the account to sign in with".to_owned());
            }
            (String::new(), account_id.trim().to_owned())
        }
    };
    conn.execute(
        "INSERT INTO calendar_sources (source_id, name, url, auth, username, account_id, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(source_id) DO UPDATE SET name = excluded.name, url = excluded.url, auth = excluded.auth,
             username = excluded.username, account_id = excluded.account_id, enabled = excluded.enabled",
        params![id, s.name.trim(), url.as_str(), s.auth.kind(), username, account_id, i64::from(s.enabled)],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Removes the source and its calendar with the events in it. (A password in the
/// vault stays there: the credential file can only be added to.)
pub fn delete_source(conn: &Connection, id: &str) -> Result<(), String> {
    delete_calendar(conn, &calendar_id(id))?;
    conn.execute("DELETE FROM calendar_sources WHERE source_id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Puts a source's password in the vault.
pub fn save_password(id: &str, username: &str, password: &str) -> Result<(), String> {
    set_credentials_backend(crate::frb_api::primary_config_use_keychain()?);
    let path = resolve_credentials_file_path().ok_or_else(|| "could not resolve credentials path".to_owned())?;
    save_credential(&path, &credential_key(id), username.trim(), password)
}

/// Whether the vault holds a password for the source.
pub fn has_password(id: &str) -> bool {
    load_mail_credential(&credential_key(id), credentials_use_keychain()).is_ok_and(|e| !e.password_or_token.is_empty())
}

/// Google serves a calendar only to a grant that included the calendar. A stored OAuth
/// grant that says it did not is turned down here, with the way forward; a bare access
/// token says nothing about its scopes and is tried.
pub fn check_calendar_scope(stored_secret: &str) -> Result<(), String> {
    let s = stored_secret.trim_start();
    if !s.starts_with('{') {
        return Ok(());
    }
    match OAuthTokenEntry::from_json(s) {
        Some(e) if e.has_scope(CALENDAR_SCOPE) => Ok(()),
        _ => Err("This account has not allowed access to its calendar. Sign in to it again (Settings, Accounts) and allow it.".to_owned()),
    }
}

/// What to sign in with for this source, now.
pub fn resolve_auth(s: &CalendarSource) -> Result<CaldavAuth, String> {
    match &s.auth {
        SourceAuth::None => Ok(CaldavAuth::None),
        SourceAuth::Basic { username } => {
            let entry = load_mail_credential(&credential_key(&s.id), crate::frb_api::primary_config_use_keychain()?)
                .map_err(|_| "no saved password for this calendar; edit it and enter the password".to_owned())?;
            let user = if username.trim().is_empty() { entry.username.trim().to_owned() } else { username.trim().to_owned() };
            Ok(CaldavAuth::Basic { username: user, password: entry.password_or_token })
        }
        SourceAuth::Account { account_id } => {
            let (acc, use_keychain) = crate::frb_api::resolve_mail_account(account_id)?;
            let service = oauth_provider_for_store(&acc).ok_or_else(|| "this account does not sign in with OAuth".to_owned())?;
            if matches!(service, OAuthService::Google) {
                check_calendar_scope(&load_mail_credential(acc.id.trim(), use_keychain)?.password_or_token)?;
            }
            let (_, token) = resolve_xoauth_secret(service, acc.id.trim(), use_keychain)?;
            Ok(CaldavAuth::Bearer(token))
        }
    }
}

/// Pulls every source that is on, signing in as `resolve` says. A source that cannot be
/// signed in to or pulled is reported by name and the rest carry on. Waits on the network.
pub fn sync_sources_with(conn: &Connection, resolve: &mut dyn FnMut(&CalendarSource) -> Result<CaldavAuth, String>) -> SyncReport {
    let mut report = SyncReport::default();
    let sources = match list_sources(conn) {
        Ok(s) => s,
        Err(e) => {
            report.failed.push(("calendar sources".to_owned(), e));
            return report;
        }
    };
    for s in sources.iter().filter(|s| s.enabled) {
        let pulled = resolve(s).and_then(|auth| pull_calendar_collection(conn, &calendar_id(&s.id), &s.id, &s.name, &s.url, &auth));
        match pulled {
            Ok(o) => {
                report.calendars += 1;
                report.events += o.imported_events;
            }
            Err(e) => report.failed.push((s.name.clone(), e)),
        }
    }
    report
}

/// [`sync_sources_with`], signing in as each source says.
pub fn sync_sources(conn: &Connection) -> SyncReport {
    sync_sources_with(conn, &mut resolve_auth)
}
