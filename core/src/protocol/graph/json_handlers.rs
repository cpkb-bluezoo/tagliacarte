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

//! JsonContentHandler implementations for Microsoft Graph API JSON responses.
//!
//! Each handler is a small state machine that tracks nesting depth and current
//! key, extracting only the fields it needs and delivering results via callbacks
//! or accumulating into typed structs. No DOM tree, no serde.

use std::collections::HashSet;

use crate::json::{JsonContentHandler, JsonNumber};
use crate::message_id::MessageId;
use crate::store::{
    Address, Attachment, ConversationSummary, Envelope, Flag, FolderInfo, Message,
};

use super::base64_decode;
use super::parse_graph_datetime;

// ── FolderListHandler ─────────────────────────────────────────────────

/// Parses `{"value":[{"id","displayName","childFolderCount"},...]}`
/// and calls `on_folder` for each folder object.
pub struct FolderListHandler {
    on_folder: Box<dyn Fn(GraphFolderEntry) + Send>,
    depth: usize,
    in_value_array: bool,
    current_key: Option<String>,
    // Per-folder state
    folder_id: Option<String>,
    display_name: Option<String>,
    child_folder_count: u32,
}

/// Raw folder entry from the Graph API.
#[derive(Debug, Clone)]
pub struct GraphFolderEntry {
    pub id: String,
    pub display_name: String,
    pub child_folder_count: u32,
}

impl FolderListHandler {
    pub fn new(on_folder: impl Fn(GraphFolderEntry) + Send + 'static) -> Self {
        Self {
            on_folder: Box::new(on_folder),
            depth: 0,
            in_value_array: false,
            current_key: None,
            folder_id: None,
            display_name: None,
            child_folder_count: 0,
        }
    }

    fn reset_folder(&mut self) {
        self.folder_id = None;
        self.display_name = None;
        self.child_folder_count = 0;
    }

    fn emit_folder(&mut self) {
        if let (Some(id), Some(name)) = (self.folder_id.take(), self.display_name.take()) {
            (self.on_folder)(GraphFolderEntry {
                id,
                display_name: name,
                child_folder_count: self.child_folder_count,
            });
        }
    }
}

