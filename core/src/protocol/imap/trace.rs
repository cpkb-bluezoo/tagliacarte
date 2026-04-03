/*
 * trace.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
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

//! Optional IMAP wire logging for debugging auth and protocol issues.
//!
//! Enable with environment variable **`TAGLIACARTE_IMAP_TRACE=1`** (also `true`, `yes`).
//! For **MIME/body extraction diagnostics** on any store (Maildir, IMAP, …) without full wire
//! logging, set **`TAGLIACARTE_MAIL_BODY_TRACE=1`** — this enables stderr lines from
//! `get_folder_message_json` (sizes, fallback paths, parse errors). IMAP trace implies body trace.
//! Lines are printed to **stderr** with prefixes `>>` (client → server) and `<<` (server → client).
//!
//! By default **secrets are redacted** (LOGIN passwords, AUTHENTICATE payloads, SASL continuations).
//! To log raw outbound lines (**unsafe** — exposes passwords on TLS or in core dumps), set
//! **`TAGLIACARTE_IMAP_TRACE_FULL=1`** as well.
//!
//! **FETCH literals** (message bodies): when trace is enabled, the literal bytes are logged after
//! the line that announces `{N}` (up to 256 KiB, UTF-8 lossy, truncated with a note if longer).
//! Large mail may produce large logs.

use std::sync::OnceLock;

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

static TRACE_ENABLED: OnceLock<bool> = OnceLock::new();
static TRACE_FULL: OnceLock<bool> = OnceLock::new();

/// `TAGLIACARTE_IMAP_TRACE` set to a truthy value.
pub fn enabled() -> bool {
    *TRACE_ENABLED.get_or_init(|| env_truthy("TAGLIACARTE_IMAP_TRACE"))
}

/// Wire trace or **`TAGLIACARTE_MAIL_BODY_TRACE`** — log MIME/body extraction outcomes in the app layer.
pub fn mail_body_debug_enabled() -> bool {
    enabled() || env_truthy("TAGLIACARTE_MAIL_BODY_TRACE")
}

/// `TAGLIACARTE_IMAP_TRACE_FULL` — log outbound lines without redaction (dangerous).
pub fn full_secrets() -> bool {
    *TRACE_FULL.get_or_init(|| env_truthy("TAGLIACARTE_IMAP_TRACE_FULL"))
}

pub fn log_outbound_line(line: &str) {
    if !enabled() {
        return;
    }
    if full_secrets() {
        eprintln!("[imap trace] >> {line}");
    } else {
        eprintln!("[imap trace] >> {}", sanitize_outbound(line));
    }
}

pub fn log_inbound_line(line: &str, pending_literal: Option<u32>) {
    if !enabled() {
        return;
    }
    match pending_literal {
        Some(n) if n > 0 => eprintln!(
            "[imap trace] << {line}  (followed by {n} byte literal; payload logged next if trace enabled)"
        ),
        _ => eprintln!("[imap trace] << {line}"),
    }
}

/// Log a literal byte block (e.g. IMAP FETCH `BODY[]`). Only when [enabled] is true.
/// Output is lossy UTF-8; capped at [MAX_LITERAL_TRACE_BYTES].
pub fn log_inbound_literal_bytes(label: &str, data: &[u8]) {
    if !enabled() {
        return;
    }
    const MAX_LITERAL_TRACE_BYTES: usize = 256 * 1024;
    let total = data.len();
    let truncated = total > MAX_LITERAL_TRACE_BYTES;
    let slice = if truncated {
        &data[..MAX_LITERAL_TRACE_BYTES]
    } else {
        data
    };
    let preview = String::from_utf8_lossy(slice);
    eprintln!(
        "[imap trace] << {label}: {total} bytes{}",
        if truncated {
            format!(" (showing first {MAX_LITERAL_TRACE_BYTES} only)")
        } else {
            String::new()
        }
    );
    eprintln!("[imap trace] << ----- begin {label} -----\n{preview}\n----- end {label} -----");
}

pub fn log_append_command(cmd_head: &str) {
    if !enabled() {
        return;
    }
    let s = cmd_head.trim_end_matches(['\r', '\n']);
    if full_secrets() {
        eprintln!("[imap trace] >> {s}");
    } else {
        eprintln!("[imap trace] >> {}", sanitize_append_preview(s));
    }
}

fn sanitize_append_preview(s: &str) -> String {
    // First line is `TAG APPEND "mailbox" {N}\r\n` — body is written separately; never log it here.
    if s.to_ascii_uppercase().contains("APPEND") {
        return format!("{s}  (binary message literal follows on wire, not logged)");
    }
    s.to_string()
}

pub fn sanitize_outbound(s: &str) -> String {
    let u = s.to_ascii_uppercase();
    if u.contains(" LOGIN ") {
        if let Some(redacted) = redact_login_line(s) {
            return redacted;
        }
    }
    if u.contains("AUTHENTICATE") {
        if let Some(redacted) = redact_authenticate_line(s) {
            return redacted;
        }
    }
    if is_probable_client_sasl_continuation(s) {
        return "<client SASL / base64 response redacted>".to_string();
    }
    s.to_string()
}

fn find_substr_ci(hay: &str, needle: &str) -> Option<usize> {
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    for i in 0..=h.len() - n.len() {
        if h[i..i + n.len()].eq_ignore_ascii_case(n) {
            return Some(i);
        }
    }
    None
}

/// Parse one IMAP quoted string at start of `input`; returns (quoted including `"`…`"`, rest).
fn read_imap_quoted(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut i = 1usize;
    let mut out = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                i += 1;
                out.push(bytes[i] as char);
                i += 1;
            }
            b'"' => {
                let rest = &input[i + 1..];
                return Some((format!("\"{out}\""), rest));
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    None
}

fn redact_login_line(line: &str) -> Option<String> {
    let idx = find_substr_ci(line, " LOGIN ")?;
    let before = &line[..idx];
    let after_login = line[idx + " LOGIN ".len()..].trim_start();
    let (user_q, rest) = read_imap_quoted(after_login)?;
    let rest = rest.trim_start();
    let (_pass_q, tail) = read_imap_quoted(rest)?;
    Some(format!("{before} LOGIN {user_q} \"**redacted**\"{tail}"))
}

fn redact_authenticate_line(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    let mut start_word = 0usize;
    let mut words: Vec<&str> = Vec::new();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'"' {
            depth ^= 1;
        }
        if depth == 0 && b == b' ' {
            if start_word < i {
                words.push(line[start_word..i].trim());
            }
            start_word = i + 1;
        }
    }
    if start_word < line.len() {
        words.push(line[start_word..].trim());
    }
    if words.len() < 2 || !words[1].eq_ignore_ascii_case("AUTHENTICATE") {
        return None;
    }
    if words.len() <= 3 {
        return Some(line.to_string());
    }
    let head = words[..3].join(" ");
    Some(format!("{head} <initial response redacted>"))
}

fn is_probable_client_sasl_continuation(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.contains(' ') {
        return false;
    }
    if t.len() < 8 {
        return false;
    }
    t.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}
