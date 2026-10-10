/*
 * relay.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */
//! Relay connections on hopf: our WebSocket client plus our JSON push
//! parser. Each WebSocket text frame is one JSON array, parsed into a
//! [`RelayMessage`] and delivered as [`StreamMessage`]s to a
//! [`StreamSink`] from the connection's reactor thread. Nothing here
//! blocks; the `*_blocking` wrappers at the end are for synchronous
//! callers on their own threads.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use hopf_core::TimerHandle;

use crate::json::{parse_str_complete, ContentHandler, Number, ParseResult};
use crate::protocol::websocket::{WebSocketClient, WebSocketHandler, WebSocketSession};

use super::types::{self, filter_to_json, Event, Filter, ProfileMetadata};

// ============================================================
// Relay connection backoff (prevents reconnect storms)
// ============================================================

const CONNECT_TIMEOUT_SECS: u64 = 5;
const BACKOFF_BASE_SECS: u64 = 10;
const BACKOFF_MAX_SECS: u64 = 300;

struct RelayBackoffState {
    last_failure: Instant,
    consecutive_failures: u32,
}

fn relay_backoff_map() -> &'static Mutex<HashMap<String, RelayBackoffState>> {
    static INSTANCE: OnceLock<Mutex<HashMap<String, RelayBackoffState>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn backoff_seconds(failures: u32) -> u64 {
    if failures == 0 {
        return 0;
    }
    let exp = (failures - 1).min(31);
    BACKOFF_BASE_SECS
        .saturating_mul(1u64 << exp)
        .min(BACKOFF_MAX_SECS)
}

fn record_relay_failure(relay_url: &str) {
    let mut map = relay_backoff_map().lock().unwrap();
    if let Some(state) = map.get_mut(relay_url) {
        let wait = backoff_seconds(state.consecutive_failures);
        if state.last_failure.elapsed().as_secs() >= wait {
            state.consecutive_failures += 1;
            state.last_failure = Instant::now();
        }
    } else {
        map.insert(
            relay_url.to_string(),
            RelayBackoffState {
                last_failure: Instant::now(),
                consecutive_failures: 1,
            },
        );
    }
}

fn record_relay_success(relay_url: &str) {
    let mut map = relay_backoff_map().lock().unwrap();
    map.remove(relay_url);
}

/// Seconds a relay in backoff still has to wait, if any.
fn backoff_remaining(relay_url: &str) -> Option<u64> {
    let map = relay_backoff_map().lock().unwrap();
    let state = map.get(relay_url)?;
    let wait = backoff_seconds(state.consecutive_failures);
    let elapsed = state.last_failure.elapsed().as_secs();
    (elapsed < wait).then_some(wait - elapsed)
}

/// Records the dial's outcome for the backoff map, then steps aside.
struct Backoff {
    relay_url: String,
    inner: Box<dyn WebSocketHandler>,
    opened: bool,
}

impl WebSocketHandler for Backoff {
    fn connected(&mut self, ws: &WebSocketSession) {
        self.opened = true;
        record_relay_success(&self.relay_url);
        self.inner.connected(ws);
    }
    fn text_frame(&mut self, ws: &WebSocketSession, data: &[u8]) {
        self.inner.text_frame(ws, data);
    }
    fn binary_frame(&mut self, ws: &WebSocketSession, data: &[u8]) {
        self.inner.binary_frame(ws, data);
    }
    fn close(&mut self, code: Option<u16>, reason: &str) {
        self.inner.close(code, reason);
    }
    fn ping(&mut self, ws: &WebSocketSession, data: &[u8]) {
        self.inner.ping(ws, data);
    }
    fn pong(&mut self, ws: &WebSocketSession, data: &[u8]) {
        self.inner.pong(ws, data);
    }
    fn failed(&mut self, error: &io::Error) {
        if self.opened {
            self.inner.failed(error);
            return;
        }
        record_relay_failure(&self.relay_url);
        let msg = if error.kind() == io::ErrorKind::TimedOut {
            format!("Connection timeout to {}", self.relay_url)
        } else {
            format!("Failed to connect to {}: {}", self.relay_url, error)
        };
        self.inner.failed(&io::Error::new(error.kind(), msg));
    }
}

/// Connect to a relay with timeout and exponential backoff. `handler`
/// hears `connected`, or `failed` (at once when the relay is in backoff).
fn connect_to_relay(relay_url: &str, mut handler: Box<dyn WebSocketHandler>) {
    if let Some(remaining) = backoff_remaining(relay_url) {
        handler.failed(&io::Error::new(
            io::ErrorKind::Other,
            format!("Relay {} in backoff ({} seconds remaining)", relay_url, remaining),
        ));
        return;
    }
    WebSocketClient::connect_with_timeout(
        relay_url,
        Duration::from_secs(CONNECT_TIMEOUT_SECS),
        Box::new(Backoff { relay_url: relay_url.to_string(), inner: handler, opened: false }),
    );
}

