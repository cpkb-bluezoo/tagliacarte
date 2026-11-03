/*
 * calendar_view.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * The calendar for a UI: which calendars there are, the events in a range, and
 * making, changing and cancelling an event, written straight to the store (the
 * earlier interface built an iCalendar text for every change and imported that).
 * Nothing here knows about a window. Each function takes the database, so that a
 * caller opens it once and a test uses one of its own; `open_db` is the usual way.
 *
 * Where times are concerned the store holds milliseconds in UTC, and an all-day
 * event is its first day at UTC midnight with the end the midnight after its last
 * day, which is how a date-valued iCalendar event is read. A UI shows an all-day
 * event by its UTC date, so that it falls on the same day wherever the viewer is.
 */

use rusqlite::Connection;
use tagliacarte_core::calendar::{CalendarDescriptor, CalendarEvent};
use tagliacarte_core::config::tagliacarte_data_dir;

use crate::calendar_store::{
    delete_event, get_event, list_calendars as store_calendars, list_events_in_range, open_calendar_db,
    upsert_calendar, upsert_event,
};
use crate::frb_api::FrbAccount;

/// The calendar database, for callers that do not use SQLite themselves.
pub type Db = Connection;

/// A calendar that is always there, for events that belong to no service.
pub const LOCAL_CALENDAR: &str = "local";
/// Where invitations from mail go.
pub const INBOX_CALENDAR: &str = "inbox";

/// What the editor sends for one event. With an `event_id` it changes that event.
#[derive(Debug, Clone)]
pub struct EventInput {
    pub event_id: Option<String>,
    pub calendar_id: String,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub starts_at_utc_ms: i64,
    pub ends_at_utc_ms: i64,
    pub all_day: bool,
    pub attendees: Vec<String>,
}

/// The calendar database in the app's data directory.
pub fn open_db() -> Result<Connection, String> {
    let dir = tagliacarte_data_dir().ok_or_else(|| "missing app data dir".to_owned())?;
    open_calendar_db(dir)
}

/// The calendar database in a directory of the caller's choosing.
pub fn open_in(dir: &std::path::Path) -> Result<Connection, String> {
    open_calendar_db(dir)
}

/// The calendars, with the local one among them whether or not anything else is.
pub fn list_calendars(conn: &Connection) -> Result<Vec<CalendarDescriptor>, String> {
    let mut all = store_calendars(conn)?;
    if !all.iter().any(|c| c.calendar_id == LOCAL_CALENDAR) {
        let local = CalendarDescriptor {
            calendar_id: LOCAL_CALENDAR.to_owned(),
            source_id: LOCAL_CALENDAR.to_owned(),
            name: "Local".to_owned(),
            color_hex: None,
            enabled: true,
        };
        upsert_calendar(conn, &local)?;
        all.push(local);
    }
    Ok(all)
}

fn is_cancelled(e: &CalendarEvent) -> bool {
    e.status.as_deref().is_some_and(|s| s.trim().eq_ignore_ascii_case("cancelled"))
}

/// The events that touch the range, in time order, leaving out those that are
/// cancelled. No calendar ids means every calendar.
pub fn events_in_range(
    conn: &Connection,
    start_utc_ms: i64,
    end_utc_ms: i64,
    calendar_ids: &[String],
) -> Result<Vec<CalendarEvent>, String> {
    if end_utc_ms < start_utc_ms {
        return Err("the end of the range is before its start".to_owned());
    }
    Ok(list_events_in_range(conn, start_utc_ms, end_utc_ms, calendar_ids)?
        .into_iter()
        .filter(|e| !is_cancelled(e))
        .collect())
}

