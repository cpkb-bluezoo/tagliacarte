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

//! Async relay connection: our WebSocket client + our JSON push parser.
//! Each WebSocket text frame is one JSON array; we parse it and emit RelayMessage / StreamMessage.
//! All functions are async; no thread-blocking code. The reactor pattern is preserved:
//! `conn.run()` yields at each `stream.read().await` and fires handler callbacks only when data arrives.

use bytes::BytesMut;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::json::{JsonContentHandler, JsonNumber, JsonParser};
use crate::protocol::websocket::{WebSocketClient, WebSocketConnection, WebSocketHandler};

use super::types::{self, Event, Filter, ProfileMetadata, filter_to_json};

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
    BACKOFF_BASE_SECS.saturating_mul(1u64 << exp).min(BACKOFF_MAX_SECS)
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
        map.insert(relay_url.to_string(), RelayBackoffState {
            last_failure: Instant::now(),
            consecutive_failures: 1,
        });
    }
}

fn record_relay_success(relay_url: &str) {
    let mut map = relay_backoff_map().lock().unwrap();
    map.remove(relay_url);
}

/// Connect to a relay with timeout and exponential backoff.
async fn connect_to_relay(relay_url: &str) -> Result<WebSocketConnection, String> {
    {
        let map = relay_backoff_map().lock().unwrap();
        if let Some(state) = map.get(relay_url) {
            let wait = backoff_seconds(state.consecutive_failures);
            let elapsed = state.last_failure.elapsed().as_secs();
            if elapsed < wait {
                return Err(format!(
                    "Relay {} in backoff ({} seconds remaining)",
                    relay_url, wait - elapsed
                ));
            }
        }
    }

    let result = tokio::time::timeout(
        Duration::from_secs(CONNECT_TIMEOUT_SECS),
        WebSocketClient::connect(relay_url),
    ).await;

    match result {
        Ok(Ok(conn)) => {
            record_relay_success(relay_url);
            Ok(conn)
        }
        Ok(Err(e)) => {
            record_relay_failure(relay_url);
            Err(format!("Failed to connect to {}: {}", relay_url, e))
        }
        Err(_) => {
            record_relay_failure(relay_url);
            Err(format!("Connection timeout to {}", relay_url))
        }
    }
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

impl JsonContentHandler for RelayMessageHandler {
    fn start_object(&mut self) {
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
    }

    fn end_object(&mut self) {
        self.depth -= 1;
        if self.depth == 1
            && self.msg_type.as_deref() == Some("EVENT")
            && self.second_str.is_some()
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
    }

    fn start_array(&mut self) {
        self.depth += 1;
        if self.depth == 1 {
            self.top_level_index = 0;
        } else if self.tags_depth == 1 {
            self.tags_depth = 2;
        } else if self.tags_depth == 2 {
            self.current_tag.clear();
        }
    }

    fn end_array(&mut self) {
        if self.tags_depth == 2 && self.depth == 4 {
            if !self.current_tag.is_empty() {
                self.event_tags.push(self.current_tag.clone());
            }
            self.current_tag.clear();
        } else if self.tags_depth == 2 && self.depth == 3 {
            self.tags_depth = 0;
        }
        self.depth -= 1;
    }

    fn key(&mut self, key: &str) {
        self.current_field = Some(key.to_string());
        if self.depth == 2 && key == "tags" {
            self.tags_depth = 1;
        }
    }

    fn string_value(&mut self, value: &str) {
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
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.depth == 2 {
            if let Some(ref f) = self.current_field {
                if f == "created_at" {
                    self.event_created_at = number.as_f64().max(0.0) as u64;
                } else if f == "kind" {
                    self.event_kind = number.as_f64().max(0.0) as u32;
                }
            }
        }
    }

    fn boolean_value(&mut self, value: bool) {
        if self.depth == 1 {
            self.top_level_index += 1;
            if self.top_level_index == 3 && self.msg_type.as_deref() == Some("OK") {
                self.ok_success = value;
            }
        }
    }

    fn null_value(&mut self) {}
}

/// Parse a single relay message using our JSON push parser.
/// The message is one complete JSON array (one WebSocket text frame). We call receive() once
/// with the full buffer; the parser consumes complete tokens. Then close() validates the
/// document is complete (unconsumed bytes at that point would mean malformed JSON).
pub fn parse_relay_message(message: &str) -> Result<RelayMessage, String> {
    let raw = message.to_string();
    let mut handler = RelayMessageHandler::new(raw.clone());
    let mut parser = JsonParser::new();
    let mut buf = BytesMut::from(message.as_bytes());
    parser
        .receive(&mut buf, &mut handler)
        .map_err(|e| format!("Relay message parse error: {}", e))?;
    parser
        .close(&mut handler)
        .map_err(|e| format!("Relay message parse error: {}", e))?;
    handler.take_result()
}

/// Run one relay's feed stream over our WebSocket client. Each text frame is parsed with our JSON
/// parser and turned into StreamMessage; events and EOSE are sent to `tx`.
pub async fn run_relay_feed_stream(
    relay_url: String,
    filter: Filter,
    timeout_seconds: u32,
    tx: mpsc::UnboundedSender<StreamMessage>,
    secret_key: Option<String>,
) {
    let conn = match connect_to_relay(&relay_url).await {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(e));
            let _ = tx.send(StreamMessage::Eose);
            return;
        }
    };

    let subscription_id = format!(
        "tc_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let filter_json = filter_to_json(&filter);
    let req_message = format!("[\"REQ\",\"{}\",{}]", subscription_id, filter_json);
    eprintln!("[nostr] REQ to {}: {}", relay_url, req_message);

    let mut conn = conn;
    if conn.send_text(req_message.as_bytes()).await.is_err() {
        return;
    }

    let mut handler = NostrRelayHandler {
        relay_url,
        tx: tx.clone(),
        should_stop: false,
        exit_on_eose: true,
        allowed_kinds: None,
        secret_key,
        auth_state: AuthState::None,
        auth_event_id: None,
        req_message: Some(req_message),
        pending_out: Vec::new(),
    };

    let timeout_duration = Duration::from_secs(timeout_seconds as u64);
    let _ = tokio::time::timeout(timeout_duration, conn.run(&mut handler)).await;

    let _ = tx.send(StreamMessage::Eose);
}

