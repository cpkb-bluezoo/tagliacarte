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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub recurrence_rule: Option<String>,
    pub status: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcalendarEventDraft {
    pub uid: String,
    pub sequence: i64,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub dtstart: String,
    pub dtend: String,
    pub organizer: Option<String>,
    pub attendees: Vec<String>,
    pub method: Option<String>,
    pub status: Option<String>,
    pub rrule: Option<String>,
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

fn property_name(head: &str) -> &str {
    if let Some((name, _)) = head.split_once(';') {
        name
    } else {
        head
    }
}

/// Parse one or more VEVENT blocks from an iCalendar payload.
pub fn parse_ical_events(payload: &str) -> Vec<IcalendarEventDraft> {
    let mut in_vevent = false;
    let mut current = IcalendarEventDraft {
        uid: String::new(),
        sequence: 0,
        summary: String::new(),
        description: None,
        location: None,
        dtstart: String::new(),
        dtend: String::new(),
        organizer: None,
        attendees: Vec::new(),
        method: None,
        status: None,
        rrule: None,
    };
    let mut events = Vec::new();
    let mut calendar_method: Option<String> = None;

    for line in unfold_ical_lines(payload) {
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            in_vevent = true;
            current = IcalendarEventDraft {
                uid: String::new(),
                sequence: 0,
                summary: String::new(),
                description: None,
                location: None,
                dtstart: String::new(),
                dtend: String::new(),
                organizer: None,
                attendees: Vec::new(),
                method: calendar_method.clone(),
                status: None,
                rrule: None,
            };
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
        let name = property_name(head).to_ascii_uppercase();
        if !in_vevent {
            if name == "METHOD" && !val.trim().is_empty() {
                calendar_method = Some(val.trim().to_string());
            }
            continue;
        }
        match name.as_str() {
            "UID" => current.uid = val.trim().to_string(),
            "SEQUENCE" => current.sequence = val.trim().parse::<i64>().unwrap_or(0),
            "SUMMARY" => current.summary = val.trim().to_string(),
            "DESCRIPTION" => current.description = Some(val.trim().to_string()),
            "LOCATION" => current.location = Some(val.trim().to_string()),
            "DTSTART" => current.dtstart = val.trim().to_string(),
            "DTEND" => current.dtend = val.trim().to_string(),
            "ORGANIZER" => current.organizer = Some(val.trim().to_string()),
            "ATTENDEE" => current.attendees.push(val.trim().to_string()),
            "STATUS" => current.status = Some(val.trim().to_string()),
            "RRULE" => current.rrule = Some(val.trim().to_string()),
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
    out.push_str(ev.summary.trim());
    out.push_str("\r\n");
    out.push_str("DTSTART:");
    out.push_str(ev.dtstart.trim());
    out.push_str("\r\n");
    out.push_str("DTEND:");
    out.push_str(ev.dtend.trim());
    out.push_str("\r\n");
    if let Some(ref v) = ev.description {
        out.push_str("DESCRIPTION:");
        out.push_str(v.trim());
        out.push_str("\r\n");
    }
    if let Some(ref v) = ev.location {
        out.push_str("LOCATION:");
        out.push_str(v.trim());
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