/// Makes a new event, or changes the one `input.event_id` names. A change keeps
/// what the editor does not show (who organised it, how it repeats, the server's
/// tag) and is a step on in sequence, which is how a calendar server tells a newer
/// version from an older one. `now_micros` makes a new event's identity.
pub fn save_event(conn: &Connection, input: EventInput, now_micros: i64) -> Result<CalendarEvent, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("an event needs a title".to_owned());
    }
    if input.all_day {
        if input.ends_at_utc_ms <= input.starts_at_utc_ms {
            return Err("the end of an all-day event must be after its start".to_owned());
        }
    } else if input.ends_at_utc_ms < input.starts_at_utc_ms {
        return Err("the end of an event must not be before its start".to_owned());
    }
    if !list_calendars(conn)?.iter().any(|c| c.calendar_id == input.calendar_id) {
        return Err(format!("no such calendar '{}'", input.calendar_id));
    }
    let mut attendees: Vec<String> = Vec::new();
    for a in input.attendees.iter().map(|a| a.trim()).filter(|a| !a.is_empty()) {
        if !attendees.iter().any(|x| x == a) {
            attendees.push(a.to_owned());
        }
    }
    let clean = |v: Option<String>| v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());

    let existing = match &input.event_id {
        Some(id) => Some(get_event(conn, id)?.ok_or_else(|| format!("no such event '{id}'"))?),
        None => None,
    };
    let uid = existing.as_ref().map_or_else(|| format!("tc-{now_micros}"), |e| e.uid.clone());
    let event = CalendarEvent {
        event_id: format!("{}:{uid}", input.calendar_id),
        calendar_id: input.calendar_id.clone(),
        uid,
        sequence: existing.as_ref().map_or(0, |e| e.sequence + 1),
        title: title.to_owned(),
        description: clean(input.description),
        location: clean(input.location),
        starts_at_utc_ms: input.starts_at_utc_ms,
        ends_at_utc_ms: input.ends_at_utc_ms,
        all_day: input.all_day,
        organizer: existing.as_ref().and_then(|e| e.organizer.clone()),
        attendees,
        recurrence_rule: existing.as_ref().and_then(|e| e.recurrence_rule.clone()),
        status: Some(existing.as_ref().and_then(|e| e.status.clone()).unwrap_or_else(|| "CONFIRMED".to_owned())),
        etag: existing.as_ref().and_then(|e| e.etag.clone()),
    };
    upsert_event(conn, &event)?;
    // Moved to another calendar: its identity names the calendar, so the old row goes.
    if let Some(old) = existing.filter(|e| e.event_id != event.event_id) {
        delete_event(conn, &old.event_id)?;
    }
    Ok(event)
}

/// Marks an event cancelled, which hides it. It is kept, a step on in sequence,
/// so that a newer version from a server replaces it as it would any other.
pub fn cancel_event(conn: &Connection, event_id: &str) -> Result<(), String> {
    let mut e = get_event(conn, event_id)?.ok_or_else(|| format!("no such event '{event_id}'"))?;
    e.status = Some("CANCELLED".to_owned());
    e.sequence += 1;
    upsert_event(conn, &e)
}

/// Brings the events of an iCalendar text into a calendar, which is made, named
/// `name`, if it is not there. Importing the same text again changes nothing, as
/// an event is known by its UID. Returns how many events the text held.
pub fn import_ics(conn: &Connection, calendar_id: &str, name: &str, ics: &str) -> Result<usize, String> {
    let id = calendar_id.trim();
    if id.is_empty() {
        return Err("empty calendar id".to_owned());
    }
    let events = crate::calendar_sync::events_from_ics_payload(id, ics, None);
    if !store_calendars(conn)?.iter().any(|c| c.calendar_id == id) {
        upsert_calendar(
            conn,
            &CalendarDescriptor {
                calendar_id: id.to_owned(),
                source_id: format!("source:{id}"),
                name: name.to_owned(),
                color_hex: None,
                enabled: true,
            },
        )?;
    }
    for e in &events {
        upsert_event(conn, e)?;
    }
    Ok(events.len())
}

/// Whether an account has a calendar service to sync: Gmail, Microsoft Graph or
/// Exchange, or anything signed in with Google or Microsoft.
pub fn is_provider_account(a: &FrbAccount) -> bool {
    let kind = a.backend_type.trim().to_ascii_lowercase();
    let provider = a.attrs.get("oauthProvider").map(|p| p.trim().to_ascii_lowercase()).unwrap_or_default();
    matches!(kind.as_str(), "gmail" | "graph" | "exchange") || matches!(provider.as_str(), "google" | "microsoft")
}

