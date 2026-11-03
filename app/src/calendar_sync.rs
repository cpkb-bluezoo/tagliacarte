/*
 * calendar_sync.rs
 * Copyright (C) 2026 Chris Burdess
 */

use tagliacarte_core::calendar::{parse_ical_events, CalendarEvent, CalendarSourceKind};

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 24 * HOUR_MS;

/// A date or time from a file as UTC milliseconds, and the zone it was read in when that mattered.
///
/// - `YYYYMMDD` is a date: UTC midnight, which is how this app keeps an all-day event;
/// - `...Z` is UTC;
/// - anything else is a time on a clock: in the zone `tzid` names if it is a zone known to the
///   tz database, and otherwise as UTC (a zone it does not know, such as Windows' own names, or
///   none at all), so the event is still there if at the wrong hour.
///
/// A time that does not exist on that clock (the hour that spring forward skips) is read as the
/// time an hour later, and one that happens twice as the first.
fn to_utc(value: &str, tzid: Option<&str>) -> Option<(i64, Option<String>)> {
    use chrono::{NaiveDate, NaiveDateTime, TimeZone};
    let t = value.trim();
    if t.len() == 8 {
        let date = NaiveDate::parse_from_str(t, "%Y%m%d").ok()?;
        return Some((date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis(), None));
    }
    let utc = t.ends_with('Z') || t.ends_with('z');
    let naive = NaiveDateTime::parse_from_str(t.trim_end_matches(['Z', 'z']), "%Y%m%dT%H%M%S").ok()?;
    let zone = if utc { None } else { tzid.and_then(|z| z.trim().parse::<chrono_tz::Tz>().ok()) };
    let Some(zone) = zone else {
        return Some((naive.and_utc().timestamp_millis(), None));
    };
    let local = zone
        .from_local_datetime(&naive)
        .earliest()
        .or_else(|| zone.from_local_datetime(&(naive + chrono::Duration::hours(1))).earliest())?;
    Some((local.timestamp_millis(), Some(zone.name().to_owned())))
}

/// `PT1H30M`, `P2DT3H`, `P1W`: how long, in milliseconds.
fn parse_duration_ms(v: &str) -> Option<i64> {
    let v = v.trim();
    let (sign, v) = match v.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, v.strip_prefix('+').unwrap_or(v)),
    };
    let mut rest = v.strip_prefix('P')?;
    let mut total = 0i64;
    let mut in_time = false;
    let mut digits = String::new();
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        match c {
            '0'..='9' => digits.push(c),
            'T' => in_time = true,
            'W' | 'D' | 'H' | 'M' | 'S' => {
                let n: i64 = digits.parse().ok()?;
                digits.clear();
                total += n * match (c, in_time) {
                    ('W', false) => 7 * DAY_MS,
                    ('D', false) => DAY_MS,
                    ('H', true) => HOUR_MS,
                    ('M', true) => 60_000,
                    ('S', true) => 1000,
                    _ => return None,
                };
            }
            _ => return None,
        }
    }
    if !digits.is_empty() {
        return None;
    }
    Some(sign * total)
}

pub fn events_from_ics_payload(
    calendar_id: &str,
    payload: &str,
    etag: Option<&str>,
) -> Vec<CalendarEvent> {
    let drafts = parse_ical_events(payload);
    let mut out = Vec::new();
    for d in drafts {
        let Some((starts, tzid)) = to_utc(d.dtstart.as_str(), d.dtstart_tzid.as_deref()) else {
            continue;
        };
        let all_day = d.dtstart.trim().len() == 8;
        let ends = if !d.dtend.trim().is_empty() {
            to_utc(d.dtend.as_str(), d.dtend_tzid.as_deref()).map(|(e, _)| e)
        } else {
            d.duration.as_deref().and_then(parse_duration_ms).map(|ms| starts + ms)
        }
        .unwrap_or(starts + if all_day { DAY_MS } else { HOUR_MS });
        let exdates_utc_ms = d.exdates.iter().filter_map(|x| to_utc(&x.value, x.tzid.as_deref().or(tzid.as_deref())).map(|(ms, _)| ms)).collect();
        let recurrence_id_utc_ms = d.recurrence_id.as_ref().and_then(|r| to_utc(&r.value, r.tzid.as_deref().or(tzid.as_deref())).map(|(ms, _)| ms));
        // The series keeps the id it has always had; what replaces one of its occurrences has its own.
        let event_id = match recurrence_id_utc_ms {
            Some(ms) => format!("{calendar_id}:{}#{ms}", d.uid),
            None => format!("{calendar_id}:{}", d.uid),
        };
        out.push(CalendarEvent {
            event_id,
            calendar_id: calendar_id.to_string(),
            uid: d.uid,
            sequence: d.sequence,
            title: d.summary,
            description: d.description,
            location: d.location,
            starts_at_utc_ms: starts,
            ends_at_utc_ms: ends,
            all_day,
            organizer: d.organizer,
            attendees: d.attendees,
            recurrence_rule: d.rrule,
            status: d.status,
            etag: etag.map(|s| s.to_string()),
            tzid,
            exdates_utc_ms,
            recurrence_id_utc_ms,
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
