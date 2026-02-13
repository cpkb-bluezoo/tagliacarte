/*
 * rfc2047.rs
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

//! RFC 2047 encoded-word decoding (e.g. =?charset?q?text?=).
//! Used for header values and parameter values; SMTPUTF8 handling for header bytes→string and raw 8-bit.

use crate::mime::base64;
use crate::mime::quoted_printable;

const REPLACEMENT_CHAR: char = '\u{FFFD}';

/// Expand RFC 2047 encoded-words in the string. Does not apply raw 8-bit re-interpretation (no SMTPUTF8).
/// Used for parameter values and as a step inside full header decode.
pub fn decode_encoded_words(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        if let Some(start) = find_encoded_word_start(bytes, pos) {
            // Copy literal from pos to start
            out.push_str(std::str::from_utf8(&bytes[pos..start]).unwrap_or(""));
            pos = start;
            if let Some((decoded, end)) = decode_one_encoded_word(bytes, len, &mut pos) {
                out.push_str(&decoded);
                pos = end;
            } else {
                out.push_str(std::str::from_utf8(&bytes[pos..pos + 2.min(len - pos)]).unwrap_or(""));
                pos = (pos + 2).min(len);
            }
        } else {
            out.push_str(std::str::from_utf8(&bytes[pos..]).unwrap_or(""));
            break;
        }
    }
    out
}

fn find_encoded_word_start(bytes: &[u8], from: usize) -> Option<usize> {
    let rest = bytes.get(from..)?;
    let needle = b"=?";
    rest.windows(needle.len())
        .position(|w| w == needle)
        .map(|i| from + i)
}

/// Decode one encoded-word at current pos. Returns (decoded_string, position_after_?=) or None.
fn decode_one_encoded_word(bytes: &[u8], len: usize, pos: &mut usize) -> Option<(String, usize)> {
    if *pos + 4 > len || &bytes[*pos..*pos + 2] != b"=?" {
        return None;
    }
    *pos += 2;
    let charset_start = *pos;
    let qmark1 = bytes[*pos..].iter().position(|&b| b == b'?')? + *pos;
    if qmark1 < charset_start + 1 || qmark1 + 2 >= len {
        return None;
    }
    let charset = std::str::from_utf8(&bytes[charset_start..qmark1]).ok()?.trim();
    let encoding = bytes[qmark1 + 1].to_ascii_lowercase();
    if bytes[qmark1 + 2] != b'?' {
        return None;
    }
    *pos = qmark1 + 3;
    let payload_start = *pos;
    let rest = &bytes[*pos..];
    let end_in_rest = rest.windows(2).position(|w| w[0] == b'?' && w[1] == b'=')?;
    let payload_end = *pos + end_in_rest;
    *pos = payload_end + 2; // consume ?=

    let payload = &bytes[payload_start..payload_end];
    let decoded_bytes = match encoding {
        b'b' => decode_b(payload),
        b'q' => decode_q(payload),
        _ => return None,
    };
    let decoded = charset_bytes_to_string(&decoded_bytes, charset);
    Some((decoded, *pos))
}

trait ToAsciiLowercase {
    fn to_ascii_lowercase(self) -> u8;
}
impl ToAsciiLowercase for u8 {
    fn to_ascii_lowercase(self) -> u8 {
        if self >= b'A' && self <= b'Z' {
            self + (b'a' - b'A')
        } else {
            self
        }
    }
}

fn decode_b(payload: &[u8]) -> Vec<u8> {
    let mut src_pos = 0;
    let mut dst = vec![0u8; payload.len() * 3 / 4 + 4];
    let mut dst_pos = 0;
    base64::decode(payload, &mut src_pos, &mut dst, &mut dst_pos, payload.len(), true);
    dst.truncate(dst_pos);
    dst
}

/// Q encoding: _ = space, rest is quoted-printable.
fn decode_q(payload: &[u8]) -> Vec<u8> {
    let mut preprocessed = Vec::with_capacity(payload.len() * 2);
    for &b in payload {
        if b == b'_' {
            preprocessed.extend_from_slice(b"=20");
        } else {
            preprocessed.push(b);
        }
    }
    let mut src_pos = 0;
    let mut dst = vec![0u8; preprocessed.len()];
    let mut dst_pos = 0;
    quoted_printable::decode(
        &preprocessed,
        &mut src_pos,
        &mut dst,
        &mut dst_pos,
        preprocessed.len(),
        true,
    );
    dst.truncate(dst_pos);
    dst
}

fn charset_bytes_to_string(bytes: &[u8], charset: &str) -> String {
    let charset_lower = charset.to_ascii_lowercase();
    match charset_lower.as_str() {
        "utf-8" | "utf8" => String::from_utf8_lossy(bytes).into_owned(),
        "iso-8859-1" | "latin1" | "iso_8859-1" => bytes.iter().map(|&b| b as char).collect(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[allow(dead_code)]
trait ToAsciiLowercaseStr {
    fn to_ascii_lowercase(&self) -> String;
}
impl ToAsciiLowercaseStr for str {
    fn to_ascii_lowercase(&self) -> String {
        self.chars()
            .map(|c| {
                if c >= 'A' && c <= 'Z' {
                    ((c as u8) + (b'a' - b'A')) as char
                } else {
                    c
                }
            })
            .collect()
    }
}

/// Convert header value bytes to string. When smtp_utf8, try UTF-8 first and fall back to ISO-8859-1 on U+FFFD.
/// Does not apply RFC 2047 or raw 8-bit handling.
pub fn bytes_to_string(bytes: &[u8], smtp_utf8: bool) -> String {
    if smtp_utf8 {
        bytes_to_string_utf8_then_iso8859(bytes)
    } else {
        bytes.iter().map(|&b| b as char).collect::<String>()
    }
}

/// Decode header value from raw bytes. When smtp_utf8, try UTF-8 first and fall back to ISO-8859-1 on U+FFFD.
pub fn decode_header_value_bytes(bytes: &[u8], smtp_utf8: bool) -> String {
    let raw = if smtp_utf8 {
        bytes_to_string_utf8_then_iso8859(bytes)
    } else {
        bytes.iter().map(|&b| b as char).collect::<String>()
    };
    let decoded = decode_encoded_words(&raw);
    handle_raw_8bit_data(&decoded, smtp_utf8)
}

/// Decode header value from an already-decoded string (e.g. after parser bytes→string). Applies encoded-words and raw 8-bit.
#[allow(dead_code)]
pub fn decode_header_value_string(s: &str, smtp_utf8: bool) -> String {
    let decoded = decode_encoded_words(s);
    handle_raw_8bit_data(&decoded, smtp_utf8)
}

fn bytes_to_string_utf8_then_iso8859(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) if !s.contains(REPLACEMENT_CHAR) => s.to_string(),
        _ => bytes.iter().map(|&b| b as char).collect::<String>(),
    }
}

/// Re-interpret raw 8-bit segments. Do not damage strings that already contain code points > 255.
pub fn handle_raw_8bit_data(s: &str, smtp_utf8: bool) -> String {
    let has_high = s.chars().any(|c| c as u32 > 0x7F || c == '\0');
    if !has_high {
        return s.to_string();
    }
    if s.chars().any(|c| c as u32 > 255) {
        return s.to_string();
    }
    let bytes: Vec<u8> = s.chars().map(|c| c as u8).collect();
    if smtp_utf8 {
        match std::str::from_utf8(&bytes) {
            Ok(t) if !t.contains(REPLACEMENT_CHAR) => t.to_string(),
            _ => bytes.iter().map(|&b| b as char).collect::<String>(),
        }
    } else {
        match std::str::from_utf8(&bytes) {
            Ok(t) if !t.contains(REPLACEMENT_CHAR) => t.to_string(),
            _ => bytes.iter().map(|&b| b as char).collect::<String>(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_encoded_words_b() {
        // =?UTF-8?B?SGVsbG8=?= -> Hello
        let s = "=?UTF-8?B?SGVsbG8=?=";
        assert_eq!(decode_encoded_words(s), "Hello");
    }

    #[test]
    fn decode_encoded_words_q() {
        // =?UTF-8?Q?Hello_World?= -> Hello World
        let s = "=?UTF-8?Q?Hello_World?=";
        assert_eq!(decode_encoded_words(s), "Hello World");
    }

    #[test]
    fn decode_encoded_words_mixed() {
        let s = "Hello =?UTF-8?B?V29ybGQ=?=!";
        assert_eq!(decode_encoded_words(s), "Hello World!");
    }

    #[test]
    fn handle_raw_8bit_ascii_unchanged() {
        assert_eq!(handle_raw_8bit_data("abc", true), "abc");
        assert_eq!(handle_raw_8bit_data("abc", false), "abc");
    }

    #[test]
    fn handle_raw_8bit_above_255_unchanged() {
        let s = "Hello \u{1F600}";
        assert_eq!(handle_raw_8bit_data(s, true), s);
        assert_eq!(handle_raw_8bit_data(s, false), s);
    }
}