impl JsonContentHandler for FolderListHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.in_value_array && self.depth == 2 {
            self.reset_folder();
        }
    }

    fn end_object(&mut self) {
        if self.in_value_array && self.depth == 2 {
            self.emit_folder();
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        self.depth += 1;
        if self.depth == 1 && self.current_key.as_deref() == Some("value") {
            self.in_value_array = true;
        }
    }

    fn end_array(&mut self) {
        if self.in_value_array && self.depth == 1 {
            self.in_value_array = false;
        }
        self.depth -= 1;
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.in_value_array && self.depth == 2 {
            match self.current_key.as_deref() {
                Some("id") => self.folder_id = Some(value.to_string()),
                Some("displayName") => self.display_name = Some(value.to_string()),
                _ => {}
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.in_value_array && self.depth == 2 {
            if self.current_key.as_deref() == Some("childFolderCount") {
                self.child_folder_count = number.as_i64().unwrap_or(0) as u32;
            }
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, _value: bool) {
        self.current_key = None;
    }

    fn null_value(&mut self) {
        self.current_key = None;
    }
}

// ── MessageCountHandler ───────────────────────────────────────────────

/// Parses `{"totalItemCount": N, ...}` and extracts the count.
pub struct MessageCountHandler {
    current_key: Option<String>,
    pub total: u64,
}

impl MessageCountHandler {
    pub fn new() -> Self {
        Self {
            current_key: None,
            total: 0,
        }
    }
}

impl JsonContentHandler for MessageCountHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, _value: &str) {
        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.current_key.as_deref() == Some("totalItemCount") {
            self.total = number.as_i64().unwrap_or(0) as u64;
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, _value: bool) {
        self.current_key = None;
    }

    fn null_value(&mut self) {
        self.current_key = None;
    }
}

// ── MessageListHandler ────────────────────────────────────────────────

/// Parses `{"value":[{message},...]}`
/// and calls `on_summary` for each message as it completes.
pub struct MessageListHandler {
    on_summary: Box<dyn Fn(ConversationSummary) + Send>,
    depth: usize,
    in_value_array: bool,
    current_key: Option<String>,
    // Per-message state
    msg: MessageFields,
    // Recipient list parsing state
    in_recipients: RecipientListKind,
    recipient: RecipientFields,
    in_email_address: bool,
}

#[derive(Default)]
struct MessageFields {
    id: Option<String>,
    subject: Option<String>,
    received_date_time: Option<String>,
    is_read: bool,
    is_draft: bool,
    importance: Option<String>,
    size: u64,
    internet_message_id: Option<String>,
    from: Option<Address>,
    to: Vec<Address>,
    cc: Vec<Address>,
}

#[derive(Default)]
struct RecipientFields {
    name: Option<String>,
    address: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum RecipientListKind {
    None,
    From,
    To,
    Cc,
}

impl MessageListHandler {
    pub fn new(on_summary: impl Fn(ConversationSummary) + Send + 'static) -> Self {
        Self {
            on_summary: Box::new(on_summary),
            depth: 0,
            in_value_array: false,
            current_key: None,
            msg: MessageFields::default(),
            in_recipients: RecipientListKind::None,
            recipient: RecipientFields::default(),
            in_email_address: false,
        }
    }

    fn emit_message(&mut self) {
        let id = self.msg.id.take().unwrap_or_default();
        let date = self.msg.received_date_time.as_deref().and_then(parse_graph_datetime);
        let importance = self.msg.importance.as_deref().unwrap_or("normal");

        let mut flags = HashSet::new();
        if self.msg.is_read {
            flags.insert(Flag::Seen);
        }
        if self.msg.is_draft {
            flags.insert(Flag::Draft);
        }
        if importance == "high" {
            flags.insert(Flag::Flagged);
        }

        let from = self.msg.from.take().unwrap_or(Address {
            display_name: None,
            local_part: String::new(),
            domain: None,
        });

        let summary = ConversationSummary {
            id: MessageId::new(&id),
            envelope: Envelope {
                from: vec![from],
                to: std::mem::take(&mut self.msg.to),
                cc: std::mem::take(&mut self.msg.cc),
                date,
                subject: self.msg.subject.take(),
                message_id: self.msg.internet_message_id.take(),
            },
            flags,
            size: self.msg.size,
        };
        (self.on_summary)(summary);
    }

    fn finish_recipient(&mut self) {
        let addr = build_address(&self.recipient);
        match self.in_recipients {
            RecipientListKind::From => self.msg.from = Some(addr),
            RecipientListKind::To => self.msg.to.push(addr),
            RecipientListKind::Cc => self.msg.cc.push(addr),
            RecipientListKind::None => {}
        }
        self.recipient = RecipientFields::default();
    }
}

impl JsonContentHandler for MessageListHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.in_value_array && self.depth == 2 {
            self.msg = MessageFields::default();
        }
        // emailAddress sub-object inside a recipient
        if self.in_recipients != RecipientListKind::None && self.current_key.as_deref() == Some("emailAddress") {
            self.in_email_address = true;
        }
    }

    fn end_object(&mut self) {
        if self.in_email_address {
            self.in_email_address = false;
        } else if self.in_recipients != RecipientListKind::None && self.depth == 3 {
            // End of a recipient object
            self.finish_recipient();
        } else if self.in_value_array && self.depth == 2 {
            self.emit_message();
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        self.depth += 1;
        if self.depth == 1 && self.current_key.as_deref() == Some("value") {
            self.in_value_array = true;
        } else if self.in_value_array && self.depth == 2 {
            match self.current_key.as_deref() {
                Some("toRecipients") => self.in_recipients = RecipientListKind::To,
                Some("ccRecipients") => self.in_recipients = RecipientListKind::Cc,
                _ => {}
            }
        }
    }

    fn end_array(&mut self) {
        if self.in_recipients != RecipientListKind::None && self.depth == 2 {
            self.in_recipients = RecipientListKind::None;
        }
        if self.in_value_array && self.depth == 1 {
            self.in_value_array = false;
        }
        self.depth -= 1;
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
        // "from" is a single recipient object, not an array
        if self.in_value_array && self.depth == 2 && key == "from" {
            self.in_recipients = RecipientListKind::From;
        }
    }

    fn string_value(&mut self, value: &str) {
        if self.in_email_address {
            match self.current_key.as_deref() {
                Some("name") => self.recipient.name = Some(value.to_string()),
                Some("address") => self.recipient.address = Some(value.to_string()),
                _ => {}
            }
        } else if self.in_value_array && self.depth == 2 {
            match self.current_key.as_deref() {
                Some("id") => self.msg.id = Some(value.to_string()),
                Some("subject") => self.msg.subject = Some(value.to_string()),
                Some("receivedDateTime") => self.msg.received_date_time = Some(value.to_string()),
                Some("importance") => self.msg.importance = Some(value.to_string()),
                Some("internetMessageId") => self.msg.internet_message_id = Some(value.to_string()),
                _ => {}
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.in_value_array && self.depth == 2 {
            if self.current_key.as_deref() == Some("size") {
                self.msg.size = number.as_i64().unwrap_or(0) as u64;
            }
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, value: bool) {
        if self.in_value_array && self.depth == 2 {
            match self.current_key.as_deref() {
                Some("isRead") => self.msg.is_read = value,
                Some("isDraft") => self.msg.is_draft = value,
                _ => {}
            }
        }
        self.current_key = None;
    }

    fn null_value(&mut self) {
        self.current_key = None;
    }
}

// ── SingleMessageHandler ──────────────────────────────────────────────

/// Parses a full single message object (with body and attachments) into a Message.
pub struct SingleMessageHandler {
    depth: usize,
    current_key: Option<String>,
    // Core message fields
    msg: MessageFields,
    // Body
    body_content_type: Option<String>,
    body_content: Option<String>,
    in_body: bool,
    // Attachments
    attachments: Vec<Attachment>,
    in_attachments: bool,
    att_name: Option<String>,
    att_content_type: Option<String>,
    att_content_bytes: Option<String>,
    // Recipients
    in_recipients: RecipientListKind,
    recipient: RecipientFields,
    in_email_address: bool,
    // Result
    pub result: Option<Message>,
}

impl SingleMessageHandler {
    pub fn new() -> Self {
        Self {
            depth: 0,
            current_key: None,
            msg: MessageFields::default(),
            body_content_type: None,
            body_content: None,
            in_body: false,
            attachments: Vec::new(),
            in_attachments: false,
            att_name: None,
            att_content_type: None,
            att_content_bytes: None,
            in_recipients: RecipientListKind::None,
            recipient: RecipientFields::default(),
            in_email_address: false,
            result: None,
        }
    }

    fn finish_recipient(&mut self) {
        let addr = build_address(&self.recipient);
        match self.in_recipients {
            RecipientListKind::From => self.msg.from = Some(addr),
            RecipientListKind::To => self.msg.to.push(addr),
            RecipientListKind::Cc => self.msg.cc.push(addr),
            RecipientListKind::None => {}
        }
        self.recipient = RecipientFields::default();
    }

    fn finish_attachment(&mut self) {
        let name = self.att_name.take();
        let mime = self.att_content_type.take().unwrap_or_else(|| "application/octet-stream".to_string());
        let content = self.att_content_bytes.take()
            .map(|b64| base64_decode(&b64))
            .unwrap_or_default();
        self.attachments.push(Attachment {
            filename: name,
            mime_type: mime,
            content,
        });
    }

    fn build_result(&mut self) {
        let id = self.msg.id.take().unwrap_or_default();
        let date = self.msg.received_date_time.as_deref().and_then(parse_graph_datetime);
        let importance = self.msg.importance.as_deref().unwrap_or("normal");

        let mut flags = HashSet::new();
        if self.msg.is_read {
            flags.insert(Flag::Seen);
        }
        if self.msg.is_draft {
            flags.insert(Flag::Draft);
        }
        if importance == "high" {
            flags.insert(Flag::Flagged);
        }

        let from = self.msg.from.take().unwrap_or(Address {
            display_name: None,
            local_part: String::new(),
            domain: None,
        });

        let body_type = self.body_content_type.as_deref().unwrap_or("text");
        let body = self.body_content.take().unwrap_or_default();
        let (body_plain, body_html) = if body_type.eq_ignore_ascii_case("html") {
            (None, Some(body))
        } else {
            (Some(body), None)
        };

        self.result = Some(Message {
            id: MessageId::new(&id),
            envelope: Envelope {
                from: vec![from],
                to: std::mem::take(&mut self.msg.to),
                cc: std::mem::take(&mut self.msg.cc),
                date,
                subject: self.msg.subject.take(),
                message_id: self.msg.internet_message_id.take(),
            },
            flags,
            size: self.msg.size,
            body_plain,
            body_html,
            attachments: std::mem::take(&mut self.attachments),
            raw: None,
        });
    }
}

impl JsonContentHandler for SingleMessageHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.current_key.as_deref() == Some("body") && self.depth == 2 {
            self.in_body = true;
        }
        if self.current_key.as_deref() == Some("emailAddress") {
            self.in_email_address = true;
        }
    }

    fn end_object(&mut self) {
        if self.in_email_address {
            self.in_email_address = false;
        } else if self.in_body && self.depth == 2 {
            self.in_body = false;
        } else if self.in_attachments && self.depth == 2 {
            self.finish_attachment();
        } else if self.in_recipients != RecipientListKind::None && !self.in_email_address && self.depth == 2 {
            self.finish_recipient();
            if self.in_recipients == RecipientListKind::From {
                self.in_recipients = RecipientListKind::None;
            }
        } else if self.depth == 1 {
            self.build_result();
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        self.depth += 1;
        if self.depth == 1 {
            match self.current_key.as_deref() {
                Some("attachments") => self.in_attachments = true,
                Some("toRecipients") => self.in_recipients = RecipientListKind::To,
                Some("ccRecipients") => self.in_recipients = RecipientListKind::Cc,
                _ => {}
            }
        }
    }

    fn end_array(&mut self) {
        if self.in_attachments && self.depth == 1 {
            self.in_attachments = false;
        }
        if self.in_recipients != RecipientListKind::None && self.depth == 1 {
            self.in_recipients = RecipientListKind::None;
        }
        self.depth -= 1;
    }

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
        if self.depth == 1 && key == "from" {
            self.in_recipients = RecipientListKind::From;
        }
    }

    fn string_value(&mut self, value: &str) {
        if self.in_email_address {
            match self.current_key.as_deref() {
                Some("name") => self.recipient.name = Some(value.to_string()),
                Some("address") => self.recipient.address = Some(value.to_string()),
                _ => {}
            }
        } else if self.in_body {
            match self.current_key.as_deref() {
                Some("contentType") => self.body_content_type = Some(value.to_string()),
                Some("content") => self.body_content = Some(value.to_string()),
                _ => {}
            }
        } else if self.in_attachments && self.depth == 2 {
            match self.current_key.as_deref() {
                Some("name") => self.att_name = Some(value.to_string()),
                Some("contentType") => self.att_content_type = Some(value.to_string()),
                Some("contentBytes") => self.att_content_bytes = Some(value.to_string()),
                _ => {}
            }
        } else if self.depth == 1 {
            match self.current_key.as_deref() {
                Some("id") => self.msg.id = Some(value.to_string()),
                Some("subject") => self.msg.subject = Some(value.to_string()),
                Some("receivedDateTime") => self.msg.received_date_time = Some(value.to_string()),
                Some("importance") => self.msg.importance = Some(value.to_string()),
                Some("internetMessageId") => self.msg.internet_message_id = Some(value.to_string()),
                _ => {}
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.depth == 1 && self.current_key.as_deref() == Some("size") {
            self.msg.size = number.as_i64().unwrap_or(0) as u64;
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, value: bool) {
        if self.depth == 1 {
            match self.current_key.as_deref() {
                Some("isRead") => self.msg.is_read = value,
                Some("isDraft") => self.msg.is_draft = value,
                _ => {}
            }
        }
        self.current_key = None;
    }

    fn null_value(&mut self) {
        self.current_key = None;
    }
}

// ── StatusOnlyHandler ─────────────────────────────────────────────────

/// Handler that only records the HTTP status (for POST/PATCH/DELETE calls
/// that return empty or uninteresting bodies). Body is discarded.
pub struct StatusOnlyHandler {
    pub success: bool,
    pub status_code: u16,
    pub error_body: String,
}

impl StatusOnlyHandler {
    pub fn new() -> Self {
        Self {
            success: false,
            status_code: 0,
            error_body: String::new(),
        }
    }
}

impl crate::protocol::http::ResponseHandler for StatusOnlyHandler {
    fn ok(&mut self, response: crate::protocol::http::Response) {
        self.success = true;
        self.status_code = response.code;
    }

    fn error(&mut self, response: crate::protocol::http::Response) {
        self.success = false;
        self.status_code = response.code;
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        if !self.success {
            if let Ok(s) = std::str::from_utf8(data) {
                self.error_body.push_str(s);
            }
        }
    }

    fn end_body(&mut self) {}
    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        self.success = false;
        self.error_body = error.to_string();
    }
}

// ── JsonCollectHandler ────────────────────────────────────────────────

/// ResponseHandler that collects body chunks, records status, and then parses
/// JSON via a provided JsonContentHandler on complete.
/// Used for moderate-size responses (folder list, message list) where we need
/// the full JSON in a BytesMut before parsing.
pub struct JsonCollectHandler {
    pub success: bool,
    pub status_code: u16,
    pub body_buf: bytes::BytesMut,
    pub error_body: String,
}

impl JsonCollectHandler {
    pub fn new() -> Self {
        Self {
            success: false,
            status_code: 0,
            body_buf: bytes::BytesMut::with_capacity(8192),
            error_body: String::new(),
        }
    }
}

impl crate::protocol::http::ResponseHandler for JsonCollectHandler {
    fn ok(&mut self, response: crate::protocol::http::Response) {
        self.success = true;
        self.status_code = response.code;
    }

    fn error(&mut self, response: crate::protocol::http::Response) {
        self.success = false;
        self.status_code = response.code;
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        if self.success {
            self.body_buf.extend_from_slice(data);
        } else {
            if let Ok(s) = std::str::from_utf8(data) {
                self.error_body.push_str(s);
            }
        }
    }

    fn end_body(&mut self) {}
    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        self.success = false;
        self.error_body = error.to_string();
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn build_address(r: &RecipientFields) -> Address {
    let addr = r.address.as_deref().unwrap_or("");
    let (local_part, domain) = if let Some(at) = addr.find('@') {
        (addr[..at].to_string(), Some(addr[at + 1..].to_string()))
    } else {
        (addr.to_string(), None)
    };
    Address {
        display_name: r.name.clone(),
        local_part,
        domain,
    }
}

/// Convert a FolderListHandler entry into a FolderInfo.
impl GraphFolderEntry {
    pub fn to_folder_info(&self) -> FolderInfo {
        FolderInfo {
            name: self.display_name.clone(),
            delimiter: Some('/'),
            attributes: Vec::new(),
        }
    }
}
