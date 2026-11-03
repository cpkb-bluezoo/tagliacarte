/*
 * calendar_recur.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * The occurrences of a repeating event that fall in a range. A series is stored as one
 * event, at its first start, with its repeat rule; what is shown in a month is worked out
 * here when it is asked for, so a series with no end costs nothing until it is looked at,
 * and a range far from where the series began is as quick as one near it.
 *
 * The rule is kept in the zone the event was made in and stepped there, so a meeting at
 * ten o'clock stays at ten o'clock when the clocks change. The rule itself is worked out
 * by the `rrule` crate (RFC 5545); this module is the part between it and our events.
 */

use chrono::{NaiveDateTime, TimeZone, Utc};
use rrule::{RRuleSet, Tz};

use tagliacarte_core::calendar::CalendarEvent;

/// The most occurrences of one series that one range can ask for.
const LIMIT: u16 = 5000;

/// The span a series is worked out over: a range asked for beyond it (an "everything" of
/// 0..i64::MAX, say) is cut to it, since neither a calendar nor the rule engine means anything
/// out there.
const EARLIEST_MS: i64 = -2_208_988_800_000; // 1900-01-01
const LATEST_MS: i64 = 7_258_118_400_000; // 2200-01-01

fn utc_text(ms: i64) -> Option<String> {
    Some(Utc.timestamp_millis_opt(ms).single()?.format("%Y%m%dT%H%M%SZ").to_string())
}

/// The rule's `UNTIL`, written the way the start is: a date (as an all-day event writes it) is
/// that whole day, and a time with no zone is a time on the clock of the series.
fn normalised_rule(rule: &str, zone: Option<chrono_tz::Tz>) -> String {
    rule.split(';')
        .map(|part| match part.strip_prefix("UNTIL=").or_else(|| part.strip_prefix("until=")) {
            Some(v) if v.len() == 8 => format!("UNTIL={v}T235959Z"),
            Some(v) if v.len() == 15 && !v.ends_with(['Z', 'z']) => {
                let utc = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok().and_then(|n| match zone {
                    Some(z) => z.from_local_datetime(&n).earliest().map(|d| d.timestamp_millis()),
                    None => Some(n.and_utc().timestamp_millis()),
                });
                utc.and_then(utc_text).map_or_else(|| part.to_owned(), |t| format!("UNTIL={t}"))
            }
            _ => part.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// The starts, in UTC milliseconds, of the occurrences from `from_ms` to `to_ms`; None if the
/// rule cannot be understood.
fn starts_between(series: &CalendarEvent, rule: &str, from_ms: i64, to_ms: i64) -> Option<Vec<i64>> {
    let zone = series.tzid.as_deref().and_then(|z| z.parse::<chrono_tz::Tz>().ok()).filter(|_| !series.all_day);
    let dtstart = match zone {
        Some(z) => format!("DTSTART;TZID={}:{}", z.name(), z.timestamp_millis_opt(series.starts_at_utc_ms).single()?.format("%Y%m%dT%H%M%S")),
        None => format!("DTSTART:{}", utc_text(series.starts_at_utc_ms)?),
    };
    let set: RRuleSet = format!("{dtstart}\nRRULE:{}", normalised_rule(rule, zone)).parse().ok()?;
    let after = Tz::UTC.timestamp_millis_opt(from_ms).single()?;
    let before = Tz::UTC.timestamp_millis_opt(to_ms).single()?;
    let found = set.after(after).before(before).all(LIMIT);
    Some(found.dates.iter().map(|d| d.timestamp_millis()).collect())
}

/// The occurrences of `series` that touch `from_ms`..`to_ms`, as events. The first keeps the
/// series' own id and the others are the series' id and where they start, so each can be told
/// apart. `replaced` are the starts of the occurrences that have an event of their own instead.
/// A rule that cannot be understood leaves the event as the one occurrence it plainly is.
pub fn occurrences(series: &CalendarEvent, replaced: &[i64], from_ms: i64, to_ms: i64) -> Vec<CalendarEvent> {
    let duration = (series.ends_at_utc_ms - series.starts_at_utc_ms).max(0);
    let touches = |start: i64| start + duration >= from_ms && start <= to_ms;
    let Some(rule) = series.recurrence_rule.as_deref().map(str::trim).filter(|r| !r.is_empty()) else {
        return if touches(series.starts_at_utc_ms) { vec![series.clone()] } else { vec![] };
    };
    let window_from = from_ms.saturating_sub(duration).saturating_sub(1).clamp(EARLIEST_MS, LATEST_MS);
    let Some(starts) = starts_between(series, rule, window_from, to_ms.clamp(EARLIEST_MS, LATEST_MS)) else {
        return if touches(series.starts_at_utc_ms) { vec![series.clone()] } else { vec![] };
    };
    starts
        .into_iter()
        .filter(|s| touches(*s) && !series.exdates_utc_ms.contains(s) && !replaced.contains(s))
        .map(|start| {
            let mut e = series.clone();
            if start != series.starts_at_utc_ms {
                e.event_id = format!("{}#{start}", series.event_id);
            }
            e.starts_at_utc_ms = start;
            e.ends_at_utc_ms = start + duration;
            e
        })
        .collect()
}
