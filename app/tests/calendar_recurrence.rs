//! Repeating events: reading them from a file (zones, durations, exceptions, replacements),
//! keeping them, and showing the occurrences that fall in a range.

use chrono::{TimeZone, Utc};
use tagliacarte_app::calendar_recur::occurrences;
use tagliacarte_app::calendar_store::{get_event, open_calendar_db, upsert_event};
use tagliacarte_app::calendar_sync::events_from_ics_payload;
use tagliacarte_core::calendar::CalendarEvent;

fn ics(body: &str) -> String {
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\n{body}END:VCALENDAR\r\n")
}

fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().timestamp_millis()
}

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

#[test]
fn a_time_in_a_zone_is_read_in_that_zone() {
    // London is an hour ahead of UTC in the summer and level with it in the winter.
    let summer = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Standup\r\nDTSTART;TZID=Europe/London:20260601T100000\r\nDTEND;TZID=Europe/London:20260601T103000\r\nEND:VEVENT\r\n"), None)[0];
    assert_eq!((summer.starts_at_utc_ms, summer.ends_at_utc_ms), (utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 9, 30)));
    assert_eq!(summer.tzid.as_deref(), Some("Europe/London"));
    let winter = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=Europe/London:20261201T100000\r\nEND:VEVENT\r\n"), None)[0];
    assert_eq!(winter.starts_at_utc_ms, utc(2026, 12, 1, 10, 0));
    // Kolkata is half an hour off a whole hour, and a zone to the west of UTC goes the other way.
    let kolkata = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=Asia/Kolkata:20260601T100000\r\nEND:VEVENT\r\n"), None)[0];
    assert_eq!(kolkata.starts_at_utc_ms, utc(2026, 6, 1, 4, 30));
    let ny = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=America/New_York:20260601T100000\r\nEND:VEVENT\r\n"), None)[0];
    assert_eq!(ny.starts_at_utc_ms, utc(2026, 6, 1, 14, 0));
    // UTC and a time that names no zone are as before.
    let z = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART:20260601T100000Z\r\nEND:VEVENT\r\n"), None)[0];
    assert_eq!((z.starts_at_utc_ms, z.tzid.clone()), (utc(2026, 6, 1, 10, 0), None));
}

#[test]
fn a_zone_that_is_not_known_is_read_as_utc_rather_than_dropping_the_event() {
    let e = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=W. Europe Standard Time:20260601T100000\r\nEND:VEVENT\r\n"), None);
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].starts_at_utc_ms, e[0].tzid.clone()), (utc(2026, 6, 1, 10, 0), None));
}

#[test]
fn how_long_it_lasts_comes_from_the_end_or_the_duration() {
    let one = |lines: &str| events_from_ics_payload("c", &ics(&format!("BEGIN:VEVENT\r\nUID:a\r\n{lines}END:VEVENT\r\n")), None).remove(0);
    let d = one("DTSTART:20260601T100000Z\r\nDURATION:PT1H30M\r\n");
    assert_eq!(d.ends_at_utc_ms - d.starts_at_utc_ms, 90 * 60_000);
    let days = one("DTSTART:20260601T100000Z\r\nDURATION:P2DT3H\r\n");
    assert_eq!(days.ends_at_utc_ms - days.starts_at_utc_ms, 2 * DAY + 3 * HOUR);
    let week = one("DTSTART;VALUE=DATE:20260601\r\nDURATION:P1W\r\n");
    assert_eq!(week.ends_at_utc_ms - week.starts_at_utc_ms, 7 * DAY);
    // The end wins over a duration; with neither, a timed event is an hour and a day is a day.
    let both = one("DTSTART:20260601T100000Z\r\nDTEND:20260601T101500Z\r\nDURATION:PT5H\r\n");
    assert_eq!(both.ends_at_utc_ms - both.starts_at_utc_ms, 15 * 60_000);
    let neither = one("DTSTART:20260601T100000Z\r\n");
    assert_eq!(neither.ends_at_utc_ms - neither.starts_at_utc_ms, HOUR);
    let day = one("DTSTART;VALUE=DATE:20260601\r\n");
    assert!(day.all_day);
    assert_eq!((day.starts_at_utc_ms, day.ends_at_utc_ms), (utc(2026, 6, 1, 0, 0), utc(2026, 6, 2, 0, 0)));
}

