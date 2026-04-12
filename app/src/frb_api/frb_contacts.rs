/*
 * frb_contacts.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

use base64::Engine;

use crate::carddav_sync;
use crate::contacts_store;
use crate::contacts_vcard_import;
use crate::vcard_lite;
use rusqlite::params;
use rusqlite::OptionalExtension;
use std::collections::HashSet;
use std::sync::Mutex;
use tagliacarte_core::config::tagliacarte_data_dir;
use tagliacarte_core::mime::{emit_message_parts, parse_email_address_list};

static CONTACTS: Mutex<Option<rusqlite::Connection>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub struct FrbContactSearchRow {
    pub id: i64,
    pub display_name: String,
    pub emails: Vec<String>,
}

/// Inline compose recipient completion: [full_address] on Tab; [ghost_suffix] is grey hint after the typed prefix.
#[derive(Debug, Clone)]
pub struct FrbRecipientCompletion {
    pub full_address: String,
    pub ghost_suffix: String,
}

#[derive(Debug, Clone)]
pub struct FrbContactRepositoryRow {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub base_url: String,
    pub collection_path: String,
    pub credential_key: String,
    pub default_new_contact: bool,
    pub ctag: String,
    pub last_collection_sync_at: Option<i64>,
    pub sync_error: String,
}

#[derive(Debug, Clone)]
pub struct FrbContactGroupRow {
    pub id: i64,
    pub name: String,
    pub color_argb: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FrbContactCompactRow {
    pub id: i64,
    pub display_name: String,
    pub externally_share_ok: bool,
    pub import_origin: String,
    pub primary_email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrbGroupRepositoryTargetRow {
    pub group_id: i64,
    pub repository_id: i64,
}

#[derive(Debug, Clone)]
pub struct FrbContactGroupMemberRow {
    pub id: i64,
    pub display_name: String,
    pub primary_email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrbContactRepositoryLinkRow {
    pub repository_id: i64,
    pub name: String,
    pub kind: String,
    pub linked: bool,
    pub local_dirty: bool,
}

/// Upsert payload for [frb_contacts_repository_upsert] (replaces JSON wire format).
#[derive(Debug, Clone)]
pub struct FrbRepositoryUpsert {
    pub id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub enabled: Option<bool>,
    pub base_url: Option<String>,
    pub collection_path: Option<String>,
    pub credential_key: Option<String>,
    pub default_new_contact: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct FrbContactsRowId {
    pub id: i64,
}

/// Upsert payload for [frb_contacts_group_upsert].
#[derive(Debug, Clone)]
pub struct FrbGroupUpsert {
    pub id: Option<i64>,
    pub name: String,
    pub color_argb: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FrbPlatformContactItem {
    pub display_name: Option<String>,
    pub emails: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FrbMergePlatformContacts {
    pub items: Vec<FrbPlatformContactItem>,
    pub repository_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FrbMergePlatformResult {
    pub imported: i32,
}

#[derive(Debug, Clone)]
pub struct FrbContactEmailRow {
    pub email: String,
    pub label: String,
}

/// Full contact row for [frb_contacts_get] / [frb_contacts_lookup_by_email].
#[derive(Debug, Clone)]
pub struct FrbContactDetail {
    pub id: i64,
    pub display_name: String,
    pub notes: String,
    pub import_origin: String,
    pub externally_share_ok: bool,
    pub pgp_fingerprint: Option<String>,
    pub pgp_key_path: Option<String>,
    pub smime_cert_path: Option<String>,
    pub smime_notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub emails: Vec<FrbContactEmailRow>,
}

#[derive(Debug, Clone)]
pub struct FrbContactEmailInput {
    pub email: String,
    pub label: Option<String>,
}

/// Upsert payload for [frb_contacts_upsert].
#[derive(Debug, Clone)]
pub struct FrbContactUpsert {
    pub id: Option<i64>,
    pub display_name: Option<String>,
    pub notes: Option<String>,
    pub import_origin: Option<String>,
    pub externally_share_ok: Option<bool>,
    pub emails: Option<Vec<FrbContactEmailInput>>,
    pub pgp_fingerprint: Option<String>,
    pub pgp_key_path: Option<String>,
    pub smime_cert_path: Option<String>,
    pub smime_notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrbLearnFromMailResult {
    pub id: i64,
    pub updated: bool,
}

#[derive(Debug, Clone)]
pub struct FrbContactsApplyGroupRulesResult {
    pub materialized: i64,
}

#[derive(Debug, Clone)]
pub struct FrbImportVcardResult {
    pub imported: i32,
}

/// vCard 3.0/4.0 text from [frb_contacts_export_vcard] (opaque wire format, not JSON).
#[derive(Debug, Clone)]
pub struct FrbExportedVcard {
    pub vcard_text: String,
}

#[derive(Debug, Clone)]
pub struct FrbVcardEmailRow {
    pub addr: String,
    pub type_label: String,
}

#[derive(Debug, Clone)]
pub struct FrbParsedTelRow {
    pub number: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct FrbParsedUrlRow {
    pub url: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct FrbParsedAdrRow {
    pub label: String,
    pub po_box: String,
    pub extended: String,
    pub street: String,
    pub locality: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
}

#[derive(Debug, Clone)]
pub struct FrbParsedPhotoRow {
    pub mime_type: String,
    /// Embedded PHOTO; empty when [source_uri] is set.
    pub data_base64: String,
    /// `PHOTO;VALUE=uri` when not embedded.
    pub source_uri: String,
}

#[derive(Debug, Clone)]
pub struct FrbParsedVcard {
    pub formatted_name: String,
    pub nickname: String,
    pub organization: String,
    pub title: String,
    pub birthday: String,
    pub emails: Vec<FrbVcardEmailRow>,
    pub tels: Vec<FrbParsedTelRow>,
    pub urls: Vec<FrbParsedUrlRow>,
    pub addresses: Vec<FrbParsedAdrRow>,
    pub photos: Vec<FrbParsedPhotoRow>,
    pub key_raw: Option<String>,
    pub cert_raw: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrbCarddavPullResult {
    pub ok: bool,
    pub fetched_resources: i32,
    pub imported_contacts: i32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct FrbCarddavPushResult {
    pub ok: bool,
    pub pushed: i32,
    pub failed: i32,
    pub message: String,
}

impl From<crate::carddav_sync::CarddavPullOutcome> for FrbCarddavPullResult {
    fn from(o: crate::carddav_sync::CarddavPullOutcome) -> Self {
        Self {
            ok: o.ok,
            fetched_resources: o.fetched_resources,
            imported_contacts: o.imported_contacts,
            message: o.message,
        }
    }
}

impl From<crate::carddav_sync::CarddavPushOutcome> for FrbCarddavPushResult {
    fn from(o: crate::carddav_sync::CarddavPushOutcome) -> Self {
        Self {
            ok: o.ok,
            pushed: o.pushed,
            failed: o.failed,
            message: o.message,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrbContactsSyncRepositoryResult {
    pub ok: bool,
    pub message: String,
}

fn with_db<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    let mut g = CONTACTS
        .lock()
        .map_err(|_| "contacts db lock poisoned".to_string())?;
    if g.is_none() {
        let dir = tagliacarte_data_dir().ok_or_else(|| {
            "data directory unavailable (set TAGLIACARTE_DATA_DIR or use default app data path)"
                .to_string()
        })?;
        *g = Some(contacts_store::open_contacts_db(&dir)?);
    }
    f(g.as_ref().unwrap())
}

fn normalize_email(s: &str) -> String {
    s.trim().to_lowercase()
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn contact_row_to_detail(c: &rusqlite::Connection, contact_id: i64) -> Result<FrbContactDetail, String> {
    let row = c
        .query_row(
            r#"SELECT id, display_name, notes, import_origin, externally_share_ok,
                      pgp_fingerprint, pgp_key_path, smime_cert_path, smime_notes,
                      created_at, updated_at
               FROM contacts WHERE id = ?1"#,
            [contact_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, i64>(10)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut stmt = c
        .prepare("SELECT email, label FROM contact_emails WHERE contact_id = ?1 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let emails: Vec<FrbContactEmailRow> = stmt
        .query_map([contact_id], |r| {
            Ok(FrbContactEmailRow {
                email: r.get(0)?,
                label: r.get::<_, String>(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(FrbContactDetail {
        id: row.0,
        display_name: row.1,
        notes: row.2,
        import_origin: row.3,
        externally_share_ok: row.4 != 0,
        pgp_fingerprint: row.5,
        pgp_key_path: row.6,
        smime_cert_path: row.7,
        smime_notes: row.8,
        created_at: row.9,
        updated_at: row.10,
        emails,
    })
}

/// Search contacts for autocomplete (display name + email).
pub fn frb_contacts_search(query: String, limit: i64) -> Result<Vec<FrbContactSearchRow>, String> {
    let lim = limit.clamp(1, 200);
    with_db(|c| {
        let rows = contacts_store::search_contacts(c, query.trim(), lim)?;
        Ok(rows
            .into_iter()
            .map(|(id, display_name, emails)| FrbContactSearchRow {
                id,
                display_name,
                emails,
            })
            .collect())
    })
}

/// Best recipient completion for the current comma-separated tail (addr-spec or display-name prefix).
pub fn frb_contacts_recipient_completion(
    prefix: String,
) -> Result<Option<FrbRecipientCompletion>, String> {
    with_db(|c| {
        let opt = contacts_store::recipient_completion_best(c, prefix.as_str())?;
        Ok(opt.map(|(full_address, ghost_suffix)| FrbRecipientCompletion {
            full_address,
            ghost_suffix,
        }))
    })
}

/// Increment compose-frequency metrics for addresses (normalized `local@domain`) after a successful send.
pub(crate) fn contacts_bump_compose_to_counts(normalized_emails: &[String]) {
    let mut v: Vec<String> = normalized_emails
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if v.is_empty() {
        return;
    }
    v.sort();
    v.dedup();
    let _ = with_db(|c| contacts_store::bump_compose_to_counts(c, &v));
}

pub fn frb_contacts_get(contact_id: i64) -> Result<FrbContactDetail, String> {
    with_db(|c| contact_row_to_detail(c, contact_id))
}

/// Lookup one contact matching email (normalized). When several contacts share the same address,
/// the lowest [contact_id] wins (merge UI can reconcile later).
pub fn frb_contacts_lookup_by_email(email: String) -> Result<Option<FrbContactDetail>, String> {
    let e = normalize_email(&email);
    if e.is_empty() {
        return Ok(None);
    }
    with_db(|c| {
        let id: Option<i64> = c
            .query_row(
                "SELECT contact_id FROM contact_emails WHERE lower(email) = lower(?1) ORDER BY contact_id LIMIT 1",
                [e.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match id {
            Some(i) => Ok(Some(contact_row_to_detail(c, i)?)),
            None => Ok(None),
        }
    })
}

pub fn frb_contacts_upsert(u: FrbContactUpsert) -> Result<FrbContactsRowId, String> {
    with_db(|c| {
        let t = now_ms();
        let display = u.display_name.unwrap_or_default();
        let notes = u.notes.unwrap_or_default();
        let origin = u.import_origin.unwrap_or_else(|| "manual".to_string());
        let share_ok = if let Some(s) = u.externally_share_ok {
            if s { 1 } else { 0 }
        } else if origin == "learned_from_mail" {
            0
        } else {
            1
        };
        let id = if let Some(id) = u.id {
            c.execute(
                r#"UPDATE contacts SET display_name = ?1, notes = ?2, import_origin = ?3,
                    externally_share_ok = ?4, pgp_fingerprint = ?5, pgp_key_path = ?6,
                    smime_cert_path = ?7, smime_notes = ?8, updated_at = ?9
                  WHERE id = ?10"#,
                params![
                    display,
                    notes,
                    origin,
                    share_ok,
                    u.pgp_fingerprint,
                    u.pgp_key_path,
                    u.smime_cert_path,
                    u.smime_notes,
                    t,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
            id
        } else {
            c.execute(
                r#"INSERT INTO contacts (display_name, notes, import_origin, externally_share_ok,
                    pgp_fingerprint, pgp_key_path, smime_cert_path, smime_notes, created_at, updated_at)
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                params![
                    display,
                    notes,
                    origin,
                    share_ok,
                    u.pgp_fingerprint,
                    u.pgp_key_path,
                    u.smime_cert_path,
                    u.smime_notes,
                    t,
                    t
                ],
            )
            .map_err(|e| e.to_string())?;
            c.last_insert_rowid()
        };
        if let Some(emails) = u.emails {
            c.execute("DELETE FROM contact_emails WHERE contact_id = ?1", [id])
                .map_err(|e| e.to_string())?;
            for e in emails {
                let addr = normalize_email(&e.email);
                if addr.is_empty() {
                    continue;
                }
                c.execute(
                    "INSERT INTO contact_emails (contact_id, email, label) VALUES (?1, ?2, ?3)",
                    params![id, addr, e.label.unwrap_or_default()],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        contacts_store::refresh_contact_fts(c, id)?;
        Ok(FrbContactsRowId { id })
    })
}

pub fn frb_contacts_delete(contact_id: i64) -> Result<(), String> {
    with_db(|c| {
        c.execute("DELETE FROM contacts WHERE id = ?1", [contact_id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn frb_contacts_validate_external_sharing(contact_id: i64, ok: bool) -> Result<(), String> {
    with_db(|c| {
        c.execute(
            "UPDATE contacts SET externally_share_ok = ?1, updated_at = ?2 WHERE id = ?3",
            params![if ok { 1 } else { 0 }, now_ms(), contact_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// One mailbox from [tagliacarte_core::mime::parse_email_address_list] for Flutter.
/// [display_name] is empty when the address is addr-spec only (no phrase).
#[derive(Debug, Clone)]
pub struct FrbParsedEmailAddressRow {
    pub display_name: String,
    pub email: String,
}

/// Parse a `From` / `To` / `Cc`-style header using the in-tree RFC 5322 address parser.
pub fn frb_contacts_parse_email_address_list(
    header_value: String,
) -> Result<Vec<FrbParsedEmailAddressRow>, String> {
    let value = header_value.trim();
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let Some(list) = parse_email_address_list(value) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(list.len());
    for a in list {
        let email = a.address();
        let display_name = a
            .display_name()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_default();
        out.push(FrbParsedEmailAddressRow {
            display_name,
            email,
        });
    }
    Ok(out)
}

pub fn frb_contacts_learn_from_mail(
    display_name: String,
    email: String,
) -> Result<FrbLearnFromMailResult, String> {
    let addr = normalize_email(&email);
    if addr.is_empty() {
        return Err("empty email".to_string());
    }
    with_db(|c| {
        let existing: Option<i64> = c
            .query_row(
                "SELECT contact_id FROM contact_emails WHERE lower(email) = lower(?1) ORDER BY contact_id LIMIT 1",
                [addr.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(id) = existing {
            return Ok(FrbLearnFromMailResult {
                id,
                updated: false,
            });
        }
        let t = now_ms();
        c.execute(
            r#"INSERT INTO contacts (display_name, notes, import_origin, externally_share_ok, created_at, updated_at)
               VALUES (?1, '', 'learned_from_mail', 0, ?2, ?2)"#,
            params![display_name.trim(), t],
        )
        .map_err(|e| e.to_string())?;
        let id = c.last_insert_rowid();
        c.execute(
            "INSERT INTO contact_emails (contact_id, email, label) VALUES (?1, ?2, '')",
            params![id, addr],
        )
        .map_err(|e| e.to_string())?;
        contacts_store::refresh_contact_fts(c, id)?;
        Ok(FrbLearnFromMailResult {
            id,
            updated: true,
        })
    })
}

/// When a classic mailbox message is opened ([`crate::frb_api::frb_mail::get_folder_message_detail`]),
/// best-effort insert `learned_from_mail` rows for addresses in From/To/Cc.
///
/// Policy matches Flutter `isEmailMailboxBackend` (see `mail_account_policy.dart`).
pub(crate) fn contacts_learn_from_message_headers_if_email_backend(
    backend_type: &str,
    from: &str,
    to: &str,
    cc: Option<&str>,
) {
    let b = backend_type.trim().to_ascii_lowercase();
    if b == "nostr" || b == "matrix" {
        return;
    }
    let classic = b.is_empty()
        || b == "email"
        || b == "imap"
        || b == "pop3"
        || b == "gmail"
        || b == "exchange"
        || b == "nntp"
        || b == "maildir"
        || b == "mbox";
    if !classic {
        return;
    }
    let mut seen: HashSet<String> = HashSet::new();
    for line in [from, to, cc.unwrap_or("")] {
        if line.trim().is_empty() {
            continue;
        }
        let Some(list) = parse_email_address_list(line.trim()) else {
            continue;
        };
        for a in list {
            let email = a.address();
            let n = normalize_email(&email);
            if n.is_empty() || !seen.insert(n) {
                continue;
            }
            let display_name = a
                .display_name()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let _ = frb_contacts_learn_from_mail(display_name, email);
        }
    }
}

pub fn frb_contacts_repositories_list() -> Result<Vec<FrbContactRepositoryRow>, String> {
    with_db(|c| {
        let mut stmt = c
            .prepare(
                r#"SELECT id, name, kind, enabled, base_url, collection_path, credential_key,
                          default_new_contact, ctag, last_collection_sync_at, sync_error
                   FROM contact_repositories ORDER BY id"#,
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(FrbContactRepositoryRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    kind: r.get(2)?,
                    enabled: r.get::<_, i64>(3)? != 0,
                    base_url: r.get(4)?,
                    collection_path: r.get(5)?,
                    credential_key: r.get(6)?,
                    default_new_contact: r.get::<_, i64>(7)? != 0,
                    ctag: r.get(8)?,
                    last_collection_sync_at: r.get(9)?,
                    sync_error: r.get(10)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_repository_upsert(u: FrbRepositoryUpsert) -> Result<FrbContactsRowId, String> {
    with_db(|c| {
        let en = u.enabled.unwrap_or(true) as i32;
        let dnc = u.default_new_contact.unwrap_or(false) as i32;
        let id = if let Some(id) = u.id {
            c.execute(
                r#"UPDATE contact_repositories SET name = ?1, kind = ?2, enabled = ?3,
                    base_url = ?4, collection_path = ?5, credential_key = ?6, default_new_contact = ?7
                  WHERE id = ?8"#,
                params![
                    u.name,
                    u.kind,
                    en,
                    u.base_url.unwrap_or_default(),
                    u.collection_path.unwrap_or_default(),
                    u.credential_key.unwrap_or_default(),
                    dnc,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
            id
        } else {
            c.execute(
                r#"INSERT INTO contact_repositories (name, kind, enabled, base_url, collection_path, credential_key, default_new_contact)
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![
                    u.name,
                    u.kind,
                    en,
                    u.base_url.unwrap_or_default(),
                    u.collection_path.unwrap_or_default(),
                    u.credential_key.unwrap_or_default(),
                    dnc
                ],
            )
            .map_err(|e| e.to_string())?;
            c.last_insert_rowid()
        };
        Ok(FrbContactsRowId { id })
    })
}

pub fn frb_contacts_repository_delete(repository_id: i64) -> Result<(), String> {
    with_db(|c| {
        c.execute(
            "DELETE FROM contact_repositories WHERE id = ?1",
            [repository_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn frb_contacts_set_repository_membership(
    contact_id: i64,
    repository_id: i64,
    include: bool,
) -> Result<(), String> {
    with_db(|c| {
        contacts_store::upsert_repository_membership(c, contact_id, repository_id, include)
    })
}

pub fn frb_contacts_groups_list() -> Result<Vec<FrbContactGroupRow>, String> {
    with_db(|c| {
        let mut stmt = c
            .prepare("SELECT id, name, color_argb FROM contact_groups ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(FrbContactGroupRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color_argb: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_group_upsert(u: FrbGroupUpsert) -> Result<FrbContactsRowId, String> {
    with_db(|c| {
        let id = if let Some(id) = u.id {
            c.execute(
                "UPDATE contact_groups SET name = ?1, color_argb = ?2 WHERE id = ?3",
                params![u.name, u.color_argb, id],
            )
            .map_err(|e| e.to_string())?;
            id
        } else {
            c.execute(
                "INSERT INTO contact_groups (name, color_argb) VALUES (?1, ?2)",
                params![u.name, u.color_argb],
            )
            .map_err(|e| e.to_string())?;
            c.last_insert_rowid()
        };
        Ok(FrbContactsRowId { id })
    })
}

pub fn frb_contacts_group_delete(group_id: i64) -> Result<(), String> {
    with_db(|c| {
        c.execute("DELETE FROM contact_groups WHERE id = ?1", [group_id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn frb_contacts_group_add_member(contact_id: i64, group_id: i64) -> Result<(), String> {
    with_db(|c| {
        c.execute(
            "INSERT OR IGNORE INTO contact_group_members (contact_id, group_id) VALUES (?1, ?2)",
            params![contact_id, group_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn frb_contacts_group_remove_member(contact_id: i64, group_id: i64) -> Result<(), String> {
    with_db(|c| {
        c.execute(
            "DELETE FROM contact_group_members WHERE contact_id = ?1 AND group_id = ?2",
            params![contact_id, group_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn frb_contacts_set_group_repository_rule(
    group_id: i64,
    repository_id: i64,
    enable: bool,
) -> Result<(), String> {
    with_db(|c| {
        if enable {
            c.execute(
                "INSERT OR IGNORE INTO group_repository_targets (group_id, repository_id) VALUES (?1, ?2)",
                params![group_id, repository_id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            c.execute(
                "DELETE FROM group_repository_targets WHERE group_id = ?1 AND repository_id = ?2",
                params![group_id, repository_id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

pub fn frb_contacts_apply_group_repository_rules() -> Result<FrbContactsApplyGroupRulesResult, String> {
    with_db(|c| {
        let n = contacts_store::materialize_group_repository_targets(c)?;
        Ok(FrbContactsApplyGroupRulesResult {
            materialized: i64::from(n),
        })
    })
}

/// Compact contact rows for settings UI (picker / list).
pub fn frb_contacts_list_compact(limit: i64) -> Result<Vec<FrbContactCompactRow>, String> {
    let lim = limit.clamp(1, 10_000);
    with_db(|c| {
        let mut stmt = c
            .prepare(
                r#"SELECT c.id, c.display_name, c.externally_share_ok, c.import_origin,
                    (SELECT email FROM contact_emails e WHERE e.contact_id = c.id ORDER BY e.id LIMIT 1)
                   FROM contacts c
                   ORDER BY c.display_name COLLATE NOCASE
                   LIMIT ?1"#,
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([lim], |r| {
                Ok(FrbContactCompactRow {
                    id: r.get(0)?,
                    display_name: r.get(1)?,
                    externally_share_ok: r.get::<_, i64>(2)? != 0,
                    import_origin: r.get(3)?,
                    primary_email: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    })
}

/// All repositories with link + dirty state for one contact.
pub fn frb_contacts_repository_links_for_contact(
    contact_id: i64,
) -> Result<Vec<FrbContactRepositoryLinkRow>, String> {
    with_db(|c| {
        let mut stmt = c
            .prepare(
                r#"SELECT r.id, r.name, r.kind,
                    CASE WHEN s.contact_id IS NULL THEN 0 ELSE 1 END,
                    COALESCE(s.local_dirty, 0)
                   FROM contact_repositories r
                   LEFT JOIN contact_repository_state s
                     ON s.repository_id = r.id AND s.contact_id = ?1
                   ORDER BY r.id"#,
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([contact_id], |r| {
                Ok(FrbContactRepositoryLinkRow {
                    repository_id: r.get(0)?,
                    name: r.get(1)?,
                    kind: r.get(2)?,
                    linked: r.get::<_, i64>(3)? != 0,
                    local_dirty: r.get::<_, i64>(4)? != 0,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_group_repository_targets_list() -> Result<Vec<FrbGroupRepositoryTargetRow>, String> {
    with_db(|c| {
        let mut stmt = c
            .prepare(
                "SELECT group_id, repository_id FROM group_repository_targets ORDER BY group_id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(FrbGroupRepositoryTargetRow {
                    group_id: r.get(0)?,
                    repository_id: r.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_group_members_list(
    group_id: i64,
) -> Result<Vec<FrbContactGroupMemberRow>, String> {
    with_db(|c| {
        let mut stmt = c
            .prepare(
                r#"SELECT c.id, c.display_name,
                    (SELECT email FROM contact_emails e WHERE e.contact_id = c.id ORDER BY e.id LIMIT 1)
                   FROM contacts c
                   JOIN contact_group_members m ON m.contact_id = c.id AND m.group_id = ?1
                   ORDER BY c.display_name COLLATE NOCASE"#,
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([group_id], |r| {
                Ok(FrbContactGroupMemberRow {
                    id: r.get(0)?,
                    display_name: r.get(1)?,
                    primary_email: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_bulk_set_repository_membership(
    contact_ids: Vec<i64>,
    repository_id: i64,
    include: bool,
) -> Result<(), String> {
    with_db(|c| {
        for id in contact_ids {
            contacts_store::upsert_repository_membership(c, id, repository_id, include)?;
        }
        Ok(())
    })
}

/// CardDAV pull using HTTP Basic auth (credentials are not persisted by this call).
pub fn frb_contacts_carddav_pull(
    repository_id: i64,
    username: String,
    password: String,
) -> Result<FrbCarddavPullResult, String> {
    with_db(|c| {
        let (base, coll): (String, String) = c
            .query_row(
                "SELECT base_url, collection_path FROM contact_repositories WHERE id = ?1",
                [repository_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let coll_t = coll.trim();
        let url = if coll_t.starts_with("http://") || coll_t.starts_with("https://") {
            coll_t.to_string()
        } else {
            let sep = if base.trim_end().ends_with('/') || coll_t.starts_with('/') {
                ""
            } else {
                "/"
            };
            format!("{}{}{}", base.trim_end(), sep, coll_t.trim_start())
        };
        let outcome = carddav_sync::pull_addressbook(
            c,
            repository_id,
            url.trim(),
            username.trim(),
            password.trim(),
        )?;
        Ok(outcome.into())
    })
}

/// Push dirty contacts to CardDAV (PUT vCard). Rows need `remote_href` (set after a successful pull).
pub fn frb_contacts_carddav_push(
    repository_id: i64,
    username: String,
    password: String,
) -> Result<FrbCarddavPushResult, String> {
    with_db(|c| {
        let (base, coll): (String, String) = c
            .query_row(
                "SELECT base_url, collection_path FROM contact_repositories WHERE id = ?1",
                [repository_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let coll_t = coll.trim();
        let url = if coll_t.starts_with("http://") || coll_t.starts_with("https://") {
            coll_t.to_string()
        } else {
            let sep = if base.trim_end().ends_with('/') || coll_t.starts_with('/') {
                ""
            } else {
                "/"
            };
            format!("{}{}{}", base.trim_end(), sep, coll_t.trim_start())
        };
        let outcome = carddav_sync::push_addressbook(
            c,
            repository_id,
            url.trim(),
            username.trim(),
            password.trim(),
        )?;
        Ok(outcome.into())
    })
}

pub fn frb_contacts_import_vcard_bytes(bytes: Vec<u8>) -> Result<FrbImportVcardResult, String> {
    let ids = with_db(|c| contacts_vcard_import::import_vcards_from_bytes(c, &bytes, false))?;
    Ok(FrbImportVcardResult {
        imported: ids.len() as i32,
    })
}

pub fn frb_contacts_export_vcard(contact_ids: Vec<i64>) -> Result<FrbExportedVcard, String> {
    let text = with_db(|c| {
        let ids: Vec<i64> = if contact_ids.is_empty() {
            let mut stmt = c
                .prepare("SELECT id FROM contacts ORDER BY id")
                .map_err(|e| e.to_string())?;
            stmt.query_map([], |r| r.get(0))
                .map_err(|e| e.to_string())?
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        } else {
            contact_ids
        };
        let mut out = String::new();
        for id in ids {
            out.push_str(&contacts_store::contact_vcard_body(c, id)?);
        }
        Ok(out)
    })?;
    Ok(FrbExportedVcard { vcard_text: text })
}

/// Walk MIME parts and concatenate embedded vCard bodies.
pub fn frb_contacts_extract_vcards_from_raw_message(raw: Vec<u8>) -> Result<Vec<FrbParsedVcard>, String> {
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    emit_message_parts(&raw, |ct, body, _fname| {
        let ct_lower = ct.to_ascii_lowercase();
        if ct_lower.starts_with("text/vcard")
            || ct_lower.starts_with("text/x-vcard")
            || ct_lower.starts_with("application/vcard")
        {
            chunks.push(body.to_vec());
        }
    })
    .map_err(|e| e.to_string())?;
    let mut all = Vec::new();
    for b in chunks {
        for pv in vcard_lite::parse_vcards_utf8(&b) {
            let emails: Vec<FrbVcardEmailRow> = pv
                .emails
                .into_iter()
                .map(|(addr, type_label)| FrbVcardEmailRow { addr, type_label })
                .collect();
            let tels: Vec<FrbParsedTelRow> = pv
                .tels
                .into_iter()
                .map(|(number, label)| FrbParsedTelRow { number, label })
                .collect();
            let urls: Vec<FrbParsedUrlRow> = pv
                .urls
                .into_iter()
                .map(|(url, label)| FrbParsedUrlRow { url, label })
                .collect();
            let addresses: Vec<FrbParsedAdrRow> = pv
                .adrs
                .into_iter()
                .map(|a| FrbParsedAdrRow {
                    label: a.label,
                    po_box: a.po_box,
                    extended: a.extended,
                    street: a.street,
                    locality: a.locality,
                    region: a.region,
                    postal_code: a.postal_code,
                    country: a.country,
                })
                .collect();
            let photos: Vec<FrbParsedPhotoRow> = pv
                .photos
                .into_iter()
                .map(|p| FrbParsedPhotoRow {
                    mime_type: p.mime_type,
                    data_base64: p
                        .data
                        .as_ref()
                        .map(|d| base64::engine::general_purpose::STANDARD.encode(d))
                        .unwrap_or_default(),
                    source_uri: p.source_uri.unwrap_or_default(),
                })
                .collect();
            all.push(FrbParsedVcard {
                formatted_name: pv.fn_,
                nickname: pv.nickname,
                organization: pv.organization,
                title: pv.title,
                birthday: pv.birthday,
                emails,
                tels,
                urls,
                addresses,
                photos,
                key_raw: pv.key_raw,
                cert_raw: pv.cert_raw,
            });
        }
    }
    Ok(all)
}

pub fn frb_contacts_import_vcards_from_raw_message(raw: Vec<u8>) -> Result<FrbImportVcardResult, String> {
    let mut buf: Vec<u8> = Vec::new();
    emit_message_parts(&raw, |ct, body, _fname| {
        let ct_lower = ct.to_ascii_lowercase();
        if ct_lower.starts_with("text/vcard")
            || ct_lower.starts_with("text/x-vcard")
            || ct_lower.starts_with("application/vcard")
        {
            buf.extend_from_slice(body);
            buf.push(b'\n');
        }
    })
    .map_err(|e| e.to_string())?;
    frb_contacts_import_vcard_bytes(buf)
}

/// Merge contacts from the platform address book (Flutter).
pub fn frb_contacts_merge_platform(req: FrbMergePlatformContacts) -> Result<FrbMergePlatformResult, String> {
    let repo_opt = req.repository_id;
    let mut n = 0i32;
    with_db(|c| {
        for item in req.items {
            let name = item.display_name.unwrap_or_default();
            for em in item.emails {
                let addr = normalize_email(&em);
                if addr.is_empty() {
                    continue;
                }
                let existing: Option<i64> = c
                    .query_row(
                        "SELECT contact_id FROM contact_emails WHERE lower(email) = lower(?1) ORDER BY contact_id LIMIT 1",
                        [addr.as_str()],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                if existing.is_some() {
                    continue;
                }
                let t = now_ms();
                c.execute(
                    r#"INSERT INTO contacts (display_name, notes, import_origin, externally_share_ok, created_at, updated_at)
                       VALUES (?1, '', 'platform_pull', 1, ?2, ?2)"#,
                    params![name.as_str(), t],
                )
                .map_err(|e| e.to_string())?;
                let id = c.last_insert_rowid();
                c.execute(
                    "INSERT INTO contact_emails (contact_id, email, label) VALUES (?1, ?2, '')",
                    params![id, addr],
                )
                .map_err(|e| e.to_string())?;
                contacts_store::refresh_contact_fts(c, id)?;
                if let Some(rid) = repo_opt {
                    let _ = contacts_store::upsert_repository_membership(c, id, rid, true);
                }
                n += 1;
            }
        }
        Ok(())
    })?;
    Ok(FrbMergePlatformResult { imported: n })
}

pub fn frb_contacts_sync_repository(repository_id: i64) -> Result<FrbContactsSyncRepositoryResult, String> {
    with_db(|c| {
        let kind: String = c
            .query_row(
                "SELECT kind FROM contact_repositories WHERE id = ?1",
                [repository_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let msg = match kind.as_str() {
            "carddav" => {
                c.execute(
                    "UPDATE contact_repositories SET sync_error = ?1, last_collection_sync_at = ?2 WHERE id = ?3",
                    params![
                        "Use Contacts settings: CardDAV pull (enter username and password)",
                        now_ms(),
                        repository_id
                    ],
                )
                .map_err(|e| e.to_string())?;
                "carddav: use frb_contacts_carddav_pull with credentials"
            }
            "platform" => {
                c.execute(
                    "UPDATE contact_repositories SET sync_error = ?1, last_collection_sync_at = ?2 WHERE id = ?3",
                    params![
                        "Use merge from Flutter (platform contacts API)",
                        now_ms(),
                        repository_id
                    ],
                )
                .map_err(|e| e.to_string())?;
                "platform: pull via frb_contacts_merge_platform"
            }
            _ => "unknown repository kind",
        };
        Ok(FrbContactsSyncRepositoryResult {
            ok: true,
            message: msg.to_string(),
        })
    })
}

pub fn frb_contacts_sync_status() -> Result<Vec<FrbContactRepositoryRow>, String> {
    frb_contacts_repositories_list()
}
