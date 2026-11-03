//! The calendar: which calendars there are, the events in a range, and making,
//! changing and cancelling one. As elsewhere, anything that is a list arrives as a
//! sequence of small events (a range of events is `calendarEventsStarted`, an event
//! each, `calendarEventsDone`, all carrying the request id, so an answer to a range
//! the page has moved on from can be told apart and dropped), and a change is a
//! small command after which the page asks again.

use std::io::Write;
use std::sync::{Arc, Mutex};

use tagliacarte_app::calendar_view::{self, Db, EventInput, INBOX_CALENDAR};
use tagliacarte_app::mail_view::{self, MessageRef};
use tagliacarte_core::calendar::{CalendarDescriptor, CalendarEvent};
use tagliacarte_core::json::{JsonWriter, writer_into_string};

use crate::args::Args;
use crate::commands::Ctx;
use crate::events::{flag, int, opt_str, str, string_array};

/// The most of an invitation read from a message.
const MAX_INVITE_BYTES: usize = 2 * 1024 * 1024;

fn calendar_json(c: &CalendarDescriptor) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "calendarFound");
    str(&mut w, "id", &c.calendar_id);
    str(&mut w, "sourceId", &c.source_id);
    str(&mut w, "name", &c.name);
    opt_str(&mut w, "colorHex", &c.color_hex);
    flag(&mut w, "enabled", c.enabled);
    w.write_end_object();
    writer_into_string(w)
}

fn event_json(request_id: &str, e: &CalendarEvent) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "calendarEvent");
    str(&mut w, "requestId", request_id);
    str(&mut w, "id", &e.event_id);
    str(&mut w, "calendarId", &e.calendar_id);
    str(&mut w, "uid", &e.uid);
    int(&mut w, "sequence", e.sequence);
    str(&mut w, "title", &e.title);
    opt_str(&mut w, "description", &e.description);
    opt_str(&mut w, "location", &e.location);
    int(&mut w, "startMs", e.starts_at_utc_ms);
    int(&mut w, "endMs", e.ends_at_utc_ms);
    flag(&mut w, "allDay", e.all_day);
    opt_str(&mut w, "organizer", &e.organizer);
    string_array(&mut w, "attendees", &e.attendees);
    opt_str(&mut w, "status", &e.status);
    flag(&mut w, "repeats", e.recurrence_rule.is_some());
    w.write_end_object();
    writer_into_string(w)
}

fn marker_json(kind: &str, request_id: Option<&str>, count: Option<usize>, error: Option<&str>) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", kind);
    if let Some(r) = request_id {
        str(&mut w, "requestId", r);
    }
    if let Some(n) = count {
        int(&mut w, "count", i64::try_from(n).unwrap_or(i64::MAX));
    }
    if let Some(e) = error {
        str(&mut w, "error", e);
    }
    w.write_end_object();
    writer_into_string(w)
}

/// The calendars, as `calendarsStarted`, one `calendarFound` each, `calendarsLoaded`.
fn emit_calendars(conn: &Db, emit: &mut dyn FnMut(String)) -> Result<(), String> {
    let all = calendar_view::list_calendars(conn)?;
    emit(marker_json("calendarsStarted", None, None, None));
    for c in &all {
        emit(calendar_json(c));
    }
    emit(marker_json("calendarsLoaded", None, Some(all.len()), None));
    Ok(())
}

/// The events of a range, as `calendarEventsStarted`, one `calendarEvent` each,
/// `calendarEventsDone`. A calendar id list that is empty means every calendar.
fn emit_events(conn: &Db, a: &Args, emit: &mut dyn FnMut(String)) -> Result<(), String> {
    let request_id = a.str("requestId")?;
    let found = calendar_view::events_in_range(conn, a.i64("startMs")?, a.i64("endMs")?, &a.strings("calendarIds").unwrap_or_default());
    emit(marker_json("calendarEventsStarted", Some(&request_id), None, None));
    match found {
        Ok(events) => {
            for e in &events {
                emit(event_json(&request_id, e));
            }
            emit(marker_json("calendarEventsDone", Some(&request_id), Some(events.len()), None));
        }
        Err(e) => emit(marker_json("calendarEventsDone", Some(&request_id), Some(0), Some(&e))),
    }
    Ok(())
}

/// What the editor sent, as an event to make or change.
fn input_from_args(a: &Args) -> Result<EventInput, String> {
    Ok(EventInput {
        event_id: a.opt_str("eventId").map(str::to_owned).filter(|s| !s.is_empty()),
        calendar_id: a.str("calendarId")?,
        title: a.str("title")?,
        description: a.opt_str("description").map(str::to_owned),
        location: a.opt_str("location").map(str::to_owned),
        starts_at_utc_ms: a.i64("startMs")?,
        ends_at_utc_ms: a.i64("endMs")?,
        all_day: a.bool_or("allDay", false),
        attendees: a.strings("attendees").unwrap_or_default(),
    })
}

fn now_micros() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX))
}

fn save(conn: &Db, a: &Args) -> Result<String, String> {
    let saved = calendar_view::save_event(conn, input_from_args(a)?, now_micros())?;
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "eventId", &saved.event_id);
    w.write_end_object();
    Ok(writer_into_string(w))
}