/// Run a long-lived DM subscription (kind 4) with two filters. Does not exit on EOSE.
pub async fn run_relay_dm_stream(
    relay_url: String,
    filter_received: Filter,
    filter_sent: Filter,
    tx: mpsc::UnboundedSender<StreamMessage>,
    secret_key: Option<String>,
) {
    let conn = match connect_to_relay(&relay_url).await {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(e));
            return;
        }
    };

    let subscription_id = format!(
        "tc_dm_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let f1 = filter_to_json(&filter_received);
    let f2 = filter_to_json(&filter_sent);
    let req_message = format!("[\"REQ\",\"{}\",{},{}]", subscription_id, f1, f2);

    let mut conn = conn;
    if conn.send_text(req_message.as_bytes()).await.is_err() {
        return;
    }

    let dm_kinds = vec![super::types::KIND_DM];
    let mut handler = NostrRelayHandler {
        relay_url,
        tx,
        should_stop: false,
        exit_on_eose: false,
        allowed_kinds: Some(dm_kinds),
        secret_key,
        auth_state: AuthState::None,
        auth_event_id: None,
        req_message: Some(req_message),
        pending_out: Vec::new(),
    };
    let _ = conn.run(&mut handler).await;
}

/// Run a long-lived DM subscription for NIP-04 (kind 4) and NIP-17 (kind 1059) with three filters.
/// Exits on EOSE when `exit_on_eose` is true, otherwise runs indefinitely.
pub async fn run_relay_dm_stream_nip17(
    relay_url: String,
    filter_received: Filter,
    filter_sent: Filter,
    filter_gift_wraps: Filter,
    exit_on_eose: bool,
    tx: mpsc::UnboundedSender<StreamMessage>,
    secret_key: Option<String>,
) {
    let conn = match connect_to_relay(&relay_url).await {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(e));
            if exit_on_eose {
                let _ = tx.send(StreamMessage::Eose);
            }
            return;
        }
    };

    let subscription_id = format!(
        "tc_dm17_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let f1 = filter_to_json(&filter_received);
    let f2 = filter_to_json(&filter_sent);
    let f3 = filter_to_json(&filter_gift_wraps);
    let req_message = format!("[\"REQ\",\"{}\",{},{},{}]", subscription_id, f1, f2, f3);
    eprintln!("[nostr] REQ to {}: {}", relay_url, req_message);

    let mut conn = conn;
    if conn.send_text(req_message.as_bytes()).await.is_err() {
        eprintln!("[nostr] send_text failed to {}", relay_url);
        return;
    }

    let dm_kinds = vec![super::types::KIND_DM, super::types::KIND_GIFT_WRAP];
    let mut handler = NostrRelayHandler {
        relay_url,
        tx: tx.clone(),
        should_stop: false,
        exit_on_eose,
        allowed_kinds: Some(dm_kinds),
        secret_key,
        auth_state: AuthState::None,
        auth_event_id: None,
        req_message: Some(req_message),
        pending_out: Vec::new(),
    };
    let timeout_duration = Duration::from_secs(if exit_on_eose { 30 } else { 3600 });
    let _ = tokio::time::timeout(timeout_duration, conn.run(&mut handler)).await;

    if exit_on_eose {
        let _ = tx.send(StreamMessage::Eose);
    }
}

