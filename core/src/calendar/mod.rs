/*
 * calendar/mod.rs
 * Copyright (C) 2026 Chris Burdess
 */

//! Calendar domain models and a lightweight iCalendar parser/writer.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarSourceKind {
    Caldav,
    Google,
    Graph,
    LocalIcs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarSource {
    pub source_id: String,
    pub account_id: String,
    pub kind: CalendarSourceKind,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarDescriptor {
    pub calendar_id: String,
    pub source_id: String,
    pub name: String,
    pub color_hex: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CalendarEvent {
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
    /// The `RRULE` of a series (the value, as `FREQ=WEEKLY;BYDAY=MO`), on its first event.
    pub recurrence_rule: Option<String>,
    pub status: Option<String>,
    pub etag: Option<String>,
    /// The zone the times are kept in (an IANA name), which a series needs to repeat at the same
    /// wall-clock time across a change of clocks. None for UTC, for a date and for a zone that
    /// is not known.
    pub tzid: Option<String>,
    /// The occurrences of a series that do not happen: the start of each, in UTC.
    pub exdates_utc_ms: Vec<i64>,
    /// On an event that replaces one occurrence of a series: the start that occurrence had.
    pub recurrence_id_utc_ms: Option<i64>,
}

/// A date or time as the file wrote it, with the zone it named (none for UTC, a date, or a
/// time that names no zone).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IcalValue {
    pub value: String,
    pub tzid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IcalendarEventDraft {
    pub uid: String,
    pub sequence: i64,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub dtstart: String,
    /// The zone `dtstart` is in, when it names one.
    pub dtstart_tzid: Option<String>,
    pub dtend: String,
    pub dtend_tzid: Option<String>,
    /// `DURATION`, when there is no `DTEND`: `PT1H30M`, `P1D`.
    pub duration: Option<String>,
    pub organizer: Option<String>,
    pub attendees: Vec<String>,
    pub method: Option<String>,
    pub status: Option<String>,
    pub rrule: Option<String>,
    /// The occurrences of the series that do not happen.
    pub exdates: Vec<IcalValue>,
    /// Set on an event that replaces one occurrence of a series: which occurrence.
    pub recurrence_id: Option<IcalValue>,
}

fn unfold_ical_lines(input: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in input.replace("\r\n", "\n").split('\n') {
        if raw.is_empty() {
            continue;
        }
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !out.is_empty() {
            let tail = raw.trim_start_matches([' ', '\t']);
            if let Some(last) = out.last_mut() {
                last.push_str(tail);
            }
        } else {
            out.push(raw.to_string());
        }
    }
    out
}

fn split_ical_property(line: &str) -> Option<(&str, &str)> {
    let pos = line.find(':')?;
    Some((&line[..pos], &line[pos + 1..]))
}

/// The name of a content line's head and its parameters (`TZID=Europe/London`), names in
/// upper case and quotes taken off values.
fn name_and_params(head: &str) -> (String, Vec<(String, String)>) {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in head.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    let name = parts.remove(0).to_ascii_uppercase();
    let params = parts
        .into_iter()
        .filter_map(|p| p.split_once('=').map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim().trim_matches('"').to_owned())))
        .collect();
    (name, params)
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
}

