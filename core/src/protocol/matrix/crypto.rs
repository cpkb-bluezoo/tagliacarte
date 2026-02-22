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

//! CryptoMachine: orchestrates all Matrix E2EE operations.
//!
//! Holds the Olm `Account`, manages Olm and Megolm sessions, device key caches,
//! and delegates persistence to `CryptoStore`. All public methods build JSON
//! payloads with `JsonWriter` (no serde in application-level serialization).

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use vodozemac::olm::{
    Account, OlmMessage, Session as OlmSession, SessionConfig as OlmSessionConfig,
};
use vodozemac::megolm::{
    GroupSession, InboundGroupSession, SessionConfig as MegolmSessionConfig,
    SessionKey, DecryptedMessage,
};
use vodozemac::{Curve25519PublicKey, Ed25519PublicKey, KeyId};

use crate::json::{JsonNumber, JsonWriter};
use crate::store::StoreError;

use super::crypto_store::CryptoStore;
use super::device::DeviceKeys;

const MAX_OTK_FRACTION: usize = 2;
const MEGOLM_ROTATION_MESSAGES: u32 = 100;
const MEGOLM_ROTATION_SECS: u64 = 7 * 24 * 3600; // 1 week

/// Central orchestrator for all E2EE operations on one device.
pub struct CryptoMachine {
    account: RwLock<Account>,
    store: CryptoStore,
    olm_sessions: RwLock<HashMap<String, Vec<OlmSession>>>,
    inbound_group_sessions: RwLock<HashMap<(String, String), InboundGroupSession>>,
    outbound_group_sessions: RwLock<HashMap<String, OutboundInfo>>,
    pub device_keys_cache: RwLock<HashMap<String, HashMap<String, DeviceKeys>>>,
    pub user_id: String,
    pub device_id: String,
}

struct OutboundInfo {
    session: GroupSession,
    created_at: u64,
}

impl CryptoMachine {
    /// Create a new CryptoMachine, loading an existing account from the store
    /// or creating a fresh one.
    pub fn new_or_load(
        user_id: &str,
        device_id: &str,
        access_token: &str,
    ) -> Result<Self, StoreError> {
        let store = CryptoStore::open(user_id, access_token)?;
        let account = match store.load_account() {
            Ok(Some(a)) => a,
            Ok(None) => {
                let a = Account::new();
                store.save_account(&a)?;
                a
            }
            Err(e) => {
                eprintln!("[matrix] WARNING: crypto store load failed ({}), creating fresh account — old E2EE sessions are lost", e);
                let a = Account::new();
                store.save_account(&a)?;
                a
            }
        };

        Ok(Self {
            account: RwLock::new(account),
            store,
            olm_sessions: RwLock::new(HashMap::new()),
            inbound_group_sessions: RwLock::new(HashMap::new()),
            outbound_group_sessions: RwLock::new(HashMap::new()),
            device_keys_cache: RwLock::new(HashMap::new()),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
        })
    }

    // ── Identity ─────────────────────────────────────────────────────

    pub fn curve25519_key(&self) -> Curve25519PublicKey {
        self.account.read().unwrap().curve25519_key()
    }

    pub fn ed25519_key(&self) -> Ed25519PublicKey {
        self.account.read().unwrap().ed25519_key()
    }

    pub fn sign(&self, message: &str) -> String {
        let sig = self.account.read().unwrap().sign(message);
        sig.to_base64()
    }

    // ── One-time keys ────────────────────────────────────────────────

    /// Generate one-time keys if the server count is below half the max.
    /// Returns the unpublished one-time keys (to be uploaded).
    pub fn generate_one_time_keys_if_needed(
        &self,
        server_count: usize,
    ) -> HashMap<KeyId, Curve25519PublicKey> {
        let mut account = self.account.write().unwrap();
        let max = account.max_number_of_one_time_keys();
        let target = max / MAX_OTK_FRACTION;
        if server_count < target {
            let to_generate = target - server_count;
            account.generate_one_time_keys(to_generate);
        }
        account.one_time_keys()
    }

    pub fn mark_keys_as_published(&self) {
        let mut account = self.account.write().unwrap();
        account.mark_keys_as_published();
        let _ = self.store.save_account(&account);
    }

    pub fn fallback_key(&self) -> HashMap<KeyId, Curve25519PublicKey> {
        self.account.read().unwrap().fallback_key()
    }

    pub fn generate_fallback_key(&self) {
        let mut account = self.account.write().unwrap();
        account.generate_fallback_key();
        let _ = self.store.save_account(&account);
    }

