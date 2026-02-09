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
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Async, non-blocking relay connection: tokio-tungstenite WebSocket + Actson push parser.
//!
//! We feed each WebSocket text frame into the JSON parser and emit semantic events (RelayMessage,
//! StreamMessage) as soon as we have complete values—no need to wait for the end of the frame.
//! When the connection pauses mid-frame, a future streaming feeder (e.g. a jsonparser-style
//! push feeder) can feed chunks as they arrive and we still emit events for complete tokens
//! and complete messages. Currently each WSS message is one complete JSON array, so we use
//! SliceJsonFeeder; the same event-pulling loop works with a chunked feeder returning
//! NeedMoreInput until more bytes are pushed.

use actson::feeder::SliceJsonFeeder;
use actson::{JsonEvent, JsonParser};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use url::Url;

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

/// Parse a single relay message using Actson (push feeder, pull events).
/// Each complete message is one WebSocket frame; we feed its bytes and pull events to build RelayMessage.
/// This way we parse whatever has been delivered so far and emit semantic events in real time;
/// when using a streaming feeder (future: jsonparser-style), partial frames can be fed and we
/// emit when we have a complete top-level array.
pub fn parse_relay_message_actson(message: &str) -> Result<RelayMessage, String> {
    let bytes = message.as_bytes();
    let feeder = SliceJsonFeeder::new(bytes);
    let mut parser = JsonParser::new(feeder);

    let mut depth: i32 = 0;
    let mut top_level_index: i32 = -1;
    let mut msg_type: Option<String> = None;
    let mut second_str: Option<String> = None;
    let mut sub_id: Option<String> = None;
    let mut ok_event_id: Option<String> = None;
    let mut ok_success = false;
    let mut ok_message: Option<String> = None;
    let mut current_field: Option<String> = None;
    let mut event_id: Option<String> = None;
    let mut event_pubkey: Option<String> = None;
    let mut event_created_at: u64 = 0;
    let mut event_kind: u32 = 0;
    let mut event_content = String::new();
    let mut event_sig: Option<String> = None;
    let mut event_tags: Vec<Vec<String>> = Vec::new();
    let mut current_tag: Vec<String> = Vec::new();
    let mut tags_depth: i32 = 0; // 0 = not in tags, 1 = in tags array, 2 = in one tag array

    loop {
        let event = match parser.next_event() {
            Ok(Some(ev)) => ev,
            Ok(None) => break,
            Err(e) => return Err(format!("Relay message parse error: {}", e)),
        };

        match event {
            JsonEvent::NeedMoreInput => break,
            JsonEvent::StartArray => {
                depth += 1;
                if depth == 1 {
                    top_level_index = 0;
                } else if tags_depth == 1 {
                    tags_depth = 2;
                } else if tags_depth == 2 {
                    current_tag.clear();
                }
            }
            JsonEvent::EndArray => {
                if tags_depth == 2 && depth == 4 {
                    if !current_tag.is_empty() {
                        event_tags.push(current_tag.clone());
                    }
                    current_tag.clear();
                } else if tags_depth == 2 && depth == 3 {
                    tags_depth = 0;
                }
                depth -= 1;
            }
            JsonEvent::StartObject => {
                depth += 1;
                if depth == 1 {
                    top_level_index += 1;
                }
                if depth == 2 && msg_type.as_deref() == Some("EVENT") {
                    current_field = None;
                    event_id = None;
                    event_pubkey = None;
                    event_created_at = 0;
                    event_kind = 0;
                    event_content.clear();
                    event_sig = None;
                    event_tags.clear();
                }
            }
            JsonEvent::EndObject => {
                depth -= 1;
                if depth == 1 && msg_type.as_deref() == Some("EVENT") && second_str.is_some() {
                    let sub_id_owned = sub_id.clone().unwrap_or_default();
                    let ev = Event {
                        id: event_id.unwrap_or_default(),
                        pubkey: event_pubkey.unwrap_or_default(),
                        created_at: event_created_at,
                        kind: event_kind,
                        tags: event_tags.clone(),
                        content: event_content.clone(),
                        sig: event_sig.unwrap_or_default(),
                    };
                    return Ok(RelayMessage::Event {
                        _subscription_id: sub_id_owned,
                        event: ev,
                    });
                }
            }
            JsonEvent::FieldName => {
                if let Ok(s) = parser.current_str() {
                    current_field = Some(s.to_string());
                    if depth == 2 && s == "tags" {
                        tags_depth = 1;
                    }
                }
            }
            JsonEvent::ValueString => {
                let s = parser.current_str().map(|x| x.to_string()).unwrap_or_default();
                if depth == 1 {
                    top_level_index += 1;
                    if top_level_index == 1 {
                        msg_type = Some(s);
                    } else if top_level_index == 2 {
                        second_str = Some(s.clone());
                        sub_id = Some(s.clone());
                        ok_event_id = Some(s);
                    } else if top_level_index == 4 && msg_type.as_deref() == Some("OK") {
                        ok_message = Some(s);
                    }
                } else if tags_depth == 2 {
                    current_tag.push(s);
                } else if depth >= 2 && tags_depth == 0 {
                    if let Some(ref f) = current_field {
                        match f.as_str() {
                            "id" => event_id = Some(s),
                            "pubkey" => event_pubkey = Some(s),
                            "content" => event_content = s,
                            "sig" => event_sig = Some(s),
                            _ => {}
                        }
                    }
                }
            }
            JsonEvent::ValueInt => {
                if depth == 2 {
                    if let Some(ref f) = current_field {
                        if f == "created_at" {
                            if let Ok(n) = parser.current_int::<i64>() {
                                event_created_at = n.max(0) as u64;
                            }
                        } else if f == "kind" {
                            if let Ok(n) = parser.current_int::<i32>() {
                                event_kind = n.max(0) as u32;
                            }
                        }
                    }
                }
            }
            JsonEvent::ValueFloat => {
                if depth == 2 {
                    if let Some(ref f) = current_field {
                        if f == "created_at" {
                            if let Ok(n) = parser.current_float() {
                                event_created_at = n.max(0.0) as u64;
                            }
                        } else if f == "kind" {
                            if let Ok(n) = parser.current_float() {
                                event_kind = n.max(0.0) as u32;
                            }
                        }
                    }
                }
            }
            JsonEvent::ValueTrue | JsonEvent::ValueFalse => {
                if depth == 1 {
                    top_level_index += 1;
                    if top_level_index == 3 && msg_type.as_deref() == Some("OK") {
                        ok_success = matches!(event, JsonEvent::ValueTrue);
                    }
                }
            }
            JsonEvent::ValueNull => {}
        }
    }

    match msg_type.as_deref() {
        Some("EOSE") => Ok(RelayMessage::EndOfStoredEvents {
            _subscription_id: second_str.unwrap_or_default(),
        }),
        Some("NOTICE") => Ok(RelayMessage::Notice {
            message: second_str.unwrap_or_else(|| "Unknown notice".to_string()),
        }),
        Some("OK") => Ok(RelayMessage::Ok {
            event_id: ok_event_id.unwrap_or_default(),
            success: ok_success,
            message: ok_message.unwrap_or_default(),
        }),
        _ => Ok(RelayMessage::Unknown {
            _raw: message.to_string(),
        }),
    }
}