/// Relay message from Nostr wire format: ["EVENT", sub_id, event], ["EOSE", sub_id], ["NOTICE", msg], ["OK", id, ok, msg], ["CLOSED", sub_id, msg].
#[derive(Debug)]
pub enum RelayMessage {
    Event {
        _subscription_id: String,
        event: Event,
    },
    EndOfStoredEvents {
        _subscription_id: String,
    },
    Notice {
        message: String,
    },
    Ok {
        event_id: String,
        success: bool,
        message: String,
    },
    Closed {
        _subscription_id: String,
        message: String,
    },
    Auth {
        challenge: String,
    },
    Unknown {
        _raw: String,
    },
}

/// Message sent from async relay stream to the UI or store layer.
#[derive(Debug)]
pub enum StreamMessage {
    Event(Event),
    Eose,
    Notice(String),
    /// Relay requires NIP-42 authentication we don't support; carries the relay URL.
    AuthRequired(String),
}

/// Handler that accumulates state while parsing one relay message (top-level JSON array).
struct RelayMessageHandler {
    depth: i32,
    top_level_index: i32,
    msg_type: Option<String>,
    second_str: Option<String>,
    sub_id: Option<String>,
    ok_event_id: Option<String>,
    ok_success: bool,
    ok_message: Option<String>,
    current_field: Option<String>,
    event_id: Option<String>,
    event_pubkey: Option<String>,
    event_created_at: u64,
    event_kind: u32,
    event_content: String,
    event_sig: Option<String>,
    event_tags: Vec<Vec<String>>,
    current_tag: Vec<String>,
    tags_depth: i32,
    result: Option<RelayMessage>,
    raw: String,
}

impl RelayMessageHandler {
    fn new(raw: String) -> Self {
        Self {
            depth: 0,
            top_level_index: -1,
            msg_type: None,
            second_str: None,
            sub_id: None,
            ok_event_id: None,
            ok_success: false,
            ok_message: None,
            current_field: None,
            event_id: None,
            event_pubkey: None,
            event_created_at: 0,
            event_kind: 0,
            event_content: String::new(),
            event_sig: None,
            event_tags: Vec::new(),
            current_tag: Vec::new(),
            tags_depth: 0,
            result: None,
            raw,
        }
    }

    fn take_result(&mut self) -> Result<RelayMessage, String> {
        if let Some(r) = self.result.take() {
            return Ok(r);
        }
        match self.msg_type.as_deref() {
            Some("EOSE") => Ok(RelayMessage::EndOfStoredEvents {
                _subscription_id: self.second_str.clone().unwrap_or_default(),
            }),
            Some("NOTICE") => Ok(RelayMessage::Notice {
                message: self
                    .second_str
                    .clone()
                    .unwrap_or_else(|| "Unknown notice".to_string()),
            }),
            Some("OK") => Ok(RelayMessage::Ok {
                event_id: self.ok_event_id.clone().unwrap_or_default(),
                success: self.ok_success,
                message: self.ok_message.clone().unwrap_or_default(),
            }),
            Some("CLOSED") => Ok(RelayMessage::Closed {
                _subscription_id: self.second_str.clone().unwrap_or_default(),
                message: self.ok_message.clone().unwrap_or_default(),
            }),
            Some("AUTH") => Ok(RelayMessage::Auth {
                challenge: self.second_str.clone().unwrap_or_default(),
            }),
            _ => Ok(RelayMessage::Unknown {
                _raw: self.raw.clone(),
            }),
        }
    }
}

impl ContentHandler for RelayMessageHandler {
    fn start_object(&mut self) -> ParseResult<()> {
        self.depth += 1;
        if self.depth == 1 {
            self.top_level_index += 1;
        }
        if self.depth == 2 && self.msg_type.as_deref() == Some("EVENT") {
            self.current_field = None;
            self.event_id = None;
            self.event_pubkey = None;
            self.event_created_at = 0;
            self.event_kind = 0;
            self.event_content.clear();
            self.event_sig = None;
            self.event_tags.clear();
        }
        Ok(())
    }

    fn end_object(&mut self) -> ParseResult<()> {
        self.depth -= 1;
        if self.depth == 1 && self.msg_type.as_deref() == Some("EVENT") && self.second_str.is_some()
        {
            let sub_id_owned = self.sub_id.clone().unwrap_or_default();
            let ev = Event {
                id: self.event_id.clone().unwrap_or_default(),
                pubkey: self.event_pubkey.clone().unwrap_or_default(),
                created_at: self.event_created_at,
                kind: self.event_kind,
                tags: self.event_tags.clone(),
                content: self.event_content.clone(),
                sig: self.event_sig.clone().unwrap_or_default(),
            };
            self.result = Some(RelayMessage::Event {
                _subscription_id: sub_id_owned,
                event: ev,
            });
        }
        Ok(())
    }

    fn start_array(&mut self) -> ParseResult<()> {
        self.depth += 1;
        if self.depth == 1 {
            self.top_level_index = 0;
        } else if self.tags_depth == 1 {
            self.tags_depth = 2;
        } else if self.tags_depth == 2 {
            self.current_tag.clear();
        }
        Ok(())
    }

    fn end_array(&mut self) -> ParseResult<()> {
        if self.tags_depth == 2 && self.depth == 4 {
            if !self.current_tag.is_empty() {
                self.event_tags.push(self.current_tag.clone());
            }
            self.current_tag.clear();
        } else if self.tags_depth == 2 && self.depth == 3 {
            self.tags_depth = 0;
        }
        self.depth -= 1;
        Ok(())
    }