/// What a sync of the accounts' calendars came to.
#[derive(Debug, Default, Clone)]
pub struct SyncReport {
    pub calendars: u64,
    pub events: u64,
    /// The accounts that could not be synced, and why.
    pub failed: Vec<(String, String)>,
}

/// Syncs the calendars of every account that has a calendar service, and the calendars
/// added by address, for events
/// from `start_utc_ms` to `end_utc_ms`. Waits on the network: not for a thread
/// that draws a window or runs a connection.
pub fn sync_providers(config_path: &str, start_utc_ms: i64, end_utc_ms: i64) -> Result<SyncReport, String> {
    let cfg = crate::frb_api::frb_load_config(config_path.to_owned());
    let mut report = SyncReport::default();
    for a in cfg.accounts.iter().filter(|a| is_provider_account(a)) {
        match crate::frb_api::frb_calendar_sync_provider_account(a.id.clone(), start_utc_ms, end_utc_ms) {
            Ok(r) => {
                report.calendars += r.fetched_count;
                report.events += r.imported_count;
            }
            Err(e) => report.failed.push((a.id.clone(), e)),
        }
    }
    // The calendars the user added by address.
    match open_db() {
        Ok(conn) => {
            let sources = crate::calendar_sources::sync_sources(&conn);
            report.calendars += sources.calendars;
            report.events += sources.events;
            report.failed.extend(sources.failed);
        }
        Err(e) => report.failed.push(("calendar sources".to_owned(), e)),
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar_store::{open_calendar_db, upsert_calendar, upsert_event};

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_calendar_db(dir.path()).unwrap();
        (dir, conn)
    }

    fn input(title: &str, start: i64, end: i64) -> EventInput {
        EventInput {
            event_id: None,
            calendar_id: LOCAL_CALENDAR.to_owned(),
            title: title.to_owned(),
            description: None,
            location: None,
            starts_at_utc_ms: start,
            ends_at_utc_ms: end,
            all_day: false,
            attendees: vec![],
        }
    }

    const HOUR: i64 = 3_600_000;
    const DAY: i64 = 24 * HOUR;

    #[test]
    fn there_is_always_a_local_calendar_and_only_one() {
        let (_d, conn) = db();
        let first = list_calendars(&conn).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!((first[0].calendar_id.as_str(), first[0].enabled), (LOCAL_CALENDAR, true));
        assert_eq!(list_calendars(&conn).unwrap().len(), 1, "asking again adds nothing");

        upsert_calendar(&conn, &CalendarDescriptor { calendar_id: "work".into(), source_id: "s".into(), name: "Work".into(), color_hex: Some("#336699".into()), enabled: true }).unwrap();
        let names: Vec<String> = list_calendars(&conn).unwrap().into_iter().map(|c| c.calendar_id).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"work".to_owned()) && names.contains(&LOCAL_CALENDAR.to_owned()));
    }

    #[test]
    fn a_new_event_gets_its_own_identity_and_is_listed() {
        let (_d, conn) = db();
        let mut i = input("  Planning  ", 10 * DAY, 10 * DAY + HOUR);
        i.attendees = vec![" a@example.org ".into(), "".into(), "a@example.org".into(), "b@example.org".into()];
        i.location = Some("Room 1".into());
        let e = save_event(&conn, i, 1_234_567).unwrap();
        assert_eq!(e.uid, "tc-1234567");
        assert_eq!(e.event_id, "local:tc-1234567");
        assert_eq!((e.sequence, e.title.as_str(), e.status.as_deref()), (0, "Planning", Some("CONFIRMED")));
        assert_eq!(e.attendees, ["a@example.org", "b@example.org"]);
        let listed = events_in_range(&conn, 9 * DAY, 11 * DAY, &[]).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].location.as_deref(), Some("Room 1"));
        // Two made in the same microsecond would be one event; the clock moves on.
        assert_ne!(save_event(&conn, input("Other", 10 * DAY, 10 * DAY + HOUR), 1_234_568).unwrap().event_id, e.event_id);
    }

    #[test]
    fn editing_keeps_what_it_was_and_raises_the_sequence() {
        let (_d, conn) = db();
        list_calendars(&conn).unwrap();
        // Something that came from a server, with what the editor does not show.
        upsert_event(&conn, &CalendarEvent {
            event_id: "local:srv-1".into(), calendar_id: "local".into(), uid: "srv-1".into(), sequence: 4, title: "Standup".into(),
            description: None, location: None, starts_at_utc_ms: 5 * DAY, ends_at_utc_ms: 5 * DAY + HOUR, all_day: false,
            organizer: Some("boss@example.org".into()), attendees: vec![], recurrence_rule: Some("FREQ=DAILY".into()),
            status: Some("CONFIRMED".into()), etag: Some("\"abc\"".into()),
        }).unwrap();
        let mut i = input("Standup (moved)", 5 * DAY + HOUR, 5 * DAY + 2 * HOUR);
        i.event_id = Some("local:srv-1".into());
        let e = save_event(&conn, i, 99).unwrap();
        assert_eq!((e.uid.as_str(), e.event_id.as_str(), e.sequence), ("srv-1", "local:srv-1", 5));
        assert_eq!(e.organizer.as_deref(), Some("boss@example.org"));
        assert_eq!(e.recurrence_rule.as_deref(), Some("FREQ=DAILY"));
        assert_eq!(e.etag.as_deref(), Some("\"abc\""));
        assert_eq!(events_in_range(&conn, 0, 20 * DAY, &[]).unwrap().len(), 1, "edited, not copied");
    }

    #[test]
    fn moving_an_event_to_another_calendar_leaves_nothing_behind() {
        let (_d, conn) = db();
        upsert_calendar(&conn, &CalendarDescriptor { calendar_id: "work".into(), source_id: "s".into(), name: "Work".into(), color_hex: None, enabled: true }).unwrap();
        let e = save_event(&conn, input("Review", DAY, DAY + HOUR), 7).unwrap();
        let mut i = input("Review", DAY, DAY + HOUR);
        i.event_id = Some(e.event_id.clone());
        i.calendar_id = "work".into();
        let moved = save_event(&conn, i, 8).unwrap();
        assert_eq!((moved.calendar_id.as_str(), moved.event_id.as_str(), moved.uid.as_str()), ("work", "work:tc-7", "tc-7"));
        let all = events_in_range(&conn, 0, 10 * DAY, &[]).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].calendar_id, "work");
    }

    #[test]
    fn what_cannot_be_an_event_is_refused() {
        let (_d, conn) = db();
        assert!(save_event(&conn, input("   ", DAY, DAY + HOUR), 1).unwrap_err().contains("title"));
        assert!(save_event(&conn, input("Backwards", DAY, DAY - 1), 1).unwrap_err().contains("end"));
        let mut all_day = input("Holiday", DAY, DAY);
        all_day.all_day = true;
        assert!(save_event(&conn, all_day, 1).unwrap_err().contains("end"), "an all-day event covers at least a day");
        let mut elsewhere = input("Lost", DAY, DAY + HOUR);
        elsewhere.calendar_id = "no-such".into();
        assert!(save_event(&conn, elsewhere, 1).unwrap_err().contains("calendar"));
        let mut missing = input("Ghost", DAY, DAY + HOUR);
        missing.event_id = Some("local:nobody".into());
        assert!(save_event(&conn, missing, 1).unwrap_err().contains("no such event"));
        // A zero-length event (a reminder) is fine.
        assert!(save_event(&conn, input("Ping", DAY, DAY), 2).is_ok());
    }

    #[test]
    fn a_cancelled_event_is_no_longer_listed() {
        let (_d, conn) = db();
        let e = save_event(&conn, input("Gone soon", DAY, DAY + HOUR), 5).unwrap();
        cancel_event(&conn, &e.event_id).unwrap();
        assert!(events_in_range(&conn, 0, 10 * DAY, &[]).unwrap().is_empty());
        // Kept, as cancelled and a step on, so that a later update from a server can replace it.
        let kept = crate::calendar_store::get_event(&conn, &e.event_id).unwrap().unwrap();
        assert_eq!((kept.status.as_deref(), kept.sequence), (Some("CANCELLED"), 1));
        // One a server cancelled is hidden the same way, whatever the case.
        upsert_event(&conn, &CalendarEvent {
            event_id: "local:s".into(), calendar_id: "local".into(), uid: "s".into(), sequence: 2, title: "Cancelled by them".into(),
            description: None, location: None, starts_at_utc_ms: DAY, ends_at_utc_ms: DAY + HOUR, all_day: false, organizer: None,
            attendees: vec![], recurrence_rule: None, status: Some("cancelled".into()), etag: None,
        }).unwrap();
        assert!(events_in_range(&conn, 0, 10 * DAY, &[]).unwrap().is_empty());
        assert!(cancel_event(&conn, "local:nobody").unwrap_err().contains("no such event"));
    }

    #[test]
    fn calendars_can_be_filtered() {
        let (_d, conn) = db();
        upsert_calendar(&conn, &CalendarDescriptor { calendar_id: "work".into(), source_id: "s".into(), name: "Work".into(), color_hex: None, enabled: true }).unwrap();
        save_event(&conn, input("Mine", DAY, DAY + HOUR), 1).unwrap();
        let mut w = input("Theirs", DAY, DAY + HOUR);
        w.calendar_id = "work".into();
        save_event(&conn, w, 2).unwrap();
        assert_eq!(events_in_range(&conn, 0, 5 * DAY, &["work".to_owned()]).unwrap()[0].title, "Theirs");
        assert_eq!(events_in_range(&conn, 0, 5 * DAY, &[]).unwrap().len(), 2);
    }

    #[test]
    fn an_invitation_is_imported_into_its_own_calendar_and_again_without_doubling() {
        let (_d, conn) = db();
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:a@x\r\nSEQUENCE:0\r\nSUMMARY:Planning\r\nDTSTART:20260105T090000Z\r\nDTEND:20260105T100000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:b@x\r\nSUMMARY:Holiday\r\nDTSTART;VALUE=DATE:20260110\r\nDTEND;VALUE=DATE:20260112\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(import_ics(&conn, "inbox", "Invitations", ics).unwrap(), 2);
        assert_eq!(import_ics(&conn, "inbox", "Invitations", ics).unwrap(), 2);
        let all = events_in_range(&conn, 0, i64::MAX / 2, &["inbox".to_owned()]).unwrap();
        assert_eq!(all.len(), 2, "the same invitation twice is still two events");
        let holiday = all.iter().find(|e| e.title == "Holiday").unwrap();
        assert!(holiday.all_day);
        let cal = list_calendars(&conn).unwrap().into_iter().find(|c| c.calendar_id == "inbox").unwrap();
        assert_eq!(cal.name, "Invitations");
        assert_eq!(import_ics(&conn, "inbox", "Invitations", "not a calendar").unwrap(), 0);
    }

    #[test]
    fn only_accounts_with_a_calendar_service_are_synced() {
        let account = |ty: &str, provider: Option<&str>| {
            let mut a = crate::frb_api::FrbAccount { id: "x".into(), label: "x".into(), backend_type: ty.into(), ..Default::default() };
            if let Some(p) = provider {
                a.attrs.insert("oauthProvider".into(), p.into());
            }
            a
        };
        for ty in ["gmail", "Gmail", "graph", "exchange"] {
            assert!(is_provider_account(&account(ty, None)), "{ty}");
        }
        assert!(is_provider_account(&account("imap", Some("google"))));
        assert!(is_provider_account(&account("imap", Some(" Microsoft "))));
        for ty in ["imap", "pop3", "maildir", "nostr", "matrix"] {
            assert!(!is_provider_account(&account(ty, None)), "{ty}");
        }
    }
}
