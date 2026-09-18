/*
 * frb_api/frb_calendar.rs
 */

use tagliacarte_core::calendar::{CalendarDescriptor, CalendarEvent};
use tagliacarte_core::config::tagliacarte_data_dir;

#[derive(Debug, Clone)]
pub struct FrbCalendarDescriptor {
    pub calendar_id: String,
    pub source_id: String,
    pub name: String,
    pub color_hex: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct FrbCalendarEvent {
    pub event_id: String,
    pub calendar_id: String,
    pub uid: String,
    pub sequence: i64,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub starts_at_utc_ms: i64,
    pub ends_at_utc_ms: i64,
    pub all_day: bool,
    pub organizer: Option<String>,
    pub attendees: Vec<String>,
    pub recurrence_rule: Option<String>,
    pub status: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrbCalendarEventRangeResult {
    pub events: Vec<FrbCalendarEvent>,
}

#[derive(Debug, Clone)]
pub struct FrbCalendarSyncResult {
    pub source_kind: String,
    pub imported_count: u64,
    pub fetched_count: u64,
}

fn calendar_db() -> Result<rusqlite::Connection, String> {
    let dir = tagliacarte_data_dir().ok_or_else(|| "missing app data dir".to_string())?;
    crate::calendar_store::open_calendar_db(dir)
}

fn to_frb_calendar(c: CalendarDescriptor) -> FrbCalendarDescriptor {
    FrbCalendarDescriptor {
        calendar_id: c.calendar_id,
        source_id: c.source_id,
        name: c.name,
        color_hex: c.color_hex,
        enabled: c.enabled,
    }
}

fn to_frb_event(e: CalendarEvent) -> FrbCalendarEvent {
    FrbCalendarEvent {
        event_id: e.event_id,
        calendar_id: e.calendar_id,
        uid: e.uid,
        sequence: e.sequence,
        title: e.title,
        description: e.description,
        location: e.location,
        starts_at_utc_ms: e.starts_at_utc_ms,
        ends_at_utc_ms: e.ends_at_utc_ms,
        all_day: e.all_day,
        organizer: e.organizer,
        attendees: e.attendees,
        recurrence_rule: e.recurrence_rule,
        status: e.status,
        etag: e.etag,
    }
}

pub(crate) fn list_calendars() -> Result<Vec<FrbCalendarDescriptor>, String> {
    let conn = calendar_db()?;
    Ok(crate::calendar_store::list_calendars(&conn)?
        .into_iter()
        .map(to_frb_calendar)
        .collect())
}

pub(crate) fn list_events_range(
    start_utc_ms: i64,
    end_utc_ms: i64,
    calendar_ids: Vec<String>,
) -> Result<FrbCalendarEventRangeResult, String> {
    if end_utc_ms < start_utc_ms {
        return Err("end_utc_ms must be >= start_utc_ms".to_string());
    }
    let conn = calendar_db()?;
    let events = crate::calendar_store::list_events_in_range(
        &conn,
        start_utc_ms,
        end_utc_ms,
        calendar_ids.as_slice(),
    )?;
    Ok(FrbCalendarEventRangeResult {
        events: events.into_iter().map(to_frb_event).collect(),
    })
}

pub(crate) fn import_ics(
    calendar_id: String,
    ics_payload: String,
    source_hint: String,
) -> Result<FrbCalendarSyncResult, String> {
    let cid = calendar_id.trim();
    if cid.is_empty() {
        return Err("empty calendar_id".to_string());
    }
    let events = crate::calendar_sync::events_from_ics_payload(cid, ics_payload.as_str(), None);
    let conn = calendar_db()?;
    crate::calendar_store::upsert_calendar(
        &conn,
        &CalendarDescriptor {
            calendar_id: cid.to_string(),
            source_id: format!("source:{}", source_hint.trim()),
            name: cid.to_string(),
            color_hex: None,
            enabled: true,
        },
    )?;
    let mut n = 0u64;
    for ev in events {
        crate::calendar_store::upsert_event(&conn, &ev)?;
        n += 1;
    }
    let source_kind = format!("{:?}", crate::calendar_sync::source_kind_from_backend(&source_hint))
        .to_ascii_lowercase();
    Ok(FrbCalendarSyncResult {
        source_kind,
        imported_count: n,
        fetched_count: n,
    })
}

pub(crate) fn sync_caldav(
    calendar_id: String,
    source_id: String,
    collection_url: String,
    username: String,
    password: String,
) -> Result<FrbCalendarSyncResult, String> {
    let conn = calendar_db()?;
    let out = crate::calendar_caldav_sync::pull_calendar_collection(
        &conn,
        calendar_id.trim(),
        source_id.trim(),
        collection_url.trim(),
        username.trim(),
        password.as_str(),
    )?;
    Ok(FrbCalendarSyncResult {
        source_kind: "caldav".to_string(),
        imported_count: out.imported_events,
        fetched_count: out.fetched_resources,
    })
}

pub(crate) fn sync_provider_account(
    account_id: String,
    start_utc_ms: i64,
    end_utc_ms: i64,
) -> Result<FrbCalendarSyncResult, String> {
    if end_utc_ms < start_utc_ms {
        return Err("end_utc_ms must be >= start_utc_ms".to_string());
    }
    let (acc, use_keychain) = super::resolve_mail_account(account_id.trim())?;
    let conn = calendar_db()?;
    let out = crate::calendar_provider_sync::sync_provider_account(
        &conn,
        &acc,
        use_keychain,
        start_utc_ms,
        end_utc_ms,
    )?;
    Ok(FrbCalendarSyncResult {
        source_kind: "provider".to_string(),
        imported_count: out.fetched_events,
        fetched_count: out.fetched_calendars,
    })
}
