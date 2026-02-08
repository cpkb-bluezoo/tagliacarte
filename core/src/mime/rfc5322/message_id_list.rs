/*
 * message_id_list.rs
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

//! RFC 5322 Message-ID list parsing (References, In-Reply-To).

use crate::mime::content_id::ContentID;

/// Parse a list of msg-ids from a header value. Accepts whitespace, comments, and commas as separators.
pub fn parse_message_id_list(value: &str) -> Option<Vec<ContentID>> {
    let value = value.trim();
    if value.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut pos = 0;
    let bytes = value.as_bytes();
    let len = bytes.len();

    while pos < len {
        skip_cfws(bytes, len, &mut pos);
        if pos >= len {
            break;
        }
        if bytes[pos] != b'<' {
            return None;
        }
        pos += 1;
        let start = pos;
        while pos < len && bytes[pos] != b'>' {
            pos += 1;
        }
        if pos >= len {
            return None;
        }
        let inner = std::str::from_utf8(&bytes[start..pos]).ok()?;
        pos += 1;
        let at = inner.find('@')?;
        if at == 0 || at >= inner.len() - 1 {
            continue;
        }
        let local = inner[..at].trim();
        let domain = inner[at + 1..].trim();
        if !local.is_empty() && !domain.is_empty() {
            out.push(ContentID::new(local, domain));
        }
        skip_cfws(bytes, len, &mut pos);
    }
    Some(out)
}

fn skip_cfws(bytes: &[u8], len: usize, pos: &mut usize) {
    while *pos < len {
        let b = bytes[*pos];
        if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
            *pos += 1;
        } else if b == b'(' {
            *pos += 1;
            let mut depth = 1;
            while *pos < len && depth > 0 {
                if bytes[*pos] == b'(' {
                    depth += 1;
                } else if bytes[*pos] == b')' {
                    depth -= 1;
                } else if bytes[*pos] == b'\\' && *pos + 1 < len {
                    *pos += 2;
                    continue;
                }
                *pos += 1;
            }
        } else if b == b',' {
            *pos += 1;
        } else {
            break;
        }
    }
}