/// Publish a single event to a relay. Connects, sends `["EVENT", <event_json>]`, waits for OK.
/// Returns `Ok(())` on success or `Err` with reason.
pub async fn publish_event(relay_url: &str, event_json: &str) -> Result<(), String> {
    let conn = connect_to_relay(relay_url).await?;
    let msg = format!("[\"EVENT\",{}]", event_json);
    let mut conn = conn;
    conn.send_text(msg.as_bytes()).await
        .map_err(|e| format!("Send event to {}: {}", relay_url, e))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<PublishResult>();
    let mut handler = PublishHandler { tx, should_stop: false };
    let timeout = Duration::from_secs(10);
    let _ = tokio::time::timeout(timeout, conn.run(&mut handler)).await;

    match rx.try_recv() {
        Ok(PublishResult::Ok { success, message }) => {
            if success {
                Ok(())
            } else {
                Err(format!("Relay rejected event: {}", message))
            }
        }
        Ok(PublishResult::Notice(msg)) => Err(format!("Relay notice: {}", msg)),
        Err(_) => Err(format!("No response from {}", relay_url)),
    }
}

enum PublishResult {
    Ok { success: bool, message: String },
    Notice(String),
}

struct PublishHandler {
    tx: mpsc::UnboundedSender<PublishResult>,
    should_stop: bool,
}

impl WebSocketHandler for PublishHandler {
    fn connected(&mut self) {}

    fn text_frame(&mut self, data: &[u8]) {
        let text = match std::str::from_utf8(data) {
            Ok(t) => t,
            Err(_) => return,
        };
        match parse_relay_message(text) {
            Ok(RelayMessage::Ok { success, message, .. }) => {
                let _ = self.tx.send(PublishResult::Ok { success, message });
                self.should_stop = true;
            }
            Ok(RelayMessage::Notice { message }) => {
                let _ = self.tx.send(PublishResult::Notice(message));
                self.should_stop = true;
            }
            _ => {}
        }
    }

    fn binary_frame(&mut self, _data: &[u8]) {}
    fn close(&mut self, _code: Option<u16>, _reason: &str) { self.should_stop = true; }
    fn ping(&mut self, _data: &[u8]) {}
    fn pong(&mut self, _data: &[u8]) {}
    fn failed(&mut self, _error: &std::io::Error) { self.should_stop = true; }
    fn should_stop(&self) -> bool { self.should_stop }
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

/// WebSocket handler for Nostr relay: parses each text frame as JSON and sends StreamMessage to tx.
struct NostrRelayHandler {
    relay_url: String,
    tx: mpsc::UnboundedSender<StreamMessage>,
    should_stop: bool,
    exit_on_eose: bool,
    /// Only forward events whose kind is in this set. None = forward all.
    allowed_kinds: Option<Vec<u32>>,
    /// Secret key for NIP-42 auth. None = auth not possible → dead-relay on challenge.
    secret_key: Option<String>,
    /// NIP-42 auth state machine.
    auth_state: AuthState,
    /// Event ID of our AUTH response (to match against OK).
    auth_event_id: Option<String>,
    /// Original REQ message to re-send after successful auth.
    req_message: Option<String>,
    /// Text frames to send back to the relay (drained by the connection run loop).
    pending_out: Vec<Vec<u8>>,
}

impl NostrRelayHandler {
    /// Build and queue a NIP-42 AUTH response for the given challenge.
    fn respond_to_auth(&mut self, challenge: &str) {
        let secret = match &self.secret_key {
            Some(s) => s.clone(),
            None => return,
        };
        let pubkey = match super::crypto::get_public_key_from_secret(&secret) {
            Ok(pk) => pk,
            Err(e) => {
                eprintln!("[nostr] {} AUTH: failed to derive pubkey: {}", self.relay_url, e);
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
            eprintln!("[nostr] {} AUTH: sign failed: {}", self.relay_url, e);
            return;
        }
        self.auth_event_id = Some(event.id.clone());
        let event_json = types::event_to_json(&event);
        let msg = format!("[\"AUTH\",{}]", event_json);
        eprintln!("[nostr] {} AUTH response queued (event {})", self.relay_url, &event.id[..8.min(event.id.len())]);
        self.pending_out.push(msg.into_bytes());
        self.auth_state = AuthState::Challenged;
    }
}

impl WebSocketHandler for NostrRelayHandler {
    fn connected(&mut self) {
        eprintln!("[nostr] {} connected", self.relay_url);
    }