    fn key(&mut self, key: &str) -> ParseResult<()> {
        self.current_field = Some(key.to_string());
        if self.depth == 2 && key == "tags" {
            self.tags_depth = 1;
        }
        Ok(())
    }

    fn string_value(&mut self, value: &str) -> ParseResult<()> {
        let s = value.to_string();
        if self.depth == 1 {
            self.top_level_index += 1;
            if self.top_level_index == 1 {
                self.msg_type = Some(s.clone());
            } else if self.top_level_index == 2 {
                self.second_str = Some(s.clone());
                self.sub_id = Some(s.clone());
                self.ok_event_id = Some(s);
            } else if self.top_level_index == 3 && self.msg_type.as_deref() == Some("CLOSED") {
                self.ok_message = Some(s);
            } else if self.top_level_index == 4 && self.msg_type.as_deref() == Some("OK") {
                self.ok_message = Some(s);
            }
        } else if self.tags_depth == 2 {
            self.current_tag.push(s);
        } else if self.depth >= 2 && self.tags_depth == 0 {
            if let Some(ref f) = self.current_field {
                match f.as_str() {
                    "id" => self.event_id = Some(s),
                    "pubkey" => self.event_pubkey = Some(s),
                    "content" => self.event_content = s,
                    "sig" => self.event_sig = Some(s),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn number_value(&mut self, number: &Number) -> ParseResult<()> {
        if self.depth == 2 {
            if let Some(ref f) = self.current_field {
                if f == "created_at" {
                    self.event_created_at = number.as_f64().unwrap_or(0.0).max(0.0) as u64;
                } else if f == "kind" {
                    self.event_kind = number.as_f64().unwrap_or(0.0).max(0.0) as u32;
                }
            }
        }
        Ok(())
    }

    fn boolean_value(&mut self, value: bool) -> ParseResult<()> {
        if self.depth == 1 {
            self.top_level_index += 1;
            if self.top_level_index == 3 && self.msg_type.as_deref() == Some("OK") {
                self.ok_success = value;
            }
        }
        Ok(())
    }

    fn null_value(&mut self) -> ParseResult<()> { Ok(()) }
}

/// Parse a single relay message using our JSON push parser.
/// The message is one complete JSON array (one WebSocket text frame). We call receive() once
/// with the full buffer; the parser consumes complete tokens. Then close() validates the
/// document is complete (unconsumed bytes at that point would mean malformed JSON).
pub fn parse_relay_message(message: &str) -> Result<RelayMessage, String> {
    let raw = message.to_string();
    let mut handler = RelayMessageHandler::new(raw.clone());
    parse_str_complete(message, &mut handler).map_err(|e| format!("Relay message parse error: {}", e))?;
    handler.take_result()
}

/// Hears what one relay subscription produces, from the connection's
/// reactor thread; `finished` once, when the stream is over (EOSE on a
/// one-shot subscription, the time limit, a close, or a failure).
pub trait StreamSink: Send {
    fn message(&mut self, msg: StreamMessage);
    fn finished(&mut self);
}

fn subscription_id(prefix: &str) -> String {
    format!(
        "{}{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    )
}

/// Run one relay's feed stream: `REQ` with `filter`, events to `sink`
/// until EOSE or `timeout_seconds`, then `finished`.
pub fn run_relay_feed_stream(
    relay_url: String,
    filter: Filter,
    timeout_seconds: u32,
    secret_key: Option<String>,
    sink: Box<dyn StreamSink>,
) {
    let req_message = format!("[\"REQ\",\"{}\",{}]", subscription_id("tc_"), filter_to_json(&filter));
    crate::trace_log!("nostr", "REQ to {}: {}", relay_url, req_message);
    let handler = NostrRelayHandler::new(
        relay_url.clone(),
        sink,
        true,
        None,
        secret_key,
        req_message,
        Some(Duration::from_secs(timeout_seconds as u64)),
    );
    connect_to_relay(&relay_url, Box::new(handler));
}

/// A long-lived DM subscription (kind 4) with two filters. Does not exit on EOSE.
pub fn run_relay_dm_stream(
    relay_url: String,
    filter_received: Filter,
    filter_sent: Filter,
    secret_key: Option<String>,
    sink: Box<dyn StreamSink>,
) {
    let req_message = format!(
        "[\"REQ\",\"{}\",{},{}]",
        subscription_id("tc_dm_"),
        filter_to_json(&filter_received),
        filter_to_json(&filter_sent)
    );
    let handler = NostrRelayHandler::new(
        relay_url.clone(),
        sink,
        false,
        Some(vec![super::types::KIND_DM]),
        secret_key,
        req_message,
        None,
    );
    connect_to_relay(&relay_url, Box::new(handler));
}

/// A DM subscription for NIP-04 (kind 4) and NIP-17 (kind 1059) with three
/// filters. Finishes on EOSE when `exit_on_eose` is true, otherwise after an
/// hour at most.
pub fn run_relay_dm_stream_nip17(
    relay_url: String,
    filter_received: Filter,
    filter_sent: Filter,
    filter_gift_wraps: Filter,
    exit_on_eose: bool,
    secret_key: Option<String>,
    sink: Box<dyn StreamSink>,
) {
    let req_message = format!(
        "[\"REQ\",\"{}\",{},{},{}]",
        subscription_id("tc_dm17_"),
        filter_to_json(&filter_received),
        filter_to_json(&filter_sent),
        filter_to_json(&filter_gift_wraps)
    );
    crate::trace_log!("nostr", "REQ to {}: {}", relay_url, req_message);
    let handler = NostrRelayHandler::new(
        relay_url.clone(),
        sink,
        exit_on_eose,
        Some(vec![super::types::KIND_DM, super::types::KIND_GIFT_WRAP]),
        secret_key,
        req_message,
        Some(Duration::from_secs(if exit_on_eose { 60 } else { 3600 })),
    );
    connect_to_relay(&relay_url, Box::new(handler));
}

/// Hears one outcome.
pub type Done<T> = Box<dyn FnOnce(Result<T, String>) + Send>;

/// Publish a single event to a relay: connect, send `["EVENT", <event_json>]`,
/// wait for the `OK` (ten seconds at most).
pub fn publish_event(relay_url: &str, event_json: &str, done: Done<()>) {
    let handler = PublishHandler {
        relay_url: relay_url.to_string(),
        message: format!("[\"EVENT\",{}]", event_json),
        done: Some(done),
        timer: None,
    };
    connect_to_relay(relay_url, Box::new(handler));
}

struct PublishHandler {
    relay_url: String,
    message: String,
    done: Option<Done<()>>,
    timer: Option<TimerHandle>,
}

impl PublishHandler {
    fn finish(&mut self, ws: Option<&WebSocketSession>, r: Result<(), String>) {
        if let Some(t) = self.timer.take() {
            t.cancel();
        }
        if let Some(ws) = ws {
            ws.close();
        }
        if let Some(done) = self.done.take() {
            done(r);
        }
    }
}

impl WebSocketHandler for PublishHandler {
    fn connected(&mut self, ws: &WebSocketSession) {
        if let Err(e) = ws.send_text(&self.message) {
            self.finish(Some(ws), Err(format!("Send event to {}: {}", self.relay_url, e)));
            return;
        }
        let closer = ws.clone();
        self.timer = Some(ws.schedule_timer(Duration::from_secs(10), Box::new(move || closer.close())));
    }

    fn text_frame(&mut self, ws: &WebSocketSession, data: &[u8]) {
        let Ok(text) = std::str::from_utf8(data) else { return };
        match parse_relay_message(text) {
            Ok(RelayMessage::Ok { success, message, .. }) => {
                let r = if success { Ok(()) } else { Err(format!("Relay rejected event: {}", message)) };
                self.finish(Some(ws), r);
            }
            Ok(RelayMessage::Notice { message }) => self.finish(Some(ws), Err(format!("Relay notice: {}", message))),
            _ => {}
        }
    }

    fn binary_frame(&mut self, _ws: &WebSocketSession, _data: &[u8]) {}

    fn close(&mut self, _code: Option<u16>, _reason: &str) {
        let url = self.relay_url.clone();
        self.finish(None, Err(format!("No response from {}", url)));
    }

    fn failed(&mut self, error: &io::Error) {
        let msg = if self.timer.is_some() { format!("No response from {}", self.relay_url) } else { error.to_string() };
        self.finish(None, Err(msg));
    }
}

/// NIP-42 authentication state for a relay connection.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AuthState {
    /// No AUTH challenge received yet.
    None,
    /// AUTH challenge received, response queued but not yet acknowledged.
    Challenged,
    /// Relay accepted our AUTH.
    Authenticated,
}

const KIND_AUTH: u32 = 22242;

/// WebSocket handler for a Nostr relay subscription: parses each text frame
/// as JSON and delivers [`StreamMessage`]s to the sink.
struct NostrRelayHandler {
    relay_url: String,
    sink: Option<Box<dyn StreamSink>>,
    exit_on_eose: bool,
    /// Only forward events whose kind is in this set. None = forward all.
    allowed_kinds: Option<Vec<u32>>,
    /// Secret key for NIP-42 auth. None = auth not possible → dead-relay on challenge.
    secret_key: Option<String>,
    /// NIP-42 auth state machine.
    auth_state: AuthState,
    /// Event ID of our AUTH response (to match against OK).
    auth_event_id: Option<String>,
    /// The subscription: sent on connect, and again after a successful AUTH.
    req_message: Option<String>,
    /// Text frames to send back to the relay.
    pending_out: Vec<Vec<u8>>,
    /// How long the subscription may stay open.
    time_limit: Option<Duration>,
    timer: Option<TimerHandle>,
}

impl NostrRelayHandler {
    fn new(
        relay_url: String,
        sink: Box<dyn StreamSink>,
        exit_on_eose: bool,
        allowed_kinds: Option<Vec<u32>>,
        secret_key: Option<String>,
        req_message: String,
        time_limit: Option<Duration>,
    ) -> Self {
        Self {
            relay_url,
            sink: Some(sink),
            exit_on_eose,
            allowed_kinds,
            secret_key,
            auth_state: AuthState::None,
            auth_event_id: None,
            req_message: Some(req_message),
            pending_out: Vec::new(),
            time_limit,
            timer: None,
        }
    }

    fn emit(&mut self, msg: StreamMessage) {
        if let Some(s) = self.sink.as_mut() {
            s.message(msg);
        }
    }

    /// The stream is over: close the connection and tell the sink once.
    fn finish(&mut self, ws: Option<&WebSocketSession>) {
        if let Some(t) = self.timer.take() {
            t.cancel();
        }
        if let Some(ws) = ws {
            ws.close();
        }
        if let Some(mut s) = self.sink.take() {
            s.finished();
        }
    }

    fn flush(&mut self, ws: &WebSocketSession) {
        for frame in std::mem::take(&mut self.pending_out) {
            if let Ok(text) = std::str::from_utf8(&frame) {
                if ws.send_text(text).is_err() {
                    crate::trace_log!("nostr", "send_text failed to {}", self.relay_url);
                }
            }
        }
    }

    /// Build and queue a NIP-42 AUTH response for the given challenge.
    fn respond_to_auth(&mut self, challenge: &str) {
        let secret = match &self.secret_key {
            Some(s) => s.clone(),
            None => return,
        };
        let pubkey = match super::crypto::get_public_key_from_secret(&secret) {
            Ok(pk) => pk,
            Err(e) => {
                crate::trace_log!(
                    "nostr",
                    "{} AUTH: failed to derive pubkey: {}",
                    self.relay_url,
                    e
                );
                return;
            }
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut event = Event {
            id: String::new(),
            pubkey,
            created_at: now,
            kind: KIND_AUTH,
            tags: vec![
                vec!["relay".to_string(), self.relay_url.clone()],
                vec!["challenge".to_string(), challenge.to_string()],
            ],
            content: String::new(),
            sig: String::new(),
        };
        if let Err(e) = super::crypto::sign_event(&mut event, &secret) {
            crate::trace_log!("nostr", "{} AUTH: sign failed: {}", self.relay_url, e);
            return;
        }
        self.auth_event_id = Some(event.id.clone());
        let event_json = types::event_to_json(&event);
        let msg = format!("[\"AUTH\",{}]", event_json);
        crate::trace_log!(
            "nostr",
            "{} AUTH response queued (event {})",
            self.relay_url,
            &event.id[..8.min(event.id.len())]
        );
        if crate::trace::full_enabled("nostr") {
            crate::trace_log!("nostr", "{} AUTH outbound frame: {}", self.relay_url, msg);
        }
        self.pending_out.push(msg.into_bytes());
        self.auth_state = AuthState::Challenged;
    }
}

impl WebSocketHandler for NostrRelayHandler {
    fn connected(&mut self, ws: &WebSocketSession) {
        crate::trace_log!("nostr", "{} connected", self.relay_url);
        if let Some(req) = self.req_message.clone() {
            self.pending_out.push(req.into_bytes());
        }
        self.flush(ws);
        if let Some(limit) = self.time_limit {
            let closer = ws.clone();
            self.timer = Some(ws.schedule_timer(limit, Box::new(move || closer.close())));
        }
    }

    fn text_frame(&mut self, ws: &WebSocketSession, data: &[u8]) {
        let text = match std::str::from_utf8(data) {
            Ok(t) => t,
            Err(_) => return,
        };
        let mut stop = false;
        match parse_relay_message(text) {
            Ok(RelayMessage::Event { event, .. }) => {
                crate::trace_log!(
                    "nostr",
                    "{} event: kind={}, id={}",
                    self.relay_url,
                    event.kind,
                    &event.id[..8.min(event.id.len())]
                );
                let send = match &self.allowed_kinds {
                    Some(kinds) => kinds.contains(&event.kind),
                    None => true,
                };
                if send {
                    self.emit(StreamMessage::Event(event));
                }
            }
            Ok(RelayMessage::EndOfStoredEvents { .. }) => {
                crate::trace_log!("nostr", "{} EOSE", self.relay_url);
                self.emit(StreamMessage::Eose);
                if self.exit_on_eose {
                    stop = true;
                }
            }
            Ok(RelayMessage::Notice { message }) => {
                crate::trace_log!("nostr", "{} NOTICE: {}", self.relay_url, message);
                self.emit(StreamMessage::Notice(message));
            }
            Ok(RelayMessage::Closed { message, .. }) => {
                crate::trace_log!("nostr", "{} CLOSED: {}", self.relay_url, message);
                if self.auth_state == AuthState::Challenged {
                    // Subscription was closed because we haven't authenticated yet;
                    // our AUTH response is on its way.
                } else {
                    if self.auth_state == AuthState::Authenticated {
                        // Auth succeeded but relay denied access (private/restricted).
                        crate::trace_log!(
                            "nostr",
                            "{} relay rejected after auth, marking as dead",
                            self.relay_url
                        );
                        let url = self.relay_url.clone();
                        self.emit(StreamMessage::AuthRequired(url));
                    }
                    stop = true;
                }
            }
            Ok(RelayMessage::Auth { challenge }) => {
                if self.secret_key.is_some() && self.auth_state == AuthState::None {
                    if crate::trace::enabled("nostr") {
                        if crate::trace::full_enabled("nostr") {
                            eprintln!(
                                "[nostr trace] {} AUTH challenge (NIP-42): {}",
                                self.relay_url, challenge
                            );
                        } else {
                            eprintln!(
                                "[nostr trace] {} AUTH challenge (NIP-42, {} chars redacted; add nostr to TAGLIACARTE_TRACE_FULL for value)",
                                self.relay_url,
                                challenge.len()
                            );
                        }
                    }
                    self.respond_to_auth(&challenge);
                } else {
                    crate::trace_log!(
                        "nostr",
                        "{} AUTH: cannot authenticate (no key or already attempted)",
                        self.relay_url
                    );
                    let url = self.relay_url.clone();
                    self.emit(StreamMessage::AuthRequired(url));
                    stop = true;
                }
            }
            Ok(RelayMessage::Ok { event_id, success, message }) => {
                if self.auth_event_id.as_deref() == Some(event_id.as_str()) {
                    if success {
                        crate::trace_log!("nostr", "{} AUTH accepted", self.relay_url);
                        self.auth_state = AuthState::Authenticated;
                        // Re-send the original subscription now that we're authenticated.
                        if let Some(req) = self.req_message.clone() {
                            crate::trace_log!("nostr", "{} re-sending REQ after auth", self.relay_url);
                            if crate::trace::enabled("nostr") && crate::trace::full_enabled("nostr") {
                                crate::trace_log!("nostr", "{} REQ body: {}", self.relay_url, req);
                            }
                            self.pending_out.push(req.into_bytes());
                        }
                    } else {
                        crate::trace_log!("nostr", "{} AUTH rejected: {}", self.relay_url, message);
                        let url = self.relay_url.clone();
                        self.emit(StreamMessage::AuthRequired(url));
                        stop = true;
                    }
                }
            }
            Ok(msg) => {
                crate::trace_log!("nostr", "{} unhandled: {:?}", self.relay_url, msg);
            }
            Err(e) => {
                crate::trace_log!("nostr", "{} parse error: {}", self.relay_url, e);
            }
        }
        if stop {
            self.finish(Some(ws));
        } else {
            self.flush(ws);
        }
    }

    fn binary_frame(&mut self, _ws: &WebSocketSession, _data: &[u8]) {}

    fn close(&mut self, _code: Option<u16>, _reason: &str) {
        self.finish(None);
    }

    fn failed(&mut self, error: &io::Error) {
        if self.timer.is_none() {
            // Never connected: say why, as the old stream did.
            self.emit(StreamMessage::Notice(error.to_string()));
        }
        self.finish(None);
    }
}

// ============================================================
// High-level fetch helpers (profile, relay list)
// ============================================================

/// Sentinel error string returned by `fetch_notes_from_relay` when a relay requires NIP-42 auth.
pub const AUTH_REQUIRED_SENTINEL: &str = "AUTH_REQUIRED";

/// Collects one subscription's events until it finishes.
struct Collect {
    events: Vec<Event>,
    auth_required: bool,
    done: Option<Done<Vec<Event>>>,
}

impl StreamSink for Collect {
    fn message(&mut self, msg: StreamMessage) {
        match msg {
            StreamMessage::Event(event) => self.events.push(event),
            StreamMessage::AuthRequired(_) => self.auth_required = true,
            StreamMessage::Eose | StreamMessage::Notice(_) => {}
        }
    }

    fn finished(&mut self) {
        if let Some(done) = self.done.take() {
            if self.auth_required {
                done(Err(AUTH_REQUIRED_SENTINEL.to_string()))
            } else {
                done(Ok(std::mem::take(&mut self.events)))
            }
        }
    }
}

/// Collect all events from a single relay for the given filter.
pub fn fetch_notes_from_relay(relay_url: &str, filter: &Filter, timeout_seconds: u32, secret_key: Option<String>, done: Done<Vec<Event>>) {
    run_relay_feed_stream(
        relay_url.to_string(),
        filter.clone(),
        timeout_seconds,
        secret_key,
        Box::new(Collect { events: Vec::new(), auth_required: false, done: Some(done) }),
    );
}

/// The newest event of `kind`.
fn newest_of_kind(events: &[Event], kind: u32) -> Option<&Event> {
    let mut best: Option<&Event> = None;
    for event in events {
        if event.kind == kind {
            match &best {
                None => best = Some(event),
                Some(current) if event.created_at > current.created_at => best = Some(event),
                _ => {}
            }
        }
    }
    best
}

/// Fetch profile metadata (kind 0) for a pubkey from a single relay.
pub fn fetch_profile_from_relay(relay_url: &str, pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: Done<Option<ProfileMetadata>>) {
    let filter = types::filter_profile_by_author(pubkey);
    fetch_notes_from_relay(
        relay_url,
        &filter,
        timeout_seconds,
        secret_key,
        Box::new(move |events| {
            done(events.and_then(|events| match newest_of_kind(&events, types::KIND_METADATA) {
                Some(event) => types::parse_profile(&event.content).map(|mut profile| {
                    profile.created_at = Some(event.created_at);
                    Some(profile)
                }),
                None => Ok(None),
            }))
        }),
    );
}

/// Fetch a user's relay list (kind 10002) from a single relay.
pub fn fetch_relay_list_from_relay(relay_url: &str, pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: Done<Option<Vec<String>>>) {
    let filter = types::filter_relay_list_by_author(pubkey);
    fetch_notes_from_relay(
        relay_url,
        &filter,
        timeout_seconds,
        secret_key,
        Box::new(move |events| {
            done(events.map(|events| {
                newest_of_kind(&events, types::KIND_RELAY_LIST).and_then(|event| types::parse_relay_list(event).ok())
            }))
        }),
    );
}

/// Fetch relay URLs from a kind 3 contacts event's content field (fallback for users
/// without a kind 10002 relay list).
pub fn fetch_contacts_relay_list_from_relay(relay_url: &str, pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: Done<Vec<String>>) {
    let filter = types::filter_contacts_by_author(pubkey);
    fetch_notes_from_relay(
        relay_url,
        &filter,
        timeout_seconds,
        secret_key,
        Box::new(move |events| {
            done(events.map(|events| {
                newest_of_kind(&events, types::KIND_CONTACTS)
                    .map(|event| types::parse_contacts_relay_list(&event.content))
                    .unwrap_or_default()
            }))
        }),
    );
}

/// Fetch a user's NIP-17 DM relay list (kind 10050) from a single relay.
pub fn fetch_dm_relay_list_from_relay(relay_url: &str, pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: Done<Option<Vec<String>>>) {
    let filter = types::filter_dm_relay_list_by_author(pubkey);
    fetch_notes_from_relay(
        relay_url,
        &filter,
        timeout_seconds,
        secret_key,
        Box::new(move |events| {
            done(events.map(|events| {
                newest_of_kind(&events, types::KIND_DM_RELAY_LIST)
                    .and_then(|event| types::parse_dm_relay_list(event).ok())
                    .filter(|urls| !urls.is_empty())
            }))
        }),
    );
}

/// What a fan-out over several relays reports: the first usable answer
/// (or none), and the relays that required NIP-42 authentication.
pub type OnFanOut<T> = Box<dyn FnOnce((Result<T, String>, Vec<String>)) + Send>;

struct FanOut<T> {
    pending: usize,
    /// Relays that answered (an event or an end-of-stored-events with none).
    answered: usize,
    /// The last failure, reported when no relay answered at all.
    last_error: Option<String>,
    dead: Vec<String>,
    done: Option<OnFanOut<Option<T>>>,
}

/// Ask every relay; the first `Some` answer is delivered at once (with the
/// relays found dead so far), otherwise `None` once all have answered. When
/// not one relay answered (every dial, handshake or request failed or timed
/// out) that is an error, not "nothing there": callers must not conclude a
/// profile does not exist from a network failure.
fn fan_out<T: Send + 'static>(
    relay_urls: &[String],
    ask: impl Fn(&str, Done<Option<T>>),
    done: OnFanOut<Option<T>>,
) {
    if relay_urls.is_empty() {
        return done((Ok(None), Vec::new()));
    }
    let state = Arc::new(Mutex::new(FanOut { pending: relay_urls.len(), answered: 0, last_error: None, dead: Vec::new(), done: Some(done) }));
    for url in relay_urls {
        let state = Arc::clone(&state);
        let relay = url.clone();
        ask(
            url,
            Box::new(move |r| {
                let fire = {
                    let mut g = state.lock().unwrap();
                    g.pending -= 1;
                    match r {
                        Ok(Some(v)) => {
                            g.answered += 1;
                            g.done.take().map(|d| (d, Ok(Some(v)), std::mem::take(&mut g.dead)))
                        }
                        Ok(None) => {
                            g.answered += 1;
                            None
                        }
                        Err(ref e) if e == AUTH_REQUIRED_SENTINEL => {
                            g.dead.push(relay.clone());
                            None
                        }
                        Err(e) => {
                            g.last_error = Some(e);
                            None
                        }
                    }
                    .or_else(|| {
                        if g.pending == 0 {
                            let outcome = if g.answered == 0 && !g.dead.is_empty() {
                                // Every relay wanted authentication we could not give: the
                                // callers' dead-relay handling takes over, as before.
                                Ok(None)
                            } else if g.answered == 0 {
                                Err(g.last_error.take().unwrap_or_else(|| "no relay answered".to_string()))
                            } else {
                                Ok(None)
                            };
                            g.done.take().map(|d| (d, outcome, std::mem::take(&mut g.dead)))
                        } else {
                            None
                        }
                    })
                };
                if let Some((d, r, dead)) = fire {
                    d((r, dead));
                }
            }),
        );
    }
}

/// Fetch profile from multiple relays in parallel, returning the first result
/// and the list of relay URLs that required authentication.
pub fn fetch_profile_from_relays(relay_urls: &[String], pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: OnFanOut<Option<ProfileMetadata>>) {
    fan_out(
        relay_urls,
        |url, k| fetch_profile_from_relay(url, pubkey, timeout_seconds, secret_key.clone(), k),
        done,
    );
}

fn lists(done: OnFanOut<Vec<String>>) -> OnFanOut<Option<Vec<String>>> {
    Box::new(move |(r, dead)| done((r.map(Option::unwrap_or_default), dead)))
}

/// Fetch relay list from multiple relays in parallel, returning the first successful result
/// and the list of relay URLs that required authentication.
pub fn fetch_relay_list_from_relays(relay_urls: &[String], pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: OnFanOut<Vec<String>>) {
    fan_out(
        relay_urls,
        |url, k| {
            fetch_relay_list_from_relay(url, pubkey, timeout_seconds, secret_key.clone(), Box::new(move |r| k(r.map(|o| o.filter(|u| !u.is_empty())))))
        },
        lists(done),
    );
}

/// Fetch relay URLs from kind 3 contacts across multiple relays in parallel.
/// Returns the relay list and the list of relay URLs that required authentication.
pub fn fetch_contacts_relay_list_from_relays(relay_urls: &[String], pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: OnFanOut<Vec<String>>) {
    fan_out(
        relay_urls,
        |url, k| {
            fetch_contacts_relay_list_from_relay(url, pubkey, timeout_seconds, secret_key.clone(), Box::new(move |r| k(r.map(|u| (!u.is_empty()).then_some(u)))))
        },
        lists(done),
    );
}

/// Fetch DM relay list (kind 10050) from multiple relays in parallel; first non-empty wins.
pub fn fetch_dm_relay_list_from_relays(relay_urls: &[String], pubkey: &str, timeout_seconds: u32, secret_key: Option<String>, done: OnFanOut<Vec<String>>) {
    fan_out(
        relay_urls,
        |url, k| fetch_dm_relay_list_from_relay(url, pubkey, timeout_seconds, secret_key.clone(), k),
        lists(done),
    );
}

// ============================================================
// Blocking forms for synchronous callers (never a reactor thread)
// ============================================================

fn wait_for<T: Send + 'static>(start: impl FnOnce(Box<dyn FnOnce(T) + Send>)) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    start(Box::new(move |v| {
        let _ = tx.send(v);
    }));
    rx.recv().ok()
}