/// A TEXT value as the file escapes it (`\n`, `\,`, `\;`, `\\`), as plain text.
fn unescape_text(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn escape_text(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ',' => out.push_str("\\,"),
            ';' => out.push_str("\\;"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Parse one or more VEVENT blocks from an iCalendar payload. What a sub-component of an
/// event (an alarm) says is not what the event says, and is left out. Several events can
/// share a UID: a series, and the events that replace some of its occurrences.
pub fn parse_ical_events(payload: &str) -> Vec<IcalendarEventDraft> {
    let mut in_vevent = false;
    // How many components deep inside the event (an alarm is one).
    let mut inner = 0usize;
    let mut current = IcalendarEventDraft::default();
    let mut events = Vec::new();
    let mut calendar_method: Option<String> = None;

    for line in unfold_ical_lines(payload) {
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            in_vevent = true;
            inner = 0;
            current = IcalendarEventDraft { method: calendar_method.clone(), ..IcalendarEventDraft::default() };
            continue;
        }
        if line.eq_ignore_ascii_case("END:VEVENT") {
            if !current.uid.is_empty() && !current.dtstart.is_empty() {
                events.push(current.clone());
            }
            in_vevent = false;
            continue;
        }
        let Some((head, val)) = split_ical_property(&line) else {
            continue;
        };
        let (name, params) = name_and_params(head);
        if !in_vevent {
            if name == "METHOD" && !val.trim().is_empty() {
                calendar_method = Some(val.trim().to_string());
            }
            continue;
        }
        if name == "BEGIN" {
            inner += 1;
            continue;
        }
        if name == "END" {
            inner = inner.saturating_sub(1);
            continue;
        }
        if inner > 0 {
            continue;
        }
        let value = val.trim();
        match name.as_str() {
            "UID" => current.uid = value.to_string(),
            "SEQUENCE" => current.sequence = value.parse::<i64>().unwrap_or(0),
            "SUMMARY" => current.summary = unescape_text(value),
            "DESCRIPTION" => current.description = Some(unescape_text(value)),
            "LOCATION" => current.location = Some(unescape_text(value)),
            "DTSTART" => {
                current.dtstart = value.to_string();
                current.dtstart_tzid = param(&params, "TZID").map(str::to_owned);
            }
            "DTEND" => {
                current.dtend = value.to_string();
                current.dtend_tzid = param(&params, "TZID").map(str::to_owned);
            }
            "DURATION" => current.duration = Some(value.to_string()),
            "ORGANIZER" => current.organizer = Some(value.to_string()),
            "ATTENDEE" => current.attendees.push(value.to_string()),
            "STATUS" => current.status = Some(value.to_string()),
            "RRULE" => current.rrule = Some(value.to_string()),
            "EXDATE" => {
                let tzid = param(&params, "TZID").map(str::to_owned);
                current.exdates.extend(value.split(',').map(str::trim).filter(|v| !v.is_empty()).map(|v| IcalValue { value: v.to_owned(), tzid: tzid.clone() }));
            }
            "RECURRENCE-ID" => current.recurrence_id = Some(IcalValue { value: value.to_string(), tzid: param(&params, "TZID").map(str::to_owned) }),
            _ => {}
        }
    }
    events
}

/// Serialize a simple VCALENDAR with one VEVENT.
pub fn write_ical_event(ev: &IcalendarEventDraft) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Tagliacarte//Calendar//EN\r\n");
    if let Some(ref method) = ev.method {
        if !method.trim().is_empty() {
            out.push_str("METHOD:");
            out.push_str(method.trim());
            out.push_str("\r\n");
        }
    }
    out.push_str("BEGIN:VEVENT\r\n");
    out.push_str("UID:");
    out.push_str(ev.uid.trim());
    out.push_str("\r\n");
    out.push_str("SEQUENCE:");
    out.push_str(ev.sequence.to_string().as_str());
    out.push_str("\r\n");
    out.push_str("SUMMARY:");
    out.push_str(&escape_text(ev.summary.trim()));
    out.push_str("\r\n");
    out.push_str("DTSTART:");
    out.push_str(ev.dtstart.trim());
    out.push_str("\r\n");
    out.push_str("DTEND:");
    out.push_str(ev.dtend.trim());
    out.push_str("\r\n");
    if let Some(ref v) = ev.description {
        out.push_str("DESCRIPTION:");
        out.push_str(&escape_text(v.trim()));
        out.push_str("\r\n");
    }
    if let Some(ref v) = ev.location {
        out.push_str("LOCATION:");
        out.push_str(&escape_text(v.trim()));
        out.push_str("\r\n");
    }
    if let Some(ref v) = ev.organizer {
        out.push_str("ORGANIZER:");
        out.push_str(v.trim());
        out.push_str("\r\n");
    }
    for a in &ev.attendees {
        out.push_str("ATTENDEE:");
        out.push_str(a.trim());
        out.push_str("\r\n");
    }
    if let Some(ref v) = ev.status {
        out.push_str("STATUS:");
        out.push_str(v.trim());
        out.push_str("\r\n");
    }
    if let Some(ref v) = ev.rrule {
        out.push_str("RRULE:");
        out.push_str(v.trim());
        out.push_str("\r\n");
    }
    out.push_str("END:VEVENT\r\nEND:VCALENDAR\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_event_with_method() {
        let raw = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:abc-1\r\nSEQUENCE:2\r\nSUMMARY:Standup\r\nDTSTART:20260415T090000Z\r\nDTEND:20260415T093000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let out = parse_ical_events(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].uid, "abc-1");
        assert_eq!(out[0].sequence, 2);
        assert_eq!(out[0].summary, "Standup");
        assert_eq!(out[0].method.as_deref(), Some("REQUEST"));
    }

    fn vevent(lines: &[&str]) -> String {
        let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:u1\r\nSUMMARY:S\r\n");
        for l in lines {
            out.push_str(l);
            out.push_str("\r\n");
        }
        out.push_str("END:VEVENT\r\nEND:VCALENDAR\r\n");
        out
    }

    #[test]
    fn the_zone_on_a_time_is_kept_and_a_date_is_not_a_time() {
        let e = &parse_ical_events(&vevent(&["DTSTART;TZID=Europe/London:20260601T100000", "DTEND;TZID=\"Europe/London\":20260601T110000"]))[0];
        assert_eq!((e.dtstart.as_str(), e.dtstart_tzid.as_deref()), ("20260601T100000", Some("Europe/London")));
        assert_eq!((e.dtend.as_str(), e.dtend_tzid.as_deref()), ("20260601T110000", Some("Europe/London")), "quotes around a zone are not part of it");
        let utc = &parse_ical_events(&vevent(&["DTSTART:20260601T100000Z"]))[0];
        assert_eq!((utc.dtstart.as_str(), utc.dtstart_tzid.clone()), ("20260601T100000Z", None));
        let day = &parse_ical_events(&vevent(&["DTSTART;VALUE=DATE:20260601", "DTEND;VALUE=DATE:20260602"]))[0];
        assert_eq!((day.dtstart.as_str(), day.dtstart_tzid.clone(), day.dtend.as_str()), ("20260601", None, "20260602"));
    }

    #[test]
    fn what_an_alarm_says_is_not_what_the_event_says() {
        let raw = vevent(&["DTSTART:20260601T100000Z", "DESCRIPTION:The agenda", "BEGIN:VALARM", "ACTION:DISPLAY", "DESCRIPTION:Reminder", "TRIGGER:-PT15M", "END:VALARM", "LOCATION:Room 1"]);
        let e = &parse_ical_events(&raw)[0];
        assert_eq!((e.description.as_deref(), e.location.as_deref()), (Some("The agenda"), Some("Room 1")));
        let bare = vevent(&["DTSTART:20260601T100000Z", "BEGIN:VALARM", "DESCRIPTION:Reminder", "END:VALARM"]);
        assert_eq!(parse_ical_events(&bare)[0].description, None, "an alarm's text is not the event's");
    }

    #[test]
    fn a_duration_exceptions_and_a_replacement_are_read() {
        let raw = vevent(&[
            "DTSTART;TZID=Europe/London:20260601T100000",
            "DURATION:PT1H30M",
            "RRULE:FREQ=WEEKLY;BYDAY=MO",
            "EXDATE;TZID=Europe/London:20260608T100000,20260615T100000",
            "EXDATE;TZID=Europe/London:20260629T100000",
        ]);
        let e = &parse_ical_events(&raw)[0];
        assert_eq!(e.duration.as_deref(), Some("PT1H30M"));
        assert_eq!(e.rrule.as_deref(), Some("FREQ=WEEKLY;BYDAY=MO"));
        let ex: Vec<(&str, Option<&str>)> = e.exdates.iter().map(|x| (x.value.as_str(), x.tzid.as_deref())).collect();
        assert_eq!(ex, [("20260608T100000", Some("Europe/London")), ("20260615T100000", Some("Europe/London")), ("20260629T100000", Some("Europe/London"))]);
        assert!(e.recurrence_id.is_none());

        let replaced = &parse_ical_events(&vevent(&["DTSTART;TZID=Europe/London:20260622T140000", "RECURRENCE-ID;TZID=Europe/London:20260622T100000"]))[0];
        let r = replaced.recurrence_id.as_ref().unwrap();
        assert_eq!((r.value.as_str(), r.tzid.as_deref()), ("20260622T100000", Some("Europe/London")));
    }

    #[test]
    fn a_series_and_its_replacements_are_all_there() {
        let raw = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:u\r\nDTSTART:20260601T100000Z\r\nRRULE:FREQ=DAILY\r\nEND:VEVENT\r\n\
                   BEGIN:VEVENT\r\nUID:u\r\nRECURRENCE-ID:20260603T100000Z\r\nDTSTART:20260603T130000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let all = parse_ical_events(raw);
        assert_eq!(all.len(), 2);
        assert!(all[0].recurrence_id.is_none() && all[1].recurrence_id.is_some());
    }

    #[test]
    fn text_is_unescaped_when_read_and_escaped_when_written() {
        let e = &parse_ical_events(&vevent(&["DTSTART:20260601T100000Z", r"SUMMARY:Lunch\, with Bob\; and Al", r"DESCRIPTION:One\nTwo\\Three"]))[0];
        assert_eq!(e.summary, "Lunch, with Bob; and Al");
        assert_eq!(e.description.as_deref(), Some("One\nTwo\\Three"));
        let back = write_ical_event(e);
        assert!(back.contains(r"SUMMARY:Lunch\, with Bob\; and Al") && back.contains(r"DESCRIPTION:One\nTwo\\Three"), "{back}");
        let again = &parse_ical_events(&back)[0];
        assert_eq!((again.summary.as_str(), again.description.as_deref()), (e.summary.as_str(), e.description.as_deref()));
    }

    #[test]
    fn write_and_parse_roundtrip_key_fields() {
        let ev = IcalendarEventDraft {
            uid: "uid-x".to_string(),
            sequence: 4,
            summary: "Project sync".to_string(),
            description: Some("Weekly".to_string()),
            location: Some("Room 2".to_string()),
            dtstart: "20260416T120000Z".to_string(),
            dtend: "20260416T130000Z".to_string(),
            organizer: None,
            attendees: vec!["mailto:a@example.com".to_string()],
            method: Some("REQUEST".to_string()),
            status: Some("CONFIRMED".to_string()),
            rrule: None,
            ..IcalendarEventDraft::default()
        };
        let text = write_ical_event(&ev);
        let parsed = parse_ical_events(text.as_str());
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].uid, ev.uid);
        assert_eq!(parsed[0].summary, ev.summary);
        assert_eq!(parsed[0].dtstart, ev.dtstart);
        assert_eq!(parsed[0].dtend, ev.dtend);
    }
}
