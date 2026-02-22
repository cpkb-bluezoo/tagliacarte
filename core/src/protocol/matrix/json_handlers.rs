/*
 * json_handlers.rs
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

//! JsonContentHandler implementations for Matrix Client-Server API responses.
//!
//! Each handler is a state machine that tracks depth and keys, extracting needed
//! fields and delivering results via callbacks or shared output. No DOM, no serde.

use std::sync::{Arc, Mutex};

use crate::json::{JsonContentHandler, JsonNumber};

use super::types::{
    DeviceKeysResponse, KeyClaimResult, KeyQueryResult,
    KeyUploadCounts, LoginResponse, Profile, RoomEvent, RoomSummary, WellKnown,
    EVENT_ROOM_AVATAR, EVENT_ROOM_ENCRYPTED, EVENT_ROOM_MESSAGE, EVENT_ROOM_NAME, EVENT_ROOM_TOPIC,
};
use std::collections::HashMap;

// ── MatrixErrorHandler ───────────────────────────────────────────────

/// Parses `{"errcode": "...", "error": "..."}`.
pub struct MatrixErrorHandler {
    current_key: Option<String>,
    pub errcode: String,
    pub error: String,
}

impl MatrixErrorHandler {
    pub fn new() -> Self {
        Self {
            current_key: None,
            errcode: String::new(),
            error: String::new(),
        }
    }
}

impl JsonContentHandler for MatrixErrorHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        match self.current_key.as_deref() {
            Some("errcode") => self.errcode = value.to_string(),
            Some("error") => self.error = value.to_string(),
            _ => {}
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── LoginResponseHandler ─────────────────────────────────────────────

/// Parses `{"access_token": "...", "user_id": "...", "device_id": "..."}`.
pub struct LoginResponseHandler {
    current_key: Option<String>,
    access_token: Option<String>,
    user_id: Option<String>,
    device_id: Option<String>,
    out: Arc<Mutex<Option<LoginResponse>>>,
}

impl LoginResponseHandler {
    pub fn new(out: Arc<Mutex<Option<LoginResponse>>>) -> Self {
        Self {
            current_key: None,
            access_token: None,
            user_id: None,
            device_id: None,
            out,
        }
    }
}

impl JsonContentHandler for LoginResponseHandler {
    fn start_object(&mut self) {}

    fn end_object(&mut self) {
        if let (Some(token), Some(user)) = (self.access_token.take(), self.user_id.take()) {
            let resp = LoginResponse {
                access_token: token,
                user_id: user,
                device_id: self.device_id.take().unwrap_or_default(),
            };
            if let Ok(mut o) = self.out.lock() {
                *o = Some(resp);
            }
        }
    }

    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        match self.current_key.as_deref() {
            Some("access_token") => self.access_token = Some(value.to_string()),
            Some("user_id") => self.user_id = Some(value.to_string()),
            Some("device_id") => self.device_id = Some(value.to_string()),
            _ => {}
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── ProfileHandler ───────────────────────────────────────────────────

/// Parses `{"displayname": "...", "avatar_url": "mxc://..."}`.
pub struct ProfileHandler {
    current_key: Option<String>,
    out: Arc<Mutex<Profile>>,
}

impl ProfileHandler {
    pub fn new(out: Arc<Mutex<Profile>>) -> Self {
        Self { current_key: None, out }
    }
}

impl JsonContentHandler for ProfileHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if let Ok(mut p) = self.out.lock() {
            match self.current_key.as_deref() {
                Some("displayname") => p.displayname = Some(value.to_string()),
                Some("avatar_url") => p.avatar_url = Some(value.to_string()),
                _ => {}
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── WellKnownHandler ─────────────────────────────────────────────────

/// Parses `{"m.homeserver": {"base_url": "https://..."}}`.
pub struct WellKnownHandler {
    depth: usize,
    in_homeserver: bool,
    current_key: Option<String>,
    out: Arc<Mutex<Option<WellKnown>>>,
}

impl WellKnownHandler {
    pub fn new(out: Arc<Mutex<Option<WellKnown>>>) -> Self {
        Self {
            depth: 0,
            in_homeserver: false,
            current_key: None,
            out,
        }
    }
}

impl JsonContentHandler for WellKnownHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.current_key.as_deref() == Some("m.homeserver") {
            self.in_homeserver = true;
        }
    }

    fn end_object(&mut self) {
        if self.depth == 2 {
            self.in_homeserver = false;
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.in_homeserver && self.current_key.as_deref() == Some("base_url") {
            if let Ok(mut o) = self.out.lock() {
                *o = Some(WellKnown {
                    homeserver_base_url: value.trim_end_matches('/').to_string(),
                });
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── JoinedRoomsHandler ───────────────────────────────────────────────

/// Parses `{"joined_rooms": ["!room1:server", "!room2:server", ...]}`.
pub struct JoinedRoomsHandler {
    depth: usize,
    in_array: bool,
    current_key: Option<String>,
    out: Arc<Mutex<Vec<String>>>,
}

impl JoinedRoomsHandler {
    pub fn new(out: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            depth: 0,
            in_array: false,
            current_key: None,
            out,
        }
    }
}

impl JsonContentHandler for JoinedRoomsHandler {
    fn start_object(&mut self) { self.depth += 1; }
    fn end_object(&mut self) { self.depth -= 1; }

    fn start_array(&mut self) {
        if self.depth == 1 && self.current_key.as_deref() == Some("joined_rooms") {
            self.in_array = true;
        }
    }

    fn end_array(&mut self) {
        self.in_array = false;
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.in_array {
            if let Ok(mut v) = self.out.lock() {
                v.push(value.to_string());
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── SyncResponseHandler ──────────────────────────────────────────────

/// Parses a `/sync` response, extracting rooms and timeline events.
///
/// Sync response structure (simplified):
/// ```text
/// {
///   "next_batch": "s123",
///   "rooms": {
///     "join": {
///       "!room_id:server": {
///         "state": { "events": [ {event}, ... ] },
///         "timeline": { "events": [ {event}, ... ], "prev_batch": "..." }
///       }
///     },
///     "invite": {
///       "!room_id:server": { "invite_state": { "events": [...] } }
///     }
///   }
/// }
/// ```
///
/// State machine tracks depth through the nested structure. Callbacks fire
/// as complete rooms and events are recognized.
pub struct SyncResponseHandler {
    on_room: Box<dyn Fn(RoomSummary) + Send>,
    on_event: Box<dyn Fn(RoomEvent) + Send>,
    next_batch: Arc<Mutex<Option<String>>>,

    // E2EE callbacks
    otk_count: Arc<Mutex<Option<usize>>>,
    device_lists_changed: Arc<Mutex<Vec<String>>>,
    on_to_device: Box<dyn Fn(String, String, String) + Send>,

    depth: usize,
    current_key: Option<String>,
    section: SyncSection,
    current_room_id: Option<String>,
    room_state: RoomState,
    in_events_array: bool,
    event_fields: EventFields,
    event_content_depth: usize,
    in_content: bool,
    prev_batch: Option<String>,

    // E2EE state tracking
    in_otk_counts: bool,
    in_device_lists: bool,
    in_device_lists_changed_array: bool,
    in_to_device: bool,
    in_to_device_events: bool,
    to_device_event_type: Option<String>,
    to_device_sender: Option<String>,
    to_device_content_buf: Vec<u8>,
    to_device_content_depth: usize,
    in_to_device_content: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum SyncSection {
    None,
    Join,
    Invite,
}

#[derive(Default)]
struct RoomState {
    name: Option<String>,
    avatar_url: Option<String>,
    topic: Option<String>,
}

#[derive(Default)]
struct EventFields {
    event_id: Option<String>,
    event_type: Option<String>,
    sender: Option<String>,
    origin_server_ts: i64,
    body: Option<String>,
    msgtype: Option<String>,
    url: Option<String>,
    state_key: Option<String>,
    algorithm: Option<String>,
    sender_key: Option<String>,
    session_id: Option<String>,
    ciphertext: Option<String>,
    device_id: Option<String>,
}

impl EventFields {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SyncResponseHandler {
    pub fn new(
        on_room: impl Fn(RoomSummary) + Send + 'static,
        on_event: impl Fn(RoomEvent) + Send + 'static,
        next_batch: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self::with_e2ee(on_room, on_event, next_batch,
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(Vec::new())),
            Box::new(|_, _, _| {}),
        )
    }

    pub fn with_e2ee(
        on_room: impl Fn(RoomSummary) + Send + 'static,
        on_event: impl Fn(RoomEvent) + Send + 'static,
        next_batch: Arc<Mutex<Option<String>>>,
        otk_count: Arc<Mutex<Option<usize>>>,
        device_lists_changed: Arc<Mutex<Vec<String>>>,
        on_to_device: Box<dyn Fn(String, String, String) + Send>,
    ) -> Self {
        Self {
            on_room: Box::new(on_room),
            on_event: Box::new(on_event),
            next_batch,
            otk_count,
            device_lists_changed,
            on_to_device,
            depth: 0,
            current_key: None,
            section: SyncSection::None,
            current_room_id: None,
            room_state: RoomState::default(),
            in_events_array: false,
            event_fields: EventFields::default(),
            event_content_depth: 0,
            in_content: false,
            prev_batch: None,
            in_otk_counts: false,
            in_device_lists: false,
            in_device_lists_changed_array: false,
            in_to_device: false,
            in_to_device_events: false,
            to_device_event_type: None,
            to_device_sender: None,
            to_device_content_buf: Vec::new(),
            to_device_content_depth: 0,
            in_to_device_content: false,
        }
    }

    fn emit_event(&mut self) {
        let room_id = match self.current_room_id.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };
        let event_type = match self.event_fields.event_type.take() {
            Some(t) => t,
            None => return,
        };

        // Update room state from state events
        match event_type.as_str() {
            EVENT_ROOM_NAME => {
                if let Some(ref name) = self.event_fields.body {
                    self.room_state.name = Some(name.clone());
                }
            }
            EVENT_ROOM_AVATAR => {
                if let Some(ref url) = self.event_fields.url {
                    self.room_state.avatar_url = Some(url.clone());
                }
            }
            EVENT_ROOM_TOPIC => {
                if let Some(ref topic) = self.event_fields.body {
                    self.room_state.topic = Some(topic.clone());
                }
            }
            _ => {}
        }

        // Emit timeline message events (both plaintext and encrypted)
        if event_type == EVENT_ROOM_MESSAGE || event_type == EVENT_ROOM_ENCRYPTED {
            if let Some(event_id) = self.event_fields.event_id.take() {
                let event = RoomEvent {
                    event_id,
                    event_type,
                    sender: self.event_fields.sender.take().unwrap_or_default(),
                    origin_server_ts: self.event_fields.origin_server_ts,
                    body: self.event_fields.body.take(),
                    msgtype: self.event_fields.msgtype.take(),
                    url: self.event_fields.url.take(),
                    room_id,
                    algorithm: self.event_fields.algorithm.take(),
                    sender_key: self.event_fields.sender_key.take(),
                    session_id: self.event_fields.session_id.take(),
                    ciphertext: self.event_fields.ciphertext.take(),
                    device_id: self.event_fields.device_id.take(),
                };
                (self.on_event)(event);
            }
        }

        self.event_fields.reset();
    }

    fn emit_room(&mut self) {
        if let Some(room_id) = self.current_room_id.take() {
            let summary = RoomSummary {
                room_id,
                name: self.room_state.name.take(),
                avatar_url: self.room_state.avatar_url.take(),
                topic: self.room_state.topic.take(),
            };
            (self.on_room)(summary);
        }
        self.room_state = RoomState::default();
    }
}

/// Depth guide (for the `join` section):
///   1: root `{`
///   2: `"rooms": {`
///   3: `"join": {`
///   4: `"!room:server": {`
///   5: `"state": {` or `"timeline": {`
///   6: event objects inside `"events": [...]`
///   7: `"content": {` inside an event
impl JsonContentHandler for SyncResponseHandler {
    fn start_object(&mut self) {
        self.depth += 1;

        // E2EE: device_one_time_keys_count at depth 2
        if self.depth == 2 && self.current_key.as_deref() == Some("device_one_time_keys_count") {
            self.in_otk_counts = true;
        }
        // E2EE: device_lists at depth 2
        if self.depth == 2 && self.current_key.as_deref() == Some("device_lists") {
            self.in_device_lists = true;
        }
        // E2EE: to_device at depth 2
        if self.depth == 2 && self.current_key.as_deref() == Some("to_device") {
            self.in_to_device = true;
        }
        // E2EE: to_device event object
        if self.in_to_device_events && self.depth == 3 {
            self.to_device_event_type = None;
            self.to_device_sender = None;
            self.to_device_content_buf.clear();
            self.in_to_device_content = false;
        }
        // E2EE: to_device event content
        if self.in_to_device_events && self.depth == 4 && self.current_key.as_deref() == Some("content") {
            self.in_to_device_content = true;
            self.to_device_content_depth = self.depth;
            self.to_device_content_buf.push(b'{');
        } else if self.in_to_device_content && self.depth > self.to_device_content_depth {
            if self.to_device_content_buf.last() != Some(&b'{')
                && self.to_device_content_buf.last() != Some(&b'[')
                && self.to_device_content_buf.last() != Some(&b':')
            {
                self.to_device_content_buf.push(b',');
            }
            self.to_device_content_buf.push(b'{');
        }

        // Room object at depth 4
        if self.depth == 4 && (self.section == SyncSection::Join || self.section == SyncSection::Invite) {
            self.current_room_id = self.current_key.clone();
            self.room_state = RoomState::default();
        }

        // Event object at depth 6 inside events array
        if self.depth == 6 && self.in_events_array {
            self.event_fields.reset();
        }

        // "content" at depth 7 inside an event
        if self.depth == 7 && self.in_events_array && self.current_key.as_deref() == Some("content") {
            self.in_content = true;
            self.event_content_depth = self.depth;
        }
    }

    fn end_object(&mut self) {
        // E2EE: to_device content
        if self.in_to_device_content {
            if self.depth == self.to_device_content_depth {
                self.to_device_content_buf.push(b'}');
                self.in_to_device_content = false;
            } else if self.depth > self.to_device_content_depth {
                self.to_device_content_buf.push(b'}');
            }
        }
        // E2EE: to_device event complete
        if self.in_to_device_events && self.depth == 3 {
            if let (Some(etype), Some(sender)) =
                (self.to_device_event_type.take(), self.to_device_sender.take())
            {
                let content = String::from_utf8_lossy(&self.to_device_content_buf).to_string();
                (self.on_to_device)(etype, sender, content);
            }
            self.to_device_content_buf.clear();
        }
        // E2EE: end of to_device
        if self.depth == 2 {
            self.in_otk_counts = false;
            self.in_device_lists = false;
            self.in_to_device = false;
        }

        // Leaving "content"
        if self.in_content && self.depth == self.event_content_depth {
            self.in_content = false;
        }

        // End of an event object at depth 6
        if self.depth == 6 && self.in_events_array {
            self.emit_event();
        }

        // End of a room object at depth 4
        if self.depth == 4 && (self.section == SyncSection::Join || self.section == SyncSection::Invite) {
            self.emit_room();
        }

        // Leaving "join" or "invite" at depth 3
        if self.depth == 3 {
            self.section = SyncSection::None;
        }

        self.depth -= 1;
        self.current_key = None;
    }

    fn start_array(&mut self) {
        // "events" array at depth 5 (inside state or timeline)
        if self.depth == 5 && self.current_key.as_deref() == Some("events") {
            self.in_events_array = true;
        }
        // E2EE: device_lists.changed array
        if self.in_device_lists && self.current_key.as_deref() == Some("changed") {
            self.in_device_lists_changed_array = true;
        }
        // E2EE: to_device.events array
        if self.in_to_device && self.current_key.as_deref() == Some("events") {
            self.in_to_device_events = true;
        }
        // Raw content passthrough
        if self.in_to_device_content {
            if self.to_device_content_buf.last() != Some(&b'{')
                && self.to_device_content_buf.last() != Some(&b'[')
                && self.to_device_content_buf.last() != Some(&b':')
            {
                self.to_device_content_buf.push(b',');
            }
            self.to_device_content_buf.push(b'[');
        }
    }

    fn end_array(&mut self) {
        if self.in_events_array && self.depth == 5 {
            self.in_events_array = false;
        }
        if self.in_device_lists_changed_array {
            self.in_device_lists_changed_array = false;
        }
        if self.in_to_device_events && self.in_to_device && self.depth == 2 {
            self.in_to_device_events = false;
        }
        if self.in_to_device_content {
            self.to_device_content_buf.push(b']');
        }
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());

        // Detect section at depth 3 (inside "rooms")
        if self.depth == 3 {
            match key {
                "join" => self.section = SyncSection::Join,
                "invite" => self.section = SyncSection::Invite,
                _ => {}
            }
        }

        // Raw content passthrough for to_device
        if self.in_to_device_content {
            let buf = &mut self.to_device_content_buf;
            if buf.last() != Some(&b'{') && buf.last() != Some(&b'[') {
                buf.push(b',');
            }
            buf.push(b'"');
            buf.extend_from_slice(key.as_bytes());
            buf.push(b'"');
            buf.push(b':');
        }
    }

    fn string_value(&mut self, value: &str) {
        // next_batch at root level
        if self.depth == 1 && self.current_key.as_deref() == Some("next_batch") {
            if let Ok(mut nb) = self.next_batch.lock() {
                *nb = Some(value.to_string());
            }
        }

        // E2EE: device_lists.changed
        if self.in_device_lists_changed_array {
            if let Ok(mut dl) = self.device_lists_changed.lock() {
                dl.push(value.to_string());
            }
        }

        // E2EE: to_device event fields (outside content)
        if self.in_to_device_events && self.depth == 3 && !self.in_to_device_content {
            match self.current_key.as_deref() {
                Some("type") => self.to_device_event_type = Some(value.to_string()),
                Some("sender") => self.to_device_sender = Some(value.to_string()),
                _ => {}
            }
        }

        // Raw content passthrough for to_device
        if self.in_to_device_content {
            let buf = &mut self.to_device_content_buf;
            if buf.last() != Some(&b':') && buf.last() != Some(&b'[') && buf.last() != Some(&b'{') {
                buf.push(b',');
            }
            buf.push(b'"');
            for &b in value.as_bytes() {
                if b == b'"' { buf.extend_from_slice(b"\\\""); }
                else if b == b'\\' { buf.extend_from_slice(b"\\\\"); }
                else { buf.push(b); }
            }
            buf.push(b'"');
            self.current_key = None;
            return;
        }

        // prev_batch inside timeline
        if self.depth == 5 && self.current_key.as_deref() == Some("prev_batch") {
            self.prev_batch = Some(value.to_string());
        }

        // Event fields at depth 6
        if self.depth == 6 && self.in_events_array && !self.in_content {
            match self.current_key.as_deref() {
                Some("event_id") => self.event_fields.event_id = Some(value.to_string()),
                Some("type") => self.event_fields.event_type = Some(value.to_string()),
                Some("sender") => self.event_fields.sender = Some(value.to_string()),
                Some("state_key") => self.event_fields.state_key = Some(value.to_string()),
                _ => {}
            }
        }

        // Content fields at depth 7
        if self.in_content {
            match self.current_key.as_deref() {
                Some("body") | Some("name") => self.event_fields.body = Some(value.to_string()),
                Some("msgtype") => self.event_fields.msgtype = Some(value.to_string()),
                Some("url") => self.event_fields.url = Some(value.to_string()),
                Some("topic") => self.event_fields.body = Some(value.to_string()),
                Some("algorithm") => self.event_fields.algorithm = Some(value.to_string()),
                Some("sender_key") => self.event_fields.sender_key = Some(value.to_string()),
                Some("session_id") => self.event_fields.session_id = Some(value.to_string()),
                Some("ciphertext") => self.event_fields.ciphertext = Some(value.to_string()),
                Some("device_id") => self.event_fields.device_id = Some(value.to_string()),
                _ => {}
            }
        }

        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        // E2EE: device_one_time_keys_count.signed_curve25519
        if self.in_otk_counts && self.current_key.as_deref() == Some("signed_curve25519") {
            if let Ok(mut c) = self.otk_count.lock() {
                *c = Some(number.as_i64().unwrap_or(0) as usize);
            }
        }

        // Raw content passthrough for to_device
        if self.in_to_device_content {
            let buf = &mut self.to_device_content_buf;
            if buf.last() != Some(&b':') && buf.last() != Some(&b'[') && buf.last() != Some(&b'{') {
                buf.push(b',');
            }
            match number {
                JsonNumber::I64(n) => buf.extend_from_slice(format!("{}", n).as_bytes()),
                JsonNumber::F64(f) => buf.extend_from_slice(format!("{}", f).as_bytes()),
            }
            self.current_key = None;
            return;
        }

        if self.depth == 6 && self.in_events_array && !self.in_content {
            if self.current_key.as_deref() == Some("origin_server_ts") {
                self.event_fields.origin_server_ts = number.as_i64().unwrap_or(0);
            }
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, v: bool) {
        if self.in_to_device_content {
            let buf = &mut self.to_device_content_buf;
            if buf.last() != Some(&b':') && buf.last() != Some(&b'[') && buf.last() != Some(&b'{') {
                buf.push(b',');
            }
            buf.extend_from_slice(if v { b"true" } else { b"false" });
        }
        self.current_key = None;
    }

    fn null_value(&mut self) {
        if self.in_to_device_content {
            let buf = &mut self.to_device_content_buf;
            if buf.last() != Some(&b':') && buf.last() != Some(&b'[') && buf.last() != Some(&b'{') {
                buf.push(b',');
            }
            buf.extend_from_slice(b"null");
        }
        self.current_key = None;
    }
}

// ── RoomMessagesHandler ──────────────────────────────────────────────

/// Parses a `/rooms/{id}/messages` response:
/// `{"start": "...", "end": "...", "chunk": [{event}, ...]}`.
pub struct RoomMessagesHandler {
    on_event: Box<dyn Fn(RoomEvent) + Send>,
    room_id: String,
    end_token: Arc<Mutex<Option<String>>>,

    depth: usize,
    in_chunk: bool,
    in_content: bool,
    current_key: Option<String>,
    event_fields: EventFields,
}

impl RoomMessagesHandler {
    pub fn new(
        room_id: String,
        on_event: impl Fn(RoomEvent) + Send + 'static,
        end_token: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            on_event: Box::new(on_event),
            room_id,
            end_token,
            depth: 0,
            in_chunk: false,
            in_content: false,
            current_key: None,
            event_fields: EventFields::default(),
        }
    }

    fn emit_event(&mut self) {
        if let (Some(event_id), Some(event_type)) =
            (self.event_fields.event_id.take(), self.event_fields.event_type.take())
        {
            let event = RoomEvent {
                event_id,
                event_type,
                sender: self.event_fields.sender.take().unwrap_or_default(),
                origin_server_ts: self.event_fields.origin_server_ts,
                body: self.event_fields.body.take(),
                msgtype: self.event_fields.msgtype.take(),
                url: self.event_fields.url.take(),
                room_id: self.room_id.clone(),
                algorithm: self.event_fields.algorithm.take(),
                sender_key: self.event_fields.sender_key.take(),
                session_id: self.event_fields.session_id.take(),
                ciphertext: self.event_fields.ciphertext.take(),
                device_id: self.event_fields.device_id.take(),
            };
            (self.on_event)(event);
        }
        self.event_fields.reset();
    }
}

impl JsonContentHandler for RoomMessagesHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.in_chunk && self.depth == 2 {
            self.event_fields.reset();
        }
        if self.in_chunk && self.depth == 3 && self.current_key.as_deref() == Some("content") {
            self.in_content = true;
        }
    }

    fn end_object(&mut self) {
        if self.in_content && self.depth == 3 {
            self.in_content = false;
        }
        if self.in_chunk && self.depth == 2 {
            self.emit_event();
        }
        self.depth -= 1;
        self.current_key = None;
    }

    fn start_array(&mut self) {
        if self.depth == 1 && self.current_key.as_deref() == Some("chunk") {
            self.in_chunk = true;
        }
    }

    fn end_array(&mut self) {
        if self.in_chunk && self.depth == 1 {
            self.in_chunk = false;
        }
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        // Pagination token
        if self.depth == 1 && self.current_key.as_deref() == Some("end") {
            if let Ok(mut et) = self.end_token.lock() {
                *et = Some(value.to_string());
            }
        }

        if self.in_chunk && self.depth == 2 && !self.in_content {
            match self.current_key.as_deref() {
                Some("event_id") => self.event_fields.event_id = Some(value.to_string()),
                Some("type") => self.event_fields.event_type = Some(value.to_string()),
                Some("sender") => self.event_fields.sender = Some(value.to_string()),
                _ => {}
            }
        }

        if self.in_content {
            match self.current_key.as_deref() {
                Some("body") => self.event_fields.body = Some(value.to_string()),
                Some("msgtype") => self.event_fields.msgtype = Some(value.to_string()),
                Some("url") => self.event_fields.url = Some(value.to_string()),
                Some("algorithm") => self.event_fields.algorithm = Some(value.to_string()),
                Some("sender_key") => self.event_fields.sender_key = Some(value.to_string()),
                Some("session_id") => self.event_fields.session_id = Some(value.to_string()),
                Some("ciphertext") => self.event_fields.ciphertext = Some(value.to_string()),
                Some("device_id") => self.event_fields.device_id = Some(value.to_string()),
                _ => {}
            }
        }

        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.in_chunk && self.depth == 2 && !self.in_content {
            if self.current_key.as_deref() == Some("origin_server_ts") {
                self.event_fields.origin_server_ts = number.as_i64().unwrap_or(0);
            }
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── SingleEventHandler ───────────────────────────────────────────────

/// Parses a single event from `/rooms/{id}/event/{eventId}`.
pub struct SingleEventHandler {
    room_id: String,
    depth: usize,
    in_content: bool,
    current_key: Option<String>,
    event_fields: EventFields,
    out: Arc<Mutex<Option<RoomEvent>>>,
}

impl SingleEventHandler {
    pub fn new(room_id: String, out: Arc<Mutex<Option<RoomEvent>>>) -> Self {
        Self {
            room_id,
            depth: 0,
            in_content: false,
            current_key: None,
            event_fields: EventFields::default(),
            out,
        }
    }
}

impl JsonContentHandler for SingleEventHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.current_key.as_deref() == Some("content") {
            self.in_content = true;
        }
    }

    fn end_object(&mut self) {
        if self.in_content && self.depth == 2 {
            self.in_content = false;
        }
        if self.depth == 1 {
            if let (Some(event_id), Some(event_type)) =
                (self.event_fields.event_id.take(), self.event_fields.event_type.take())
            {
                let event = RoomEvent {
                    event_id,
                    event_type,
                    sender: self.event_fields.sender.take().unwrap_or_default(),
                    origin_server_ts: self.event_fields.origin_server_ts,
                    body: self.event_fields.body.take(),
                    msgtype: self.event_fields.msgtype.take(),
                    url: self.event_fields.url.take(),
                    room_id: self.room_id.clone(),
                    algorithm: self.event_fields.algorithm.take(),
                    sender_key: self.event_fields.sender_key.take(),
                    session_id: self.event_fields.session_id.take(),
                    ciphertext: self.event_fields.ciphertext.take(),
                    device_id: self.event_fields.device_id.take(),
                };
                if let Ok(mut o) = self.out.lock() {
                    *o = Some(event);
                }
            }
        }
        self.depth -= 1;
        self.current_key = None;
    }

    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.depth == 1 && !self.in_content {
            match self.current_key.as_deref() {
                Some("event_id") => self.event_fields.event_id = Some(value.to_string()),
                Some("type") => self.event_fields.event_type = Some(value.to_string()),
                Some("sender") => self.event_fields.sender = Some(value.to_string()),
                _ => {}
            }
        }

        if self.in_content {
            match self.current_key.as_deref() {
                Some("body") => self.event_fields.body = Some(value.to_string()),
                Some("msgtype") => self.event_fields.msgtype = Some(value.to_string()),
                Some("url") => self.event_fields.url = Some(value.to_string()),
                Some("algorithm") => self.event_fields.algorithm = Some(value.to_string()),
                Some("sender_key") => self.event_fields.sender_key = Some(value.to_string()),
                Some("session_id") => self.event_fields.session_id = Some(value.to_string()),
                Some("ciphertext") => self.event_fields.ciphertext = Some(value.to_string()),
                Some("device_id") => self.event_fields.device_id = Some(value.to_string()),
                _ => {}
            }
        }

        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.depth == 1 && !self.in_content {
            if self.current_key.as_deref() == Some("origin_server_ts") {
                self.event_fields.origin_server_ts = number.as_i64().unwrap_or(0);
            }
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── NoOpHandler ──────────────────────────────────────────────────────

/// No-op handler for endpoints where only the HTTP status matters.
pub struct NoOpHandler;

impl JsonContentHandler for NoOpHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, _: &str) {}
    fn string_value(&mut self, _: &str) {}
    fn number_value(&mut self, _: JsonNumber) {}
    fn boolean_value(&mut self, _: bool) {}
    fn null_value(&mut self) {}
}

// ── MediaUploadHandler ───────────────────────────────────────────────

/// Parses `{"content_uri": "mxc://..."}` from media upload response.
pub struct MediaUploadHandler {
    current_key: Option<String>,
    out: Arc<Mutex<Option<String>>>,
}

impl MediaUploadHandler {
    pub fn new(out: Arc<Mutex<Option<String>>>) -> Self {
        Self { current_key: None, out }
    }
}

impl JsonContentHandler for MediaUploadHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.current_key.as_deref() == Some("content_uri") {
            if let Ok(mut o) = self.out.lock() {
                *o = Some(value.to_string());
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ══ E2EE handlers ════════════════════════════════════════════════════

// ── KeyUploadResponseHandler ─────────────────────────────────────────

pub struct KeyUploadResponseHandler {
    depth: usize,
    in_counts: bool,
    current_key: Option<String>,
    out: Arc<Mutex<KeyUploadCounts>>,
}

impl KeyUploadResponseHandler {
    pub fn new(out: Arc<Mutex<KeyUploadCounts>>) -> Self {
        Self { depth: 0, in_counts: false, current_key: None, out }
    }
}

impl JsonContentHandler for KeyUploadResponseHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.current_key.as_deref() == Some("one_time_key_counts") {
            self.in_counts = true;
        }
    }
    fn end_object(&mut self) {
        if self.depth == 2 { self.in_counts = false; }
        self.depth -= 1;
    }
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, key: &str) { self.current_key = Some(key.to_string()); }
    fn string_value(&mut self, _: &str) { self.current_key = None; }
    fn number_value(&mut self, number: JsonNumber) {
        if self.in_counts && self.current_key.as_deref() == Some("signed_curve25519") {
            if let Ok(mut out) = self.out.lock() {
                out.signed_curve25519 = number.as_i64().unwrap_or(0) as usize;
            }
        }
        self.current_key = None;
    }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── KeyQueryResponseHandler ──────────────────────────────────────────

pub struct KeyQueryResponseHandler {
    depth: usize,
    section: KqSection,
    current_key: Option<String>,
    current_user_id: Option<String>,
    current_device: Option<DeviceKeysResponse>,
    in_keys: bool,
    in_signatures: bool,
    sig_user: Option<String>,
    out: Arc<Mutex<KeyQueryResult>>,
}

#[derive(Clone, Copy, PartialEq)]
enum KqSection { None, DeviceKeys }

impl KeyQueryResponseHandler {
    pub fn new(out: Arc<Mutex<KeyQueryResult>>) -> Self {
        Self {
            depth: 0, section: KqSection::None, current_key: None,
            current_user_id: None, current_device: None,
            in_keys: false, in_signatures: false, sig_user: None, out,
        }
    }
}

impl JsonContentHandler for KeyQueryResponseHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.current_key.as_deref() == Some("device_keys") {
            self.section = KqSection::DeviceKeys;
        }
        if self.depth == 3 && self.section == KqSection::DeviceKeys {
            self.current_user_id = self.current_key.clone();
        }
        if self.depth == 4 && self.section == KqSection::DeviceKeys && self.current_user_id.is_some() {
            self.current_device = Some(DeviceKeysResponse {
                user_id: self.current_user_id.clone().unwrap_or_default(),
                device_id: self.current_key.clone().unwrap_or_default(),
                algorithms: Vec::new(),
                ed25519_key: None,
                curve25519_key: None,
                signatures: HashMap::new(),
            });
        }
        if self.depth == 5 && self.current_device.is_some() {
            match self.current_key.as_deref() {
                Some("keys") => self.in_keys = true,
                Some("signatures") => self.in_signatures = true,
                _ => {}
            }
        }
        if self.depth == 6 && self.in_signatures {
            self.sig_user = self.current_key.clone();
        }
    }

    fn end_object(&mut self) {
        if self.depth == 6 && self.in_signatures { self.sig_user = None; }
        if self.depth == 5 { self.in_keys = false; self.in_signatures = false; }
        if self.depth == 4 && self.section == KqSection::DeviceKeys {
            if let Some(device) = self.current_device.take() {
                if let Ok(mut out) = self.out.lock() {
                    out.device_keys.entry(device.user_id.clone()).or_default()
                        .insert(device.device_id.clone(), device);
                }
            }
        }
        if self.depth == 3 { self.current_user_id = None; }
        if self.depth == 2 { self.section = KqSection::None; }
        self.depth -= 1;
        self.current_key = None;
    }

    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, key: &str) { self.current_key = Some(key.to_string()); }

    fn string_value(&mut self, value: &str) {
        if let Some(ref mut device) = self.current_device {
            if self.in_keys {
                if let Some(ref k) = self.current_key {
                    if k.starts_with("ed25519:") {
                        device.ed25519_key = Some(value.to_string());
                    } else if k.starts_with("curve25519:") {
                        device.curve25519_key = Some(value.to_string());
                    }
                }
            }
            if self.in_signatures {
                if let (Some(ref user), Some(ref k)) = (&self.sig_user, &self.current_key) {
                    device.signatures.entry(user.clone()).or_default()
                        .insert(k.clone(), value.to_string());
                }
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── KeyClaimResponseHandler ──────────────────────────────────────────

pub struct KeyClaimResponseHandler {
    depth: usize,
    in_otk: bool,
    current_key: Option<String>,
    current_user: Option<String>,
    current_device: Option<String>,
    current_key_id: Option<String>,
    out: Arc<Mutex<KeyClaimResult>>,
}

impl KeyClaimResponseHandler {
    pub fn new(out: Arc<Mutex<KeyClaimResult>>) -> Self {
        Self {
            depth: 0, in_otk: false, current_key: None,
            current_user: None, current_device: None, current_key_id: None, out,
        }
    }
}

impl JsonContentHandler for KeyClaimResponseHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.current_key.as_deref() == Some("one_time_keys") { self.in_otk = true; }
        if self.in_otk && self.depth == 3 { self.current_user = self.current_key.clone(); }
        if self.in_otk && self.depth == 4 { self.current_device = self.current_key.clone(); }
        if self.in_otk && self.depth == 5 { self.current_key_id = self.current_key.clone(); }
    }
    fn end_object(&mut self) {
        if self.depth == 5 { self.current_key_id = None; }
        if self.depth == 4 { self.current_device = None; }
        if self.depth == 3 { self.current_user = None; }
        if self.depth == 2 { self.in_otk = false; }
        self.depth -= 1;
        self.current_key = None;
    }
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, key: &str) { self.current_key = Some(key.to_string()); }
    fn string_value(&mut self, value: &str) {
        if self.in_otk && self.depth == 5 && self.current_key.as_deref() == Some("key") {
            if let (Some(ref user), Some(ref device), Some(ref key_id)) =
                (&self.current_user, &self.current_device, &self.current_key_id)
            {
                if let Ok(mut out) = self.out.lock() {
                    out.one_time_keys.entry(user.clone()).or_default()
                        .insert(device.clone(), (key_id.clone(), value.to_string()));
                }
            }
        }
        self.current_key = None;
    }
    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

// ── RawBodyHandler ───────────────────────────────────────────────────

pub struct RawBodyHandler {
    buf: Vec<u8>,
    out: Arc<Mutex<Option<Vec<u8>>>>,
    depth: usize,
}

impl RawBodyHandler {
    pub fn new(out: Arc<Mutex<Option<Vec<u8>>>>) -> Self {
        Self { buf: Vec::new(), out, depth: 0 }
    }

    fn need_comma(&self) -> bool {
        match self.buf.last() {
            Some(&b'{') | Some(&b'[') | Some(&b':') | None => false,
            _ => true,
        }
    }

    fn write_json_str(&mut self, s: &str) {
        self.buf.push(b'"');
        for ch in s.bytes() {
            match ch {
                b'"' => self.buf.extend_from_slice(b"\\\""),
                b'\\' => self.buf.extend_from_slice(b"\\\\"),
                c => self.buf.push(c),
            }
        }
        self.buf.push(b'"');
    }
}

impl JsonContentHandler for RawBodyHandler {
    fn start_object(&mut self) {
        if self.need_comma() { self.buf.push(b','); }
        self.buf.push(b'{');
        self.depth += 1;
    }
    fn end_object(&mut self) {
        self.buf.push(b'}');
        self.depth -= 1;
        if self.depth == 0 {
            if let Ok(mut o) = self.out.lock() {
                *o = Some(std::mem::take(&mut self.buf));
            }
        }
    }
    fn start_array(&mut self) {
        if self.need_comma() { self.buf.push(b','); }
        self.buf.push(b'[');
    }
    fn end_array(&mut self) { self.buf.push(b']'); }
    fn key(&mut self, key: &str) {
        if self.need_comma() { self.buf.push(b','); }
        self.write_json_str(key);
        self.buf.push(b':');
    }
    fn string_value(&mut self, value: &str) {
        if self.need_comma() { self.buf.push(b','); }
        self.write_json_str(value);
    }
    fn number_value(&mut self, number: JsonNumber) {
        if self.need_comma() { self.buf.push(b','); }
        match number {
            JsonNumber::I64(n) => self.buf.extend_from_slice(format!("{}", n).as_bytes()),
            JsonNumber::F64(f) => self.buf.extend_from_slice(format!("{}", f).as_bytes()),
        }
    }
    fn boolean_value(&mut self, v: bool) {
        if self.need_comma() { self.buf.push(b','); }
        self.buf.extend_from_slice(if v { b"true" } else { b"false" });
    }
    fn null_value(&mut self) {
        if self.need_comma() { self.buf.push(b','); }
        self.buf.extend_from_slice(b"null");
    }
}

// ── VersionResponseHandler ───────────────────────────────────────────

pub struct VersionResponseHandler {
    current_key: Option<String>,
    out: Arc<Mutex<Option<String>>>,
}

impl VersionResponseHandler {
    pub fn new(out: Arc<Mutex<Option<String>>>) -> Self {
        Self { current_key: None, out }
    }
}

impl JsonContentHandler for VersionResponseHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, key: &str) { self.current_key = Some(key.to_string()); }
    fn string_value(&mut self, value: &str) {
        if self.current_key.as_deref() == Some("version") {
            if let Ok(mut o) = self.out.lock() { *o = Some(value.to_string()); }
        }
        self.current_key = None;
    }
    fn number_value(&mut self, _: JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}