/// [`publish_event`] waited for on this thread.
pub fn publish_event_blocking(relay_url: &str, event_json: &str) -> Result<(), String> {
    wait_for(|k| publish_event(relay_url, event_json, k)).unwrap_or_else(|| Err("publish ended without a result".to_string()))
}

/// [`fetch_profile_from_relays`] waited for on this thread.
pub fn fetch_profile_from_relays_blocking(relay_urls: &[String], pubkey: &str, timeout_seconds: u32, secret_key: Option<String>) -> (Result<Option<ProfileMetadata>, String>, Vec<String>) {
    wait_for(|k| fetch_profile_from_relays(relay_urls, pubkey, timeout_seconds, secret_key, k))
        .unwrap_or_else(|| (Err("profile fetch ended without a result".to_string()), Vec::new()))
}

/// [`fetch_relay_list_from_relays`] waited for on this thread.
pub fn fetch_relay_list_from_relays_blocking(relay_urls: &[String], pubkey: &str, timeout_seconds: u32, secret_key: Option<String>) -> (Result<Vec<String>, String>, Vec<String>) {
    wait_for(|k| fetch_relay_list_from_relays(relay_urls, pubkey, timeout_seconds, secret_key, k))
        .unwrap_or_else(|| (Err("relay list fetch ended without a result".to_string()), Vec::new()))
}

