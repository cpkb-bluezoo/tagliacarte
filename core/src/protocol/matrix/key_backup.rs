/*
 * key_backup.rs
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

//! Server-Side Secret Storage (SSSS) and key backup for Matrix E2EE.
//!
//! Handles:
//! - Recovery key generation and display (base58)
//! - SSSS encryption of cross-signing private keys stored in account data
//! - Server-side key backup: uploading/downloading Megolm session keys encrypted
//!   with a backup key

use aes::Aes256;
use ctr::cipher::{KeyIvInit, StreamCipher};
use hkdf::Hkdf;
use hmac::{Hmac, Mac as HmacMac};
use sha2::Sha256;

use crate::json::{JsonNumber, JsonWriter};
use crate::store::StoreError;

type Aes256Ctr = ctr::Ctr128BE<Aes256>;
type HmacSha256 = Hmac<Sha256>;

/// A 256-bit recovery key, displayed as base58 for the user.
pub struct RecoveryKey {
    key: [u8; 32],
}

impl RecoveryKey {
    /// Generate a new random recovery key.
    pub fn generate() -> Result<Self, StoreError> {
        let mut key = [0u8; 32];
        getrandom::getrandom(&mut key)
            .map_err(|e| StoreError::new(format!("getrandom: {}", e)))?;
        Ok(Self { key })
    }

    /// Restore from a base58-encoded string (as displayed to the user).
    pub fn from_base58(encoded: &str) -> Result<Self, StoreError> {
        let bytes = bs58_decode(encoded)
            .map_err(|e| StoreError::new(format!("invalid recovery key: {}", e)))?;
        // Format: 2-byte prefix (0x8B 0x01) + 32-byte key + 1-byte parity
        if bytes.len() != 35 {
            return Err(StoreError::new("recovery key has wrong length"));
        }
        if bytes[0] != 0x8B || bytes[1] != 0x01 {
            return Err(StoreError::new("recovery key has wrong prefix"));
        }
        let parity: u8 = bytes[..34].iter().fold(0u8, |acc, &b| acc ^ b);
        if parity != bytes[34] {
            return Err(StoreError::new("recovery key parity check failed"));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[2..34]);
        Ok(Self { key })
    }

    /// Encode as base58 with prefix and parity byte for display.
    pub fn to_base58(&self) -> String {
        let mut data = Vec::with_capacity(35);
        data.push(0x8B);
        data.push(0x01);
        data.extend_from_slice(&self.key);
        let parity: u8 = data.iter().fold(0u8, |acc, &b| acc ^ b);
        data.push(parity);
        bs58_encode(&data)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

// ── SSSS encryption/decryption ───────────────────────────────────────

/// Derive a SSSS storage key from the recovery key using HKDF.
fn derive_ssss_key(recovery_key: &[u8; 32], key_name: &str) -> ([u8; 32], [u8; 16]) {
    let hk = Hkdf::<Sha256>::new(None, recovery_key);
    let info = format!("m.secret_storage.key.{}", key_name);
    let mut okm = [0u8; 48]; // 32 bytes AES key + 16 bytes IV
    hk.expand(info.as_bytes(), &mut okm).expect("HKDF expand");
    let mut aes_key = [0u8; 32];
    aes_key.copy_from_slice(&okm[..32]);
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&okm[32..48]);
    (aes_key, iv)
}

/// Encrypt a secret with SSSS (AES-256-CTR + HMAC-SHA-256).
pub fn ssss_encrypt(
    recovery_key: &[u8; 32],
    key_name: &str,
    plaintext: &[u8],
) -> Result<SsssEncrypted, StoreError> {
    let (aes_key, iv) = derive_ssss_key(recovery_key, key_name);

    let mut ciphertext = plaintext.to_vec();
    let mut cipher = Aes256Ctr::new((&aes_key).into(), (&iv).into());
    cipher.apply_keystream(&mut ciphertext);

    let mut mac = HmacSha256::new_from_slice(&aes_key)
        .map_err(|e| StoreError::new(format!("hmac: {}", e)))?;
    hmac::Mac::update(&mut mac, &ciphertext);
    let mac_bytes = mac.finalize().into_bytes();

    Ok(SsssEncrypted {
        iv: base64_encode(&iv),
        ciphertext: base64_encode(&ciphertext),
        mac: base64_encode(&mac_bytes),
    })
}

/// Decrypt a SSSS-encrypted secret.
pub fn ssss_decrypt(
    recovery_key: &[u8; 32],
    key_name: &str,
    encrypted: &SsssEncrypted,
) -> Result<Vec<u8>, StoreError> {
    let (aes_key, _iv) = derive_ssss_key(recovery_key, key_name);

    let ciphertext = base64_decode(&encrypted.ciphertext)
        .map_err(|_| StoreError::new("invalid base64 ciphertext"))?;
    let expected_mac = base64_decode(&encrypted.mac)
        .map_err(|_| StoreError::new("invalid base64 mac"))?;
    let iv = base64_decode(&encrypted.iv)
        .map_err(|_| StoreError::new("invalid base64 iv"))?;

    // Verify MAC
    let mut mac = HmacSha256::new_from_slice(&aes_key)
        .map_err(|e| StoreError::new(format!("hmac: {}", e)))?;
    hmac::Mac::update(&mut mac, &ciphertext);
    mac.verify_slice(&expected_mac)
        .map_err(|_| StoreError::new("SSSS MAC verification failed"))?;

    let mut plaintext = ciphertext;
    if iv.len() != 16 {
        return Err(StoreError::new("SSSS iv wrong length"));
    }
    let mut iv_arr = [0u8; 16];
    iv_arr.copy_from_slice(&iv);
    let mut cipher = Aes256Ctr::new((&aes_key).into(), (&iv_arr).into());
    cipher.apply_keystream(&mut plaintext);

    Ok(plaintext)
}

pub struct SsssEncrypted {
    pub iv: String,
    pub ciphertext: String,
    pub mac: String,
}

// ── Key backup request/response builders ─────────────────────────────

/// Build `/room_keys/version` creation body.
pub fn build_create_key_backup_body(
    public_key_b64: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("algorithm");
    w.write_string("m.megolm_backup.v1.curve25519-aes-sha2");
    w.write_key("auth_data");
    w.write_start_object();
    w.write_key("public_key");
    w.write_string(public_key_b64);
    w.write_end_object();
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Build body for uploading room keys to backup.
/// `sessions` maps room_id -> { session_id -> encrypted_session_data_json }.
pub fn build_upload_room_keys_body(
    sessions: &std::collections::HashMap<String, std::collections::HashMap<String, BackupSessionData>>,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("rooms");
    w.write_start_object();

    for (room_id, room_sessions) in sessions {
        w.write_key(room_id);
        w.write_start_object();
        w.write_key("sessions");
        w.write_start_object();

        for (session_id, data) in room_sessions {
            w.write_key(session_id);
            w.write_start_object();
            w.write_key("first_message_index");
            w.write_number(JsonNumber::I64(data.first_message_index as i64));
            w.write_key("forwarded_count");
            w.write_number(JsonNumber::I64(data.forwarded_count as i64));
            w.write_key("is_verified");
            w.write_bool(data.is_verified);
            w.write_key("session_data");
            w.write_start_object();
            w.write_key("ciphertext");
            w.write_string(&data.ciphertext);
            w.write_key("ephemeral");
            w.write_string(&data.ephemeral);
            w.write_key("mac");
            w.write_string(&data.mac);
            w.write_end_object();
            w.write_end_object();
        }

        w.write_end_object();
        w.write_end_object();
    }

    w.write_end_object();
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Build account data body for SSSS key description.
pub fn build_ssss_key_description(
    _key_id: &str,
    iv: &str,
    mac: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("algorithm");
    w.write_string("m.secret_storage.v1.aes-hmac-sha2");
    w.write_key("iv");
    w.write_string(iv);
    w.write_key("mac");
    w.write_string(mac);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Build default key account data: `{"key": "<key_id>"}`.
pub fn build_ssss_default_key(key_id: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("key");
    w.write_string(key_id);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Data for one session in a key backup upload.
pub struct BackupSessionData {
    pub first_message_index: u32,
    pub forwarded_count: u32,
    pub is_verified: bool,
    pub ciphertext: String,
    pub ephemeral: String,
    pub mac: String,
}

// ── Backup session decryption (m.megolm_backup.v1.curve25519-aes-sha2) ──

/// Decrypt a single backed-up session using the recovery key.
///
/// Algorithm:
/// 1. ECDH(recovery_private, ephemeral_public) -> shared_secret
/// 2. HKDF-SHA-256(ikm=shared_secret, salt="", info="") -> 80 bytes
///    [0..32] = AES key, [32..64] = HMAC key, [64..80] = AES IV
/// 3. Verify HMAC-SHA-256(hmac_key, ciphertext) == mac (first 8 bytes)
/// 4. AES-256-CTR decrypt -> session export JSON
pub fn decrypt_backup_session(
    recovery_key: &RecoveryKey,
    ephemeral_b64: &str,
    ciphertext_b64: &str,
    mac_b64: &str,
) -> Result<Vec<u8>, StoreError> {
    use vodozemac::{Curve25519PublicKey, Curve25519SecretKey};

    let ephemeral_bytes = base64_decode_permissive(ephemeral_b64)
        .map_err(|_| StoreError::new("invalid base64 ephemeral key"))?;
    if ephemeral_bytes.len() != 32 {
        return Err(StoreError::new("ephemeral key wrong length"));
    }
    let ephemeral_pub = Curve25519PublicKey::from_bytes(
        ephemeral_bytes.as_slice().try_into().unwrap(),
    );
    let ciphertext = base64_decode_permissive(ciphertext_b64)
        .map_err(|_| StoreError::new("invalid base64 ciphertext"))?;
    let mac_bytes = base64_decode_permissive(mac_b64)
        .map_err(|_| StoreError::new("invalid base64 mac"))?;

    let secret_key = Curve25519SecretKey::from_slice(recovery_key.as_bytes());
    let shared_secret = secret_key.diffie_hellman(&ephemeral_pub);

    let hk = Hkdf::<Sha256>::new(Some(&[]), shared_secret.as_bytes());
    let mut okm = [0u8; 80];
    hk.expand(&[], &mut okm)
        .map_err(|_| StoreError::new("HKDF expand failed"))?;

    let aes_key = &okm[0..32];
    let hmac_key = &okm[32..64];
    let aes_iv = &okm[64..80];

    let mut mac = HmacSha256::new_from_slice(hmac_key)
        .map_err(|e| StoreError::new(format!("hmac init: {}", e)))?;
    hmac::Mac::update(&mut mac, &ciphertext);
    let computed_mac = mac.finalize().into_bytes();
    let mac_len = mac_bytes.len().min(computed_mac.len());
    if mac_len < 8 || computed_mac[..mac_len] != mac_bytes[..mac_len] {
        return Err(StoreError::new("backup MAC verification failed"));
    }

    let mut plaintext = ciphertext;
    let mut iv = [0u8; 16];
    iv.copy_from_slice(aes_iv);
    let mut cipher = Aes256Ctr::new(aes_key.into(), (&iv).into());
    cipher.apply_keystream(&mut plaintext);

    Ok(plaintext)
}

/// Encrypt a session export for backup upload using the backup public key.
pub fn encrypt_backup_session(
    recovery_key: &RecoveryKey,
    plaintext: &[u8],
) -> Result<BackupSessionData, StoreError> {
    use vodozemac::{Curve25519PublicKey, Curve25519SecretKey};

    let backup_secret = Curve25519SecretKey::from_slice(recovery_key.as_bytes());
    let backup_public = Curve25519PublicKey::from(&backup_secret);

    let ephemeral_secret = Curve25519SecretKey::new();
    let ephemeral_public = Curve25519PublicKey::from(&ephemeral_secret);
    let shared_secret = ephemeral_secret.diffie_hellman(&backup_public);

    let hk = Hkdf::<Sha256>::new(Some(&[]), shared_secret.as_bytes());
    let mut okm = [0u8; 80];
    hk.expand(&[], &mut okm)
        .map_err(|_| StoreError::new("HKDF expand failed"))?;

    let aes_key = &okm[0..32];
    let hmac_key = &okm[32..64];
    let aes_iv = &okm[64..80];

    let mut ciphertext = plaintext.to_vec();
    let mut iv = [0u8; 16];
    iv.copy_from_slice(aes_iv);
    let mut cipher = Aes256Ctr::new(aes_key.into(), (&iv).into());
    cipher.apply_keystream(&mut ciphertext);

    let mut mac = HmacSha256::new_from_slice(hmac_key)
        .map_err(|e| StoreError::new(format!("hmac init: {}", e)))?;
    hmac::Mac::update(&mut mac, &ciphertext);
    let mac_result = mac.finalize().into_bytes();

    Ok(BackupSessionData {
        first_message_index: 0,
        forwarded_count: 0,
        is_verified: false,
        ciphertext: base64_encode(&ciphertext),
        ephemeral: base64_encode(&ephemeral_public.to_bytes()),
        mac: base64_encode(&mac_result[..8]),
    })
}

/// Get the backup public key (base64) for the given recovery key.
pub fn backup_public_key(recovery_key: &RecoveryKey) -> String {
    use vodozemac::{Curve25519PublicKey, Curve25519SecretKey};
    let secret = Curve25519SecretKey::from_slice(recovery_key.as_bytes());
    let public = Curve25519PublicKey::from(&secret);
    base64_encode(&public.to_bytes())
}

// ── Base64 / Base58 helpers ──────────────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD_NO_PAD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(s)
}

fn base64_decode_permissive(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    let trimmed = s.trim_end_matches('=');
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed)
}

const BS58_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

fn bs58_encode(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let mut digits: Vec<u8> = vec![0];
    for &byte in data {
        let mut carry = byte as u32;
        for digit in digits.iter_mut() {
            carry += (*digit as u32) * 256;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();
    let mut result = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        result.push('1');
    }
    for &d in digits.iter().rev() {
        result.push(BS58_ALPHABET[d as usize] as char);
    }
    result
}

fn bs58_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    // Build reverse lookup
    let mut table = [255u8; 128];
    for (i, &ch) in BS58_ALPHABET.iter().enumerate() {
        table[ch as usize] = i as u8;
    }
    let mut bytes: Vec<u8> = vec![0];
    for ch in s.chars() {
        let idx = if (ch as u32) < 128 { table[ch as usize] } else { 255 };
        if idx == 255 {
            return Err(format!("invalid base58 character: {}", ch));
        }
        let mut carry = idx as u32;
        for byte in bytes.iter_mut() {
            carry += (*byte as u32) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    let leading_ones = s.chars().take_while(|&c| c == '1').count();
    let mut result = Vec::with_capacity(leading_ones + bytes.len());
    for _ in 0..leading_ones {
        result.push(0);
    }
    for &b in bytes.iter().rev() {
        result.push(b);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_key_roundtrip() {
        let key = RecoveryKey::generate().unwrap();
        let encoded = key.to_base58();
        let decoded = RecoveryKey::from_base58(&encoded).unwrap();
        assert_eq!(key.key, decoded.key);
    }

    #[test]
    fn test_ssss_roundtrip() {
        let recovery = RecoveryKey::generate().unwrap();
        let plaintext = b"secret cross-signing key data";
        let encrypted = ssss_encrypt(recovery.as_bytes(), "test_key", plaintext).unwrap();
        let decrypted = ssss_decrypt(recovery.as_bytes(), "test_key", &encrypted).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_base58_roundtrip() {
        let data = vec![0x8B, 0x01, 0xAA, 0xBB, 0xCC];
        let encoded = bs58_encode(&data);
        let decoded = bs58_decode(&encoded).unwrap();
        assert_eq!(data, decoded);
    }
}