#[test]
fn a_series_and_what_replaces_one_of_its_occurrences_are_two_events() {
    let raw = ics(
        "BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Standup\r\nDTSTART;TZID=Europe/London:20260601T100000\r\nDTEND;TZID=Europe/London:20260601T103000\r\nRRULE:FREQ=DAILY;COUNT=5\r\n\
         EXDATE;TZID=Europe/London:20260604T100000\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Standup (moved)\r\nRECURRENCE-ID;TZID=Europe/London:20260603T100000\r\n\
         DTSTART;TZID=Europe/London:20260603T140000\r\nDTEND;TZID=Europe/London:20260603T143000\r\nEND:VEVENT\r\n",
    );
    let evs = events_from_ics_payload("c", &raw, None);
    assert_eq!(evs.len(), 2);
    let (master, moved) = (&evs[0], &evs[1]);
    assert_eq!(master.event_id, "c:s", "a series keeps the id it always had");
    assert_eq!(master.recurrence_rule.as_deref(), Some("FREQ=DAILY;COUNT=5"));
    assert_eq!(master.exdates_utc_ms, [utc(2026, 6, 4, 9, 0)]);
    assert_eq!(master.recurrence_id_utc_ms, None);
    // The replacement says which occurrence it replaces, and has an id of its own.
    assert_eq!(moved.recurrence_id_utc_ms, Some(utc(2026, 6, 3, 9, 0)));
    assert_eq!(moved.event_id, format!("c:s#{}", utc(2026, 6, 3, 9, 0)));
    assert_eq!((moved.title.as_str(), moved.starts_at_utc_ms), ("Standup (moved)", utc(2026, 6, 3, 13, 0)));
    assert_eq!(moved.recurrence_rule, None);
}

#[test]
fn the_zone_exceptions_and_replacements_are_kept_and_an_older_database_is_brought_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    // A database as the last version made it, without the new columns, with an event in it.
    {
        let old = rusqlite::Connection::open(dir.path().join("calendar.db")).unwrap();
        old.execute_batch(
            "CREATE TABLE calendars (id INTEGER PRIMARY KEY AUTOINCREMENT, calendar_id TEXT NOT NULL UNIQUE, source_id TEXT NOT NULL, name TEXT NOT NULL, color_hex TEXT, enabled INTEGER NOT NULL DEFAULT 1);
             CREATE TABLE calendar_events (id INTEGER PRIMARY KEY AUTOINCREMENT, event_id TEXT NOT NULL UNIQUE, calendar_id TEXT NOT NULL, uid TEXT NOT NULL, sequence INTEGER NOT NULL DEFAULT 0, title TEXT NOT NULL DEFAULT '', description TEXT, location TEXT, starts_at_utc_ms INTEGER NOT NULL, ends_at_utc_ms INTEGER NOT NULL, all_day INTEGER NOT NULL DEFAULT 0, organizer TEXT, attendees_csv TEXT NOT NULL DEFAULT '', recurrence_rule TEXT, status TEXT, etag TEXT);
             INSERT INTO calendar_events (event_id, calendar_id, uid, title, starts_at_utc_ms, ends_at_utc_ms, recurrence_rule) VALUES ('c:old', 'c', 'old', 'Old', 1000, 2000, 'FREQ=DAILY');",
        )
        .unwrap();
    }
    let conn = open_calendar_db(dir.path()).unwrap();
    let old = get_event(&conn, "c:old").unwrap().unwrap();
    assert_eq!((old.title.as_str(), old.recurrence_rule.as_deref(), old.tzid.clone(), old.exdates_utc_ms.clone(), old.recurrence_id_utc_ms), ("Old", Some("FREQ=DAILY"), None, vec![], None));

    let raw = ics("BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:A\r\nDTSTART;TZID=Europe/London:20260601T100000\r\nRRULE:FREQ=DAILY\r\nEXDATE;TZID=Europe/London:20260602T100000,20260603T100000\r\nEND:VEVENT\r\n\
                   BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:B\r\nRECURRENCE-ID;TZID=Europe/London:20260604T100000\r\nDTSTART;TZID=Europe/London:20260604T120000\r\nEND:VEVENT\r\n");
    for e in events_from_ics_payload("c", &raw, None) {
        upsert_event(&conn, &e).unwrap();
        assert_eq!(get_event(&conn, &e.event_id).unwrap().unwrap(), e, "{} comes back as it went in", e.event_id);
    }
}

