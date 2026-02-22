/*
 * crypto.rs
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

//! Nostr cryptography: event ID computation (SHA-256), Schnorr signing/verification (BIP-340),
//! NIP-04 encrypted DMs (AES-256-CBC), NIP-44 versioned encryption (ChaCha20 + HMAC),
//! NIP-59 gift wrap (rumor/seal/gift-wrap chain for NIP-17 private DMs).

use secp256k1::ecdh::shared_secret_point;
use secp256k1::{schnorr, Keypair, Parity, PublicKey, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha256};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use cbc::{Decryptor, Encryptor};
type Aes256CbcEnc = Encryptor<aes::Aes256>;
type Aes256CbcDec = Decryptor<aes::Aes256>;

use chacha20::cipher::StreamCipher;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};

use super::keys::{bytes_to_hex, hex_to_bytes};
use super::types::{Event, KIND_CHAT_MESSAGE, KIND_DM, KIND_GIFT_WRAP, KIND_SEAL};

type HmacSha256 = Hmac<Sha256>;

// ============================================================
// Event ID & Signature
// ============================================================

/// Compute event ID: SHA-256 of `[0, pubkey, created_at, kind, tags, content]`.
pub fn compute_event_id(event: &Event) -> Result<String, String> {
    let serialized = serialize_event_for_id(event)?;
    Ok(bytes_to_hex(&sha256_hash(serialized.as_bytes())))
}

fn serialize_event_for_id(event: &Event) -> Result<String, String> {
    let mut json = String::new();
    json.push_str("[0,\"");
    json.push_str(&event.pubkey.to_lowercase());
    json.push_str("\",");
    json.push_str(&event.created_at.to_string());
    json.push(',');
    json.push_str(&event.kind.to_string());
    json.push_str(",[");
    for (i, tag) in event.tags.iter().enumerate() {
        json.push('[');
        for (j, item) in tag.iter().enumerate() {
            json.push('"');
            json.push_str(&escape_json_string(item));
            json.push('"');
            if j < tag.len() - 1 {
                json.push(',');
            }
        }
        json.push(']');
        if i < event.tags.len() - 1 {
            json.push(',');
        }
    }
    json.push_str("],\"");
    json.push_str(&escape_json_string(&event.content));
    json.push_str("\"]");
    Ok(json)
}

pub fn verify_event_signature(event: &Event) -> Result<bool, String> {
    let secp = Secp256k1::verification_only();
    let pubkey_bytes = hex_to_bytes(&event.pubkey)?;
    if pubkey_bytes.len() != 32 {
        return Err(format!("Invalid pubkey length: {}", pubkey_bytes.len()));
    }
    let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| format!("Invalid public key: {}", e))?;
    let sig_bytes = hex_to_bytes(&event.sig)?;
    if sig_bytes.len() != 64 {
        return Err(format!("Invalid signature length: {}", sig_bytes.len()));
    }
    let signature = schnorr::Signature::from_slice(&sig_bytes)
        .map_err(|e| format!("Invalid signature: {}", e))?;
    let serialized = serialize_event_for_id(event)?;
    let message_hash = sha256_hash(serialized.as_bytes());
    let message = secp256k1::Message::from_digest_slice(&message_hash)
        .map_err(|e| format!("Message error: {}", e))?;
    match secp.verify_schnorr(&signature, &message, &xonly_pubkey) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub fn verify_event_id(event: &Event) -> Result<bool, String> {
    let computed = compute_event_id(event)?;
    Ok(computed.to_lowercase() == event.id.to_lowercase())
}

/// Derive the 32-byte x-only public key hex from a secret key hex.
pub fn get_public_key_from_secret(secret_key_hex: &str) -> Result<String, String> {
    let secret_bytes = hex_to_bytes(secret_key_hex)?;
    if secret_bytes.len() != 32 {
        return Err(format!("Invalid secret key length: {}", secret_bytes.len()));
    }
    let secret_key = SecretKey::from_slice(&secret_bytes)
        .map_err(|e| format!("Invalid secret key: {}", e))?;
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &secret_key);
    let (xonly, _) = XOnlyPublicKey::from_keypair(&keypair);
    Ok(bytes_to_hex(&xonly.serialize()))
}

/// Sign an event: compute ID and Schnorr signature. Event must have pubkey, created_at, kind, tags, content set.
pub fn sign_event(event: &mut Event, secret_key_hex: &str) -> Result<(), String> {
    let secret_bytes = hex_to_bytes(secret_key_hex)?;
    if secret_bytes.len() != 32 {
        return Err(format!("Invalid secret key length: {}", secret_bytes.len()));
    }
    let secret_key = SecretKey::from_slice(&secret_bytes)
        .map_err(|e| format!("Invalid secret key: {}", e))?;
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &secret_key);
    let (xonly, _) = XOnlyPublicKey::from_keypair(&keypair);
    let derived_pubkey = bytes_to_hex(&xonly.serialize());
    if derived_pubkey.to_lowercase() != event.pubkey.to_lowercase() {
        return Err(format!(
            "Public key mismatch: event has {}, secret key produces {}",
            event.pubkey, derived_pubkey
        ));
    }
    let event_id = compute_event_id(event)?;
    event.id = event_id.clone();
    let id_bytes = hex_to_bytes(&event_id)?;
    let message = secp256k1::Message::from_digest_slice(&id_bytes)
        .map_err(|e| format!("Message error: {}", e))?;
    let signature = secp.sign_schnorr_no_aux_rand(&message, &keypair);
    event.sig = bytes_to_hex(signature.as_ref());
    Ok(())
}

/// Generate a random secp256k1 keypair. Returns (secret_key_hex, public_key_hex).
pub fn generate_keypair() -> Result<(String, String), String> {
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| format!("RNG error: {}", e))?;
    let secret_key = SecretKey::from_slice(&seed)
        .map_err(|e| format!("Key generation error: {}", e))?;
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &secret_key);
    let (xonly, _) = XOnlyPublicKey::from_keypair(&keypair);
    Ok((bytes_to_hex(&seed), bytes_to_hex(&xonly.serialize())))
}

// ============================================================
// NIP-04: Encrypted Direct Messages (AES-256-CBC)
// ============================================================

fn nip04_shared_secret(our_secret_hex: &str, their_public_hex: &str) -> Result<[u8; 32], String> {
    let our_bytes = hex_to_bytes(our_secret_hex)?;
    if our_bytes.len() != 32 {
        return Err(String::from("Invalid secret key length"));
    }
    let their_bytes = hex_to_bytes(their_public_hex)?;
    if their_bytes.len() != 32 {
        return Err(String::from("Invalid public key length"));
    }
    let secret_key = SecretKey::from_slice(&our_bytes)
        .map_err(|e| format!("Invalid secret key: {}", e))?;
    let xonly = XOnlyPublicKey::from_slice(&their_bytes)
        .map_err(|e| format!("Invalid public key: {}", e))?;
    let public_key = PublicKey::from_x_only_public_key(xonly, Parity::Even);
    let point = shared_secret_point(&public_key, &secret_key);
    let mut key = [0u8; 32];
    key.copy_from_slice(&point[0..32]);
    Ok(key)
}

/// NIP-04 encrypt: AES-256-CBC with random IV. Returns `base64(ciphertext)?iv=base64(iv)`.
pub fn nip04_encrypt(plaintext: &str, our_secret_hex: &str, their_public_hex: &str) -> Result<String, String> {
    let key = nip04_shared_secret(our_secret_hex, their_public_hex)?;
    let iv: [u8; 16] = rand::random();
    let mut buf = vec![0u8; plaintext.len() + 16];
    let len = plaintext.len();
    buf[..len].copy_from_slice(plaintext.as_bytes());
    let ciphertext = Aes256CbcEnc::new((&key).into(), (&iv).into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, len)
        .map_err(|_| String::from("Encryption failed"))?;
    Ok(format!("{}?iv={}", BASE64.encode(ciphertext), BASE64.encode(iv)))
}

/// NIP-04 decrypt. Content is `base64(ciphertext)?iv=base64(iv)`.
pub fn nip04_decrypt(content: &str, our_secret_hex: &str, their_public_hex: &str) -> Result<String, String> {
    let key = nip04_shared_secret(our_secret_hex, their_public_hex)?;
    let parts: Vec<&str> = content.splitn(2, "?iv=").collect();
    if parts.len() != 2 {
        return Err(String::from("Invalid NIP-04 content format"));
    }
    let ciphertext = BASE64.decode(parts[0].trim()).map_err(|e| format!("Invalid base64 ciphertext: {}", e))?;
    let iv: [u8; 16] = BASE64.decode(parts[1].trim())
        .map_err(|e| format!("Invalid base64 IV: {}", e))?
        .try_into()
        .map_err(|_| String::from("IV must be 16 bytes"))?;
    let mut buf = ciphertext.clone();
    let decrypted = Aes256CbcDec::new((&key).into(), (&iv).into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| String::from("Decryption failed (wrong key or corrupted data)"))?;
    String::from_utf8(decrypted.to_vec()).map_err(|e| format!("Invalid UTF-8: {}", e))
}

/// Create and sign a NIP-04 kind 4 encrypted DM.
pub fn create_signed_dm(
    recipient_pubkey_hex: &str,
    plaintext: &str,
    secret_key_hex: &str,
) -> Result<Event, String> {
    let encrypted = nip04_encrypt(plaintext, secret_key_hex, recipient_pubkey_hex)?;
    let pubkey = get_public_key_from_secret(secret_key_hex)?;
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut event = Event {
        id: String::new(),
        pubkey,
        created_at,
        kind: KIND_DM,
        tags: vec![vec![String::from("p"), recipient_pubkey_hex.to_string()]],
        content: encrypted,
        sig: String::new(),
    };
    sign_event(&mut event, secret_key_hex)?;
    Ok(event)
}

// ============================================================
// NIP-44: Versioned Encryption (v2, ChaCha20 + HMAC-SHA256)
// ============================================================

/// Derive the NIP-44 conversation key from ECDH + HKDF. Symmetric: conv_key(a, B) == conv_key(b, A).
pub fn nip44_conversation_key(our_secret_hex: &str, their_public_hex: &str) -> Result<[u8; 32], String> {
    let our_bytes = hex_to_bytes(our_secret_hex)?;
    if our_bytes.len() != 32 {
        return Err(String::from("Invalid secret key length"));
    }
    let their_bytes = hex_to_bytes(their_public_hex)?;
    if their_bytes.len() != 32 {
        return Err(String::from("Invalid public key length"));
    }
    let secret_key = SecretKey::from_slice(&our_bytes)
        .map_err(|e| format!("Invalid secret key: {}", e))?;
    let xonly = XOnlyPublicKey::from_slice(&their_bytes)
        .map_err(|e| format!("Invalid public key: {}", e))?;
    let public_key = PublicKey::from_x_only_public_key(xonly, Parity::Even);
    let point = shared_secret_point(&public_key, &secret_key);
    let shared_x = &point[0..32];
    let hk = Hkdf::<Sha256>::new(Some(b"nip44-v2"), shared_x);
    let mut conversation_key = [0u8; 32];
    hk.expand(&[], &mut conversation_key)
        .map_err(|_| String::from("HKDF expand failed for conversation key"))?;
    Ok(conversation_key)
}

fn nip44_message_keys(conversation_key: &[u8; 32], nonce: &[u8; 32]) -> Result<([u8; 32], [u8; 12], [u8; 32]), String> {
    let hk = Hkdf::<Sha256>::new(Some(conversation_key), &[]);
    let mut keys = [0u8; 76];
    hk.expand(nonce, &mut keys)
        .map_err(|_| String::from("HKDF expand failed for message keys"))?;
    let mut chacha_key = [0u8; 32];
    chacha_key.copy_from_slice(&keys[0..32]);
    let mut chacha_nonce = [0u8; 12];
    chacha_nonce.copy_from_slice(&keys[32..44]);
    let mut hmac_key = [0u8; 32];
    hmac_key.copy_from_slice(&keys[44..76]);
    Ok((chacha_key, chacha_nonce, hmac_key))
}

fn nip44_calc_padded_len(unpadded_len: usize) -> Result<usize, String> {
    if unpadded_len < 1 {
        return Err(String::from("Plaintext must be at least 1 byte"));
    }
    if unpadded_len > 65535 {
        return Err(String::from("Plaintext must be at most 65535 bytes"));
    }
    if unpadded_len <= 32 {
        return Ok(32);
    }
    let next_power = 1usize << (usize::BITS - (unpadded_len - 1).leading_zeros());
    let chunk = if next_power <= 256 { 32 } else { next_power / 8 };
    Ok(chunk * (((unpadded_len - 1) / chunk) + 1))
}

fn nip44_pad(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let padded_len = nip44_calc_padded_len(plaintext.len())?;
    let mut padded = Vec::with_capacity(2 + padded_len);
    padded.push((plaintext.len() >> 8) as u8);
    padded.push((plaintext.len() & 0xff) as u8);
    padded.extend_from_slice(plaintext);
    padded.resize(2 + padded_len, 0);
    Ok(padded)
}

fn nip44_unpad(padded: &[u8]) -> Result<String, String> {
    if padded.len() < 2 {
        return Err(String::from("Padded data too short"));
    }
    let unpadded_len = ((padded[0] as usize) << 8) | (padded[1] as usize);
    if unpadded_len == 0 {
        return Err(String::from("Invalid padding: zero length"));
    }
    if 2 + unpadded_len > padded.len() {
        return Err(String::from("Invalid padding: length exceeds data"));
    }
    let expected = nip44_calc_padded_len(unpadded_len)?;
    if padded.len() != 2 + expected {
        return Err(String::from("Invalid padding: unexpected padded size"));
    }
    String::from_utf8(padded[2..2 + unpadded_len].to_vec())
        .map_err(|e| format!("Invalid UTF-8: {}", e))
}

fn nip44_hmac_aad(hmac_key: &[u8; 32], message: &[u8], aad: &[u8; 32]) -> Result<[u8; 32], String> {
    let mut mac = HmacSha256::new_from_slice(hmac_key)
        .map_err(|_| String::from("HMAC key error"))?;
    mac.update(aad);
    mac.update(message);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    Ok(out)
}

/// NIP-44 v2 encrypt. Returns `base64(0x02 || nonce || ciphertext || mac)`.
pub fn nip44_encrypt(plaintext: &str, conversation_key: &[u8; 32]) -> Result<String, String> {
    let plaintext_bytes = plaintext.as_bytes();
    if plaintext_bytes.is_empty() || plaintext_bytes.len() > 65535 {
        return Err(String::from("Plaintext length out of range (1..65535)"));
    }
    let nonce: [u8; 32] = rand::random();
    let (chacha_key, chacha_nonce, hmac_key) = nip44_message_keys(conversation_key, &nonce)?;
    let padded = nip44_pad(plaintext_bytes)?;
    let mut ciphertext = padded;
    let mut cipher = chacha20::ChaCha20::new((&chacha_key).into(), (&chacha_nonce).into());
    cipher.apply_keystream(&mut ciphertext);
    let mac = nip44_hmac_aad(&hmac_key, &ciphertext, &nonce)?;
    let mut payload = Vec::with_capacity(1 + 32 + ciphertext.len() + 32);
    payload.push(0x02);
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);
    payload.extend_from_slice(&mac);
    Ok(BASE64.encode(&payload))
}

/// NIP-44 v2 decrypt. Payload is `base64(0x02 || nonce || ciphertext || mac)`.
pub fn nip44_decrypt(payload: &str, conversation_key: &[u8; 32]) -> Result<String, String> {
    if payload.is_empty() {
        return Err(String::from("Empty payload"));
    }
    if payload.starts_with('#') {
        return Err(String::from("Unsupported encryption version"));
    }
    let plen = payload.len();
    if plen < 132 || plen > 87472 {
        return Err(String::from("Invalid payload size"));
    }
    let data = BASE64.decode(payload).map_err(|e| format!("Invalid base64: {}", e))?;
    let dlen = data.len();
    if dlen < 99 || dlen > 65603 {
        return Err(String::from("Invalid decoded data size"));
    }
    if data[0] != 0x02 {
        return Err(format!("Unknown encryption version: {}", data[0]));
    }
    let nonce: [u8; 32] = data[1..33].try_into()
        .map_err(|_| String::from("Invalid nonce"))?;
    let ciphertext = &data[33..dlen - 32];
    let mac: [u8; 32] = data[dlen - 32..dlen].try_into()
        .map_err(|_| String::from("Invalid MAC"))?;
    let (chacha_key, chacha_nonce, hmac_key) = nip44_message_keys(conversation_key, &nonce)?;
    let expected_mac = nip44_hmac_aad(&hmac_key, ciphertext, &nonce)?;
    if !constant_time_eq(&mac, &expected_mac) {
        return Err(String::from("Invalid MAC"));
    }
    let mut padded = ciphertext.to_vec();
    let mut cipher = chacha20::ChaCha20::new((&chacha_key).into(), (&chacha_nonce).into());
    cipher.apply_keystream(&mut padded);
    nip44_unpad(&padded)
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

// ============================================================
// NIP-59: Gift Wrap (Rumor / Seal / Gift Wrap)
// ============================================================

fn random_past_timestamp() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let jitter: u64 = rand::random::<u64>() % 172800; // 0..2 days
    now.saturating_sub(jitter)
}

/// Create a kind 14 rumor (unsigned event). ID is computed, sig is empty.
pub fn create_rumor(
    content: &str,
    tags: Vec<Vec<String>>,
    sender_pubkey_hex: &str,
) -> Result<Event, String> {
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut event = Event {
        id: String::new(),
        pubkey: sender_pubkey_hex.to_string(),
        created_at,
        kind: KIND_CHAT_MESSAGE,
        tags,
        content: content.to_string(),
        sig: String::new(),
    };
    event.id = compute_event_id(&event)?;
    Ok(event)
}

/// Create a kind 13 seal: encrypts the rumor JSON with NIP-44, signed by sender, randomized timestamp.
pub fn create_seal(
    rumor: &Event,
    sender_secret_hex: &str,
    recipient_pubkey_hex: &str,
) -> Result<Event, String> {
    let sender_pubkey = get_public_key_from_secret(sender_secret_hex)?;
    let conv_key = nip44_conversation_key(sender_secret_hex, recipient_pubkey_hex)?;
    let rumor_json = super::types::event_to_json_compact(rumor);
    let encrypted = nip44_encrypt(&rumor_json, &conv_key)?;
    let mut seal = Event {
        id: String::new(),
        pubkey: sender_pubkey,
        created_at: random_past_timestamp(),
        kind: KIND_SEAL,
        tags: Vec::new(),
        content: encrypted,
        sig: String::new(),
    };
    sign_event(&mut seal, sender_secret_hex)?;
    Ok(seal)
}

/// Create a kind 1059 gift wrap: encrypts the seal with NIP-44 using an ephemeral key.
pub fn create_gift_wrap(
    seal: &Event,
    recipient_pubkey_hex: &str,
) -> Result<Event, String> {
    let (eph_secret, eph_pubkey) = generate_keypair()?;
    let conv_key = nip44_conversation_key(&eph_secret, recipient_pubkey_hex)?;
    let seal_json = super::types::event_to_json_compact(seal);
    let encrypted = nip44_encrypt(&seal_json, &conv_key)?;
    let mut wrap = Event {
        id: String::new(),
        pubkey: eph_pubkey,
        created_at: random_past_timestamp(),
        kind: KIND_GIFT_WRAP,
        tags: vec![vec![String::from("p"), recipient_pubkey_hex.to_string()]],
        content: encrypted,
        sig: String::new(),
    };
    sign_event(&mut wrap, &eph_secret)?;
    Ok(wrap)
}

/// Unwrap a kind 1059 gift wrap -> seal -> rumor. Verifies seal signature and anti-impersonation.
pub fn unwrap_gift_wrap(gift_wrap: &Event, our_secret_hex: &str) -> Result<(Event, Event), String> {
    if gift_wrap.kind != KIND_GIFT_WRAP {
        return Err(format!("Expected kind 1059, got kind {}", gift_wrap.kind));
    }
    let outer_conv = nip44_conversation_key(our_secret_hex, &gift_wrap.pubkey)?;
    let seal_json = nip44_decrypt(&gift_wrap.content, &outer_conv)?;
    let seal = super::types::parse_event(&seal_json)?;
    if seal.kind != KIND_SEAL {
        return Err(format!("Expected seal kind 13, got kind {}", seal.kind));
    }
    let seal_valid = verify_event_signature(&seal)?;
    if !seal_valid {
        return Err(String::from("Seal signature verification failed"));
    }
    let inner_conv = nip44_conversation_key(our_secret_hex, &seal.pubkey)?;
    let rumor_json = nip44_decrypt(&seal.content, &inner_conv)?;
    let rumor = super::types::parse_event(&rumor_json)?;
    if rumor.pubkey.to_lowercase() != seal.pubkey.to_lowercase() {
        return Err(String::from("Rumor pubkey does not match seal pubkey (impersonation detected)"));
    }
    Ok((seal, rumor))
}

/// Build full NIP-17 gift wrap chain. Returns (gift_wrap_for_recipient, gift_wrap_for_self).
pub fn create_nip17_dm(
    plaintext: &str,
    sender_secret_hex: &str,
    recipient_pubkey_hex: &str,
) -> Result<(Event, Event), String> {
    let sender_pubkey = get_public_key_from_secret(sender_secret_hex)?;
    let tags = vec![vec![String::from("p"), recipient_pubkey_hex.to_string()]];
    let rumor = create_rumor(plaintext, tags, &sender_pubkey)?;
    let seal = create_seal(&rumor, sender_secret_hex, recipient_pubkey_hex)?;
    let wrap_for_recipient = create_gift_wrap(&seal, recipient_pubkey_hex)?;
    let seal_self = create_seal(&rumor, sender_secret_hex, &sender_pubkey)?;
    let wrap_for_self = create_gift_wrap(&seal_self, &sender_pubkey)?;
    Ok((wrap_for_recipient, wrap_for_self))
}

// ============================================================
// Helpers
// ============================================================

fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

fn escape_json_string(input: &str) -> String {
    let mut output = String::new();
    for c in input.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                output.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => output.push(c),
        }
    }
    output
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nip04_roundtrip() {
        let (sec_a, pub_a) = generate_keypair().unwrap();
        let (sec_b, pub_b) = generate_keypair().unwrap();

        let plaintext = "Hello via NIP-04!";
        let encrypted = nip04_encrypt(plaintext, &sec_a, &pub_b).unwrap();
        let decrypted = nip04_decrypt(&encrypted, &sec_b, &pub_a).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_nip44_conversation_key_symmetric() {
        let (sec_a, pub_a) = generate_keypair().unwrap();
        let (sec_b, pub_b) = generate_keypair().unwrap();
        let ck_ab = nip44_conversation_key(&sec_a, &pub_b).unwrap();
        let ck_ba = nip44_conversation_key(&sec_b, &pub_a).unwrap();
        assert_eq!(ck_ab, ck_ba);
    }

    #[test]
    fn test_nip44_roundtrip() {
        let (sec_a, _pub_a) = generate_keypair().unwrap();
        let (_sec_b, pub_b) = generate_keypair().unwrap();
        let ck = nip44_conversation_key(&sec_a, &pub_b).unwrap();

        for msg in &["hello", "a", &"x".repeat(100), &"y".repeat(65535)] {
            let encrypted = nip44_encrypt(msg, &ck).unwrap();
            let decrypted = nip44_decrypt(&encrypted, &ck).unwrap();
            assert_eq!(&decrypted, msg);
        }
    }

    #[test]
    fn test_nip44_wrong_key_fails() {
        let (sec_a, _) = generate_keypair().unwrap();
        let (_, pub_b) = generate_keypair().unwrap();
        let (_, pub_c) = generate_keypair().unwrap();
        let ck_correct = nip44_conversation_key(&sec_a, &pub_b).unwrap();
        let ck_wrong = nip44_conversation_key(&sec_a, &pub_c).unwrap();
        let encrypted = nip44_encrypt("secret", &ck_correct).unwrap();
        assert!(nip44_decrypt(&encrypted, &ck_wrong).is_err());
    }

    #[test]
    fn test_nip44_padding() {
        assert_eq!(nip44_calc_padded_len(1).unwrap(), 32);
        assert_eq!(nip44_calc_padded_len(32).unwrap(), 32);
        assert_eq!(nip44_calc_padded_len(33).unwrap(), 64);
        assert_eq!(nip44_calc_padded_len(256).unwrap(), 256);
        assert_eq!(nip44_calc_padded_len(65535).unwrap(), 65536);
        assert!(nip44_calc_padded_len(0).is_err());
        assert!(nip44_calc_padded_len(65536).is_err());
    }

    #[test]
    fn test_nip59_gift_wrap_roundtrip() {
        let (sec_alice, pub_alice) = generate_keypair().unwrap();
        let (sec_bob, pub_bob) = generate_keypair().unwrap();

        let (wrap_for_bob, wrap_for_alice) =
            create_nip17_dm("Hello Bob!", &sec_alice, &pub_bob).unwrap();

        assert_eq!(wrap_for_bob.kind, 1059);
        assert_eq!(wrap_for_alice.kind, 1059);

        let (_seal_b, rumor_b) = unwrap_gift_wrap(&wrap_for_bob, &sec_bob).unwrap();
        assert_eq!(rumor_b.content, "Hello Bob!");
        assert_eq!(rumor_b.pubkey.to_lowercase(), pub_alice.to_lowercase());
        assert_eq!(rumor_b.kind, KIND_CHAT_MESSAGE);

        let (_seal_a, rumor_a) = unwrap_gift_wrap(&wrap_for_alice, &sec_alice).unwrap();
        assert_eq!(rumor_a.content, "Hello Bob!");
        assert_eq!(rumor_a.id, rumor_b.id);
    }

    #[test]
    fn test_nip59_wrong_recipient_fails() {
        let (sec_alice, _) = generate_keypair().unwrap();
        let (sec_bob, pub_bob) = generate_keypair().unwrap();
        let (sec_charlie, _) = generate_keypair().unwrap();

        let (wrap_for_bob, _) = create_nip17_dm("Secret", &sec_alice, &pub_bob).unwrap();
        assert!(unwrap_gift_wrap(&wrap_for_bob, &sec_charlie).is_err());

        let (_, rumor) = unwrap_gift_wrap(&wrap_for_bob, &sec_bob).unwrap();
        assert_eq!(rumor.content, "Secret");
    }

    #[test]
    fn test_signed_dm() {
        let (sec_a, pub_a) = generate_keypair().unwrap();
        let (sec_b, pub_b) = generate_keypair().unwrap();

        let event = create_signed_dm(&pub_b, "test message", &sec_a).unwrap();
        assert_eq!(event.kind, KIND_DM);
        assert_eq!(event.pubkey.to_lowercase(), pub_a.to_lowercase());
        assert!(verify_event_id(&event).unwrap());
        assert!(verify_event_signature(&event).unwrap());

        let decrypted = nip04_decrypt(&event.content, &sec_b, &pub_a).unwrap();
        assert_eq!(decrypted, "test message");
    }
}
