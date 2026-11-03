/*
 * calendar_caldav_sync.rs
 */

use std::collections::HashMap;

use rusqlite::Connection;
use tagliacarte_core::protocol::dav::DavClient;
use url::Url;

use crate::calendar_store::{self, KnownResource};
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

/// Whether a file has to be fetched: when it has not been, when the server gave no tag for it
/// (or an empty one, which says nothing), or when its tag is not the one it had.
pub fn needs_fetch(known_etag: Option<&str>, current_etag: Option<&str>) -> bool {
    match (known_etag, current_etag) {
        (Some(known), Some(current)) => known.is_empty() || known != current,
        _ => true,
    }
}

/// A file that has to be read, as the server has it.
struct Fetched {
    key: String,
    etag: Option<String>,
    body: Vec<u8>,
}

/// What `PROPFIND` of the collection found, and the files among them that were fetched.
struct Listing {
    /// Every `.ics` file the collection has, by address, with its tag.
    seen: Vec<(String, Option<String>)>,
    fetched: Vec<Fetched>,
}

/// `PROPFIND` the collection, then `GET` the `.ics` members that are new or whose tag is not the
/// one they had, on one authenticated DAV client.
fn fetch_changed(collection: &Url, auth: &CaldavAuth, known: &HashMap<String, KnownResource>) -> Result<Listing, String> {
    let (client, target) = DavClient::for_url(collection.as_str()).map_err(|e| e.to_string())?;
    let mut client = match auth {
        CaldavAuth::None => client,
        CaldavAuth::Basic { username, password } => client.with_basic_auth(username, password),
        CaldavAuth::Bearer(token) => client.with_bearer(token),
    };
    let members = client.propfind(&target).map_err(|e| format!("PROPFIND: {e}"))?;
    let col_path = collection.path().trim_end_matches('/').to_string();
    let mut listing = Listing { seen: Vec::new(), fetched: Vec::new() };
    for m in members {
        let Ok(member) = resolve_member_url(collection, &m.href) else { continue };
        if member.path().trim_end_matches('/') == col_path {
            continue;
        }
        if !member.path().to_ascii_lowercase().ends_with(".ics") {
            continue;
        }
        let key = member.as_str().to_owned();
        listing.seen.push((key.clone(), m.etag.clone()));
        if !needs_fetch(known.get(&key).map(|k| k.etag.as_str()), m.etag.as_deref()) {
            continue;
        }
        let target = client.retarget(member.as_str()).map_err(|e| e.to_string())?;
        let resp = client.get(&target).map_err(|e| format!("GET {member}: {e}"))?;
        if !resp.is_success() {
            return Err(format!("GET {member}: HTTP {}", resp.status));
        }
        listing.fetched.push(Fetched { key, etag: m.etag, body: resp.body });
    }
    Ok(listing)
}

#[derive(Debug, Clone, Default)]
pub struct CaldavPullOutcome {
    /// Files read from the server this time.
    pub fetched_resources: u64,
    /// Files whose tag said they were as they were, and so were not read.
    pub unchanged_resources: u64,
    /// Files that were gone from the server, whose events were taken away.
    pub removed_resources: u64,
    pub imported_events: u64,
}

/// Brings a calendar up to date with a CalDAV collection: files that are new or have changed
/// are read and their events replace the ones the file gave before, files that are gone take
/// their events with them, and files whose tag has not changed are not read at all. The first
/// pull of a calendar that was kept before files were tracked starts it afresh. All or nothing:
/// a failure partway leaves the calendar as it was. The calendar is called `name`.
pub fn pull_calendar_collection(
    conn: &Connection,
    calendar_id: &str,
    source_id: &str,
    name: &str,
    collection_url: &str,
    auth: &CaldavAuth,
) -> Result<CaldavPullOutcome, String> {
    let collection = Url::parse(collection_url.trim()).map_err(|e| format!("bad collection URL: {e}"))?;
    let calendar_id = calendar_id.trim();
    calendar_store::upsert_calendar(
        conn,
        &tagliacarte_core::calendar::CalendarDescriptor {
            calendar_id: calendar_id.to_string(),
            source_id: source_id.trim().to_string(),
            name: if name.trim().is_empty() { calendar_id.to_string() } else { name.trim().to_string() },
            color_hex: None,
            enabled: true,
        },
    )?;

    let known = calendar_store::known_resources(conn, calendar_id)?;
    let listing = fetch_changed(&collection, auth, &known)?;

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    // Events from before files were tracked cannot be matched to a file; the server is the truth.
    if known.is_empty() {
        calendar_store::delete_events_of(&tx, calendar_id)?;
    }
    let mut out = CaldavPullOutcome::default();
    for f in &listing.fetched {
        if let Some(old) = known.get(&f.key) {
            for id in &old.event_ids {
                calendar_store::delete_event(&tx, id)?;
            }
        }
        let payload = String::from_utf8_lossy(&f.body);
        let events = events_from_ics_payload(calendar_id, payload.as_ref(), None);
        for e in &events {
            calendar_store::upsert_event(&tx, e)?;
        }
        calendar_store::put_resource(
            &tx,
            calendar_id,
            &f.key,
            &calendar_store::KnownResource { etag: f.etag.clone().unwrap_or_default(), event_ids: events.iter().map(|e| e.event_id.clone()).collect() },
        )?;
        out.fetched_resources += 1;
        out.imported_events += events.len() as u64;
    }
    out.unchanged_resources = (listing.seen.len() - listing.fetched.len()) as u64;
    for (key, old) in &known {
        if !listing.seen.iter().any(|(k, _)| k == key) {
            for id in &old.event_ids {
                calendar_store::delete_event(&tx, id)?;
            }
            calendar_store::forget_resource(&tx, calendar_id, key)?;
            out.removed_resources += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(out)
}