    fn text_frame(&mut self, data: &[u8]) {
        let text = match std::str::from_utf8(data) {
            Ok(t) => t,
            Err(_) => return,
        };
        match parse_relay_message(text) {
            Ok(RelayMessage::Event { event, .. }) => {
                eprintln!("[nostr] {} event: kind={}, id={}", self.relay_url, event.kind, &event.id[..8.min(event.id.len())]);
                let send = match &self.allowed_kinds {
                    Some(kinds) => kinds.contains(&event.kind),
                    None => true,
                };
                if send && self.tx.send(StreamMessage::Event(event)).is_err() {
                    self.should_stop = true;
                }
            }
            Ok(RelayMessage::EndOfStoredEvents { .. }) => {
                eprintln!("[nostr] {} EOSE", self.relay_url);
                if self.exit_on_eose {
                    self.should_stop = true;
                }
            }
            Ok(RelayMessage::Notice { message }) => {
                eprintln!("[nostr] {} NOTICE: {}", self.relay_url, message);
                let _ = self.tx.send(StreamMessage::Notice(message));
            }
            Ok(RelayMessage::Closed { message, .. }) => {
                eprintln!("[nostr] {} CLOSED: {}", self.relay_url, message);
                if self.auth_state == AuthState::Challenged {
                    // Subscription was closed because we haven't authenticated yet;
                    // our AUTH response is queued and will be sent momentarily.
                } else {
                    if self.auth_state == AuthState::Authenticated {
                        // Auth succeeded but relay denied access (private/restricted).
                        eprintln!("[nostr] {} relay rejected after auth, marking as dead", self.relay_url);
                        let _ = self.tx.send(StreamMessage::AuthRequired(self.relay_url.clone()));
                    }
                    self.should_stop = true;
                }
            }
            Ok(RelayMessage::Auth { challenge }) => {
                if self.secret_key.is_some() && self.auth_state == AuthState::None {
                    eprintln!("[nostr] {} AUTH challenge, responding (NIP-42)", self.relay_url);
                    self.respond_to_auth(&challenge);
                } else {
                    eprintln!("[nostr] {} AUTH: cannot authenticate (no key or already attempted)", self.relay_url);
                    let _ = self.tx.send(StreamMessage::AuthRequired(self.relay_url.clone()));
                    self.should_stop = true;
                }
            }
            Ok(RelayMessage::Ok { event_id, success, message }) => {
                if self.auth_event_id.as_deref() == Some(event_id.as_str()) {
                    if success {
                        eprintln!("[nostr] {} AUTH accepted", self.relay_url);
                        self.auth_state = AuthState::Authenticated;
                        // Re-send the original subscription now that we're authenticated.
                        if let Some(req) = self.req_message.take() {
                            eprintln!("[nostr] {} re-sending REQ after auth", self.relay_url);
                            self.pending_out.push(req.into_bytes());
                        }
                    } else {
                        eprintln!("[nostr] {} AUTH rejected: {}", self.relay_url, message);
                        let _ = self.tx.send(StreamMessage::AuthRequired(self.relay_url.clone()));
                        self.should_stop = true;
                    }
                }
            }
            Ok(msg) => {
                eprintln!("[nostr] {} unhandled: {:?}", self.relay_url, msg);
            }
            Err(e) => {
                eprintln!("[nostr] {} parse error: {}", self.relay_url, e);
            }
        }
    }

    fn binary_frame(&mut self, _data: &[u8]) {}

    fn close(&mut self, _code: Option<u16>, _reason: &str) {
        self.should_stop = true;
    }

