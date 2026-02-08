/*
 * utils.rs
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

//! MIME parsing utilities (RFC 2045 token, RFC 2046 boundary).

/// Checks if a character is valid in an RFC 2045 token.
#[inline]
pub fn is_token_char(c: u8) -> bool {
    matches!(c,
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' |
        b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' |
        b'^' | b'_' | b'`' | b'{' | b'|' | b'}' | b'~'
    )
}

/// Checks if the string is a valid RFC 2045 token (1+ token chars).
pub fn is_token(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(is_token_char)
}

/// Checks if a character is valid in a MIME boundary (RFC 2046).
#[inline]
pub fn is_boundary_char(c: u8) -> bool {
    matches!(c,
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' |
        b'\'' | b'(' | b')' | b'+' | b'_' | b',' | b'-' | b'.' |
        b'/' | b':' | b'=' | b'?'
    )
}

/// Validates MIME boundary: 1-70 chars from boundary set (RFC 2046).
pub fn is_valid_boundary(boundary: &str) -> bool {
    let b = boundary.as_bytes();
    (1..=70).contains(&b.len()) && b.iter().copied().all(is_boundary_char)
}