// ── the occurrences of a series ──

/// A series from a file: its first event.
fn series(body: &str) -> CalendarEvent {
    events_from_ics_payload("c", &ics(&format!("BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Series\r\n{body}END:VEVENT\r\n")), None).remove(0)
}

fn starts(list: &[CalendarEvent]) -> Vec<i64> {
    list.iter().map(|e| e.starts_at_utc_ms).collect()
}

#[test]
fn a_weekly_series_has_its_occurrences_in_the_range_with_ids_of_their_own() {
    let s = series("DTSTART:20260601T100000Z\r\nDTEND:20260601T110000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=4\r\n");
    let all = occurrences(&s, &[], utc(2026, 6, 1, 0, 0), utc(2026, 7, 31, 0, 0));
    assert_eq!(starts(&all), [utc(2026, 6, 1, 10, 0), utc(2026, 6, 8, 10, 0), utc(2026, 6, 15, 10, 0), utc(2026, 6, 22, 10, 0)], "COUNT=4 is four");
    assert!(all.iter().all(|e| e.ends_at_utc_ms - e.starts_at_utc_ms == HOUR && e.title == "Series" && e.recurrence_rule.is_some()));
    // The first keeps the series' own id, the others are the series and where they start.
    assert_eq!(all[0].event_id, "c:s");
    assert_eq!(all[1].event_id, format!("c:s#{}", utc(2026, 6, 8, 10, 0)));
    // Only the ones in the range.
    let some = occurrences(&s, &[], utc(2026, 6, 5, 0, 0), utc(2026, 6, 20, 0, 0));
    assert_eq!(starts(&some), [utc(2026, 6, 8, 10, 0), utc(2026, 6, 15, 10, 0)]);
}

#[test]
fn it_keeps_the_wall_clock_time_across_a_change_of_clocks() {
    // Clocks in London go forward at 01:00 UTC on 29 March 2026: 10:00 is 10:00Z before and 09:00Z after.
    let s = series("DTSTART;TZID=Europe/London:20260326T100000\r\nDTEND;TZID=Europe/London:20260326T110000\r\nRRULE:FREQ=DAILY;COUNT=5\r\n");
    let all = occurrences(&s, &[], utc(2026, 3, 1, 0, 0), utc(2026, 4, 30, 0, 0));
    assert_eq!(starts(&all), [utc(2026, 3, 26, 10, 0), utc(2026, 3, 27, 10, 0), utc(2026, 3, 28, 10, 0), utc(2026, 3, 29, 9, 0), utc(2026, 3, 30, 9, 0)]);
    assert!(all.iter().all(|e| e.ends_at_utc_ms - e.starts_at_utc_ms == HOUR));
    let ny = series("DTSTART;TZID=America/New_York:20261030T100000\r\nRRULE:FREQ=DAILY;COUNT=4\r\n");
    // Clocks in New York go back on 1 November: 14:00Z before, 15:00Z after.
    assert_eq!(starts(&occurrences(&ny, &[], utc(2026, 10, 1, 0, 0), utc(2026, 12, 1, 0, 0))), [utc(2026, 10, 30, 14, 0), utc(2026, 10, 31, 14, 0), utc(2026, 11, 1, 15, 0), utc(2026, 11, 2, 15, 0)]);
}

