/*
 * calendar_provider_sync.rs
 */

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use std::collections::HashMap;
use std::io::Read;
use tagliacarte_core::json::{parse_bytes_complete, JsonContentHandler, JsonNumber};
use tagliacarte_core::calendar::{CalendarDescriptor, CalendarEvent};

use crate::mail_store::{oauth_provider_for_store, resolve_xoauth_secret, OAuthService};

#[derive(Debug, Clone)]
pub struct ProviderSyncOutcome {
    pub fetched_calendars: u64,
    pub fetched_events: u64,
}

fn bearer_for_account(acc: &crate::frb_api::FrbAccount, use_keychain: bool) -> Result<String, String> {
    let svc = oauth_provider_for_store(acc).ok_or_else(|| "account has no OAuth provider".to_string())?;
    let (_, tok) = resolve_xoauth_secret(svc, acc.id.trim(), use_keychain)?;
    Ok(tok)
}

fn http_get_json(url: &str, bearer: &str) -> Result<Vec<u8>, String> {
    ureq::get(url)
        .set("Authorization", format!("Bearer {bearer}").as_str())
        .set("Accept", "application/json")
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?
        .into_reader()
        .bytes()
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("read json: {e}"))
}

#[derive(Default)]
struct ObjectArrayHandler {
    target_array_key: String,
    depth: usize,
    object_depth: Option<usize>,
    current: HashMap<String, String>,
    current_key: Option<String>,
    out: Vec<HashMap<String, String>>,
}

impl ObjectArrayHandler {
    fn new(target_array_key: &str) -> Self {
        Self {
            target_array_key: target_array_key.to_string(),
            ..Self::default()
        }
    }
}

impl JsonContentHandler for ObjectArrayHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.object_depth == Some(self.depth.saturating_sub(1)) {
            self.current.clear();
            self.object_depth = Some(self.depth);
        }
    }

    fn end_object(&mut self) {
        if self.object_depth == Some(self.depth) && !self.current.is_empty() {
            self.out.push(self.current.clone());
            self.current.clear();
            self.object_depth = Some(self.depth.saturating_sub(1));
        }
        self.depth = self.depth.saturating_sub(1);
        self.current_key = None;
    }

    fn start_array(&mut self) {
        self.depth += 1;
        if self.current_key.as_deref() == Some(self.target_array_key.as_str()) {
            self.object_depth = Some(self.depth);
        }
    }

    fn end_array(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.object_depth == Some(self.depth + 1) {
            self.object_depth = None;
        }
        self.current_key = None;
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if let (Some(_), Some(k)) = (self.object_depth, self.current_key.clone()) {
            self.current.insert(k, value.to_string());
        }
    }

    fn number_value(&mut self, n: JsonNumber) {
        if let (Some(_), Some(k)) = (self.object_depth, self.current_key.clone()) {
            let v = match n {
                JsonNumber::I64(x) => x.to_string(),
                JsonNumber::F64(x) => x.to_string(),
            };
            self.current.insert(k, v);
        }
    }

    fn boolean_value(&mut self, value: bool) {
        if let (Some(_), Some(k)) = (self.object_depth, self.current_key.clone()) {
            self.current.insert(k, if value { "true" } else { "false" }.to_string());
        }
    }

    fn null_value(&mut self) {
        self.current_key = None;
    }
}

fn parse_google_calendar_list(payload: &[u8]) -> Result<Vec<CalendarDescriptor>, String> {
    let mut h = ObjectArrayHandler::new("items");
    parse_bytes_complete(payload, &mut h).map_err(|e| format!("google calendar list parse: {e}"))?;
    let mut out = Vec::new();
    for item in h.out {
        let Some(id) = item.get("id").cloned() else { continue };
        let name = item.get("summary").cloned().unwrap_or_else(|| id.clone());
        out.push(CalendarDescriptor {
            calendar_id: format!("google:{id}"),
            source_id: "google".to_string(),
            name,
            color_hex: item.get("backgroundColor").cloned(),
            enabled: true,
        });
    }
    Ok(out)
}

fn parse_graph_calendar_list(payload: &[u8]) -> Result<Vec<CalendarDescriptor>, String> {
    let mut h = ObjectArrayHandler::new("value");
    parse_bytes_complete(payload, &mut h).map_err(|e| format!("graph calendar list parse: {e}"))?;
    let mut out = Vec::new();
    for item in h.out {
        let Some(id) = item.get("id").cloned() else { continue };
        let name = item.get("name").cloned().unwrap_or_else(|| id.clone());
        out.push(CalendarDescriptor {
            calendar_id: format!("graph:{id}"),
            source_id: "graph".to_string(),
            name,
            color_hex: None,
            enabled: true,
        });
    }
    Ok(out)
}

fn parse_datetime_to_utc_ms(s: &str) -> Option<i64> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
        return Some(dt.with_timezone(&Utc).timestamp_millis());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
        return Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp_millis());
    }
    None
}