    fn ping(&mut self, _data: &[u8]) {}
    fn pong(&mut self, _data: &[u8]) {}

    fn failed(&mut self, _error: &std::io::Error) {
        self.should_stop = true;
    }

    fn should_stop(&self) -> bool {
        self.should_stop
    }

    fn pending_writes(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_out)
    }
}

// ============================================================
// High-level fetch helpers (profile, relay list)
// ============================================================

/// Collect all events from a single relay for the given filter.
pub async fn fetch_notes_from_relay(
    relay_url: &str,
    filter: &Filter,
    timeout_seconds: u32,
    secret_key: Option<String>,
) -> Result<Vec<Event>, String> {
    let (tx, mut rx) = mpsc::unbounded_channel();

    let url = relay_url.to_string();
    let f = filter.clone();
    let timeout = timeout_seconds;
    tokio::spawn(async move {
        run_relay_feed_stream(url, f, timeout, tx, secret_key).await;
    });

    let mut events: Vec<Event> = Vec::new();
    while let Some(msg) = rx.recv().await {
        match msg {
            StreamMessage::Event(event) => events.push(event),
            StreamMessage::Eose => break,
            StreamMessage::Notice(_) => {}
            StreamMessage::AuthRequired(_) => {
                return Err(AUTH_REQUIRED_SENTINEL.to_string());
            }
        }
    }
    Ok(events)
}

/// Sentinel error string returned by `fetch_notes_from_relay` when a relay requires NIP-42 auth.
pub const AUTH_REQUIRED_SENTINEL: &str = "AUTH_REQUIRED";

/// Fetch profile metadata (kind 0) for a pubkey from a single relay.
pub async fn fetch_profile_from_relay(
    relay_url: &str,
    pubkey: &str,
    timeout_seconds: u32,
    secret_key: Option<String>,
) -> Result<Option<ProfileMetadata>, String> {
    let filter = types::filter_profile_by_author(pubkey);
    let events = fetch_notes_from_relay(relay_url, &filter, timeout_seconds, secret_key).await?;

    let mut best: Option<&Event> = None;
    for event in &events {
        if event.kind == types::KIND_METADATA {
            match &best {
                None => best = Some(event),
                Some(current) if event.created_at > current.created_at => best = Some(event),
                _ => {}
            }
        }
    }

    match best {
        Some(event) => {
            match types::parse_profile(&event.content) {
                Ok(mut profile) => {
                    profile.created_at = Some(event.created_at);
                    Ok(Some(profile))
                }
                Err(e) => Err(e),
            }
        }
        None => Ok(None),
    }
}

/// Fetch profile from multiple relays in parallel, returning the first result
/// and the list of relay URLs that required authentication.
pub async fn fetch_profile_from_relays(
    relay_urls: &[String],
    pubkey: &str,
    timeout_seconds: u32,
    secret_key: Option<String>,
) -> (Result<Option<ProfileMetadata>, String>, Vec<String>) {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::unbounded_channel::<ProfileMetadata>();
    let (dead_tx, mut dead_rx) = mpsc::unbounded_channel::<String>();

    for relay_url in relay_urls {
        let url = relay_url.clone();
        let pk = pubkey.to_string();
        let tx = tx.clone();
        let dead_tx = dead_tx.clone();
        let sk = secret_key.clone();
        tokio::spawn(async move {
            match fetch_profile_from_relay(&url, &pk, timeout_seconds, sk).await {
                Ok(Some(profile)) => { let _ = tx.send(profile); }
                Err(ref e) if e == AUTH_REQUIRED_SENTINEL => { let _ = dead_tx.send(url); }
                _ => {}
            }
        });
    }
    drop(tx);
    drop(dead_tx);

    let result = match rx.recv().await {
        Some(profile) => Ok(Some(profile)),
        None => Ok(None),
    };
    let mut dead = Vec::new();
    while let Ok(url) = dead_rx.try_recv() {
        dead.push(url);
    }
    (result, dead)
}

/// Fetch a user's relay list (kind 10002) from a single relay.
pub async fn fetch_relay_list_from_relay(
    relay_url: &str,
    pubkey: &str,
    timeout_seconds: u32,
    secret_key: Option<String>,
) -> Result<Option<Vec<String>>, String> {
    let filter = types::filter_relay_list_by_author(pubkey);
    let events = fetch_notes_from_relay(relay_url, &filter, timeout_seconds, secret_key).await?;

    let mut best: Option<&Event> = None;
    for event in &events {
        if event.kind == types::KIND_RELAY_LIST {
            match &best {
                None => best = Some(event),
                Some(current) if event.created_at > current.created_at => best = Some(event),
                _ => {}
            }
        }
    }

    match best {
        Some(event) => match types::parse_relay_list(event) {
            Ok(urls) => Ok(Some(urls)),
            Err(_) => Ok(None),
        },
        None => Ok(None),
    }
}

/// Fetch relay list from multiple relays in parallel, returning the first successful result
/// and the list of relay URLs that required authentication.
pub async fn fetch_relay_list_from_relays(
    relay_urls: &[String],
    pubkey: &str,
    timeout_seconds: u32,
    secret_key: Option<String>,
) -> (Result<Vec<String>, String>, Vec<String>) {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<String>>();
    let (dead_tx, mut dead_rx) = mpsc::unbounded_channel::<String>();
    let mut count = 0usize;

    for relay_url in relay_urls {
        let url = relay_url.clone();
        let pk = pubkey.to_string();
        let tx = tx.clone();
        let dead_tx = dead_tx.clone();
        let sk = secret_key.clone();
        tokio::spawn(async move {
            match fetch_relay_list_from_relay(&url, &pk, timeout_seconds, sk).await {
                Ok(Some(urls)) if !urls.is_empty() => { let _ = tx.send(urls); }
                Err(ref e) if e == AUTH_REQUIRED_SENTINEL => { let _ = dead_tx.send(url); }
                _ => {}
            }
        });
        count += 1;
    }
    drop(tx);
    drop(dead_tx);

    if count == 0 {
        return (Ok(Vec::new()), Vec::new());
    }

    let result = match rx.recv().await {
        Some(urls) => Ok(urls),
        None => Ok(Vec::new()),
    };
    let mut dead = Vec::new();
    while let Ok(url) = dead_rx.try_recv() {
        dead.push(url);
    }
    (result, dead)
}

/// Fetch relay URLs from a kind 3 contacts event's content field (fallback for users
/// without a kind 10002 relay list).
pub async fn fetch_contacts_relay_list_from_relay(
    relay_url: &str,
    pubkey: &str,
    timeout_seconds: u32,
    secret_key: Option<String>,
) -> Result<Vec<String>, String> {
    let filter = types::filter_contacts_by_author(pubkey);
    let events = fetch_notes_from_relay(relay_url, &filter, timeout_seconds, secret_key).await?;

    let mut best: Option<&Event> = None;
    for event in &events {
        if event.kind == types::KIND_CONTACTS {
            match &best {
                None => best = Some(event),
                Some(current) if event.created_at > current.created_at => best = Some(event),
                _ => {}
            }
        }
    }

    match best {
        Some(event) => Ok(types::parse_contacts_relay_list(&event.content)),
        None => Ok(Vec::new()),
    }
}

/// Fetch relay URLs from kind 3 contacts across multiple relays in parallel.
/// Returns the relay list and the list of relay URLs that required authentication.
pub async fn fetch_contacts_relay_list_from_relays(
    relay_urls: &[String],
    pubkey: &str,
    timeout_seconds: u32,
    secret_key: Option<String>,
) -> (Result<Vec<String>, String>, Vec<String>) {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<String>>();
    let (dead_tx, mut dead_rx) = mpsc::unbounded_channel::<String>();
    let mut count = 0usize;

    for relay_url in relay_urls {
        let url = relay_url.clone();
        let pk = pubkey.to_string();
        let tx = tx.clone();
        let dead_tx = dead_tx.clone();
        let sk = secret_key.clone();
        tokio::spawn(async move {
            match fetch_contacts_relay_list_from_relay(&url, &pk, timeout_seconds, sk).await {
                Ok(urls) if !urls.is_empty() => { let _ = tx.send(urls); }
                Err(ref e) if e == AUTH_REQUIRED_SENTINEL => { let _ = dead_tx.send(url); }
                _ => {}
            }
        });
        count += 1;
    }
    drop(tx);
    drop(dead_tx);

    if count == 0 {
        return (Ok(Vec::new()), Vec::new());
    }

    let result = match rx.recv().await {
        Some(urls) => Ok(urls),
        None => Ok(Vec::new()),
    };
    let mut dead = Vec::new();
    while let Ok(url) = dead_rx.try_recv() {
        dead.push(url);
    }
    (result, dead)
}