#[test]
fn the_months_and_years_a_rule_can_name() {
    // The last Friday of the month: 30 Jan, 27 Feb, 27 Mar 2026.
    let last_friday = series("DTSTART:20260130T090000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=-1FR;COUNT=3\r\n");
    assert_eq!(starts(&occurrences(&last_friday, &[], 0, utc(2027, 1, 1, 0, 0))), [utc(2026, 1, 30, 9, 0), utc(2026, 2, 27, 9, 0), utc(2026, 3, 27, 9, 0)]);
    // The second Tuesday: 10 Feb, 10 Mar 2026.
    let second_tuesday = series("DTSTART:20260210T090000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=2TU;COUNT=2\r\n");
    assert_eq!(starts(&occurrences(&second_tuesday, &[], 0, utc(2027, 1, 1, 0, 0))), [utc(2026, 2, 10, 9, 0), utc(2026, 3, 10, 9, 0)]);
    // Every other day until a date, and the 31st: months without one are skipped.
    let every_other = series("DTSTART:20260601T080000Z\r\nRRULE:FREQ=DAILY;INTERVAL=2;UNTIL=20260607T235959Z\r\n");
    assert_eq!(starts(&occurrences(&every_other, &[], 0, utc(2027, 1, 1, 0, 0))), [utc(2026, 6, 1, 8, 0), utc(2026, 6, 3, 8, 0), utc(2026, 6, 5, 8, 0), utc(2026, 6, 7, 8, 0)]);
    let thirty_first = series("DTSTART:20260131T080000Z\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=31;COUNT=3\r\n");
    assert_eq!(starts(&occurrences(&thirty_first, &[], 0, utc(2027, 1, 1, 0, 0))), [utc(2026, 1, 31, 8, 0), utc(2026, 3, 31, 8, 0), utc(2026, 5, 31, 8, 0)]);
}

#[test]
fn a_yearly_all_day_event_is_a_day_on_the_same_date_each_year() {
    let birthday = series("DTSTART;VALUE=DATE:20200305\r\nDTEND;VALUE=DATE:20200306\r\nRRULE:FREQ=YEARLY\r\n");
    let o = occurrences(&birthday, &[], utc(2026, 3, 1, 0, 0), utc(2026, 3, 31, 0, 0));
    assert_eq!(o.len(), 1);
    assert!(o[0].all_day);
    assert_eq!((o[0].starts_at_utc_ms, o[0].ends_at_utc_ms), (utc(2026, 3, 5, 0, 0), utc(2026, 3, 6, 0, 0)));
    // UNTIL as a date, the way an all-day event writes it, includes that day.
    let until = series("DTSTART;VALUE=DATE:20200305\r\nRRULE:FREQ=YEARLY;UNTIL=20240305\r\n");
    let years: Vec<i64> = occurrences(&until, &[], 0, utc(2030, 1, 1, 0, 0)).iter().map(|e| e.starts_at_utc_ms).collect();
    assert_eq!(years, [utc(2020, 3, 5, 0, 0), utc(2021, 3, 5, 0, 0), utc(2022, 3, 5, 0, 0), utc(2023, 3, 5, 0, 0), utc(2024, 3, 5, 0, 0)]);
}

#[test]
fn exceptions_and_replaced_occurrences_do_not_happen() {
    let s = series("DTSTART;TZID=Europe/London:20260601T100000\r\nRRULE:FREQ=WEEKLY;COUNT=4\r\nEXDATE;TZID=Europe/London:20260608T100000\r\n");
    assert_eq!(s.exdates_utc_ms, [utc(2026, 6, 8, 9, 0)]);
    let o = occurrences(&s, &[utc(2026, 6, 22, 9, 0)], 0, utc(2027, 1, 1, 0, 0));
    assert_eq!(starts(&o), [utc(2026, 6, 1, 9, 0), utc(2026, 6, 15, 9, 0)], "the 8th is excepted and the 22nd is replaced by an event of its own");
}

