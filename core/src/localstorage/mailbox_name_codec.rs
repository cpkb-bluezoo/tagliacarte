/*
 * mailbox_name_codec.rs
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

//! Encode/decode mailbox names for safe filesystem storage (gumdrop MailboxNameCodec).
//! Uses =XX hex for non-ASCII, path separators, and Windows-forbidden chars.

const SAFE_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";

fn needs_encode(c: u8) -> bool {
    if c > 127 {
        return true;
    }
    if c < 32 {
        return true;
    }
    if c == b'=' {
        return true;
    }
    if c == b'/' || c == b'\\' {
        return true;
    }
    matches!(c, b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|')
        || !SAFE_CHARS.contains(&c)
}

fn hex_digit_value(c: u8) -> i32 {
    match c {
        b'0'..=b'9' => (c - b'0') as i32,
        b'A'..=b'F' => (c - b'A') as i32 + 10,
        b'a'..=b'f' => (c - b'a') as i32 + 10,
        _ => -1,
    }
}

/// Encode a mailbox name for safe filesystem storage.
pub fn encode(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    let utf8 = name.as_bytes();
    let mut needs_encoding = false;
    for &b in utf8 {
        if b > 127 || needs_encode(b) {
            needs_encoding = true;
            break;
        }
    }
    if !needs_encoding {
        return name.to_string();
    }
    let mut out = String::with_capacity(utf8.len() * 3);
    for &b in utf8 {
        let c = b as char;
        if b <= 127 && !needs_encode(b) {
            out.push(c);
        } else {
            out.push_str(&format!("={:02X}", b));
        }
    }
    out
}

/// Decode a filesystem-encoded mailbox name.
pub fn decode(encoded: &str) -> String {
    if encoded.is_empty() || !encoded.contains('=') {
        return encoded.to_string();
    }
    let bytes = encoded.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' && i + 2 < bytes.len() {
            let high = hex_digit_value(bytes[i + 1]);
            let low = hex_digit_value(bytes[i + 2]);
            if high >= 0 && low >= 0 {
                result.push((high << 4 | low) as u8);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}
