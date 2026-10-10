/*
 * calendar_caldav_sync.rs
 */

use rusqlite::Connection;
use tagliacarte_core::protocol::dav::DavClient;
use url::Url;

use crate::calendar_store;
use crate::calendar_sync::events_from_ics_payload;

fn resolve_member_url(collection: &Url, href: &str) -> Result<Url, String> {
    let h = href.trim();
    if h.starts_with("http://") || h.starts_with("https://") {
        return Url::parse(h).map_err(|e| e.to_string());
    }
    collection.join(h).map_err(|e| e.to_string())
}

/// `PROPFIND` the collection, then `GET` every `.ics` member, on one
/// authenticated DAV client. Returns the members' bodies.
fn fetch_ics_members(collection: &Url, user: &str, pass: &str) -> Result<(u64, Vec<Vec<u8>>), String> {
    let (client, target) = DavClient::for_url(collection.as_str()).map_err(|e| e.to_string())?;
    let mut client = client.with_basic_auth(user, pass);
    let members = client.propfind(&target).map_err(|e| format!("PROPFIND: {e}"))?;
    let col_path = collection.path().trim_end_matches('/').to_string();
    let mut fetched = 0u64;
    let mut bodies = Vec::new();
    for m in members {
        let Ok(member) = resolve_member_url(collection, &m.href) else { continue };
        if member.path().trim_end_matches('/') == col_path {
            continue;
        }
        if !member.path().to_ascii_lowercase().ends_with(".ics") {
            continue;
        }
        let target = client.retarget(member.as_str()).map_err(|e| e.to_string())?;
        let resp = client.get(&target).map_err(|e| format!("GET {member}: {e}"))?;
        if !resp.is_success() {
            return Err(format!("GET {member}: HTTP {}", resp.status));
        }
        fetched += 1;
        bodies.push(resp.body);
    }
    Ok((fetched, bodies))
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

    let (fetched, bodies) = fetch_ics_members(&collection, username, password)?;
    let mut imported = 0u64;
    for bytes in bodies {
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
