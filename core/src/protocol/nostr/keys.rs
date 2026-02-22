/*
 * keys.rs
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

//! Nostr key handling: nsec/npub bech32 encoding (NIP-19), hex validation, format auto-detection.

use bech32::{Bech32, Hrp};

const HRP_PUBLIC_KEY: &str = "npub";
const HRP_SECRET_KEY: &str = "nsec";

pub fn is_valid_hex_key(key: &str) -> bool {
    key.len() == 64 && key.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_npub(key: &str) -> bool {
    key.starts_with("npub1")
}

pub fn is_nsec(key: &str) -> bool {
    key.starts_with("nsec1")
}

pub fn hex_to_npub(hex_key: &str) -> Result<String, String> {
    if !is_valid_hex_key(hex_key) {
        return Err(String::from("Invalid hex key: must be 64 hex characters"));
    }
    let bytes = hex_to_bytes(hex_key)?;
    let hrp = Hrp::parse(HRP_PUBLIC_KEY).map_err(|e| format!("HRP error: {}", e))?;
    bech32::encode::<Bech32>(hrp, &bytes).map_err(|e| format!("Bech32 encode error: {}", e))
}

pub fn hex_to_nsec(hex_key: &str) -> Result<String, String> {
    if !is_valid_hex_key(hex_key) {
        return Err(String::from("Invalid hex key: must be 64 hex characters"));
    }
    let bytes = hex_to_bytes(hex_key)?;
    let hrp = Hrp::parse(HRP_SECRET_KEY).map_err(|e| format!("HRP error: {}", e))?;
    bech32::encode::<Bech32>(hrp, &bytes).map_err(|e| format!("Bech32 encode error: {}", e))
}

pub fn npub_to_hex(npub: &str) -> Result<String, String> {
    if !is_npub(npub) {
        return Err(String::from("Not an npub: must start with 'npub1'"));
    }
    let (hrp, bytes) = bech32::decode(npub).map_err(|e| format!("Invalid bech32: {}", e))?;
    if hrp.as_str() != HRP_PUBLIC_KEY {
        return Err(format!("Wrong prefix: expected '{}', got '{}'", HRP_PUBLIC_KEY, hrp));
    }
    if bytes.len() != 32 {
        return Err(format!("Invalid key length: expected 32 bytes, got {}", bytes.len()));
    }
    Ok(bytes_to_hex(&bytes))
}

pub fn nsec_to_hex(nsec: &str) -> Result<String, String> {
    if !is_nsec(nsec) {
        return Err(String::from("Not an nsec: must start with 'nsec1'"));
    }
    let (hrp, bytes) = bech32::decode(nsec).map_err(|e| format!("Invalid bech32: {}", e))?;
    if hrp.as_str() != HRP_SECRET_KEY {
        return Err(format!("Wrong prefix: expected '{}', got '{}'", HRP_SECRET_KEY, hrp));
    }
    if bytes.len() != 32 {
        return Err(format!("Invalid key length: expected 32 bytes, got {}", bytes.len()));
    }
    Ok(bytes_to_hex(&bytes))
}

/// Auto-detect npub or hex and return lowercase hex.
pub fn public_key_to_hex(key: &str) -> Result<String, String> {
    let trimmed = key.trim();
    if is_npub(trimmed) {
        npub_to_hex(trimmed)
    } else if is_valid_hex_key(trimmed) {
        Ok(trimmed.to_lowercase())
    } else {
        Err(String::from("Invalid public key: must be npub1... or 64-char hex"))
    }
}

/// Auto-detect nsec or hex and return lowercase hex.
pub fn secret_key_to_hex(key: &str) -> Result<String, String> {
    let trimmed = key.trim();
    if is_nsec(trimmed) {
        nsec_to_hex(trimmed)
    } else if is_valid_hex_key(trimmed) {
        Ok(trimmed.to_lowercase())
    } else {
        Err(String::from("Invalid secret key: must be nsec1... or 64-char hex"))
    }
}

pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err(String::from("Hex string must have even length"));
    }
    let chars: Vec<char> = hex.chars().collect();
    let mut bytes = Vec::with_capacity(chars.len() / 2);
    let mut i = 0;
    while i < chars.len() {
        let high = hex_char_val(chars[i]).ok_or_else(|| format!("Invalid hex char: {}", chars[i]))?;
        let low = hex_char_val(chars[i + 1]).ok_or_else(|| format!("Invalid hex char: {}", chars[i + 1]))?;
        bytes.push((high << 4) | low);
        i += 2;
    }
    Ok(bytes)
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_char_val(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some(c as u8 - b'0'),
        'a'..='f' => Some(c as u8 - b'a' + 10),
        'A'..='F' => Some(c as u8 - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let hex = "deadbeef01234567890abcdef0123456789abcdef0123456789abcdef01234ab";
        let bytes = hex_to_bytes(hex).unwrap();
        assert_eq!(bytes_to_hex(&bytes), hex);
    }

    #[test]
    fn npub_roundtrip() {
        let hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        let npub = hex_to_npub(hex).unwrap();
        assert!(npub.starts_with("npub1"));
        let back = npub_to_hex(&npub).unwrap();
        assert_eq!(back, hex);
    }

    #[test]
    fn nsec_roundtrip() {
        let hex = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa";
        let nsec = hex_to_nsec(hex).unwrap();
        assert!(nsec.starts_with("nsec1"));
        let back = nsec_to_hex(&nsec).unwrap();
        assert_eq!(back, hex);
    }

    #[test]
    fn auto_detect_formats() {
        let hex_pub = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        assert_eq!(public_key_to_hex(hex_pub).unwrap(), hex_pub);

        let npub = hex_to_npub(hex_pub).unwrap();
        assert_eq!(public_key_to_hex(&npub).unwrap(), hex_pub);

        let hex_sec = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa";
        assert_eq!(secret_key_to_hex(hex_sec).unwrap(), hex_sec);

        let nsec = hex_to_nsec(hex_sec).unwrap();
        assert_eq!(secret_key_to_hex(&nsec).unwrap(), hex_sec);
    }

    #[test]
    fn invalid_keys() {
        assert!(public_key_to_hex("not_a_key").is_err());
        assert!(secret_key_to_hex("not_a_key").is_err());
        assert!(npub_to_hex("nsec1abc").is_err());
        assert!(nsec_to_hex("npub1abc").is_err());
    }
}
