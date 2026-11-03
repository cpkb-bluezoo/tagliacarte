/*
 * calendar_store.rs
 * Copyright (C) 2026 Chris Burdess
 */

use rusqlite::{params, Connection};
use std::path::Path;
use tagliacarte_core::calendar::{CalendarDescriptor, CalendarEvent};

pub fn open_calendar_db(data_dir: impl AsRef<Path>) -> Result<Connection, String> {
    let p = data_dir.as_ref().join("calendar.db");
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(&p).map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS calendars (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            calendar_id TEXT NOT NULL UNIQUE,
            source_id TEXT NOT NULL,
            name TEXT NOT NULL,
            color_hex TEXT,
            enabled INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE IF NOT EXISTS calendar_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id TEXT NOT NULL UNIQUE,
            calendar_id TEXT NOT NULL,
            uid TEXT NOT NULL,
            sequence INTEGER NOT NULL DEFAULT 0,
            title TEXT NOT NULL DEFAULT '',
            description TEXT,
            location TEXT,
            starts_at_utc_ms INTEGER NOT NULL,
            ends_at_utc_ms INTEGER NOT NULL,
            all_day INTEGER NOT NULL DEFAULT 0,
            organizer TEXT,
            attendees_csv TEXT NOT NULL DEFAULT '',
            recurrence_rule TEXT,
            status TEXT,
            etag TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_time
            ON calendar_events(calendar_id, starts_at_utc_ms, ends_at_utc_ms);
        CREATE INDEX IF NOT EXISTS idx_calendar_events_uid ON calendar_events(uid);
        CREATE TABLE IF NOT EXISTS calendar_resources (
            calendar_id TEXT NOT NULL,
            href TEXT NOT NULL,
            etag TEXT NOT NULL DEFAULT '',
            event_ids TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (calendar_id, href)
        );
        CREATE TABLE IF NOT EXISTS calendar_sources (
            source_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            auth TEXT NOT NULL,
            username TEXT NOT NULL DEFAULT '',
            account_id TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1
        );
    "#,
    )
    .map_err(|e| e.to_string())?;
    // Columns added after the first version, for a database made before them.
    for (column, definition) in [("tzid", "TEXT"), ("exdates", "TEXT NOT NULL DEFAULT ''"), ("recurrence_id_ms", "INTEGER")] {
        let has: bool = conn
            .query_row("SELECT COUNT(*) FROM pragma_table_info('calendar_events') WHERE name = ?1", params![column], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            > 0;
        if !has {
            conn.execute(&format!("ALTER TABLE calendar_events ADD COLUMN {column} {definition}"), []).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn upsert_calendar(conn: &Connection, c: &CalendarDescriptor) -> Result<(), String> {
    conn.execute(
        r#"INSERT INTO calendars (calendar_id, source_id, name, color_hex, enabled)
           VALUES (?1, ?2, ?3, ?4, ?5)
           ON CONFLICT(calendar_id) DO UPDATE SET
             source_id=excluded.source_id,
             name=excluded.name,
             color_hex=excluded.color_hex,
             enabled=excluded.enabled"#,
        params![
            c.calendar_id,
            c.source_id,
            c.name,
            c.color_hex,
            if c.enabled { 1 } else { 0 },
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_calendars(conn: &Connection) -> Result<Vec<CalendarDescriptor>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT calendar_id, source_id, name, color_hex, enabled FROM calendars ORDER BY name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CalendarDescriptor {
                calendar_id: r.get(0)?,
                source_id: r.get(1)?,
                name: r.get(2)?,
                color_hex: r.get(3)?,
                enabled: r.get::<_, i64>(4)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn upsert_event(conn: &Connection, e: &CalendarEvent) -> Result<(), String> {
    conn.execute(
        r#"INSERT INTO calendar_events (
            event_id, calendar_id, uid, sequence, title, description, location,
            starts_at_utc_ms, ends_at_utc_ms, all_day, organizer, attendees_csv, recurrence_rule, status, etag,
            tzid, exdates, recurrence_id_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
        ON CONFLICT(event_id) DO UPDATE SET
            calendar_id=excluded.calendar_id,
            uid=excluded.uid,
            sequence=excluded.sequence,
            title=excluded.title,
            description=excluded.description,
            location=excluded.location,
            starts_at_utc_ms=excluded.starts_at_utc_ms,
            ends_at_utc_ms=excluded.ends_at_utc_ms,
            all_day=excluded.all_day,
            organizer=excluded.organizer,
            attendees_csv=excluded.attendees_csv,
            recurrence_rule=excluded.recurrence_rule,
            status=excluded.status,
            etag=excluded.etag,
            tzid=excluded.tzid,
            exdates=excluded.exdates,
            recurrence_id_ms=excluded.recurrence_id_ms"#,
        params![
            e.event_id,
            e.calendar_id,
            e.uid,
            e.sequence,
            e.title,
            e.description,
            e.location,
            e.starts_at_utc_ms,
            e.ends_at_utc_ms,
            if e.all_day { 1 } else { 0 },
            e.organizer,
            e.attendees.join("\n"),
            e.recurrence_rule,
            e.status,
            e.etag,
            e.tzid,
            e.exdates_utc_ms.iter().map(i64::to_string).collect::<Vec<_>>().join(","),
            e.recurrence_id_utc_ms,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

const EVENT_COLUMNS: &str = "event_id, calendar_id, uid, sequence, title, description, location, starts_at_utc_ms, ends_at_utc_ms, \
     all_day, organizer, attendees_csv, recurrence_rule, status, etag, tzid, exdates, recurrence_id_ms";

fn event_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarEvent> {
    let attendees_csv: String = r.get(11)?;
    let exdates: String = r.get::<_, Option<String>>(16)?.unwrap_or_default();
    Ok(CalendarEvent {
        event_id: r.get(0)?,
        calendar_id: r.get(1)?,
        uid: r.get(2)?,
        sequence: r.get(3)?,
        title: r.get(4)?,
        description: r.get(5)?,
        location: r.get(6)?,
        starts_at_utc_ms: r.get(7)?,
        ends_at_utc_ms: r.get(8)?,
        all_day: r.get::<_, i64>(9)? != 0,
        organizer: r.get(10)?,
        attendees: attendees_csv.split('\n').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect(),
        recurrence_rule: r.get(12)?,
        status: r.get(13)?,
        etag: r.get(14)?,
        tzid: r.get(15)?,
        exdates_utc_ms: exdates.split(',').filter_map(|v| v.trim().parse().ok()).collect(),
        recurrence_id_utc_ms: r.get(17)?,
    })
}

/// One event by its id, if there is one.
/// Removes a calendar and its events.
pub fn delete_calendar(conn: &Connection, calendar_id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM calendar_events WHERE calendar_id = ?1", params![calendar_id]).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM calendar_resources WHERE calendar_id = ?1", params![calendar_id]).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM calendars WHERE calendar_id = ?1", params![calendar_id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// A file of a server calendar as last pulled: its tag then, and the events it gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownResource {
    pub etag: String,
    pub event_ids: Vec<String>,
}

/// The files of a calendar as last pulled, by address.
pub fn known_resources(conn: &Connection, calendar_id: &str) -> Result<std::collections::HashMap<String, KnownResource>, String> {
    let mut stmt = conn
        .prepare("SELECT href, etag, event_ids FROM calendar_resources WHERE calendar_id = ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![calendar_id], |r| {
            let ids: String = r.get(2)?;
            Ok((r.get::<_, String>(0)?, KnownResource { etag: r.get(1)?, event_ids: ids.split('\n').filter(|s| !s.is_empty()).map(str::to_owned).collect() }))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

/// Records that the file at `href` has this tag and gave these events.
pub fn put_resource(conn: &Connection, calendar_id: &str, href: &str, r: &KnownResource) -> Result<(), String> {
    conn.execute(
        "INSERT INTO calendar_resources (calendar_id, href, etag, event_ids) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(calendar_id, href) DO UPDATE SET etag = excluded.etag, event_ids = excluded.event_ids",
        params![calendar_id, href, r.etag, r.event_ids.join("\n")],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn forget_resource(conn: &Connection, calendar_id: &str, href: &str) -> Result<(), String> {
    conn.execute("DELETE FROM calendar_resources WHERE calendar_id = ?1 AND href = ?2", params![calendar_id, href]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Removes the events of a calendar but not the calendar.
pub fn delete_events_of(conn: &Connection, calendar_id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM calendar_events WHERE calendar_id = ?1", params![calendar_id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_event(conn: &Connection, event_id: &str) -> Result<Option<CalendarEvent>, String> {
    let mut stmt = conn.prepare(&format!("SELECT {EVENT_COLUMNS} FROM calendar_events WHERE event_id = ?1")).map_err(|e| e.to_string())?;
    let mut rows = stmt.query_map(params![event_id], event_from_row).map_err(|e| e.to_string())?;
    rows.next().transpose().map_err(|e| e.to_string())
}

/// Removes an event row. Returns whether there was one.
pub fn delete_event(conn: &Connection, event_id: &str) -> Result<bool, String> {
    let n = conn
        .execute("DELETE FROM calendar_events WHERE event_id = ?1", params![event_id])
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

fn in_calendars(events: Vec<CalendarEvent>, calendar_ids: &[String]) -> Vec<CalendarEvent> {
    events.into_iter().filter(|e| calendar_ids.is_empty() || calendar_ids.iter().any(|id| id == &e.calendar_id)).collect()
}

/// The events stored as touching the range, each as it is stored (a series is one event,
/// at its first start; see `calendar_view::events_in_range` for the occurrences).
pub fn list_events_in_range(
    conn: &Connection,
    start_utc_ms: i64,
    end_utc_ms: i64,
    calendar_ids: &[String],
) -> Result<Vec<CalendarEvent>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM calendar_events WHERE ends_at_utc_ms >= ?1 AND starts_at_utc_ms <= ?2 ORDER BY starts_at_utc_ms ASC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![start_utc_ms, end_utc_ms], event_from_row).map_err(|e| e.to_string())?;
    Ok(in_calendars(rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?, calendar_ids))
}

/// The series that have begun by `up_to_utc_ms`, whatever else they do: the events with a
/// repeat rule that are not themselves a replacement of an occurrence.
pub fn list_series_started_by(conn: &Connection, up_to_utc_ms: i64, calendar_ids: &[String]) -> Result<Vec<CalendarEvent>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM calendar_events WHERE recurrence_rule IS NOT NULL AND recurrence_id_ms IS NULL AND starts_at_utc_ms <= ?1 ORDER BY starts_at_utc_ms ASC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![up_to_utc_ms], event_from_row).map_err(|e| e.to_string())?;
    Ok(in_calendars(rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?, calendar_ids))
}

/// The occurrences of one series that have been replaced by an event of their own: the start
/// each had.
pub fn replaced_occurrences(conn: &Connection, calendar_id: &str, uid: &str) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT recurrence_id_ms FROM calendar_events WHERE calendar_id = ?1 AND uid = ?2 AND recurrence_id_ms IS NOT NULL")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![calendar_id, uid], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_in_memory() -> Connection {
        let conn = Connection::open_in_memory().expect("open");
        migrate(&conn).expect("migrate");
        conn
    }

    #[test]
    fn upsert_and_query_event_range() {
        let conn = setup_in_memory();
        upsert_calendar(
            &conn,
            &CalendarDescriptor {
                calendar_id: "cal-a".to_string(),
                source_id: "src".to_string(),
                name: "A".to_string(),
                color_hex: None,
                enabled: true,
            },
        )
        .expect("upsert cal");
        upsert_event(
            &conn,
            &CalendarEvent {
                event_id: "ev1".to_string(),
                calendar_id: "cal-a".to_string(),
                uid: "uid-1".to_string(),
                sequence: 1,
                title: "One".to_string(),
                description: None,
                location: None,
                starts_at_utc_ms: 1_000,
                ends_at_utc_ms: 2_000,
                all_day: false,
                organizer: None,
                attendees: vec![],
                recurrence_rule: None,
                status: None,
                etag: None,
                ..Default::default()
            },
        )
        .expect("upsert event");
        let out = list_events_in_range(&conn, 500, 1500, &[]).expect("query");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].event_id, "ev1");
    }
}