/// [`fetch_dm_relay_list_from_relays`] waited for on this thread.
pub fn fetch_dm_relay_list_from_relays_blocking(relay_urls: &[String], pubkey: &str, timeout_seconds: u32, secret_key: Option<String>) -> (Result<Vec<String>, String>, Vec<String>) {
    wait_for(|k| fetch_dm_relay_list_from_relays(relay_urls, pubkey, timeout_seconds, secret_key, k))
        .unwrap_or_else(|| (Err("DM relay list fetch ended without a result".to_string()), Vec::new()))
}

#[cfg(test)]
mod fan_out_tests {
    use super::*;

    fn run(answers: Vec<Result<Option<u32>, String>>) -> (Result<Option<u32>, String>, Vec<String>) {
        let urls: Vec<String> = (0..answers.len()).map(|i| format!("wss://r{i}.example")).collect();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let answers = std::sync::Mutex::new(answers.into_iter());
        fan_out(
            &urls,
            |_, k| k(answers.lock().unwrap().next().unwrap()),
            Box::new(move |r| {
                let _ = tx.send(r);
            }),
        );
        rx.recv_timeout(Duration::from_secs(5)).expect("fan-out finished")
    }

    #[test]
    fn a_relay_that_answers_without_an_event_means_none() {
        let (r, _) = run(vec![Err("connect refused".into()), Ok(None)]);
        assert_eq!(r, Ok(None));
    }

    #[test]
    fn the_first_event_wins() {
        let (r, _) = run(vec![Ok(None), Ok(Some(7))]);
        assert_eq!(r, Ok(Some(7)));
    }

    /// No relay answering at all is not the same as "no such profile": the
    /// caller must not remember it as a negative result.
    #[test]
    fn every_relay_failing_is_an_error_not_none() {
        let (r, _) = run(vec![Err("connect refused".into()), Err("timed out".into())]);
        assert!(r.is_err(), "{r:?}");
    }
}
