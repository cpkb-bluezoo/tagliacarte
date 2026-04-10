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

use crate::protocol::smtp::address_norm::{
    store_address_domain_for_mid, store_address_header_mailbox,
};
use crate::store::{Address, DateTime, Envelope, SendPayload};
use chrono::{FixedOffset, Utc};
use rand::Rng;

/// RFC 5321 §4.5.3.1: max octets in a line excluding CRLF (servers often enforce this).
const SMTP_MAX_LINE_OCTETS: usize = 998;

fn normalize_smtp_in_reply_to_value(id: &str) -> String {
    let t = id.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.starts_with('<') && t.ends_with('>') && t.len() >= 2 {
        return t.to_string();
    }
    let inner = t.trim_matches(|c| c == '<' || c == '>').trim();
    if inner.is_empty() {
        return String::new();
    }
    format!("<{inner}>")
}

fn domain_for_message_id(from: &[Address]) -> String {
    from.iter()
        .find_map(store_address_domain_for_mid)
        .unwrap_or_else(|| "local".to_string())
}

fn generate_message_id(from: &[Address]) -> String {
    let domain = domain_for_message_id(from);
    let r: u64 = rand::thread_rng().gen();
    let t = Utc::now().timestamp_millis();
    format!("<{t}.{r:x}@{domain}>")
}

/// Normalize a caller-supplied Message-ID to angle-bracket form, or `None` if empty/invalid.
pub fn normalize_smtp_message_id_angle(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with('<') && t.ends_with('>') && t.len() >= 2 {
        return Some(t.to_string());
    }
    let inner = t.trim_matches(|c| c == '<' || c == '>').trim();
    if inner.is_empty() {
        return None;
    }
    Some(format!("<{inner}>"))
}

fn addresses_from_smtp_from_field(raw: &str) -> Vec<Address> {
    raw.split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.split('@');
            let local = parts.next()?.trim().to_owned();
            let domain = parts.next().map(|v| v.trim().to_owned());
            Some(Address {
                display_name: None,
                local_part: local,
                domain,
            })
        })
        .collect()
}

/// Generate a new RFC 5322 `Message-ID` (angle brackets) from a `From` header line (comma-separated mailboxes).
pub fn generate_smtp_message_id_angle_from_from_field(from: &str) -> String {
    let addrs = addresses_from_smtp_from_field(from);
    generate_message_id(&addrs)
}

fn message_id_angle_for_payload(payload: &SendPayload) -> String {
    if let Some(ref raw) = payload.smtp_message_id {
        if let Some(a) = normalize_smtp_message_id_angle(raw) {
            return a;
        }
    }
    generate_message_id(&payload.from)
}

/// Wrap one logical line so no physical line exceeds [SMTP_MAX_LINE_OCTETS] (UTF-8 safe).
fn wrap_smtp_logical_line(line: &str) -> String {
    if line.len() <= SMTP_MAX_LINE_OCTETS {
        return line.to_string();
    }
    let max = SMTP_MAX_LINE_OCTETS;
    let mut out = String::with_capacity(line.len() + line.len() / max * 2);
    let mut start = 0usize;
    while start < line.len() {
        if line.len() - start <= max {
            out.push_str(&line[start..]);
            break;
        }
        let mut end = (start + max).min(line.len());
        while end > start && !line.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            let ch = line[start..].chars().next().unwrap();
            end = start + ch.len_utf8();
        } else if let Some(rel) = line[start..end].rfind(' ') {
            if rel > 0 {
                end = start + rel;
            }
        }
        out.push_str(&line[start..end]);
        out.push_str("\r\n");
        start = end;
        while line.as_bytes().get(start) == Some(&b' ') {
            start += 1;
        }
    }
    out
}

