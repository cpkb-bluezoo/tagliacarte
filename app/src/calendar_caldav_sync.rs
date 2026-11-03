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

/// How a collection is signed in to. Whatever the server asks for goes in one
/// `Authorization` header on every request; the secrets never reach a log.
#[derive(Clone)]
pub enum CaldavAuth {
    None,
    Basic { username: String, password: String },
    /// An OAuth access token (Google, for one).
    Bearer(String),
}

impl std::fmt::Debug for CaldavAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaldavAuth::None => f.write_str("None"),
            CaldavAuth::Basic { username, .. } => write!(f, "Basic({username})"),
            CaldavAuth::Bearer(_) => f.write_str("Bearer"),
        }
    }
}

/// `PROPFIND` the collection, then `GET` every `.ics` member, on one
/// authenticated DAV client. Returns the members' bodies.
fn fetch_ics_members(collection: &Url, auth: &CaldavAuth) -> Result<(u64, Vec<Vec<u8>>), String> {
    let (client, target) = DavClient::for_url(collection.as_str()).map_err(|e| e.to_string())?;
    let mut client = match auth {
        CaldavAuth::None => client,
        CaldavAuth::Basic { username, password } => client.with_basic_auth(username, password),
        CaldavAuth::Bearer(token) => client.with_bearer(token),
    };
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
/// The calendar is called `name`.
pub fn pull_calendar_collection(
    conn: &Connection,
    calendar_id: &str,
    source_id: &str,
    name: &str,
    collection_url: &str,
    auth: &CaldavAuth,
) -> Result<CaldavPullOutcome, String> {
    let collection = Url::parse(collection_url.trim()).map_err(|e| format!("bad collection URL: {e}"))?;
    calendar_store::upsert_calendar(
        conn,
        &tagliacarte_core::calendar::CalendarDescriptor {
            calendar_id: calendar_id.trim().to_string(),
            source_id: source_id.trim().to_string(),
            name: if name.trim().is_empty() { calendar_id.trim().to_string() } else { name.trim().to_string() },
            color_hex: None,
            enabled: true,
        },
    )?;

    let (fetched, bodies) = fetch_ics_members(&collection, auth)?;
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
