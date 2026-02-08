/*
 * plain.rs
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

//! PLAIN SASL (RFC 4616). Requires TLS.

use super::SaslError;

/// Build PLAIN initial response: NUL authzid NUL authcid NUL password (UTF-8).
/// Caller must base64-encode for the wire (e.g. SMTP "AUTH PLAIN <base64>").
pub fn encode_plain(authzid: &str, authcid: &str, password: &str) -> Vec<u8> {
    format!("\0{}\0{}\0{}", authzid, authcid, password).into_bytes()
}

/// Same as encode_plain; returns raw payload for SASL initial response.
pub fn initial_response_plain(authzid: &str, authcid: &str, password: &str) -> Result<Vec<u8>, SaslError> {
    Ok(encode_plain(authzid, authcid, password))
}

/// Decode base64 to bytes (for server-side parsing of PLAIN; not used by client).
#[allow(dead_code)]
pub fn base64_decode(encoded: &[u8]) -> Result<Vec<u8>, SaslError> {
    let mut out = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut n = 0u32;
    let mut bits = 0u8;
    for &b in encoded {
        let v = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => continue,
            _ => return Err(SaslError::invalid("invalid base64")),
        };
        n = (n << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits) as u8);
        }
    }
    Ok(out)
}

/// Parse PLAIN credentials (authzid NUL authcid NUL password). Used when validating.
#[allow(dead_code)]
pub fn parse_plain_credentials(credentials: &[u8]) -> Result<(String, String, String), SaslError> {
    let mut first = None;
    let mut second = None;
    for (i, &b) in credentials.iter().enumerate() {
        if b == 0 {
            if first.is_none() {
                first = Some(i);
            } else {
                second = Some(i);
                break;
            }
        }
    }
    let (f, s) = first.and_then(|f| second.map(|s| (f, s))).ok_or_else(SaslError::plain_invalid)?;
    let authzid = String::from_utf8(credentials[..f].to_vec()).map_err(|_| SaslError::plain_invalid())?;
    let authcid = String::from_utf8(credentials[f + 1..s].to_vec()).map_err(|_| SaslError::plain_invalid())?;
    let password = String::from_utf8(credentials[s + 1..].to_vec()).map_err(|_| SaslError::plain_invalid())?;
    Ok((authzid, authcid, password))
}
