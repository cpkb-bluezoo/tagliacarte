/*
 * encrypted_attachments.rs
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

//! Matrix encrypted attachment handling (AES-256-CTR).
//!
//! Attachments in encrypted rooms are encrypted client-side before upload:
//! 1. Generate random 256-bit key + 128-bit IV (high 8 bytes random, low 8 zero)
//! 2. Encrypt with AES-256-CTR
//! 3. SHA-256 hash the ciphertext
//! 4. Upload ciphertext, include `EncryptedFile` metadata in the event

use aes::Aes256;
use ctr::cipher::{KeyIvInit, StreamCipher};
use sha2::{Digest, Sha256};

use crate::json::JsonWriter;
use crate::store::StoreError;

type Aes256Ctr = ctr::Ctr128BE<Aes256>;

/// Metadata for an encrypted attachment, corresponds to the Matrix `EncryptedFile` object.
pub struct EncryptedFileInfo {
    pub key: [u8; 32],
    pub iv: [u8; 16],
    pub sha256_hash: [u8; 32],
}

/// Encrypt an attachment for an encrypted room.
pub fn encrypt_attachment(plaintext: &[u8]) -> Result<(Vec<u8>, EncryptedFileInfo), StoreError> {
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key)
        .map_err(|e| StoreError::new(format!("getrandom: {}", e)))?;

    // IV: high 8 bytes random, low 8 bytes zero (counter starts at 0)
    let mut iv = [0u8; 16];
    getrandom::getrandom(&mut iv[..8])
        .map_err(|e| StoreError::new(format!("getrandom iv: {}", e)))?;

    let mut ciphertext = plaintext.to_vec();
    let mut cipher = Aes256Ctr::new((&key).into(), (&iv).into());
    cipher.apply_keystream(&mut ciphertext);

    let hash = Sha256::digest(&ciphertext);
    let mut sha256_hash = [0u8; 32];
    sha256_hash.copy_from_slice(&hash);

    Ok((ciphertext, EncryptedFileInfo { key, iv, sha256_hash }))
}

/// Decrypt an encrypted attachment.
pub fn decrypt_attachment(
    ciphertext: &[u8],
    info: &EncryptedFileInfo,
) -> Result<Vec<u8>, StoreError> {
    // Verify hash
    let hash = Sha256::digest(ciphertext);
    if hash.as_slice() != info.sha256_hash {
        return Err(StoreError::new("encrypted attachment hash mismatch"));
    }

    let mut plaintext = ciphertext.to_vec();
    let mut cipher = Aes256Ctr::new((&info.key).into(), (&info.iv).into());
    cipher.apply_keystream(&mut plaintext);
    Ok(plaintext)
}

/// Build the `file` JSON object for an encrypted attachment event.
pub fn build_encrypted_file_json(mxc_url: &str, info: &EncryptedFileInfo) -> Vec<u8> {
    let key_b64url = base64_url_encode(&info.key);
    let iv_b64 = base64_encode(&info.iv);
    let hash_b64 = base64_encode(&info.sha256_hash);

    let mut w = JsonWriter::new();
    w.write_start_object();

    w.write_key("url");
    w.write_string(mxc_url);

    w.write_key("key");
    w.write_start_object();
    w.write_key("kty");
    w.write_string("oct");
    w.write_key("key_ops");
    w.write_start_array();
    w.write_string("encrypt");
    w.write_string("decrypt");
    w.write_end_array();
    w.write_key("alg");
    w.write_string("A256CTR");
    w.write_key("k");
    w.write_string(&key_b64url);
    w.write_key("ext");
    w.write_bool(true);
    w.write_end_object();

    w.write_key("iv");
    w.write_string(&iv_b64);

    w.write_key("hashes");
    w.write_start_object();
    w.write_key("sha256");
    w.write_string(&hash_b64);
    w.write_end_object();

    w.write_key("v");
    w.write_string("v2");

    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Parse an `EncryptedFile` JSON object's key fields.
/// Expects `k` as base64url, `iv` as unpadded base64, `hashes.sha256` as unpadded base64.
pub fn parse_encrypted_file_info(
    key_b64url: &str,
    iv_b64: &str,
    hash_b64: &str,
) -> Result<EncryptedFileInfo, StoreError> {
    let key_bytes = base64_url_decode(key_b64url)
        .map_err(|_| StoreError::new("invalid base64url key"))?;
    if key_bytes.len() != 32 {
        return Err(StoreError::new("key must be 32 bytes"));
    }
    let iv_bytes = base64_decode(iv_b64)
        .map_err(|_| StoreError::new("invalid base64 iv"))?;
    if iv_bytes.len() != 16 {
        return Err(StoreError::new("iv must be 16 bytes"));
    }
    let hash_bytes = base64_decode(hash_b64)
        .map_err(|_| StoreError::new("invalid base64 hash"))?;
    if hash_bytes.len() != 32 {
        return Err(StoreError::new("hash must be 32 bytes"));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&iv_bytes);
    let mut sha256_hash = [0u8; 32];
    sha256_hash.copy_from_slice(&hash_bytes);

    Ok(EncryptedFileInfo { key, iv, sha256_hash })
}

// ── Base64 helpers ───────────────────────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD_NO_PAD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(s)
}

fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn base64_url_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello, Matrix encrypted attachment!";
        let (ciphertext, info) = encrypt_attachment(plaintext).unwrap();
        assert_ne!(ciphertext, plaintext);
        let decrypted = decrypt_attachment(&ciphertext, &info).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_hash_mismatch() {
        let plaintext = b"test data";
        let (mut ciphertext, info) = encrypt_attachment(plaintext).unwrap();
        ciphertext[0] ^= 0xff;
        assert!(decrypt_attachment(&ciphertext, &info).is_err());
    }
}