    // ── Signed device_keys JSON ──────────────────────────────────────

    /// Build the signed `device_keys` JSON payload for `/keys/upload`.
    pub fn device_keys_json(&self) -> Vec<u8> {
        let account = self.account.read().unwrap();
        let ed25519 = account.ed25519_key();
        let curve25519 = account.curve25519_key();

        let ed_key_id = format!("ed25519:{}", self.device_id);
        let curve_key_id = format!("curve25519:{}", self.device_id);
        let ed_b64 = ed25519.to_base64();
        let curve_b64 = curve25519.to_base64();

        // Build the canonical JSON (keys sorted) for signing
        let canonical = format!(
            "{{\"algorithms\":[\"m.megolm.v1.aes-sha2\",\"m.olm.v1.curve25519-aes-sha2\"],\
            \"device_id\":\"{}\",\
            \"keys\":{{\"{}\":\"{}\",\"{}\":\"{}\"}},\
            \"user_id\":\"{}\"}}",
            self.device_id,
            curve_key_id, curve_b64,
            ed_key_id, ed_b64,
            self.user_id,
        );
        let signature = account.sign(&canonical);
        let sig_key = format!("ed25519:{}", self.device_id);
        drop(account);

        let mut w = JsonWriter::new();
        w.write_start_object();

        w.write_key("algorithms");
        w.write_start_array();
        w.write_string("m.megolm.v1.aes-sha2");
        w.write_string("m.olm.v1.curve25519-aes-sha2");
        w.write_end_array();

        w.write_key("device_id");
        w.write_string(&self.device_id);

        w.write_key("keys");
        w.write_start_object();
        w.write_key(&curve_key_id);
        w.write_string(&curve_b64);
        w.write_key(&ed_key_id);
        w.write_string(&ed_b64);
        w.write_end_object();

        w.write_key("signatures");
        w.write_start_object();
        w.write_key(&self.user_id);
        w.write_start_object();
        w.write_key(&sig_key);
        w.write_string(&signature.to_base64());
        w.write_end_object();
        w.write_end_object();

        w.write_key("user_id");
        w.write_string(&self.user_id);

        w.write_end_object();
        w.take_buffer().to_vec()
    }

    /// Build the signed one-time keys JSON for `/keys/upload`.
    pub fn one_time_keys_json(
        &self,
        keys: &HashMap<vodozemac::KeyId, Curve25519PublicKey>,
    ) -> Vec<u8> {
        let account = self.account.read().unwrap();
        let mut w = JsonWriter::new();
        w.write_start_object();
        for (key_id, public_key) in keys {
            let key_id_str: String = (*key_id).into();
            let id_str = format!("signed_curve25519:{}", key_id_str);
            let key_b64 = public_key.to_base64();

            // Canonical JSON for signing
            let canonical = format!("{{\"key\":\"{}\"}}", key_b64);
            let signature = account.sign(&canonical);

            w.write_key(&id_str);
            w.write_start_object();
            w.write_key("key");
            w.write_string(&key_b64);
            w.write_key("signatures");
            w.write_start_object();
            w.write_key(&self.user_id);
            w.write_start_object();
            w.write_key(&format!("ed25519:{}", self.device_id));
            w.write_string(&signature.to_base64());
            w.write_end_object();
            w.write_end_object();
            w.write_end_object();
        }
        w.write_end_object();
        w.take_buffer().to_vec()
    }

    // ── Olm sessions ────────────────────────────────────────────────

    /// Create an outbound Olm session to a device.
    pub fn create_outbound_olm_session(
        &self,
        identity_key: &Curve25519PublicKey,
        one_time_key: &Curve25519PublicKey,
    ) -> Result<(), StoreError> {
        let account = self.account.read().unwrap();
        let session = account.create_outbound_session(
            OlmSessionConfig::default(),
            *identity_key,
            *one_time_key,
        );
        let sender_key = identity_key.to_base64();
        self.store.save_olm_session(&sender_key, &session)?;

        let mut sessions = self.olm_sessions.write().unwrap();
        sessions.entry(sender_key).or_default().push(session);
        Ok(())
    }

