/*
 * calendar_sync.rs
 * Copyright (C) 2026 Chris Burdess
 */

use tagliacarte_core::calendar::{parse_ical_events, CalendarEvent, CalendarSourceKind};

fn parse_ical_utc_ts(ical: &str) -> Option<i64> {
    // Expected forms used by v1 import paths:
    // - YYYYMMDDTHHMMSSZ
    // - YYYYMMDD
    let t = ical.trim();
    if t.len() == 8 {
        let y = t[0..4].parse::<i32>().ok()?;
        let m = t[4..6].parse::<u32>().ok()?;
        let d = t[6..8].parse::<u32>().ok()?;
        let dt = chrono::NaiveDate::from_ymd_opt(y, m, d)?.and_hms_opt(0, 0, 0)?;
        return Some(dt.and_utc().timestamp_millis());
    }
    if t.len() >= 16 {
        let base = t.trim_end_matches('Z');
        if base.len() < 15 {
            return None;
        }
        let y = base[0..4].parse::<i32>().ok()?;
        let m = base[4..6].parse::<u32>().ok()?;
        let d = base[6..8].parse::<u32>().ok()?;
        let hh = base[9..11].parse::<u32>().ok()?;
        let mm = base[11..13].parse::<u32>().ok()?;
        let ss = base[13..15].parse::<u32>().ok()?;
        let dt = chrono::NaiveDate::from_ymd_opt(y, m, d)?.and_hms_opt(hh, mm, ss)?;
        return Some(dt.and_utc().timestamp_millis());
    }
    None
}

pub fn events_from_ics_payload(
    calendar_id: &str,
    payload: &str,
    etag: Option<&str>,
) -> Vec<CalendarEvent> {
    let drafts = parse_ical_events(payload);
    let mut out = Vec::new();
    for d in drafts {
        let Some(starts) = parse_ical_utc_ts(d.dtstart.as_str()) else {
            continue;
        };
        let ends = parse_ical_utc_ts(d.dtend.as_str()).unwrap_or(starts + 60 * 60 * 1000);
        out.push(CalendarEvent {
            event_id: format!("{calendar_id}:{}", d.uid),
            calendar_id: calendar_id.to_string(),
            uid: d.uid,
            sequence: d.sequence,
            title: d.summary,
            description: d.description,
            location: d.location,
            starts_at_utc_ms: starts,
            ends_at_utc_ms: ends,
            all_day: d.dtstart.len() == 8,
            organizer: d.organizer,
            attendees: d.attendees,
            recurrence_rule: d.rrule,
            status: d.status,
            etag: etag.map(|s| s.to_string()),
        });
    }
    out
}

pub fn source_kind_from_backend(s: &str) -> CalendarSourceKind {
    match s.trim().to_ascii_lowercase().as_str() {
        "caldav" => CalendarSourceKind::Caldav,
        "google" => CalendarSourceKind::Google,
        "graph" => CalendarSourceKind::Graph,
        _ => CalendarSourceKind::LocalIcs,
    }
}