/// How far back and forward a sync of the services' calendars looks: two weeks
/// before now, and four months after.
fn sync_window(now_ms: i64) -> (i64, i64) {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    (now_ms - 14 * DAY, now_ms + 120 * DAY)
}

fn now_ms() -> i64 {
    now_micros() / 1000
}

/// A buffer an attachment is written into, that refuses more than it should hold.
#[derive(Clone, Default)]
struct Bounded(Arc<Mutex<Vec<u8>>>);

impl Write for Bounded {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut g = self.0.lock().unwrap();
        if g.len() + buf.len() > MAX_INVITE_BYTES {
            return Err(std::io::Error::other("the invitation is larger than 2 MB"));
        }
        g.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `calendar.importInvite`: reads the attachment, adds its events to the calendar
/// for invitations, and reports `calendarImported` with the count or the error.
fn import_invite(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    let request_id = a.str("requestId")?;
    let msg = MessageRef { account_id: a.str("accountId")?, folder: a.str("folder")?, message_id: a.str("messageId")? };
    let index = u32::try_from(a.u64("index")?).map_err(|_| "no such attachment".to_owned())?;
    let hub = Arc::clone(&ctx.hub);
    let buffer = Bounded::default();
    let held = buffer.clone();
    let finish = move |r: Result<usize, String>| {
        hub.push(match r {
            Ok(n) => marker_json("calendarImported", Some(&request_id), Some(n), None),
            Err(e) => marker_json("calendarImported", Some(&request_id), Some(0), Some(&e)),
        });
    };
    mail_view::save_attachment(
        msg,
        index,
        Box::new(buffer),
        Box::new(move |r| {
            let imported = r.and_then(|_| {
                let text = String::from_utf8_lossy(&held.0.lock().unwrap()).into_owned();
                let conn = calendar_view::open_db()?;
                match calendar_view::import_ics(&conn, INBOX_CALENDAR, "Invitations", &text)? {
                    0 => Err("there is no event in it".to_owned()),
                    n => Ok(n),
                }
            });
            finish(imported);
        }),
    );
    Ok(None)
}

/// `calendar.sync`: the calendars of the accounts that have a service for them,
/// then `calendarSynced` with what came of it. Waits on the network, so off the
/// thread that asked.
fn sync(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let request_id = Args::parse(body)?.str("requestId")?;
    let (hub, path) = (Arc::clone(&ctx.hub), ctx.config_path.clone());
    std::thread::spawn(move || {
        let (start, end) = sync_window(now_ms());
        let mut w = JsonWriter::new();
        w.write_start_object();
        str(&mut w, "type", "calendarSynced");
        str(&mut w, "requestId", &request_id);
        match calendar_view::sync_providers(&path, start, end) {
            Ok(r) => {
                int(&mut w, "calendars", i64::try_from(r.calendars).unwrap_or(i64::MAX));
                int(&mut w, "events", i64::try_from(r.events).unwrap_or(i64::MAX));
                int(&mut w, "failed", i64::try_from(r.failed.len()).unwrap_or(i64::MAX));
                if let Some((id, why)) = r.failed.first() {
                    str(&mut w, "firstFailure", &format!("{id}: {why}"));
                }
            }
            Err(e) => str(&mut w, "error", &e),
        }
        w.write_end_object();
        hub.push(writer_into_string(w));
    });
    Ok(None)
}

pub fn run(ctx: &Ctx, name: &str, body: &str) -> Option<Result<Option<String>, String>> {
    if !name.starts_with("calendar.") {
        return None;
    }
    Some(dispatch(ctx, name, body))
}

fn dispatch(ctx: &Ctx, name: &str, body: &str) -> Result<Option<String>, String> {
    match name {
        "calendar.load" => {
            let conn = calendar_view::open_db()?;
            ctx.hub.sequence(|| emit_calendars(&conn, &mut |e| ctx.hub.push(e)))?;
            Ok(None)
        }
        "calendar.events" => {
            let conn = calendar_view::open_db()?;
            emit_events(&conn, &Args::parse(body)?, &mut |e| ctx.hub.push(e))?;
            Ok(None)
        }
        "calendar.save" => Ok(Some(save(&calendar_view::open_db()?, &Args::parse(body)?)?)),
        "calendar.delete" => {
            calendar_view::cancel_event(&calendar_view::open_db()?, &Args::parse(body)?.str("eventId")?)?;
            Ok(None)
        }
        "calendar.sync" => sync(ctx, body),
        "calendar.importInvite" => import_invite(ctx, body),
        other => Err(format!("unknown command '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use tagliacarte_app::calendar_view::open_in;

    fn db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_in(dir.path()).unwrap();
        (dir, conn)
    }

    fn args(json: &str) -> Args {
        Args::parse(json).unwrap()
    }

    fn collect(f: impl FnOnce(&mut dyn FnMut(String))) -> Vec<Value> {
        let mut out = Vec::new();
        f(&mut |e| out.push(Value::parse(&e).unwrap()));
        out
    }

    #[test]
    fn the_calendars_are_a_sequence_with_the_local_one_in_it() {
        let (_d, conn) = db();
        let evs = collect(|emit| emit_calendars(&conn, emit).unwrap());
        let types: Vec<_> = evs.iter().map(|e| e.opt_str("type").unwrap().to_owned()).collect();
        assert_eq!(types, ["calendarsStarted", "calendarFound", "calendarsLoaded"]);
        assert_eq!(evs[1].opt_str("id"), Some("local"));
        assert_eq!(evs[1].opt_str("name"), Some("Local"));
        assert!(evs[1].bool_or("enabled", false));
        assert_eq!(evs[2].u64("count").unwrap(), 1);
    }

    #[test]
    fn saving_then_asking_for_a_range_streams_the_event_with_the_request_id() {
        let (_d, conn) = db();
        let saved = save(
            &conn,
            &args(r#"{"calendarId":"local","title":"Say \"hi\"\nthen go","startMs":86400000,"endMs":90000000,"allDay":false,
                      "location":"Room 1","attendees":["a@example.org","b@example.org"]}"#),
        )
        .unwrap();
        let id = Value::parse(&saved).unwrap().opt_str("eventId").unwrap().to_owned();
        assert!(id.starts_with("local:tc-"));

        let evs = collect(|emit| emit_events(&conn, &args(r#"{"requestId":"q7","startMs":0,"endMs":200000000,"calendarIds":[]}"#), emit).unwrap());
        let types: Vec<_> = evs.iter().map(|e| e.opt_str("type").unwrap().to_owned()).collect();
        assert_eq!(types, ["calendarEventsStarted", "calendarEvent", "calendarEventsDone"]);
        assert!(evs.iter().all(|e| e.opt_str("requestId") == Some("q7")));
        let e = &evs[1];
        assert_eq!(e.opt_str("id"), Some(id.as_str()));
        assert_eq!(e.opt_str("title"), Some("Say \"hi\"\nthen go"));
        assert_eq!((e.i64("startMs").unwrap(), e.i64("endMs").unwrap()), (86_400_000, 90_000_000));
        assert_eq!(e.opt_str("location"), Some("Room 1"));
        assert!(!e.bool_or("allDay", true));
        assert_eq!(evs[2].u64("count").unwrap(), 1);
        assert!(evs[2].opt_str("error").is_none());

        // Nothing in another range, but the answer still ends, so the page stops waiting.
        let none = collect(|emit| emit_events(&conn, &args(r#"{"requestId":"q8","startMs":900000000,"endMs":990000000}"#), emit).unwrap());
        assert_eq!(none.len(), 2);
        assert_eq!(none[1].u64("count").unwrap(), 0);

        // A range the wrong way round is an answer with an error in it, not a silence.
        let bad = collect(|emit| emit_events(&conn, &args(r#"{"requestId":"q9","startMs":10,"endMs":5}"#), emit).unwrap());
        assert_eq!(bad.last().unwrap().opt_str("type"), Some("calendarEventsDone"));
        assert!(bad.last().unwrap().opt_str("error").is_some());
    }

    #[test]
    fn an_event_the_editor_sends_is_read_whole_and_wrongly_made_ones_are_refused() {
        let i = input_from_args(&args(r#"{"eventId":"local:x","calendarId":"work","title":"T","startMs":-5000,"endMs":0,"allDay":true,"description":"d"}"#)).unwrap();
        assert_eq!((i.event_id.as_deref(), i.calendar_id.as_str(), i.starts_at_utc_ms, i.ends_at_utc_ms, i.all_day), (Some("local:x"), "work", -5000, 0, true));
        assert_eq!(i.description.as_deref(), Some("d"));
        assert!(i.attendees.is_empty() && i.location.is_none());
        // An empty event id is a new event.
        assert!(input_from_args(&args(r#"{"eventId":"","calendarId":"c","title":"T","startMs":1,"endMs":2}"#)).unwrap().event_id.is_none());
        assert!(input_from_args(&args(r#"{"calendarId":"c","startMs":1,"endMs":2}"#)).unwrap_err().contains("'title'"));
        assert!(input_from_args(&args(r#"{"calendarId":"c","title":"T","endMs":2}"#)).unwrap_err().contains("'startMs'"));
        let (_d, conn) = db();
        assert!(save(&conn, &args(r#"{"calendarId":"local","title":" ","startMs":1,"endMs":2}"#)).unwrap_err().contains("title"));
    }

    #[test]
    fn a_sync_looks_two_weeks_back_and_four_months_ahead() {
        let day = 86_400_000;
        assert_eq!(sync_window(1_000 * day), (986 * day, 1_120 * day));
    }

    #[test]
    fn an_invitation_is_read_into_a_buffer_that_will_not_grow_without_end() {
        let mut b = Bounded::default();
        b.write_all(b"BEGIN:VCALENDAR").unwrap();
        assert_eq!(b.0.lock().unwrap().len(), 15);
        assert!(b.write_all(&vec![0u8; MAX_INVITE_BYTES]).is_err());
    }
}
