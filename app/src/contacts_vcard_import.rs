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
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
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
        let emails: Vec<String> = vcard.emails.iter().map(|e| normalize_email(e)).collect();
        if emails.is_empty() && vcard.fn_.is_empty() {
            continue;
        }
        if skip_duplicate_emails {
            let mut any_taken = false;
            for e in &emails {
                if e.is_empty() {
                    continue;
                }
                let exists: Option<i64> = conn
                    .query_row(
                        "SELECT 1 FROM contact_emails WHERE email = ?1",
                        [e.as_str()],
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
        conn.execute(
            r#"INSERT INTO contacts (display_name, notes, import_origin, externally_share_ok,
                pgp_fingerprint, pgp_key_path, smime_cert_path, smime_notes, created_at, updated_at)
               VALUES (?1, '', 'vcard_import', 1, NULL, ?2, NULL, ?3, ?4, ?4)"#,
            params![vcard.fn_, vcard.key_raw, vcard.cert_raw, t],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        for e in emails {
            if e.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO contact_emails (contact_id, email, label) VALUES (?1, ?2, '')",
                params![id, e],
            )
            .map_err(|e| e.to_string())?;
        }
        contacts_store::refresh_contact_fts(conn, id)?;
        created_ids.push(id);
    }
    Ok(created_ids)
}
