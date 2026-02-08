/*
 * build_mime.rs
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

//! Build RFC 5322 / MIME message from SendPayload. Uses format from mime/rfc5322 (address, date).

use crate::mime::format_mailbox;
use crate::store::{Address, DateTime, Envelope, SendPayload};
use chrono::{FixedOffset, Utc};

/// Build RFC 822 / MIME bytes and envelope from SendPayload. Envelope is for SMTP MAIL FROM / RCPT TO.
pub fn build_rfc822_from_payload(payload: &SendPayload) -> (Vec<u8>, Envelope) {
    let mut out = Vec::new();
    let envelope = envelope_from_payload(payload);

    // Headers (RFC 5322 format, same as parsed by rfc5322 module)
    append_address_header(&mut out, "From", &payload.from);
    append_address_header(&mut out, "To", &payload.to);
    if !payload.cc.is_empty() {
        append_address_header(&mut out, "Cc", &payload.cc);
    }
    if let Some(ref s) = payload.subject {
        append_header(&mut out, "Subject", s);
    }
    let now = Utc::now();
    let fixed = now.with_timezone(&FixedOffset::east_opt(0).unwrap_or(FixedOffset::east_opt(3600).unwrap()));
    append_header(&mut out, "Date", &fixed.to_rfc2822());
    append_header(&mut out, "MIME-Version", "1.0");

    let has_attachments = !payload.attachments.is_empty();
    let has_html = payload.body_html.as_ref().map_or(false, |s| !s.is_empty());
    let has_plain = payload.body_plain.as_ref().map_or(false, |s| !s.is_empty());

    if has_attachments {
        let boundary = format!("_bound_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
        append_header(
            &mut out,
            "Content-Type",
            &format!("multipart/mixed; boundary=\"{}\"", boundary),
        );
        out.extend_from_slice(b"\r\n");
        // First part: body (plain, html, or multipart/alternative)
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"\r\n");
        append_body_parts(&mut out, payload, has_plain, has_html);
        // Attachment parts
        for att in &payload.attachments {
            out.extend_from_slice(b"\r\n--");
            out.extend_from_slice(boundary.as_bytes());
            out.extend_from_slice(b"\r\n");
            append_attachment_part(&mut out, att);
        }
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"--\r\n");
    } else {
        append_body_parts(&mut out, payload, has_plain, has_html);
    }

    (out, envelope)
}

fn envelope_from_payload(payload: &SendPayload) -> Envelope {
    let now = Utc::now();
    let fixed = now.with_timezone(&FixedOffset::east_opt(0).unwrap_or_else(|| FixedOffset::east_opt(3600).unwrap()));
    Envelope {
        from: payload.from.clone(),
        to: payload.to.clone(),
        cc: payload.cc.clone(),
        date: Some(DateTime {
            timestamp: fixed.timestamp(),
            tz_offset_secs: Some(fixed.offset().local_minus_utc()),
        }),
        subject: payload.subject.clone(),
        message_id: None,
    }
}

fn append_address_header(out: &mut Vec<u8>, name: &str, addrs: &[Address]) {
    if addrs.is_empty() {
        return;
    }
    let values: Vec<String> = addrs
        .iter()
        .map(|a| format_mailbox(a.display_name.as_deref(), &a.local_part, a.domain.as_deref().unwrap_or("")))
        .collect();
    append_header(out, name, &values.join(", "));
}

fn append_header(out: &mut Vec<u8>, name: &str, value: &str) {
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(b": ");
    out.extend_from_slice(value.as_bytes());
    out.extend_from_slice(b"\r\n");
}

fn append_body_parts(out: &mut Vec<u8>, payload: &SendPayload, has_plain: bool, has_html: bool) {
    if has_plain && has_html {
        let boundary = format!("_alt_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
        append_header(
            out,
            "Content-Type",
            &format!("multipart/alternative; boundary=\"{}\"", boundary),
        );
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(boundary.as_bytes());
        append_header(out, "Content-Type", "text/plain; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_plain {
            out.extend_from_slice(b.as_bytes());
        }
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(boundary.as_bytes());
        append_header(out, "Content-Type", "text/html; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_html {
            out.extend_from_slice(b.as_bytes());
        }
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"--\r\n");
    } else if has_html {
        append_header(out, "Content-Type", "text/html; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_html {
            out.extend_from_slice(b.as_bytes());
        }
        out.extend_from_slice(b"\r\n");
    } else {
        append_header(out, "Content-Type", "text/plain; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_plain {
            out.extend_from_slice(b.as_bytes());
        }
        out.extend_from_slice(b"\r\n");
    }
}

fn append_attachment_part(out: &mut Vec<u8>, att: &crate::store::Attachment) {
    append_header(out, "Content-Type", &att.mime_type);
    if let Some(ref name) = att.filename {
        append_header(out, "Content-Disposition", &format!("attachment; filename=\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")));
    }
    append_header(out, "Content-Transfer-Encoding", "base64");
    out.extend_from_slice(b"\r\n");
    let encoded = base64_encode(&att.content);
    for chunk in encoded.chunks(76) {
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
}

fn base64_encode(b: &[u8]) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity((b.len() + 2) / 3 * 4);
    for chunk in b.chunks(3) {
        let n = (chunk[0] as usize) << 16
            | (chunk.get(1).copied().unwrap_or(0) as usize) << 8
            | chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(ALPHABET[n >> 18]);
        out.push(ALPHABET[(n >> 12) & 63]);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) & 63]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n & 63]
        } else {
            b'='
        });
    }
    out
}
