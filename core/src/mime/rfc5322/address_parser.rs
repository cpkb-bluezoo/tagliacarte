/*
 * address_parser.rs
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

//! RFC 5322 address list parsing (From, To, Cc, etc.).

use super::email_address::EmailAddress;

/// Parse a comma-separated list of email addresses from a header value.
/// Supports "Display Name" <local@domain> and bare local@domain.
pub fn parse_email_address_list(value: &str) -> Option<Vec<EmailAddress>> {
    let value = value.trim();
    if value.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut pos = 0;
    let bytes = value.as_bytes();
    let len = bytes.len();

    while pos < len {
        skip_ws(bytes, len, &mut pos);
        if pos >= len {
            break;
        }
        let addr = parse_one_address(bytes, len, &mut pos)?;
        out.push(addr);
        skip_ws(bytes, len, &mut pos);
        if pos < len && bytes[pos] == b',' {
            pos += 1;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn skip_ws(bytes: &[u8], len: usize, pos: &mut usize) {
    while *pos < len && (bytes[*pos] == b' ' || bytes[*pos] == b'\t' || bytes[*pos] == b'\r' || bytes[*pos] == b'\n') {
        *pos += 1;
    }
}

fn parse_one_address(bytes: &[u8], len: usize, pos: &mut usize) -> Option<EmailAddress> {
    if *pos >= len {
        return None;
    }
    let mut display_name: Option<String> = None;
    if bytes[*pos] == b'"' {
        *pos += 1;
        let start = *pos;
        while *pos < len {
            if bytes[*pos] == b'\\' && *pos + 1 < len {
                *pos += 2;
                continue;
            }
            if bytes[*pos] == b'"' {
                display_name = Some(String::from_utf8_lossy(&bytes[start..*pos]).into_owned());
                *pos += 1;
                break;
            }
            *pos += 1;
        }
        skip_ws(bytes, len, pos);
    }
    let (local, domain) = if *pos < len && bytes[*pos] == b'<' {
        *pos += 1;
        let start = *pos;
        while *pos < len && bytes[*pos] != b'>' {
            *pos += 1;
        }
        if *pos >= len {
            return None;
        }
        let inner = std::str::from_utf8(&bytes[start..*pos]).ok()?;
        *pos += 1;
        let at = inner.find('@')?;
        if at == 0 || at >= inner.len() - 1 {
            return None;
        }
        (inner[..at].trim().to_string(), inner[at + 1..].trim().to_string())
    } else {
        let start = *pos;
        while *pos < len && bytes[*pos] != b',' && bytes[*pos] != b'<' {
            *pos += 1;
        }
        let part = std::str::from_utf8(&bytes[start..*pos]).ok()?.trim();
        let at = part.find('@')?;
        if at == 0 || at >= part.len() - 1 {
            return None;
        }
        (part[..at].trim().to_string(), part[at + 1..].trim().to_string())
    };
    if local.is_empty() || domain.is_empty() {
        return None;
    }
    Some(EmailAddress::new(display_name, local, domain))
}
