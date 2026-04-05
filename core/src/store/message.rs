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
use crate::mime::{
    extract_structured_body, parse_envelope, utf8_body_after_rfc822_headers, EmailAddress,
    EnvelopeHeaders,
};
use std::cmp::Ordering;
use std::collections::HashSet;

/// Payload for sending: structured fields only. Backends (e.g. SMTP) build wire format (RFC 822/MIME) from this.
#[derive(Debug, Clone, Default)]
pub struct SendPayload {
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: Option<String>,
    pub body_plain: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<Attachment>,
    /// Target newsgroups for NNTP POST (empty for non-NNTP transports).
    pub newsgroups: Vec<String>,
    /// RFC 5322 `In-Reply-To` for NNTP follow-ups (single id, angle brackets optional).
    pub nntp_in_reply_to: Option<String>,
    /// RFC 5322 `References` for NNTP (space-separated ids).
    pub nntp_references: Option<String>,
    /// RFC 3461 `NOTIFY` for SMTP `MAIL FROM` (e.g. `FAILURE` or `FAILURE,SUCCESS`). Omit when None.
    pub smtp_notify: Option<String>,
}

/// Attachment for SendPayload (filename, MIME type, content).
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: Option<String>,
    pub mime_type: String,
    pub content: Vec<u8>,
}

/// Attachment row for message detail UI. IMAP may leave [`Self::data`] empty until a part fetch.
#[derive(Debug, Clone)]
pub struct MessageAttachmentRef {
    pub filename: Option<String>,
    pub content_type: String,
    pub size_bytes: u64,
    /// IMAP `BODYSTRUCTURE` encoding for this part (e.g. `BASE64`).
    pub transfer_encoding: String,
    pub imap_section: Option<String>,
    /// Normalized Content-ID (no angle brackets) when known from BODYSTRUCTURE.
    pub content_id: Option<String>,
    pub data: Option<Vec<u8>>,
}

/// Envelope + bodies + attachment list for the message reader (no raw wire).
#[derive(Debug, Clone)]
pub struct MessageForDisplay {
    pub envelope: Envelope,
    pub body_plain: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<MessageAttachmentRef>,
}

/// Parse full RFC 822 bytes into display fields (Maildir, mbox, IMAP fallback).
pub fn message_for_display_from_raw(raw: &[u8]) -> MessageForDisplay {
    let envelope = envelope_from_rfc822(raw);
    let (mut plain, html, parts) = extract_structured_body(raw).unwrap_or((None, None, vec![]));
    if plain.is_none() && html.is_none() {
        plain = utf8_body_after_rfc822_headers(raw)
            .or_else(|| Some(String::from_utf8_lossy(raw).into_owned()));
    }
    let attachments = parts
        .into_iter()
        .map(
            |(filename, content_id, content_type, content)| MessageAttachmentRef {
                filename,
                content_type,
                size_bytes: content.len() as u64,
                transfer_encoding: String::new(),
                imap_section: None,
                content_id,
                data: Some(content),
            },
        )
        .collect();
    MessageForDisplay {
        envelope,
        body_plain: plain,
        body_html: html,
        attachments,
    }
}

fn envelope_from_rfc822(raw: &[u8]) -> Envelope {
    parse_envelope(raw)
        .map(|h| envelope_headers_into_envelope(&h))
        .unwrap_or_default()
}

fn envelope_headers_into_envelope(h: &EnvelopeHeaders) -> Envelope {
    Envelope {
        from: h.from.iter().map(email_to_address).collect(),
        to: h.to.iter().map(email_to_address).collect(),
        cc: h.cc.iter().map(email_to_address).collect(),
        date: h.date.map(|dt| DateTime {
            timestamp: dt.timestamp(),
            tz_offset_secs: Some(dt.offset().local_minus_utc()),
        }),
        subject: h.subject.clone(),
        message_id: h.message_id.as_ref().map(|c| c.to_string()),
    }
}

fn email_to_address(e: &EmailAddress) -> Address {
    Address {
        display_name: e.display_name.clone(),
        local_part: e.local_part.clone(),
        domain: Some(e.domain.clone()),
    }
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

#[derive(Clone, Copy)]
enum MailWindowSortKey {
    Date,
    From,
    Subject,
}

fn mail_window_sort_key(symbolic: &str) -> MailWindowSortKey {
    let s = symbolic.to_ascii_lowercase();
    if s.contains("subject") || s == "subasc" || s == "subdesc" {
        MailWindowSortKey::Subject
    } else if s.contains("from") {
        MailWindowSortKey::From
    } else {
        MailWindowSortKey::Date
    }
}

fn mail_window_subject_key(e: &Envelope) -> String {
    e.subject.as_deref().unwrap_or("").to_lowercase()
}

fn mail_window_from_key(e: &Envelope) -> String {
    e.from
        .first()
        .map(|a| {
            format!(
                "{}@{}",
                a.local_part.to_lowercase(),
                a.domain.as_deref().unwrap_or("").to_lowercase()
            )
        })
        .unwrap_or_default()
}

fn compare_mail_window_summaries(
    a: &ConversationSummary,
    b: &ConversationSummary,
    k: MailWindowSortKey,
) -> Ordering {
    match k {
        MailWindowSortKey::Date => {
            let ta = a.envelope.date.as_ref().map(|d| d.timestamp).unwrap_or(0);
            let tb = b.envelope.date.as_ref().map(|d| d.timestamp).unwrap_or(0);
            ta.cmp(&tb)
        }
        MailWindowSortKey::Subject => {
            mail_window_subject_key(&a.envelope).cmp(&mail_window_subject_key(&b.envelope))
        }
        MailWindowSortKey::From => mail_window_from_key(&a.envelope).cmp(&mail_window_from_key(&b.envelope)),
    }
}

/// Sort summaries for a mailbox list window in **ascending** order for the given symbolic sort
/// (Flutter `messageListSort` tokens). The UI reverses visually when the user chose descending.
pub fn sort_conversation_summaries_for_window(rows: &mut Vec<ConversationSummary>, symbolic: &str) {
    let k = mail_window_sort_key(symbolic);
    rows.sort_by(|a, b| compare_mail_window_summaries(a, b, k));
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
