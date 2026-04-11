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

use crate::carddav_sync;
use crate::contacts_store;
use crate::contacts_vcard_import;
use crate::vcard_lite;
use rusqlite::params;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::json;
use std::sync::Mutex;
use tagliacarte_core::config::tagliacarte_data_dir;
use tagliacarte_core::mime::emit_message_parts;

static CONTACTS: Mutex<Option<rusqlite::Connection>> = Mutex::new(None);

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContactUpsertJson {
    id: Option<i64>,
    display_name: Option<String>,
    notes: Option<String>,
    import_origin: Option<String>,
    externally_share_ok: Option<bool>,
    emails: Option<Vec<EmailJson>>,
    pgp_fingerprint: Option<String>,
    pgp_key_path: Option<String>,
    smime_cert_path: Option<String>,
    smime_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmailJson {
    email: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryUpsertJson {
    id: Option<i64>,
    name: String,
    kind: String,
    enabled: Option<bool>,
    base_url: Option<String>,
    collection_path: Option<String>,
    credential_key: Option<String>,
    default_new_contact: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupUpsertJson {
    id: Option<i64>,
    name: String,
    color_argb: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformContactJson {
    display_name: Option<String>,
    emails: Vec<String>,
}

fn contact_row_to_json(c: &rusqlite::Connection, contact_id: i64) -> Result<String, String> {
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
    let emails: Vec<_> = stmt
        .query_map([contact_id], |r| {
            Ok(json!({
                "email": r.get::<_, String>(0)?,
                "label": r.get::<_, String>(1)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let out = json!({
        "id": row.0,
        "displayName": row.1,
        "notes": row.2,
        "importOrigin": row.3,
        "externallyShareOk": row.4 != 0,
        "pgpFingerprint": row.5,
        "pgpKeyPath": row.6,
        "smimeCertPath": row.7,
        "smimeNotes": row.8,
        "createdAt": row.9,
        "updatedAt": row.10,
        "emails": emails,
    });
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

/// Search contacts for autocomplete (display name + email).
pub fn frb_contacts_search(query: String, limit: i64) -> Result<String, String> {
    let lim = limit.clamp(1, 200);
    with_db(|c| {
        let rows = contacts_store::search_contacts(c, query.trim(), lim)?;
        let j: Vec<_> = rows
            .into_iter()
            .map(|(id, display_name, emails)| {
                json!({
                    "id": id,
                    "displayName": display_name,
                    "emails": emails,
                })
            })
            .collect();
        serde_json::to_string(&j).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_get(contact_id: i64) -> Result<String, String> {
    with_db(|c| contact_row_to_json(c, contact_id))
}

/// Lookup first contact matching email (normalized).
pub fn frb_contacts_lookup_by_email(email: String) -> Result<String, String> {
    let e = normalize_email(&email);
    if e.is_empty() {
        return Ok("null".to_string());
    }
    with_db(|c| {
        let id: Option<i64> = c
            .query_row(
                "SELECT contact_id FROM contact_emails WHERE email = ?1",
                [e.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match id {
            Some(i) => contact_row_to_json(c, i),
            None => Ok("null".to_string()),
        }
    })
}

pub fn frb_contacts_upsert(contact_json: String) -> Result<String, String> {
    let u: ContactUpsertJson =
        serde_json::from_str(&contact_json).map_err(|e| format!("JSON: {e}"))?;
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
        serde_json::to_string(&json!({ "id": id })).map_err(|e| e.to_string())
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

pub fn frb_contacts_learn_from_mail(display_name: String, email: String) -> Result<String, String> {
    let addr = normalize_email(&email);
    if addr.is_empty() {
        return Err("empty email".to_string());
    }
    with_db(|c| {
        let existing: Option<i64> = c
            .query_row(
                "SELECT contact_id FROM contact_emails WHERE email = ?1",
                [addr.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(id) = existing {
            return serde_json::to_string(&json!({ "id": id, "updated": false }))
                .map_err(|e| e.to_string());
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
        serde_json::to_string(&json!({ "id": id, "updated": true })).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_repositories_list() -> Result<String, String> {
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
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "kind": r.get::<_, String>(2)?,
                    "enabled": r.get::<_, i64>(3)? != 0,
                    "baseUrl": r.get::<_, String>(4)?,
                    "collectionPath": r.get::<_, String>(5)?,
                    "credentialKey": r.get::<_, String>(6)?,
                    "defaultNewContact": r.get::<_, i64>(7)? != 0,
                    "ctag": r.get::<_, String>(8)?,
                    "lastCollectionSyncAt": r.get::<_, Option<i64>>(9)?,
                    "syncError": r.get::<_, String>(10)?,
                }))
            })
            .map_err(|e| e.to_string())?;
        let v: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_repository_upsert(json: String) -> Result<String, String> {
    let u: RepositoryUpsertJson =
        serde_json::from_str(&json).map_err(|e| format!("JSON: {e}"))?;
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
        serde_json::to_string(&json!({ "id": id })).map_err(|e| e.to_string())
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

pub fn frb_contacts_groups_list() -> Result<String, String> {
    with_db(|c| {
        let mut stmt = c
            .prepare("SELECT id, name, color_argb FROM contact_groups ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "colorArgb": r.get::<_, Option<i64>>(2)?,
                }))
            })
            .map_err(|e| e.to_string())?;
        let v: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_group_upsert(json: String) -> Result<String, String> {
    let u: GroupUpsertJson = serde_json::from_str(&json).map_err(|e| format!("JSON: {e}"))?;
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
        serde_json::to_string(&json!({ "id": id })).map_err(|e| e.to_string())
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

pub fn frb_contacts_apply_group_repository_rules() -> Result<String, String> {
    with_db(|c| {
        let n = contacts_store::materialize_group_repository_targets(c)?;
        serde_json::to_string(&json!({ "materialized": n })).map_err(|e| e.to_string())
    })
}

/// Compact contact rows for settings UI (picker / list).
pub fn frb_contacts_list_compact(limit: i64) -> Result<String, String> {
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
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "displayName": r.get::<_, String>(1)?,
                    "externallyShareOk": r.get::<_, i64>(2)? != 0,
                    "importOrigin": r.get::<_, String>(3)?,
                    "primaryEmail": r.get::<_, Option<String>>(4)?,
                }))
            })
            .map_err(|e| e.to_string())?;
        let v: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

/// All repositories with link + dirty state for one contact.
pub fn frb_contacts_repository_links_for_contact(contact_id: i64) -> Result<String, String> {
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
                Ok(json!({
                    "repositoryId": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "kind": r.get::<_, String>(2)?,
                    "linked": r.get::<_, i64>(3)? != 0,
                    "localDirty": r.get::<_, i64>(4)? != 0,
                }))
            })
            .map_err(|e| e.to_string())?;
        let v: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_group_repository_targets_list() -> Result<String, String> {
    with_db(|c| {
        let mut stmt = c
            .prepare(
                "SELECT group_id, repository_id FROM group_repository_targets ORDER BY group_id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(json!({
                    "groupId": r.get::<_, i64>(0)?,
                    "repositoryId": r.get::<_, i64>(1)?,
                }))
            })
            .map_err(|e| e.to_string())?;
        let v: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_group_members_list(group_id: i64) -> Result<String, String> {
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
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "displayName": r.get::<_, String>(1)?,
                    "primaryEmail": r.get::<_, Option<String>>(2)?,
                }))
            })
            .map_err(|e| e.to_string())?;
        let v: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_bulk_set_repository_membership(
    contact_ids_json: String,
    repository_id: i64,
    include: bool,
) -> Result<(), String> {
    let ids: Vec<i64> =
        serde_json::from_str(&contact_ids_json).map_err(|e| format!("JSON: {e}"))?;
    with_db(|c| {
        for id in ids {
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
) -> Result<String, String> {
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
        let v = carddav_sync::pull_addressbook(
            c,
            repository_id,
            url.trim(),
            username.trim(),
            password.trim(),
        )?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

/// Push dirty contacts to CardDAV (PUT vCard). Rows need `remote_href` (set after a successful pull).
pub fn frb_contacts_carddav_push(
    repository_id: i64,
    username: String,
    password: String,
) -> Result<String, String> {
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
        let v = carddav_sync::push_addressbook(
            c,
            repository_id,
            url.trim(),
            username.trim(),
            password.trim(),
        )?;
        serde_json::to_string(&v).map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_import_vcard_bytes(bytes: Vec<u8>) -> Result<String, String> {
    let ids = with_db(|c| contacts_vcard_import::import_vcards_from_bytes(c, &bytes, false))?;
    serde_json::to_string(&json!({ "imported": ids.len() })).map_err(|e| e.to_string())
}

pub fn frb_contacts_export_vcard(contact_ids_json: String) -> Result<String, String> {
    let ids: Vec<i64> =
        serde_json::from_str(&contact_ids_json).unwrap_or_else(|_| Vec::new());
    with_db(|c| {
        let mut out = String::new();
        let mut stmt = c
            .prepare("SELECT id, display_name, notes FROM contacts ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows: Vec<(i64, String, String)> = if ids.is_empty() {
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(|e| e.to_string())?
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        } else {
            let mut v = Vec::new();
            for id in ids {
                let row = c.query_row(
                    "SELECT id, display_name, notes FROM contacts WHERE id = ?1",
                    [id],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
                );
                if let Ok(r) = row {
                    v.push(r);
                }
            }
            v
        };
        for (id, name, notes) in rows {
            let mut es = c
                .prepare("SELECT email, label FROM contact_emails WHERE contact_id = ?1 ORDER BY id")
                .map_err(|e| e.to_string())?;
            let emails: Vec<(String, String)> = es
                .query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(|e| e.to_string())?
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?;
            let pairs: Vec<(&str, &str)> = emails
                .iter()
                .map(|(a, l)| (a.as_str(), l.as_str()))
                .collect();
            out.push_str(&vcard_lite::build_vcard(&name, pairs.as_slice(), &notes));
        }
        Ok(out)
    })
}

/// Walk MIME parts and concatenate embedded vCard bodies.
pub fn frb_contacts_extract_vcards_from_raw_message(raw: Vec<u8>) -> Result<String, String> {
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
        all.extend(vcard_lite::parse_vcards_utf8(&b));
    }
    serde_json::to_string(&all).map_err(|e| e.to_string())
}

pub fn frb_contacts_import_vcards_from_raw_message(raw: Vec<u8>) -> Result<String, String> {
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

/// Merge contacts from platform address book JSON (from Flutter).
/// Payload may be a JSON array (legacy) or `{"items":[...], "repositoryId": optional id}` to link new rows to a repository.
pub fn frb_contacts_merge_platform_json(payload: String) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(&payload).map_err(|e| format!("JSON: {e}"))?;
    let (items, repo_opt): (Vec<PlatformContactJson>, Option<i64>) = match v {
        serde_json::Value::Array(a) => (
            serde_json::from_value(serde_json::Value::Array(a))
                .map_err(|e| format!("items: {e}"))?,
            None,
        ),
        serde_json::Value::Object(m) => {
            let items_val = m
                .get("items")
                .cloned()
                .ok_or_else(|| "merge: missing items".to_string())?;
            let rid = m
                .get("repositoryId")
                .and_then(|x| x.as_i64())
                .or_else(|| m.get("repository_id").and_then(|x| x.as_i64()));
            (
                serde_json::from_value(items_val).map_err(|e| format!("items: {e}"))?,
                rid,
            )
        }
        _ => return Err("merge: expected array or object with items".to_string()),
    };
    let mut n = 0i32;
    with_db(|c| {
        for item in items {
            let name = item.display_name.unwrap_or_default();
            for em in item.emails {
                let addr = normalize_email(&em);
                if addr.is_empty() {
                    continue;
                }
                let existing: Option<i64> = c
                    .query_row(
                        "SELECT contact_id FROM contact_emails WHERE email = ?1",
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
    serde_json::to_string(&json!({ "imported": n })).map_err(|e| e.to_string())
}

pub fn frb_contacts_sync_repository(repository_id: i64) -> Result<String, String> {
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
                "platform: pull via frb_contacts_merge_platform_json"
            }
            _ => "unknown repository kind",
        };
        serde_json::to_string(&json!({ "ok": true, "message": msg }))
            .map_err(|e| e.to_string())
    })
}

pub fn frb_contacts_sync_status() -> Result<String, String> {
    frb_contacts_repositories_list()
}
