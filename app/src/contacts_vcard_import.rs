/*
 * contacts_vcard_import.rs
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
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Shared vCard → SQLite import for FRB and CardDAV.

use crate::contacts_store;
use crate::vcard_lite;
use rusqlite::params;
use rusqlite::OptionalExtension;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn normalize_email(s: &str) -> String {
    s.trim().to_lowercase()
}

fn vcard_has_identity(v: &vcard_lite::ParsedVCard) -> bool {
    !v.fn_.trim().is_empty()
        || !v.emails.is_empty()
        || !v.tels.is_empty()
        || !v.urls.is_empty()
        || !v.adrs.is_empty()
        || !v.photos.is_empty()
}

/// Import one or more vCards from raw bytes; returns ids of newly created contacts (in order).
/// When `skip_duplicate_emails` is true (e.g. CardDAV sync), skip vCards whose emails already exist locally.
pub fn import_vcards_from_bytes(
    conn: &rusqlite::Connection,
    bytes: &[u8],
    skip_duplicate_emails: bool,
) -> Result<Vec<i64>, String> {
    let cards = vcard_lite::parse_vcards_utf8(bytes);
    let mut created_ids = Vec::new();
    for vcard in cards {
        let t = now_ms();
        if !vcard_has_identity(&vcard) {
            continue;
        }
        if skip_duplicate_emails {
            let mut any_taken = false;
            for (e, _) in &vcard.emails {
                let n = normalize_email(e);
                if n.is_empty() {
                    continue;
                }
                let exists: Option<i64> = conn
                    .query_row(
                        "SELECT 1 FROM contact_emails WHERE lower(email) = lower(?1)",
                        [n.as_str()],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                if exists.is_some() {
                    any_taken = true;
                    break;
                }
            }
            if any_taken {
                continue;
            }
        }
        let birthday: Option<String> = {
            let b = vcard.birthday.trim();
            if b.is_empty() {
                None
            } else {
                Some(b.to_string())
            }
        };
        conn.execute(
            r#"INSERT INTO contacts (display_name, nickname, organization, title, birthday, notes, import_origin, externally_share_ok,
                pgp_fingerprint, pgp_key_path, smime_cert_path, smime_notes, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, '', 'vcard_import', 1, NULL, ?6, NULL, ?7, ?8, ?8)"#,
            params![
                vcard.fn_.as_str(),
                vcard.nickname.as_str(),
                vcard.organization.as_str(),
                vcard.title.as_str(),
                birthday,
                vcard.key_raw,
                vcard.cert_raw,
                t,
            ],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        for (e, label) in &vcard.emails {
            let addr = normalize_email(e);
            if addr.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO contact_emails (contact_id, email, label) VALUES (?1, ?2, ?3)",
                params![id, addr, label.as_str()],
            )
            .map_err(|e| e.to_string())?;
        }
        for (i, (num, label)) in vcard.tels.iter().enumerate() {
            let num = num.trim();
            if num.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO contact_phones (contact_id, number, label, sort_order) VALUES (?1, ?2, ?3, ?4)",
                params![id, num, label.as_str(), i as i64],
            )
            .map_err(|e| e.to_string())?;
        }
        for (i, (url, label)) in vcard.urls.iter().enumerate() {
            let url = url.trim();
            if url.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO contact_urls (contact_id, url, label, sort_order) VALUES (?1, ?2, ?3, ?4)",
                params![id, url, label.as_str(), i as i64],
            )
            .map_err(|e| e.to_string())?;
        }
        for (i, a) in vcard.adrs.iter().enumerate() {
            conn.execute(
                r#"INSERT INTO contact_postal_addresses (contact_id, label, po_box, extended, street, locality, region, postal_code, country, sort_order)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                params![
                    id,
                    a.label.as_str(),
                    a.po_box.as_str(),
                    a.extended.as_str(),
                    a.street.as_str(),
                    a.locality.as_str(),
                    a.region.as_str(),
                    a.postal_code.as_str(),
                    a.country.as_str(),
                    i as i64,
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        for (i, p) in vcard.photos.iter().enumerate() {
            conn.execute(
                "INSERT INTO contact_photos (contact_id, mime_type, data, source_uri, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, p.mime_type.as_str(), p.data.as_ref(), p.source_uri.as_deref(), i as i64],
            )
            .map_err(|e| e.to_string())?;
        }
        contacts_store::refresh_contact_fts(conn, id)?;
        created_ids.push(id);
    }
    Ok(created_ids)
}
