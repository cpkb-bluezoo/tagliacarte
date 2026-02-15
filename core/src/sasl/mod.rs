/*
 * mod.rs
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

//! SASL client: key functions only (no Realm). PLAIN, LOGIN, CRAM-MD5, SCRAM-SHA-256.
//!
//! Patterns from gumdrop; reduced to:
//! - mechanism_from_name, mechanism metadata (requires_tls, is_challenge_response)
//! - initial_client_response (first send: PLAIN full, LOGIN/CRAM/SCRAM as appropriate)
//! - respond_to_challenge (for CRAM-MD5 one-shot; SCRAM uses ScramSha256State)

mod mechanism;
mod plain;
mod scram;
mod xoauth2;

pub use mechanism::SaslMechanism;
pub use plain::{encode_plain, initial_response_plain};
pub use scram::{client_first as scram_sha256_client_first, client_final as scram_sha256_client_final, ScramSha256State};
pub use xoauth2::xoauth2_initial_response;

#[derive(Debug)]
pub struct SaslError {
    pub message: String,
}

impl SaslError {
    pub fn invalid(msg: &str) -> Self {
        Self { message: msg.to_string() }
    }
    pub fn invalid_msg(_key: &str) -> impl Fn() -> Self {
        || Self::invalid("invalid server message")
    }
    pub fn plain_invalid() -> Self {
        Self::invalid("invalid PLAIN credentials format")
    }
}

impl std::fmt::Display for SaslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SaslError {}

/// Result of the first client step: either done (PLAIN) or continue with SCRAM state.
#[derive(Debug)]
pub enum SaslFirst {
    /// Single-round: send this as initial response (e.g. PLAIN).
    Done(Vec<u8>),
    /// SCRAM: send this as initial response, then use state in respond_to_challenge.
    ScramContinue(Vec<u8>, ScramSha256State),
}

/// Build the initial client response for the given mechanism.
/// For PLAIN/LOGIN returns Done(bytes); for SCRAM-SHA-256 returns ScramContinue(bytes, state).
/// For XOAUTH2, `password` is the OAuth2 access token and `authcid` is the email address.
pub fn initial_client_response(
    mechanism: SaslMechanism,
    authzid: &str,
    authcid: &str,
    password: &str,
) -> Result<SaslFirst, SaslError> {
    match mechanism {
        SaslMechanism::Plain => {
            let bytes = initial_response_plain(authzid, authcid, password)?;
            Ok(SaslFirst::Done(bytes))
        }
        SaslMechanism::Login => {
            Ok(SaslFirst::Done(vec![]))
        }
        SaslMechanism::CramMd5 => {
            Ok(SaslFirst::Done(vec![]))
        }
        SaslMechanism::ScramSha256 => {
            let (bytes, state) = scram_sha256_client_first(authcid);
            Ok(SaslFirst::ScramContinue(bytes, state))
        }
        SaslMechanism::XOAuth2 => {
            // For XOAUTH2, authcid = email, password = access_token.
            let bytes = xoauth2_initial_response(authcid, password);
            Ok(SaslFirst::Done(bytes))
        }
    }
}

/// Respond to a server challenge (334). For CRAM-MD5 one response; for SCRAM use the state from initial_client_response.
pub fn respond_to_challenge(
    mechanism: SaslMechanism,
    challenge_b64: &str,
    authcid: &str,
    password: &str,
    scram_state: Option<&ScramSha256State>,
) -> Result<Vec<u8>, SaslError> {
    match mechanism {
        SaslMechanism::CramMd5 => cram_md5_response(authcid, password, challenge_b64),
        SaslMechanism::ScramSha256 => {
            let state = scram_state.ok_or_else(|| SaslError::invalid("SCRAM-SHA-256 requires state from initial_client_response"))?;
            scram_sha256_client_final(state, challenge_b64, password)
        }
        SaslMechanism::Plain | SaslMechanism::Login | SaslMechanism::XOAuth2 => {
            Err(SaslError::invalid("PLAIN/LOGIN/XOAUTH2 do not use respond_to_challenge"))
        }
    }
}

/// LOGIN: first challenge is "Username:", second is "Password:". Returns the appropriate response.
pub fn login_respond_to_challenge(challenge_b64: &str, authcid: &str, password: &str) -> Result<Vec<u8>, SaslError> {
    let decoded = base64_decode(challenge_b64)?;
    let s = String::from_utf8_lossy(&decoded).to_lowercase();
    if s.contains("username") || s.trim() == "username:" {
        Ok(base64_encode(authcid.as_bytes()))
    } else if s.contains("password") || s.trim() == "password:" {
        Ok(base64_encode(password.as_bytes()))
    } else {
        Err(SaslError::invalid("unexpected LOGIN challenge"))
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

fn base64_decode(encoded: &str) -> Result<Vec<u8>, SaslError> {
    let encoded = encoded.trim();
    let mut out = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut n = 0u32;
    let mut bits = 0u8;
    for b in encoded.bytes() {
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

fn cram_md5_response(authcid: &str, password: &str, challenge_b64: &str) -> Result<Vec<u8>, SaslError> {
    let challenge_bytes = base64_decode(challenge_b64)?;
    let challenge_str = String::from_utf8(challenge_bytes).map_err(|_| SaslError::invalid("CRAM-MD5 challenge not UTF-8"))?;
    let digest = hmac_md5(password.as_bytes(), challenge_str.as_bytes());
    let hex_digest = bytes_to_hex(&digest);
    let response = format!("{} {}", authcid, hex_digest);
    Ok(response.into_bytes())
}

fn hmac_md5(key: &[u8], data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut mac = md5_hmac(key);
    mac.write_all(data).unwrap();
    mac.finalize().into_bytes().to_vec()
}

fn md5_hmac(key: &[u8]) -> HmacMd5 {
    use hmac::Mac;
    HmacMd5::new_from_slice(key).expect("key length")
}

fn bytes_to_hex(b: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push(HEX[(x >> 4) as usize] as char);
        s.push(HEX[(x & 15) as usize] as char);
    }
    s
}

use hmac::Mac;
type HmacMd5 = hmac::Hmac<md5::Md5>;