/// Run one relay's feed stream over tokio-tungstenite. Each WebSocket text message is fed into
/// the Actson parser and turned into semantic events; events (and EOSE) are sent to `tx`.
pub async fn run_relay_feed_stream(
    relay_url: String,
    filter: Filter,
    timeout_seconds: u32,
    tx: mpsc::UnboundedSender<StreamMessage>,
) {
    let url = match Url::parse(&relay_url) {
        Ok(u) => u,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(format!(
                "Invalid URL {}: {}",
                relay_url, e
            )));
            return;
        }
    };

    let ws_stream = match tokio_tungstenite::connect_async(&url).await {
        Ok((s, _)) => s,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(format!(
                "Failed to connect to {}: {}",
                relay_url, e
            )));
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();
    let subscription_id = format!(
        "tc_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let filter_json = filter_to_json(&filter);
    let req_message = format!("[\"REQ\",\"{}\",{}]", subscription_id, filter_json);

    if write.send(WsMessage::Text(req_message)).await.is_err() {
        return;
    }

    let deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_seconds as u64);

    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        let timeout =
            tokio::time::timeout(tokio::time::Duration::from_secs(1), read.next());
        match timeout.await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                match parse_relay_message_actson(&text) {
                    Ok(RelayMessage::Event { event, .. }) => {
                        if tx.send(StreamMessage::Event(event)).is_err() {
                            break;
                        }
                    }
                    Ok(RelayMessage::EndOfStoredEvents { .. }) => break,
                    Ok(RelayMessage::Notice { message }) => {
                        let _ = tx.send(StreamMessage::Notice(message));
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            Ok(Some(Ok(WsMessage::Close(_)))) | Ok(Some(Err(_))) => break,
            Ok(Some(Ok(_))) => {}
            Ok(None) => break,
            Err(_) => {}
        }
    }

    let _ = tx.send(StreamMessage::Eose);
}

/// Run a long-lived DM subscription (kind 4) with two filters (received + sent). Does not exit on EOSE.
pub async fn run_relay_dm_stream(
    relay_url: String,
    filter_received: Filter,
    filter_sent: Filter,
    tx: mpsc::UnboundedSender<StreamMessage>,
) {
    let url = match Url::parse(&relay_url) {
        Ok(u) => u,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(format!(
                "Invalid URL {}: {}",
                relay_url, e
            )));
            return;
        }
    };

    let ws_stream = match tokio_tungstenite::connect_async(&url).await {
        Ok((s, _)) => s,
        Err(e) => {
            let _ = tx.send(StreamMessage::Notice(format!(
                "DM stream: failed to connect to {}: {}",
                relay_url, e
            )));
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();
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

    if write.send(WsMessage::Text(req_message)).await.is_err() {
        return;
    }

    loop {
        let timeout =
            tokio::time::timeout(tokio::time::Duration::from_secs(60), read.next());
        match timeout.await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                match parse_relay_message_actson(&text) {
                    Ok(RelayMessage::Event { event, .. }) => {
                        if event.kind == super::types::KIND_DM {
                            if tx.send(StreamMessage::Event(event)).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(RelayMessage::EndOfStoredEvents { .. }) => {}
                    Ok(RelayMessage::Notice { message }) => {
                        let _ = tx.send(StreamMessage::Notice(message));
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            Ok(Some(Ok(WsMessage::Close(_)))) | Ok(Some(Err(_))) => break,
            Ok(Some(Ok(_))) => {}
            Ok(None) => break,
            Err(_) => {}
        }
    }
}
