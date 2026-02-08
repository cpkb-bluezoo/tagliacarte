/*
 * scram.rs
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

//! SCRAM-SHA-256 SASL client (RFC 5802, 7677).

use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use sha2::{Digest, Sha256};

use super::SaslError;

type HmacSha256 = Hmac<Sha256>;

/// State carried between client-first and client-final (client nonce).
#[derive(Clone, Debug)]
pub struct ScramSha256State {
    pub(crate) client_nonce: String,
    pub(crate) gs2_header: String,
    pub(crate) client_first_bare: String,
}

/// Build client-first-message and state for SCRAM-SHA-256.
/// Format: gs2-header + client-first-bare. gs2-header = "n,," (no channel binding, no authzid).
pub fn client_first(authcid: &str) -> (Vec<u8>, ScramSha256State) {
    let nonce = generate_nonce();
    let gs2_header = "n,,";
    let client_first_bare = format!("n={},r={}", sasl_name(authcid), nonce);
    let message = format!("{}{}", gs2_header, client_first_bare);
    let state = ScramSha256State {
        client_nonce: nonce.clone(),
        gs2_header: gs2_header.to_string(),
        client_first_bare: client_first_bare.clone(),
    };
    (message.into_bytes(), state)
}

/// Build client-final-message from server-first and password.
pub fn client_final(
    state: &ScramSha256State,
    server_first_b64: &str,
    password: &str,
) -> Result<Vec<u8>, SaslError> {
    let server_first = base64_decode_str(server_first_b64)?;
    let server_first_str = String::from_utf8(server_first).map_err(|_| SaslError::invalid("server-first not UTF-8"))?;
    let (nonce, salt_b64, iter_str) = parse_server_first(&server_first_str)?;
    if !nonce.starts_with(&state.client_nonce) {
        return Err(SaslError::invalid("server nonce must extend client nonce"));
    }
    let salt = base64_decode_str(&salt_b64).map_err(|_| SaslError::invalid("invalid salt base64"))?;
    let iterations: u32 = iter_str.parse().map_err(|_| SaslError::invalid("invalid iteration count"))?;

    let salted_password = hi(password, &salt, iterations);
    let client_key = hmac(&salted_password, b"Client Key");
    let stored_key = Sha256::digest(&client_key);
    let server_key = hmac(&salted_password, b"Server Key");

    let client_final_no_proof = format!("c={},r={}", base64_encode(state.gs2_header.as_bytes()), nonce);
    let auth_message = format!(
        "{},{},{}",
        state.client_first_bare,
        &server_first_str,
        client_final_no_proof
    );
    let client_signature = hmac_slice(&stored_key, auth_message.as_bytes());
    let client_proof = xor_in_place(&client_key, &client_signature);
    let client_final_msg = format!("{},p={}", client_final_no_proof, base64_encode(&client_proof));

    let server_signature = hmac_slice(&server_key, auth_message.as_bytes());
    let _ = server_signature; // caller can verify server-final later

    Ok(client_final_msg.into_bytes())
}

fn generate_nonce() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let r: u64 = (t & 0xFFFF_FFFF) as u64;
    format!("{:016x}", r)
}

fn sasl_name(s: &str) -> String {
    s.replace('=', "=3D").replace(',', "=2C")
}

fn parse_server_first(input: &str) -> Result<(String, String, String), SaslError> {
    let mut r = None;
    let mut s = None;
    let mut i = None;
    for part in input.split(',') {
        let part = part.trim();
        if part.starts_with("r=") {
            r = Some(part[2..].to_string());
        } else if part.starts_with("s=") {
            s = Some(part[2..].to_string());
        } else if part.starts_with("i=") {
            i = Some(part[2..].to_string());
        }
    }
    let r = r.ok_or_else(|| SaslError::invalid("missing r in server-first"))?;
    let s = s.ok_or_else(|| SaslError::invalid("missing s in server-first"))?;
    let i = i.ok_or_else(|| SaslError::invalid("missing i in server-first"))?;
    Ok((r, s, i))
}

fn hi(password: &str, salt: &[u8], iterations: u32) -> Vec<u8> {
    let mut out = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut out);
    out.to_vec()
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_slice(key: &[u8], data: &[u8]) -> Vec<u8> {
    hmac(key, data)
}

fn xor_in_place(a: &[u8], b: &[u8]) -> Vec<u8> {
    a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
}

fn base64_encode(b: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((b.len() + 2) / 3 * 4);
    for chunk in b.chunks(3) {
        let n = (chunk[0] as usize) << 16
            | (chunk.get(1).copied().unwrap_or(0) as usize) << 8
            | chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(ALPHABET[n >> 18] as char);
        out.push(ALPHABET[(n >> 12) & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n & 63] as char } else { '=' });
    }
    out
}

fn base64_decode_str(encoded: &str) -> Result<Vec<u8>, SaslError> {
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
