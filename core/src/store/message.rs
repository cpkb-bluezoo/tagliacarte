/*
 * message.rs
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

//! Message and envelope types.

use crate::message_id::MessageId;
use std::collections::HashSet;

/// Payload for sending: structured fields only. Backends (e.g. SMTP) build wire format (RFC 822/MIME) from this.
#[derive(Debug, Clone, Default)]
pub struct SendPayload {
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub subject: Option<String>,
    pub body_plain: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<Attachment>,
}

/// Attachment for SendPayload (filename, MIME type, content).
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: Option<String>,
    pub mime_type: String,
    pub content: Vec<u8>,
}

/// Envelope (headers) for a message.
#[derive(Debug, Clone, Default)]
pub struct Envelope {
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub date: Option<DateTime>,
    pub subject: Option<String>,
    pub message_id: Option<String>,
}

/// Email or display address.
#[derive(Debug, Clone)]
pub struct Address {
    pub display_name: Option<String>,
    pub local_part: String,
    pub domain: Option<String>,
}

/// Date/time for message envelope.
#[derive(Debug, Clone)]
pub struct DateTime {
    pub timestamp: i64,
    pub tz_offset_secs: Option<i32>,
}

/// Summary of a conversation (thread) for list view.
#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub id: MessageId,
    pub envelope: Envelope,
    pub flags: HashSet<Flag>,
    pub size: u64,
}

/// Message flags (e.g. Seen, Answered).
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Flag {
    Seen,
    Answered,
    Flagged,
    Deleted,
    Draft,
    Custom(String),
}

/// A single message (envelope + structured body; optional raw for view source).
#[derive(Debug)]
pub struct Message {
    pub id: MessageId,
    pub envelope: Envelope,
    pub flags: HashSet<Flag>,
    pub size: u64,
    /// Plain-text body. Populated by backends from MIME (or equivalent).
    pub body_plain: Option<String>,
    /// HTML body. Populated by backends from MIME (or equivalent).
    pub body_html: Option<String>,
    /// Attachments (filename, mime_type, content). Populated by backends.
    pub attachments: Vec<Attachment>,
    /// Raw message bytes (for view source). Optional; set by backends when available.
    pub raw: Option<Vec<u8>>,
}