#[test]
fn an_occurrence_that_began_before_the_range_but_runs_into_it_is_in_it() {
    let night = series("DTSTART:20260601T220000Z\r\nDTEND:20260602T020000Z\r\nRRULE:FREQ=DAILY;COUNT=5\r\n");
    let o = occurrences(&night, &[], utc(2026, 6, 3, 1, 0), utc(2026, 6, 3, 12, 0));
    assert_eq!(starts(&o), [utc(2026, 6, 2, 22, 0)], "the one that began the evening before");
}

#[test]
fn a_series_with_no_end_is_cut_off_by_the_range_not_by_a_limit_in_the_past() {
    let daily = series("DTSTART:20150101T090000Z\r\nRRULE:FREQ=DAILY\r\n");
    let week = occurrences(&daily, &[], utc(2026, 6, 1, 0, 0), utc(2026, 6, 7, 23, 59));
    assert_eq!(week.len(), 7, "{:?}", starts(&week));
    assert_eq!(week[0].starts_at_utc_ms, utc(2026, 6, 1, 9, 0));
    // One that has not begun by the end of the range has nothing in it.
    let later = series("DTSTART:20270101T090000Z\r\nRRULE:FREQ=DAILY\r\n");
    assert!(occurrences(&later, &[], utc(2026, 6, 1, 0, 0), utc(2026, 6, 7, 0, 0)).is_empty());
}

#[test]
fn a_rule_that_cannot_be_understood_leaves_the_event_itself() {
    let broken = series("DTSTART:20260601T100000Z\r\nRRULE:FREQ=SOMETIMES\r\n");
    assert_eq!(starts(&occurrences(&broken, &[], 0, utc(2027, 1, 1, 0, 0))), [utc(2026, 6, 1, 10, 0)]);
    assert!(occurrences(&broken, &[], utc(2026, 7, 1, 0, 0), utc(2027, 1, 1, 0, 0)).is_empty(), "and only where it is");
}

#[test]
fn a_time_that_the_clocks_skip_or_repeat_is_read_sensibly() {
    // 01:30 on 29 March 2026 does not exist in London; it is read as an hour later, 02:30 BST = 01:30Z.
    let skipped = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=Europe/London:20260329T013000\r\nEND:VEVENT\r\n"), None)[0];
    assert_eq!(skipped.starts_at_utc_ms, utc(2026, 3, 29, 1, 30));
    // 01:30 on 25 October 2026 happens twice; the first is taken, 01:30 BST = 00:30Z.
    let twice = &events_from_ics_payload("c", &ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=Europe/London:20261025T013000\r\nEND:VEVENT\r\n"), None)[0];
    assert_eq!(twice.starts_at_utc_ms, utc(2026, 10, 25, 0, 30));
}

// ── asking the calendar for a range ──

fn keep(conn: &rusqlite::Connection, body: &str) {
    for e in events_from_ics_payload("c", &ics(body), None) {
        upsert_event(conn, &e).unwrap();
    }
}

fn seen(conn: &rusqlite::Connection, from: i64, to: i64, calendars: &[String]) -> Vec<(String, i64)> {
    tagliacarte_app::calendar_view::events_in_range(conn, from, to, calendars).unwrap().into_iter().map(|e| (e.title, e.starts_at_utc_ms)).collect()
}

const STANDUP: &str = "BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Standup\r\nDTSTART:20260601T100000Z\r\nDTEND:20260601T103000Z\r\nRRULE:FREQ=WEEKLY;COUNT=5\r\nEXDATE:20260608T100000Z\r\nEND:VEVENT\r\n";
const MOVED: &str = "BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Standup (moved)\r\nRECURRENCE-ID:20260615T100000Z\r\nDTSTART:20260618T150000Z\r\nDTEND:20260618T153000Z\r\nEND:VEVENT\r\n";

#[test]
fn the_calendar_shows_the_occurrences_with_the_exception_and_the_replacement_in_their_places() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(dir.path()).unwrap();
    keep(&conn, STANDUP);
    keep(&conn, MOVED);
    keep(&conn, "BEGIN:VEVENT\r\nUID:p\r\nSUMMARY:Plain\r\nDTSTART:20260616T090000Z\r\nDTEND:20260616T100000Z\r\nEND:VEVENT\r\n");
    let june = seen(&conn, utc(2026, 6, 1, 0, 0), utc(2026, 6, 30, 23, 59), &[]);
    assert_eq!(
        june,
        [
            ("Standup".to_owned(), utc(2026, 6, 1, 10, 0)),
            ("Plain".to_owned(), utc(2026, 6, 16, 9, 0)),
            ("Standup (moved)".to_owned(), utc(2026, 6, 18, 15, 0)),
            ("Standup".to_owned(), utc(2026, 6, 22, 10, 0)),
            ("Standup".to_owned(), utc(2026, 6, 29, 10, 0)),
        ],
        "the 8th is excepted, the 15th moved to the 18th, and the plain one is where it was"
    );
}