fn parse_event_objects(payload: &[u8], array_key: &str, id_key: &str, title_key: &str) -> Result<Vec<HashMap<String, String>>, String> {
    let mut h = ObjectArrayHandler::new(array_key);
    parse_bytes_complete(payload, &mut h).map_err(|e| format!("events parse: {e}"))?;
    Ok(h.out
        .into_iter()
        .filter(|m| m.get(id_key).is_some() && m.get(title_key).is_some())
        .collect())
}

pub fn sync_provider_account(
    conn: &rusqlite::Connection,
    acc: &crate::frb_api::FrbAccount,
    use_keychain: bool,
    start_utc_ms: i64,
    end_utc_ms: i64,
) -> Result<ProviderSyncOutcome, String> {
    let provider = oauth_provider_for_store(acc).ok_or_else(|| "account has no provider".to_string())?;
    let bearer = bearer_for_account(acc, use_keychain)?;
    let mut fetched_calendars = 0u64;
    let mut fetched_events = 0u64;
    match provider {
        OAuthService::Google => {
            let cals_payload = http_get_json("https://www.googleapis.com/calendar/v3/users/me/calendarList", bearer.as_str())?;
            let calendars = parse_google_calendar_list(cals_payload.as_slice())?;
            for c in calendars {
                calendar_store::upsert_calendar(conn, &c)?;
                fetched_calendars += 1;
                let raw_id = c.calendar_id.strip_prefix("google:").unwrap_or(c.calendar_id.as_str());
                let url = format!(
                    "https://www.googleapis.com/calendar/v3/calendars/{}/events?singleEvents=true&timeMin={}&timeMax={}",
                    urlencoding::encode(raw_id),
                    chrono::DateTime::<Utc>::from_timestamp_millis(start_utc_ms).unwrap_or_else(Utc::now).to_rfc3339(),
                    chrono::DateTime::<Utc>::from_timestamp_millis(end_utc_ms).unwrap_or_else(Utc::now).to_rfc3339(),
                );
                let events_payload = http_get_json(url.as_str(), bearer.as_str())?;
                let rows = parse_event_objects(events_payload.as_slice(), "items", "id", "summary")?;
                for r in rows {
                    let starts = r.get("dateTime")
                        .and_then(|v| parse_datetime_to_utc_ms(v))
                        .unwrap_or(start_utc_ms);
                    let ends = starts + 3600_000;
                    let uid = r.get("iCalUID").cloned().unwrap_or_else(|| r.get("id").cloned().unwrap_or_default());
                    let event = CalendarEvent {
                        event_id: format!("{}:{}", c.calendar_id, r.get("id").cloned().unwrap_or_default()),
                        calendar_id: c.calendar_id.clone(),
                        uid,
                        sequence: r.get("sequence").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
                        title: r.get("summary").cloned().unwrap_or_default(),
                        description: r.get("description").cloned(),
                        location: r.get("location").cloned(),
                        starts_at_utc_ms: starts,
                        ends_at_utc_ms: ends,
                        all_day: false,
                        organizer: r.get("organizer").cloned(),
                        attendees: Vec::new(),
                        recurrence_rule: None,
                        status: r.get("status").cloned(),
                        etag: r.get("etag").cloned(),
                    };
                    calendar_store::upsert_event(conn, &event)?;
                    fetched_events += 1;
                }
            }
        }
        OAuthService::Microsoft => {
            let cals_payload = http_get_json("https://graph.microsoft.com/v1.0/me/calendars", bearer.as_str())?;
            let calendars = parse_graph_calendar_list(cals_payload.as_slice())?;
            for c in calendars {
                calendar_store::upsert_calendar(conn, &c)?;
                fetched_calendars += 1;
                let raw_id = c.calendar_id.strip_prefix("graph:").unwrap_or(c.calendar_id.as_str());
                let url = format!(
                    "https://graph.microsoft.com/v1.0/me/calendars/{}/events?$select=id,subject,start,end,bodyPreview,location,lastModifiedDateTime",
                    raw_id
                );
                let events_payload = http_get_json(url.as_str(), bearer.as_str())?;
                let rows = parse_event_objects(events_payload.as_slice(), "value", "id", "subject")?;
                for r in rows {
                    let starts = r.get("dateTime")
                        .and_then(|v| parse_datetime_to_utc_ms(v))
                        .unwrap_or(start_utc_ms);
                    let ends = starts + 3600_000;
                    let event = CalendarEvent {
                        event_id: format!("{}:{}", c.calendar_id, r.get("id").cloned().unwrap_or_default()),
                        calendar_id: c.calendar_id.clone(),
                        uid: r.get("id").cloned().unwrap_or_default(),
                        sequence: 0,
                        title: r.get("subject").cloned().unwrap_or_default(),
                        description: r.get("bodyPreview").cloned(),
                        location: r.get("location").cloned(),
                        starts_at_utc_ms: starts,
                        ends_at_utc_ms: ends,
                        all_day: false,
                        organizer: None,
                        attendees: Vec::new(),
                        recurrence_rule: None,
                        status: None,
                        etag: r.get("lastModifiedDateTime").cloned(),
                    };
                    calendar_store::upsert_event(conn, &event)?;
                    fetched_events += 1;
                }
            }
        }
    }
    Ok(ProviderSyncOutcome {
        fetched_calendars,
        fetched_events,
    })
}

use crate::calendar_store;
