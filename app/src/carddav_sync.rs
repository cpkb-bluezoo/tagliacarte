/*
 * carddav_sync.rs
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

//! CardDAV address book: PROPFIND depth 1, GET each resource, import vCards; PUT dirty local rows.

use crate::contacts_store;
use crate::contacts_vcard_import;
use base64::Engine;
use tagliacarte_core::xml::collect_href_texts_from_reader;
use rusqlite::params;
use serde_json::json;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn basic_auth_header(user: &str, pass: &str) -> String {
    let combined = format!("{}:{}", user.trim(), pass);
    let b64 = base64::engine::general_purpose::STANDARD.encode(combined.as_bytes());
    format!("Basic {b64}")
}

fn propfind_reader(
    collection_url: &str,
    user: &str,
    pass: &str,
) -> Result<impl Read, String> {
    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:getetag/><d:getcontenttype/></d:prop></d:propfind>"#;
    let resp = ureq::request("PROPFIND", collection_url)
        .set("Content-Type", "application/xml; charset=utf-8")
        .set("Depth", "1")
        .set("Authorization", basic_auth_header(user, pass).as_str())
        .send_string(body)
        .map_err(|e| format!("PROPFIND: {e}"))?;
    Ok(resp.into_reader())
}

fn http_get_bytes(url: &str, user: &str, pass: &str) -> Result<Vec<u8>, String> {
    ureq::get(url)
        .set("Authorization", basic_auth_header(user, pass).as_str())
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?
        .into_reader()
        .bytes()
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("read: {e}"))
}

fn http_put_bytes(
    url: &str,
    user: &str,
    pass: &str,
    body: &[u8],
    if_match_etag: Option<&str>,
) -> Result<u16, String> {
    let mut req = ureq::put(url)
        .set("Authorization", basic_auth_header(user, pass).as_str())
        .set("Content-Type", "text/vcard; charset=utf-8");
    if let Some(et) = if_match_etag {
        let t = et.trim();
        if !t.is_empty() {
            req = req.set("If-Match", t);
        }
    }
    let resp = req
        .send_bytes(body)
        .map_err(|e| format!("PUT {url}: {e}"))?;
    Ok(resp.status())
}

fn resolve_member_url(collection: &Url, href: &str) -> Result<Url, String> {
    let h = href.trim();
    if h.starts_with("http://") || h.starts_with("https://") {
        return Url::parse(h).map_err(|e| e.to_string());
    }
    collection.join(h).map_err(|e| e.to_string())
}

/// Pull addressbook members and import vCards. `collection_url` must be the addressbook collection URL.
pub fn pull_addressbook(
    conn: &rusqlite::Connection,
    repository_id: i64,
    collection_url: &str,
    user: &str,
    pass: &str,
) -> Result<serde_json::Value, String> {
    let collection = Url::parse(collection_url.trim()).map_err(|e| format!("bad collection URL: {e}"))?;
    let mut propfind_body = propfind_reader(collection.as_str(), user, pass)?;
    let hrefs =
        collect_href_texts_from_reader(&mut propfind_body).map_err(|e| e.to_string())?;
    let mut fetched = 0i32;
    let mut imported = 0i32;
    let col_path = collection.path().to_string();

    for href in hrefs {
        let member = match resolve_member_url(&collection, &href) {
            Ok(u) => u,
            Err(_) => continue,
        };
        // Omit the collection resource when the server lists it alongside members.
        if member.path().trim_end_matches('/') == col_path.trim_end_matches('/') {
            continue;
        }
        let bytes = match http_get_bytes(member.as_str(), user, pass) {
            Ok(b) => b,
            Err(e) => {
                conn.execute(
                    "UPDATE contact_repositories SET sync_error = ?1 WHERE id = ?2",
                    params![e, repository_id],
                )
                .map_err(|e| e.to_string())?;
                return Err(e);
            }
        };
        fetched += 1;
        let lossy = String::from_utf8_lossy(&bytes);
        if lossy.contains("BEGIN:VCARD") {
            let ids = contacts_vcard_import::import_vcards_from_bytes(conn, &bytes, true)?;
            imported += ids.len() as i32;
            let remote = member.as_str();
            for id in ids {
                conn.execute(
                    r#"INSERT INTO contact_repository_state (contact_id, repository_id, remote_href, etag, local_dirty, last_error)
                       VALUES (?1, ?2, ?3, '', 0, '')
                       ON CONFLICT(contact_id, repository_id) DO UPDATE SET
                         remote_href = excluded.remote_href,
                         local_dirty = excluded.local_dirty,
                         last_error = ''"#,
                    params![id, repository_id, remote],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    let t = now_ms();
    conn.execute(
        "UPDATE contact_repositories SET sync_error = '', last_collection_sync_at = ?1, ctag = ?2 WHERE id = ?3",
        params![t, "pulled", repository_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(json!({
        "ok": true,
        "fetchedResources": fetched,
        "importedContacts": imported,
        "message": "carddav pull completed"
    }))
}

/// Push locally dirty contacts that have a `remote_href` for this repository (typically set after a pull).
pub fn push_addressbook(
    conn: &rusqlite::Connection,
    repository_id: i64,
    collection_url: &str,
    user: &str,
    pass: &str,
) -> Result<serde_json::Value, String> {
    let collection = Url::parse(collection_url.trim()).map_err(|e| format!("bad collection URL: {e}"))?;
    let mut stmt = conn
        .prepare(
            r#"SELECT contact_id, remote_href, etag FROM contact_repository_state
               WHERE repository_id = ?1 AND local_dirty = 1 AND TRIM(remote_href) != ''"#,
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([repository_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut pushed = 0i32;
    let mut failed = 0i32;
    for (contact_id, href, etag) in rows {
        let vcard = contacts_store::contact_vcard_body(conn, contact_id)?;
        let target = match resolve_member_url(&collection, href.trim()) {
            Ok(u) => u,
            Err(e) => {
                conn.execute(
                    "UPDATE contact_repository_state SET last_error = ?1 WHERE contact_id = ?2 AND repository_id = ?3",
                    params![format!("bad href: {e}"), contact_id, repository_id],
                )
                .map_err(|e| e.to_string())?;
                failed += 1;
                continue;
            }
        };
        let etag_opt = {
            let t = etag.trim();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        };
        let status = http_put_bytes(
            target.as_str(),
            user,
            pass,
            vcard.as_bytes(),
            etag_opt,
        )?;
        if matches!(status, 200..=299) {
            conn.execute(
                "UPDATE contact_repository_state SET local_dirty = 0, last_error = '' WHERE contact_id = ?1 AND repository_id = ?2",
                params![contact_id, repository_id],
            )
            .map_err(|e| e.to_string())?;
            pushed += 1;
        } else {
            conn.execute(
                "UPDATE contact_repository_state SET last_error = ?1 WHERE contact_id = ?2 AND repository_id = ?3",
                params![format!("PUT HTTP {status}"), contact_id, repository_id],
            )
            .map_err(|e| e.to_string())?;
            failed += 1;
        }
    }
    let t = now_ms();
    conn.execute(
        "UPDATE contact_repositories SET sync_error = '', last_collection_sync_at = ?1 WHERE id = ?2",
        params![t, repository_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(json!({
        "ok": true,
        "pushed": pushed,
        "failed": failed,
        "message": "carddav push completed"
    }))
}
