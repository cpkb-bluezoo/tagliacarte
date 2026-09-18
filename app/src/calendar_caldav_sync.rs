/*
 * calendar_caldav_sync.rs
 */

use base64::Engine;
use rusqlite::Connection;
use std::io::Read;
use tagliacarte_core::xml::collect_href_texts_from_reader;
use url::Url;

use crate::calendar_store;
use crate::calendar_sync::events_from_ics_payload;

fn basic_auth_header(user: &str, pass: &str) -> String {
    let combined = format!("{}:{}", user.trim(), pass);
    let b64 = base64::engine::general_purpose::STANDARD.encode(combined.as_bytes());
    format!("Basic {b64}")
}

fn resolve_member_url(collection: &Url, href: &str) -> Result<Url, String> {
    let h = href.trim();
    if h.starts_with("http://") || h.starts_with("https://") {
        return Url::parse(h).map_err(|e| e.to_string());
    }
    collection.join(h).map_err(|e| e.to_string())
}

fn propfind_members(collection_url: &str, user: &str, pass: &str) -> Result<Vec<String>, String> {
    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:getetag/><d:getcontenttype/></d:prop></d:propfind>"#;
    let mut reader = ureq::request("PROPFIND", collection_url)
        .set("Content-Type", "application/xml; charset=utf-8")
        .set("Depth", "1")
        .set("Authorization", basic_auth_header(user, pass).as_str())
        .send_string(body)
        .map_err(|e| format!("PROPFIND: {e}"))?
        .into_reader();
    collect_href_texts_from_reader(&mut reader).map_err(|e| e.to_string())
}

fn get_member(url: &str, user: &str, pass: &str) -> Result<Vec<u8>, String> {
    ureq::get(url)
        .set("Authorization", basic_auth_header(user, pass).as_str())
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?
        .into_reader()
        .bytes()
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("read: {e}"))
}

#[derive(Debug, Clone)]
pub struct CaldavPullOutcome {
    pub fetched_resources: u64,
    pub imported_events: u64,
}

/// Pull all `.ics` members from one CalDAV collection and upsert into local calendar store.
pub fn pull_calendar_collection(
    conn: &Connection,
    calendar_id: &str,
    source_id: &str,
    collection_url: &str,
    username: &str,
    password: &str,
) -> Result<CaldavPullOutcome, String> {
    let collection = Url::parse(collection_url.trim()).map_err(|e| format!("bad collection URL: {e}"))?;
    calendar_store::upsert_calendar(
        conn,
        &tagliacarte_core::calendar::CalendarDescriptor {
            calendar_id: calendar_id.trim().to_string(),
            source_id: source_id.trim().to_string(),
            name: calendar_id.trim().to_string(),
            color_hex: None,
            enabled: true,
        },
    )?;

    let hrefs = propfind_members(collection.as_str(), username, password)?;
    let mut fetched = 0u64;
    let mut imported = 0u64;
    let col_path = collection.path().trim_end_matches('/').to_string();
    for href in hrefs {
        let member = match resolve_member_url(&collection, href.as_str()) {
            Ok(u) => u,
            Err(_) => continue,
        };
        if member.path().trim_end_matches('/') == col_path {
            continue;
        }
        if !member.path().to_ascii_lowercase().ends_with(".ics") {
            continue;
        }
        let bytes = get_member(member.as_str(), username, password)?;
        fetched += 1;
        let payload = String::from_utf8_lossy(&bytes);
        for event in events_from_ics_payload(calendar_id.trim(), payload.as_ref(), None) {
            calendar_store::upsert_event(conn, &event)?;
            imported += 1;
        }
    }
    Ok(CaldavPullOutcome {
        fetched_resources: fetched,
        imported_events: imported,
    })
}