/// Ensure body text uses CRLF and no line exceeds SMTP limits (needed for strict MTAs on HTML).
fn smtp_safe_body_crlf(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + text.len() / 80);
    for line in text.lines() {
        let wrapped = wrap_smtp_logical_line(line);
        out.extend_from_slice(wrapped.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// Build RFC 822 / MIME bytes and envelope from SendPayload. Envelope is for SMTP MAIL FROM / RCPT TO.
pub fn build_rfc822_from_payload(payload: &SendPayload) -> (Vec<u8>, Envelope) {
    let mut out = Vec::new();
    let message_id_hdr = message_id_angle_for_payload(payload);
    let envelope = envelope_from_payload(payload, &message_id_hdr);

    let now = Utc::now();
    let fixed = now
        .with_timezone(&FixedOffset::east_opt(0).unwrap_or(FixedOffset::east_opt(3600).unwrap()));
    let date_str = fixed.to_rfc2822();

    // Header order: Date, From, Sender (if multi-mailbox From), To, Cc, Subject, Message-ID,
    // threading, MIME — matches common RFC 5322 practice and strict postmasters (e.g. Nemesis).
    append_header(&mut out, "Date", &date_str);
    append_address_header(&mut out, "From", &payload.from);
    if payload.from.len() > 1 {
        append_address_header(&mut out, "Sender", &payload.from[..1]);
    }
    append_address_header(&mut out, "To", &payload.to);
    if !payload.cc.is_empty() {
        append_address_header(&mut out, "Cc", &payload.cc);
    }
    if let Some(ref s) = payload.subject {
        append_header(&mut out, "Subject", s);
    }
    append_header(&mut out, "Message-ID", &message_id_hdr);
    if let Some(ref s) = payload.smtp_in_reply_to {
        let v = normalize_smtp_in_reply_to_value(s);
        if !v.is_empty() {
            append_header(&mut out, "In-Reply-To", &v);
        }
    }
    if let Some(ref s) = payload.smtp_references {
        let v = s.trim();
        if !v.is_empty() {
            append_header(&mut out, "References", v);
        }
    }
    append_header(&mut out, "MIME-Version", "1.0");

    let has_attachments = !payload.attachments.is_empty();
    let has_html = payload.body_html.as_ref().map_or(false, |s| !s.is_empty());
    let has_plain = payload.body_plain.as_ref().map_or(false, |s| !s.is_empty());

    if has_attachments {
        let boundary = format!(
            "_bound_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        append_header(
            &mut out,
            "Content-Type",
            &format!("multipart/mixed; boundary=\"{}\"", boundary),
        );
        out.extend_from_slice(b"\r\n");
        // First part: body (plain, html, or multipart/alternative). Delimiter is "--" + boundary
        // + CRLF; do not prefix with an extra CRLF — the blank line above already ends headers.
        out.extend_from_slice(b"--");
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

fn envelope_from_payload(payload: &SendPayload, message_id_angle: &str) -> Envelope {
    let now = Utc::now();
    let fixed = now.with_timezone(
        &FixedOffset::east_opt(0).unwrap_or_else(|| FixedOffset::east_opt(3600).unwrap()),
    );
    let mid = message_id_angle
        .trim()
        .trim_matches(|c| c == '<' || c == '>')
        .to_string();
    Envelope {
        from: payload.from.clone(),
        to: payload.to.clone(),
        cc: payload.cc.clone(),
        date: Some(DateTime {
            timestamp: fixed.timestamp(),
            tz_offset_secs: Some(fixed.offset().local_minus_utc()),
        }),
        subject: payload.subject.clone(),
        message_id: if mid.is_empty() {
            None
        } else {
            Some(mid)
        },
        in_reply_to: None,
        references: None,
    }
}

fn append_address_header(out: &mut Vec<u8>, name: &str, addrs: &[Address]) {
    if addrs.is_empty() {
        return;
    }
    let values: Vec<String> = addrs.iter().map(store_address_header_mailbox).collect();
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
        let boundary = format!(
            "_alt_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        append_header(
            out,
            "Content-Type",
            &format!("multipart/alternative; boundary=\"{}\"", boundary),
        );
        out.extend_from_slice(b"\r\n");
        // First boundary after this entity's headers: same rule as mixed — blank line above
        // supplies the CRLF before "--".
        out.extend_from_slice(b"--");
        out.extend_from_slice(boundary.as_bytes());
        // RFC 2046: delimiter line is "--" + boundary + CRLF (leading CRLF is the blank line).
        out.extend_from_slice(b"\r\n");
        append_header(out, "Content-Type", "text/plain; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_plain {
            out.extend_from_slice(&smtp_safe_body_crlf(b));
        }
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"\r\n");
        append_header(out, "Content-Type", "text/html; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_html {
            out.extend_from_slice(&smtp_safe_body_crlf(b));
        }
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"--\r\n");
    } else if has_html {
        append_header(out, "Content-Type", "text/html; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_html {
            out.extend_from_slice(&smtp_safe_body_crlf(b));
        }
        out.extend_from_slice(b"\r\n");
    } else {
        append_header(out, "Content-Type", "text/plain; charset=utf-8");
        out.extend_from_slice(b"\r\n");
        if let Some(ref b) = payload.body_plain {
            out.extend_from_slice(&smtp_safe_body_crlf(b));
        }
        out.extend_from_slice(b"\r\n");
    }
}

fn append_attachment_part(out: &mut Vec<u8>, att: &crate::store::Attachment) {
    append_header(out, "Content-Type", &att.mime_type);
    if let Some(ref name) = att.filename {
        append_header(
            out,
            "Content-Disposition",
            &format!(
                "attachment; filename=\"{}\"",
                name.replace('\\', "\\\\").replace('"', "\\\"")
            ),
        );
    }
    append_header(out, "Content-Transfer-Encoding", "base64");
    out.extend_from_slice(b"\r\n");
    let encoded = base64_encode(&att.content);
    for chunk in encoded.chunks(76) {
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\r\n");
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Address, Attachment, SendPayload};

    #[test]
    fn long_html_line_is_wrapped_under_smtp_limit() {
        let long = "a".repeat(1200);
        let body = format!("<html><body>{long}</body></html>");
        let payload = SendPayload {
            from: vec![Address {
                display_name: None,
                local_part: "u".into(),
                domain: Some("example.com".into()),
            }],
            to: vec![Address {
                display_name: None,
                local_part: "v".into(),
                domain: Some("example.com".into()),
            }],
            cc: vec![],
            bcc: vec![],
            subject: Some("t".into()),
            body_plain: None,
            body_html: Some(body),
            attachments: vec![],
            newsgroups: vec![],
            nntp_in_reply_to: None,
            nntp_references: None,
            smtp_notify: None,
            smtp_in_reply_to: None,
            smtp_references: None,
            smtp_message_id: None,
        };
        let (raw, _) = build_rfc822_from_payload(&payload);
        let s = String::from_utf8_lossy(&raw);
        for line in s.split("\r\n") {
            assert!(
                line.len() <= SMTP_MAX_LINE_OCTETS,
                "line len {} exceeds limit",
                line.len()
            );
        }
    }

    #[test]
    fn naive_name_addr_split_yields_valid_from_and_message_id() {
        let payload = SendPayload {
            from: vec![Address {
                display_name: None,
                local_part: "Jane Doe <jane".into(),
                domain: Some("example.com>".into()),
            }],
            to: vec![Address {
                display_name: None,
                local_part: "alice".into(),
                domain: Some("example.com".into()),
            }],
            cc: vec![],
            bcc: vec![],
            subject: Some("t".into()),
            body_plain: Some("hi".into()),
            body_html: None,
            attachments: vec![],
            newsgroups: vec![],
            nntp_in_reply_to: None,
            nntp_references: None,
            smtp_notify: None,
            smtp_in_reply_to: None,
            smtp_references: None,
            smtp_message_id: None,
        };
        let (raw, _) = build_rfc822_from_payload(&payload);
        let s = String::from_utf8_lossy(&raw);
        assert!(
            s.contains("From: Jane Doe <jane@example.com>\r\n"),
            "From line: {}",
            s.lines().find(|l| l.starts_with("From:")).unwrap_or("")
        );
        let mid_line = s
            .lines()
            .find(|l| l.starts_with("Message-ID:"))
            .expect("Message-ID");
        assert!(
            mid_line.ends_with("@example.com>") && !mid_line.ends_with("com>>"),
            "bad Message-ID: {mid_line}"
        );
        assert!(
            !mid_line.to_ascii_lowercase().contains("tagliacarte"),
            "Message-ID should not advertise client name: {mid_line}"
        );
    }

    #[test]
    fn preset_smtp_message_id_is_emitted_verbatim_normalized() {
        let payload = SendPayload {
            from: vec![Address {
                display_name: None,
                local_part: "u".into(),
                domain: Some("example.com".into()),
            }],
            to: vec![Address {
                display_name: None,
                local_part: "v".into(),
                domain: Some("example.com".into()),
            }],
            cc: vec![],
            bcc: vec![],
            subject: Some("t".into()),
            body_plain: Some("hi".into()),
            body_html: None,
            attachments: vec![],
            newsgroups: vec![],
            nntp_in_reply_to: None,
            nntp_references: None,
            smtp_notify: None,
            smtp_in_reply_to: None,
            smtp_references: None,
            smtp_message_id: Some("fixed-id@example.org".into()),
        };
        let (raw, _) = build_rfc822_from_payload(&payload);
        let s = String::from_utf8_lossy(&raw);
        assert!(s.contains("Message-ID: <fixed-id@example.org>\r\n"), "{}", s);
    }

    #[test]
    fn message_id_and_date_before_from_in_headers() {
        let payload = SendPayload {
            from: vec![Address {
                display_name: None,
                local_part: "u".into(),
                domain: Some("example.com".into()),
            }],
            to: vec![Address {
                display_name: None,
                local_part: "v".into(),
                domain: Some("example.com".into()),
            }],
            cc: vec![],
            bcc: vec![],
            subject: Some("subj".into()),
            body_plain: Some("hi".into()),
            body_html: None,
            attachments: vec![],
            newsgroups: vec![],
            nntp_in_reply_to: None,
            nntp_references: None,
            smtp_notify: None,
            smtp_in_reply_to: None,
            smtp_references: None,
            smtp_message_id: None,
        };
        let (raw, _) = build_rfc822_from_payload(&payload);
        let s = String::from_utf8_lossy(&raw);
        let date_pos = s.find("Date:").expect("Date");
        let from_pos = s.find("From:").expect("From");
        let mid_pos = s.find("Message-ID:").expect("mid");
        assert!(date_pos < from_pos);
        assert!(from_pos < mid_pos);
    }

    #[test]
    fn multipart_alternative_crlf_after_each_boundary_delimiter() {
        let payload = SendPayload {
            from: vec![Address {
                display_name: None,
                local_part: "u".into(),
                domain: Some("example.com".into()),
            }],
            to: vec![Address {
                display_name: None,
                local_part: "v".into(),
                domain: Some("example.com".into()),
            }],
            cc: vec![],
            bcc: vec![],
            subject: Some("s".into()),
            body_plain: Some("plain part".into()),
            body_html: Some("<p>html</p>".into()),
            attachments: vec![],
            newsgroups: vec![],
            nntp_in_reply_to: None,
            nntp_references: None,
            smtp_notify: None,
            smtp_in_reply_to: None,
            smtp_references: None,
            smtp_message_id: None,
        };
        let (raw, _) = build_rfc822_from_payload(&payload);
        let s = String::from_utf8_lossy(&raw);
        let prefix = "multipart/alternative; boundary=\"";
        let i = s.find(prefix).expect("multipart/alternative");
        let rest = &s[i + prefix.len()..];
        let end = rest.find('"').expect("closing quote on boundary");
        let b = &rest[..end];
        assert!(
            s.contains(&format!("\r\n--{b}\r\nContent-Type: text/plain")),
            "delimiter before plain part must end with CRLF (got fragment around boundary)"
        );
        assert!(
            s.contains(&format!("\r\n--{b}\r\nContent-Type: text/html")),
            "delimiter before html part must end with CRLF"
        );
        assert!(
            s.contains(&format!("\r\n--{b}--\r\n")),
            "closing delimiter"
        );
        assert!(
            !s.contains(&format!("--{b}Content-Type")),
            "must not concatenate boundary token with Content-Type"
        );
    }

    #[test]
    fn multipart_mixed_with_alternative_no_triple_crlf_before_boundaries() {
        let payload = SendPayload {
            from: vec![Address {
                display_name: None,
                local_part: "u".into(),
                domain: Some("example.com".into()),
            }],
            to: vec![Address {
                display_name: None,
                local_part: "v".into(),
                domain: Some("example.com".into()),
            }],
            cc: vec![],
            bcc: vec![],
            subject: Some("s".into()),
            body_plain: Some("p".into()),
            body_html: Some("<p>h</p>".into()),
            attachments: vec![Attachment {
                filename: Some("x.png".into()),
                mime_type: "image/png".into(),
                content: vec![0u8, 1, 2],
            }],
            newsgroups: vec![],
            nntp_in_reply_to: None,
            nntp_references: None,
            smtp_notify: None,
            smtp_in_reply_to: None,
            smtp_references: None,
            smtp_message_id: None,
        };
        let (raw, _) = build_rfc822_from_payload(&payload);
        let s = String::from_utf8_lossy(&raw);
        assert!(
            !s.contains("\r\n\r\n\r\n--"),
            "must not emit an extra blank line before boundary delimiters (triple CRLF)"
        );
    }
}
