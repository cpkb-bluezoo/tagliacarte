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
    "#,
    )
    .map_err(|e| e.to_string())?;
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
            starts_at_utc_ms, ends_at_utc_ms, all_day, organizer, attendees_csv, recurrence_rule, status, etag
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
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
            etag=excluded.etag"#,
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
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_events_in_range(
    conn: &Connection,
    start_utc_ms: i64,
    end_utc_ms: i64,
    calendar_ids: &[String],
) -> Result<Vec<CalendarEvent>, String> {
    let filter_all = calendar_ids.is_empty();
    let mut stmt = conn
        .prepare(
            r#"SELECT event_id, calendar_id, uid, sequence, title, description, location,
               starts_at_utc_ms, ends_at_utc_ms, all_day, organizer, attendees_csv, recurrence_rule, status, etag
               FROM calendar_events
               WHERE ends_at_utc_ms >= ?1 AND starts_at_utc_ms <= ?2
               ORDER BY starts_at_utc_ms ASC"#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![start_utc_ms, end_utc_ms], |r| {
            let calendar_id: String = r.get(1)?;
            if !filter_all && !calendar_ids.iter().any(|id| id == &calendar_id) {
                return Ok(None);
            }
            let attendees_csv: String = r.get(11)?;
            let attendees = attendees_csv
                .split('\n')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            Ok(Some(CalendarEvent {
                event_id: r.get(0)?,
                calendar_id,
                uid: r.get(2)?,
                sequence: r.get(3)?,
                title: r.get(4)?,
                description: r.get(5)?,
                location: r.get(6)?,
                starts_at_utc_ms: r.get(7)?,
                ends_at_utc_ms: r.get(8)?,
                all_day: r.get::<_, i64>(9)? != 0,
                organizer: r.get(10)?,
                attendees,
                recurrence_rule: r.get(12)?,
                status: r.get(13)?,
                etag: r.get(14)?,
            }))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        if let Some(ev) = r.map_err(|e| e.to_string())? {
            out.push(ev);
        }
    }
    Ok(out)
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
            },
        )
        .expect("upsert event");
        let out = list_events_in_range(&conn, 500, 1500, &[]).expect("query");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].event_id, "ev1");
    }
}