    /// Create an inbound Olm session from a pre-key message.
    pub fn create_inbound_olm_session(
        &self,
        their_identity_key: &Curve25519PublicKey,
        message: &OlmMessage,
    ) -> Result<Vec<u8>, StoreError> {
        let pre_key = match message {
            OlmMessage::PreKey(pk) => pk,
            _ => return Err(StoreError::new("Expected pre-key message for inbound session")),
        };

        let mut account = self.account.write().unwrap();
        let result = account.create_inbound_session(*their_identity_key, pre_key)
            .map_err(|e| StoreError::new(format!("create inbound olm session: {}", e)))?;

        self.store.save_account(&account)?;
        let sender_key = their_identity_key.to_base64();
        self.store.save_olm_session(&sender_key, &result.session)?;

        let mut sessions = self.olm_sessions.write().unwrap();
        sessions.entry(sender_key).or_default().push(result.session);
        Ok(result.plaintext)
    }

    /// Encrypt a plaintext payload to a recipient via Olm.
    pub fn olm_encrypt(
        &self,
        recipient_curve25519: &Curve25519PublicKey,
    ) -> Result<Option<OlmEncryptHandle>, StoreError> {
        let sender_key = recipient_curve25519.to_base64();
        let mut sessions = self.olm_sessions.write().unwrap();

        if let Some(session_list) = sessions.get_mut(&sender_key) {
            if let Some(session) = session_list.first_mut() {
                return Ok(Some(OlmEncryptHandle {
                    session_ptr: session as *mut OlmSession,
                    sender_key: sender_key.clone(),
                }));
            }
        }

        // Load from store
        let mut loaded = self.store.load_olm_sessions(&sender_key)?;
        if loaded.is_empty() {
            return Ok(None);
        }
        let handle = OlmEncryptHandle {
            session_ptr: &mut loaded[0] as *mut OlmSession,
            sender_key: sender_key.clone(),
        };
        sessions.insert(sender_key, loaded);
        Ok(Some(handle))
    }

    /// Encrypt plaintext and persist updated session state.
    pub fn olm_encrypt_to(
        &self,
        recipient_curve25519: &Curve25519PublicKey,
        plaintext: &[u8],
    ) -> Result<OlmMessage, StoreError> {
        let sender_key = recipient_curve25519.to_base64();
        let mut sessions = self.olm_sessions.write().unwrap();

        let session_list = if let Some(sl) = sessions.get_mut(&sender_key) {
            sl
        } else {
            let loaded = self.store.load_olm_sessions(&sender_key)?;
            if loaded.is_empty() {
                return Err(StoreError::new("No Olm session for recipient"));
            }
            sessions.insert(sender_key.clone(), loaded);
            sessions.get_mut(&sender_key).unwrap()
        };

        let session = session_list.first_mut()
            .ok_or_else(|| StoreError::new("No Olm session for recipient"))?;
        let message = session.encrypt(plaintext);
        self.store.save_olm_session(&sender_key, session)?;
        Ok(message)
    }

