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
    LoginResponse, Profile, RoomEvent, RoomSummary, WellKnown,
    EVENT_ROOM_AVATAR, EVENT_ROOM_MESSAGE, EVENT_ROOM_NAME, EVENT_ROOM_TOPIC,
};

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
        Self {
            on_room: Box::new(on_room),
            on_event: Box::new(on_event),
            next_batch,
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

        // Emit timeline message events
        if event_type == EVENT_ROOM_MESSAGE {
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

        // Entering a room object at depth 4
        if self.depth == 4 && (self.section == SyncSection::Join || self.section == SyncSection::Invite) {
            self.current_room_id = self.current_key.clone();
            self.room_state = RoomState::default();
        }

        // Entering an event object at depth 6 inside events array
        if self.depth == 6 && self.in_events_array {
            self.event_fields.reset();
        }

        // Entering "content" at depth 7 inside an event
        if self.depth == 7 && self.in_events_array && self.current_key.as_deref() == Some("content") {
            self.in_content = true;
            self.event_content_depth = self.depth;
        }
    }

    fn end_object(&mut self) {
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
    }

    fn end_array(&mut self) {
        if self.in_events_array && self.depth == 5 {
            self.in_events_array = false;
        }
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());

        // Detect section at depth 2 (inside "rooms")
        if self.depth == 3 {
            match key {
                "join" => self.section = SyncSection::Join,
                "invite" => self.section = SyncSection::Invite,
                _ => {}
            }
        }
    }

    fn string_value(&mut self, value: &str) {
        // next_batch at root level
        if self.depth == 1 && self.current_key.as_deref() == Some("next_batch") {
            if let Ok(mut nb) = self.next_batch.lock() {
                *nb = Some(value.to_string());
            }
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
                _ => {}
            }
        }

        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.depth == 6 && self.in_events_array && !self.in_content {
            if self.current_key.as_deref() == Some("origin_server_ts") {
                self.event_fields.origin_server_ts = number.as_i64().unwrap_or(0);
            }
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
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