#[test]
fn a_moved_occurrence_is_where_it_was_moved_to_not_where_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(dir.path()).unwrap();
    keep(&conn, STANDUP);
    keep(&conn, MOVED);
    // The week of the 15th has nothing in it from the series: it was moved on to the 18th.
    assert!(seen(&conn, utc(2026, 6, 14, 0, 0), utc(2026, 6, 17, 23, 59), &[]).is_empty());
    assert_eq!(seen(&conn, utc(2026, 6, 18, 0, 0), utc(2026, 6, 19, 0, 0), &[]), [("Standup (moved)".to_owned(), utc(2026, 6, 18, 15, 0))]);
}

#[test]
fn a_series_that_began_years_ago_is_in_this_months_calendar() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(dir.path()).unwrap();
    keep(&conn, "BEGIN:VEVENT\r\nUID:d\r\nSUMMARY:Daily\r\nDTSTART:20180101T090000Z\r\nDTEND:20180101T091500Z\r\nRRULE:FREQ=DAILY\r\nEND:VEVENT\r\n");
    assert_eq!(seen(&conn, utc(2026, 6, 10, 0, 0), utc(2026, 6, 12, 23, 59), &[]).len(), 3);
}

#[test]
fn a_cancelled_series_is_not_shown_and_a_cancelled_replacement_does_not_bring_back_what_it_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(dir.path()).unwrap();
    keep(&conn, STANDUP);
    keep(&conn, "BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Standup\r\nSTATUS:CANCELLED\r\nRECURRENCE-ID:20260615T100000Z\r\nDTSTART:20260615T100000Z\r\nEND:VEVENT\r\n");
    let june: Vec<i64> = seen(&conn, utc(2026, 6, 1, 0, 0), utc(2026, 6, 30, 23, 59), &[]).into_iter().map(|x| x.1).collect();
    assert_eq!(june, [utc(2026, 6, 1, 10, 0), utc(2026, 6, 22, 10, 0), utc(2026, 6, 29, 10, 0)], "no 8th (excepted), no 15th (cancelled)");

    let other = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(other.path()).unwrap();
    keep(&conn, "BEGIN:VEVENT\r\nUID:x\r\nSUMMARY:Gone\r\nSTATUS:CANCELLED\r\nDTSTART:20260601T100000Z\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\n");
    assert!(seen(&conn, 0, utc(2027, 1, 1, 0, 0), &[]).is_empty());
}

#[test]
fn a_series_belongs_to_its_calendar() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(dir.path()).unwrap();
    keep(&conn, STANDUP);
    let from = utc(2026, 6, 1, 0, 0);
    let to = utc(2026, 6, 30, 23, 59);
    assert_eq!(seen(&conn, from, to, &["c".to_owned()]).len(), 4, "the 1st, 15th, 22nd and 29th; the 8th is excepted");
    assert!(seen(&conn, from, to, &["somewhere-else".to_owned()]).is_empty());
}