    /// Decrypt an Olm message from a sender.
    pub fn olm_decrypt(
        &self,
        sender_curve25519: &Curve25519PublicKey,
        message: &OlmMessage,
    ) -> Result<Vec<u8>, StoreError> {
        let sender_key = sender_curve25519.to_base64();

        // Try existing sessions first
        {
            let mut sessions = self.olm_sessions.write().unwrap();
            if let Some(session_list) = sessions.get_mut(&sender_key) {
                for session in session_list.iter_mut() {
                    match session.decrypt(message) {
                        Ok(plaintext) => {
                            self.store.save_olm_session(&sender_key, session)?;
                            return Ok(plaintext);
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        // Try loaded sessions from store
        let mut loaded = self.store.load_olm_sessions(&sender_key)?;
        for session in loaded.iter_mut() {
            match session.decrypt(message) {
                Ok(plaintext) => {
                    self.store.save_olm_session(&sender_key, session)?;
                    let mut sessions = self.olm_sessions.write().unwrap();
                    sessions.insert(sender_key, loaded);
                    return Ok(plaintext);
                }
                Err(_) => continue,
            }
        }

        // Create inbound session if pre-key message
        let plaintext = self.create_inbound_olm_session(sender_curve25519, message)?;
        Ok(plaintext)
    }

    // ── Megolm (outbound) ────────────────────────────────────────────

    /// Get or create an outbound Megolm session for a room.
    /// Returns `(session_id, session_key, is_new)`.
    pub fn get_or_create_outbound_group_session(
        &self,
        room_id: &str,
    ) -> Result<(String, SessionKey, bool), StoreError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let mut sessions = self.outbound_group_sessions.write().unwrap();
        if let Some(info) = sessions.get(room_id) {
            let age = now.saturating_sub(info.created_at);
            if info.session.message_index() < MEGOLM_ROTATION_MESSAGES && age < MEGOLM_ROTATION_SECS {
                let id = info.session.session_id();
                let key = info.session.session_key();
                return Ok((id, key, false));
            }
        }

        let session = GroupSession::new(MegolmSessionConfig::default());
        let session_id = session.session_id();
        let session_key = session.session_key();

        // Also create our own inbound copy so we can decrypt our own messages
        let inbound = InboundGroupSession::new(&session_key, MegolmSessionConfig::default());
        self.store.save_inbound_group_session(room_id, &session_id, &inbound)?;
        {
            let mut igs = self.inbound_group_sessions.write().unwrap();
            igs.insert((room_id.to_string(), session_id.clone()), inbound);
        }

        self.store.save_outbound_group_session(room_id, &session)?;
        sessions.insert(room_id.to_string(), OutboundInfo {
            session,
            created_at: now,
        });

        Ok((session_id, session_key, true))
    }

    /// Encrypt a room event with the outbound Megolm session.
    pub fn megolm_encrypt(
        &self,
        room_id: &str,
        event_type: &str,
        content_json: &[u8],
    ) -> Result<MegolmEncrypted, StoreError> {
        let mut sessions = self.outbound_group_sessions.write().unwrap();
        let info = sessions.get_mut(room_id)
            .ok_or_else(|| StoreError::new("No outbound Megolm session for room"))?;

        // Build plaintext: {"type":"...","content":{...},"room_id":"!..."}
        let mut plaintext = Vec::with_capacity(content_json.len() + 256);
        plaintext.extend_from_slice(b"{\"type\":\"");
        for &b in event_type.as_bytes() {
            if b == b'"' { plaintext.extend_from_slice(b"\\\""); }
            else { plaintext.push(b); }
        }
        plaintext.extend_from_slice(b"\",\"content\":");
        plaintext.extend_from_slice(content_json);
        plaintext.extend_from_slice(b",\"room_id\":\"");
        for &b in room_id.as_bytes() {
            if b == b'"' { plaintext.extend_from_slice(b"\\\""); }
            else { plaintext.push(b); }
        }
        plaintext.extend_from_slice(b"\"}");

        let megolm_msg = info.session.encrypt(&plaintext);
        let session_id = info.session.session_id();

        self.store.save_outbound_group_session(room_id, &info.session)?;

        let account = self.account.read().unwrap();
        Ok(MegolmEncrypted {
            algorithm: "m.megolm.v1.aes-sha2".to_string(),
            sender_key: account.curve25519_key().to_base64(),
            ciphertext: megolm_msg.to_base64(),
            session_id,
            device_id: self.device_id.clone(),
        })
    }

    // ── Megolm (inbound) ─────────────────────────────────────────────

    /// Store an inbound group session received via `m.room_key`.
    pub fn add_inbound_group_session(
        &self,
        room_id: &str,
        session_id: &str,
        session_key: &SessionKey,
    ) -> Result<(), StoreError> {
        let session = InboundGroupSession::new(session_key, MegolmSessionConfig::default());
        self.store.save_inbound_group_session(room_id, session_id, &session)?;
        let mut igs = self.inbound_group_sessions.write().unwrap();
        igs.insert((room_id.to_string(), session_id.to_string()), session);
        Ok(())
    }

    /// Decrypt an `m.room.encrypted` event.
    pub fn megolm_decrypt(
        &self,
        room_id: &str,
        session_id: &str,
        ciphertext: &str,
    ) -> Result<DecryptedMessage, StoreError> {
        let megolm_msg = vodozemac::megolm::MegolmMessage::from_base64(ciphertext)
            .map_err(|e| StoreError::new(format!("invalid megolm ciphertext: {}", e)))?;

        // Try in-memory cache first
        {
            let mut igs = self.inbound_group_sessions.write().unwrap();
            let key = (room_id.to_string(), session_id.to_string());
            if let Some(session) = igs.get_mut(&key) {
                let result = session.decrypt(&megolm_msg)
                    .map_err(|e| StoreError::new(format!("megolm decrypt: {}", e)))?;
                self.store.save_inbound_group_session(room_id, session_id, session)?;
                return Ok(result);
            }
        }

        // Load from store
        if let Some(mut session) = self.store.load_inbound_group_session(room_id, session_id)? {
            let result = session.decrypt(&megolm_msg)
                .map_err(|e| StoreError::new(format!("megolm decrypt: {}", e)))?;
            self.store.save_inbound_group_session(room_id, session_id, &session)?;
            let mut igs = self.inbound_group_sessions.write().unwrap();
            igs.insert((room_id.to_string(), session_id.to_string()), session);
            return Ok(result);
        }

        Err(StoreError::new(format!(
            "No inbound Megolm session for room={} session={}",
            room_id, session_id
        )))
    }

    // ── Room key sharing ─────────────────────────────────────────────

    /// Build the to-device payload for sharing a Megolm room key via Olm.
    /// Returns the plaintext JSON to be Olm-encrypted per device.
    pub fn build_room_key_event(
        &self,
        room_id: &str,
        session_id: &str,
        session_key: &SessionKey,
    ) -> Vec<u8> {
        let account = self.account.read().unwrap();
        let mut w = JsonWriter::new();
        w.write_start_object();
        w.write_key("type");
        w.write_string("m.room_key");
        w.write_key("content");
        w.write_start_object();
        w.write_key("algorithm");
        w.write_string("m.megolm.v1.aes-sha2");
        w.write_key("room_id");
        w.write_string(room_id);
        w.write_key("session_id");
        w.write_string(session_id);
        w.write_key("session_key");
        w.write_string(&session_key.to_base64());
        w.write_end_object();
        w.write_key("sender");
        w.write_string(&self.user_id);
        w.write_key("sender_device");
        w.write_string(&self.device_id);
        w.write_key("keys");
        w.write_start_object();
        w.write_key("ed25519");
        w.write_string(&account.ed25519_key().to_base64());
        w.write_end_object();
        w.write_end_object();
        w.take_buffer().to_vec()
    }

    /// Build the `m.room.encrypted` event body for an Olm-encrypted to-device
    /// message (wrapping a room_key or other payload).
    pub fn build_olm_encrypted_event(
        &self,
        recipient_curve25519: &Curve25519PublicKey,
        recipient_ed25519: &Ed25519PublicKey,
        inner_plaintext: &[u8],
    ) -> Result<Vec<u8>, StoreError> {
        // Wrap the inner event with sender/recipient metadata
        let account = self.account.read().unwrap();
        let mut w = JsonWriter::new();
        w.write_start_object();
        w.write_key("sender");
        w.write_string(&self.user_id);
        w.write_key("sender_device");
        w.write_string(&self.device_id);
        w.write_key("keys");
        w.write_start_object();
        w.write_key("ed25519");
        w.write_string(&account.ed25519_key().to_base64());
        w.write_end_object();
        w.write_key("recipient");
        w.write_string(&self.user_id); // placeholder, caller sets correct recipient
        w.write_key("recipient_keys");
        w.write_start_object();
        w.write_key("ed25519");
        w.write_string(&recipient_ed25519.to_base64());
        w.write_end_object();
        w.write_end_object();
        let _ = w.take_buffer(); // discard — we only need the Olm-wrapped version below
        drop(account);

        // For the actual to-device content, we just Olm-encrypt the inner_plaintext
        let olm_message = self.olm_encrypt_to(recipient_curve25519, inner_plaintext)?;

        let (msg_type, body) = match &olm_message {
            OlmMessage::PreKey(pk) => (0u64, pk.to_base64()),
            OlmMessage::Normal(nm) => (1u64, nm.to_base64()),
        };

        let our_curve = self.curve25519_key().to_base64();
        let recipient_key = recipient_curve25519.to_base64();

        let mut w = JsonWriter::new();
        w.write_start_object();
        w.write_key("algorithm");
        w.write_string("m.olm.v1.curve25519-aes-sha2");
        w.write_key("sender_key");
        w.write_string(&our_curve);
        w.write_key("ciphertext");
        w.write_start_object();
        w.write_key(&recipient_key);
        w.write_start_object();
        w.write_key("type");
        w.write_number(JsonNumber::I64(msg_type as i64));
        w.write_key("body");
        w.write_string(&body);
        w.write_end_object();
        w.write_end_object();
        w.write_end_object();
        Ok(w.take_buffer().to_vec())
    }
}

/// Result of a Megolm encryption.
pub struct MegolmEncrypted {
    pub algorithm: String,
    pub sender_key: String,
    pub ciphertext: String,
    pub session_id: String,
    pub device_id: String,
}

/// Handle for Olm encryption (unused intermediate - kept for API symmetry).
pub struct OlmEncryptHandle {
    #[allow(dead_code)]
    session_ptr: *mut OlmSession,
    #[allow(dead_code)]
    sender_key: String,
}

unsafe impl Send for OlmEncryptHandle {}
unsafe impl Sync for OlmEncryptHandle {}
