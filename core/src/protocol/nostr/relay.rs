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

use bytes::BytesMut;
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::json::{JsonContentHandler, JsonNumber, JsonParser};
use crate::protocol::websocket::{WebSocketClient, WebSocketHandler};

use super::types::{Event, Filter, filter_to_json};

/// Relay message from Nostr wire format: ["EVENT", sub_id, event], ["EOSE", sub_id], ["NOTICE", msg], ["OK", id, ok, msg].
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
) {
    let conn = match WebSocketClient::connect(&relay_url).await {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(format!(
                "Failed to connect to {}: {}",
                relay_url, e
            )));
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

    let mut conn = conn;
    if conn.send_text(req_message.as_bytes()).await.is_err() {
        return;
    }

    let mut handler = NostrRelayHandler {
        tx: tx.clone(),
        should_stop: false,
        exit_on_eose: true,
        filter_kind_dm: None,
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
) {
    let conn = match WebSocketClient::connect(&relay_url).await {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(format!(
                "DM stream: failed to connect to {}: {}",
                relay_url, e
            )));
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

    let mut handler = NostrRelayHandler {
        tx,
        should_stop: false,
        exit_on_eose: false,
        filter_kind_dm: Some(super::types::KIND_DM),
    };
    let _ = conn.run(&mut handler).await;
}

/// WebSocket handler for Nostr relay: parses each text frame as JSON and sends StreamMessage to tx.
struct NostrRelayHandler {
    tx: mpsc::UnboundedSender<StreamMessage>,
    should_stop: bool,
    exit_on_eose: bool,
    filter_kind_dm: Option<u32>,
}

impl WebSocketHandler for NostrRelayHandler {
    fn connected(&mut self) {}

    fn text_frame(&mut self, data: &[u8]) {
        let text = match std::str::from_utf8(data) {
            Ok(t) => t,
            Err(_) => return,
        };
        match parse_relay_message(text) {
            Ok(RelayMessage::Event { event, .. }) => {
                let send = match self.filter_kind_dm {
                    Some(kind) => event.kind == kind,
                    None => true,
                };
                if send && self.tx.send(StreamMessage::Event(event)).is_err() {
                    self.should_stop = true;
                }
            }
            Ok(RelayMessage::EndOfStoredEvents { .. }) => {
                if self.exit_on_eose {
                    self.should_stop = true;
                }
            }
            Ok(RelayMessage::Notice { message }) => {
                let _ = self.tx.send(StreamMessage::Notice(message));
            }
            Ok(_) => {}
            Err(_) => {}
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
}
