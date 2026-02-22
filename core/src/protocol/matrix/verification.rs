/*
 * verification.rs
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

//! SAS (Short Authentication String) key verification for Matrix E2EE.
//!
//! Implements the full `m.key.verification.*` event flow between two devices
//! using vodozemac's SAS module for the cryptographic handshake.

use std::collections::HashMap;

use vodozemac::sas::{EstablishedSas, Mac, Sas, SasBytes};
use vodozemac::{Ed25519PublicKey, Curve25519PublicKey};

use crate::json::JsonWriter;
use crate::store::StoreError;

/// SAS emoji table from the Matrix spec (64 entries).
const SAS_EMOJIS: [(&str, &str); 64] = [
    ("🐶", "Dog"), ("🐱", "Cat"), ("🦁", "Lion"), ("🐴", "Horse"),
    ("🦄", "Unicorn"), ("🐷", "Pig"), ("🐘", "Elephant"), ("🐰", "Rabbit"),
    ("🐼", "Panda"), ("🐓", "Rooster"), ("🐧", "Penguin"), ("🐢", "Turtle"),
    ("🐟", "Fish"), ("🐙", "Octopus"), ("🦋", "Butterfly"), ("🌷", "Flower"),
    ("🌳", "Tree"), ("🌵", "Cactus"), ("🍄", "Mushroom"), ("🌏", "Globe"),
    ("🌙", "Moon"), ("☁️", "Cloud"), ("🔥", "Fire"), ("🍌", "Banana"),
    ("🍎", "Apple"), ("🍓", "Strawberry"), ("🌽", "Corn"), ("🍕", "Pizza"),
    ("🎂", "Cake"), ("❤️", "Heart"), ("😀", "Smiley"), ("🤖", "Robot"),
    ("🎩", "Hat"), ("👓", "Glasses"), ("🔧", "Spanner"), ("🎅", "Santa"),
    ("👍", "Thumbs Up"), ("☂️", "Umbrella"), ("⌛", "Hourglass"), ("⏰", "Clock"),
    ("🎁", "Gift"), ("💡", "Light Bulb"), ("📕", "Book"), ("✏️", "Pencil"),
    ("📎", "Paperclip"), ("✂️", "Scissors"), ("🔒", "Lock"), ("🔑", "Key"),
    ("🔨", "Hammer"), ("☎️", "Telephone"), ("🏁", "Flag"), ("🚂", "Train"),
    ("🚲", "Bicycle"), ("✈️", "Aeroplane"), ("🚀", "Rocket"), ("🏆", "Trophy"),
    ("⚽", "Ball"), ("🎸", "Guitar"), ("🎺", "Trumpet"), ("🔔", "Bell"),
    ("⚓", "Anchor"), ("🎧", "Headphones"), ("📁", "Folder"), ("📌", "Pin"),
];

/// State of an ongoing SAS verification.
pub enum VerificationState {
    /// We've sent or received a `m.key.verification.request`, waiting for `ready`.
    Requested {
        transaction_id: String,
        our_sas: Sas,
        we_started: bool,
    },
    /// We've exchanged `start`/`accept`, waiting for key exchange.
    Accepted {
        transaction_id: String,
        our_sas: Sas,
    },
    /// Keys exchanged, SAS computed — waiting for user to confirm emoji/decimal match.
    KeysExchanged {
        transaction_id: String,
        established: EstablishedSas,
        sas_bytes: SasBytes,
        their_ed25519: Ed25519PublicKey,
        their_curve25519: Curve25519PublicKey,
    },
    /// MACs exchanged and verified. Verification complete.
    Done {
        transaction_id: String,
    },
    /// Verification cancelled.
    Cancelled {
        transaction_id: String,
        reason: String,
    },
}

/// An active SAS verification flow.
pub struct SasVerification {
    pub state: VerificationState,
    pub their_user_id: String,
    pub their_device_id: String,
}

impl SasVerification {
    /// Start a new verification (we initiate).
    pub fn start(
        transaction_id: String,
        their_user_id: String,
        their_device_id: String,
    ) -> Self {
        let sas = Sas::new();
        Self {
            state: VerificationState::Requested {
                transaction_id,
                our_sas: sas,
                we_started: true,
            },
            their_user_id,
            their_device_id,
        }
    }

    /// Accept an incoming verification request.
    pub fn accept_incoming(
        transaction_id: String,
        their_user_id: String,
        their_device_id: String,
    ) -> Self {
        let sas = Sas::new();
        Self {
            state: VerificationState::Requested {
                transaction_id,
                our_sas: sas,
                we_started: false,
            },
            their_user_id,
            their_device_id,
        }
    }

    /// Get our public key to send in `m.key.verification.key`.
    pub fn our_public_key(&self) -> Option<String> {
        match &self.state {
            VerificationState::Requested { our_sas, .. } |
            VerificationState::Accepted { our_sas, .. } => {
                Some(our_sas.public_key().to_base64())
            }
            _ => None,
        }
    }

    /// Process the other party's public key and compute SAS.
    pub fn receive_key(
        &mut self,
        their_public_key_b64: &str,
        their_ed25519: Ed25519PublicKey,
        their_curve25519: Curve25519PublicKey,
        info_string: &str,
    ) -> Result<(), StoreError> {
        let their_key = Curve25519PublicKey::from_base64(their_public_key_b64)
            .map_err(|e| StoreError::new(format!("invalid SAS key: {}", e)))?;

        let (txn_id, sas) = match std::mem::replace(
            &mut self.state,
            VerificationState::Cancelled {
                transaction_id: String::new(),
                reason: "internal".to_string(),
            },
        ) {
            VerificationState::Requested { transaction_id, our_sas, .. } => (transaction_id, our_sas),
            VerificationState::Accepted { transaction_id, our_sas } => (transaction_id, our_sas),
            other => {
                self.state = other;
                return Err(StoreError::new("SAS not in correct state for key exchange"));
            }
        };

        let established = sas.diffie_hellman(their_key)
            .map_err(|e| StoreError::new(format!("SAS DH failed: {}", e)))?;
        let sas_bytes = established.bytes(info_string);

        self.state = VerificationState::KeysExchanged {
            transaction_id: txn_id,
            established,
            sas_bytes,
            their_ed25519,
            their_curve25519,
        };
        Ok(())
    }

    /// Get the 7 SAS emoji indices for display.
    pub fn emoji_indices(&self) -> Option<[u8; 7]> {
        match &self.state {
            VerificationState::KeysExchanged { sas_bytes, .. } => {
                Some(sas_bytes.emoji_indices())
            }
            _ => None,
        }
    }

    /// Get the 7 SAS emojis as (emoji, name) pairs.
    pub fn emojis(&self) -> Option<Vec<(&'static str, &'static str)>> {
        self.emoji_indices().map(|indices| {
            indices.iter()
                .map(|&i| SAS_EMOJIS[i as usize])
                .collect()
        })
    }

    /// Get the 3 SAS decimal numbers.
    pub fn decimals(&self) -> Option<(u16, u16, u16)> {
        match &self.state {
            VerificationState::KeysExchanged { sas_bytes, .. } => {
                Some(sas_bytes.decimals())
            }
            _ => None,
        }
    }

    /// Compute our MAC after user confirms the SAS match.
    pub fn calculate_mac(
        &self,
        our_user_id: &str,
        our_device_id: &str,
        our_ed25519: &Ed25519PublicKey,
    ) -> Option<(Mac, Mac)> {
        match &self.state {
            VerificationState::KeysExchanged { established, .. } => {
                let key_id = format!("ed25519:{}", our_device_id);
                let key_mac = established.calculate_mac(
                    &our_ed25519.to_base64(),
                    &format!(
                        "KEY_IDS{}{}{}{}",
                        our_user_id, our_device_id,
                        self.their_user_id, self.their_device_id,
                    ),
                );
                let keys_mac = established.calculate_mac(
                    &key_id,
                    &format!(
                        "KEY_IDS{}{}{}{}",
                        our_user_id, our_device_id,
                        self.their_user_id, self.their_device_id,
                    ),
                );
                Some((key_mac, keys_mac))
            }
            _ => None,
        }
    }

    /// Mark verification as complete.
    pub fn mark_done(&mut self) {
        if let VerificationState::KeysExchanged { ref transaction_id, .. } = self.state {
            let txn = transaction_id.clone();
            self.state = VerificationState::Done { transaction_id: txn };
        }
    }

    /// Cancel the verification.
    pub fn cancel(&mut self, reason: &str) {
        let txn = match &self.state {
            VerificationState::Requested { transaction_id, .. } |
            VerificationState::Accepted { transaction_id, .. } |
            VerificationState::KeysExchanged { transaction_id, .. } => transaction_id.clone(),
            VerificationState::Done { transaction_id } |
            VerificationState::Cancelled { transaction_id, .. } => transaction_id.clone(),
        };
        self.state = VerificationState::Cancelled {
            transaction_id: txn,
            reason: reason.to_string(),
        };
    }

    pub fn transaction_id(&self) -> &str {
        match &self.state {
            VerificationState::Requested { transaction_id, .. } |
            VerificationState::Accepted { transaction_id, .. } |
            VerificationState::KeysExchanged { transaction_id, .. } |
            VerificationState::Done { transaction_id } |
            VerificationState::Cancelled { transaction_id, .. } => transaction_id,
        }
    }
}

// ── Verification event builders ──────────────────────────────────────

pub fn build_verification_request_event(
    transaction_id: &str,
    from_device: &str,
    methods: &[&str],
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("from_device");
    w.write_string(from_device);
    w.write_key("methods");
    w.write_start_array();
    for m in methods {
        w.write_string(m);
    }
    w.write_end_array();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_ready_event(
    transaction_id: &str,
    from_device: &str,
    methods: &[&str],
) -> Vec<u8> {
    build_verification_request_event(transaction_id, from_device, methods)
}

pub fn build_verification_start_event(
    transaction_id: &str,
    from_device: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("from_device");
    w.write_string(from_device);
    w.write_key("method");
    w.write_string("m.sas.v1");
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("key_agreement_protocols");
    w.write_start_array();
    w.write_string("curve25519-hkdf-sha256");
    w.write_end_array();
    w.write_key("hashes");
    w.write_start_array();
    w.write_string("sha256");
    w.write_end_array();
    w.write_key("message_authentication_codes");
    w.write_start_array();
    w.write_string("hkdf-hmac-sha256.v2");
    w.write_end_array();
    w.write_key("short_authentication_string");
    w.write_start_array();
    w.write_string("emoji");
    w.write_string("decimal");
    w.write_end_array();
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_accept_event(
    transaction_id: &str,
    commitment: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("method");
    w.write_string("m.sas.v1");
    w.write_key("key_agreement_protocol");
    w.write_string("curve25519-hkdf-sha256");
    w.write_key("hash");
    w.write_string("sha256");
    w.write_key("message_authentication_code");
    w.write_string("hkdf-hmac-sha256.v2");
    w.write_key("short_authentication_string");
    w.write_start_array();
    w.write_string("emoji");
    w.write_string("decimal");
    w.write_end_array();
    w.write_key("commitment");
    w.write_string(commitment);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_key_event(
    transaction_id: &str,
    key_b64: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("key");
    w.write_string(key_b64);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_mac_event(
    transaction_id: &str,
    mac: &HashMap<String, String>,
    keys: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("mac");
    w.write_start_object();
    for (k, v) in mac {
        w.write_key(k);
        w.write_string(v);
    }
    w.write_end_object();
    w.write_key("keys");
    w.write_string(keys);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_done_event(transaction_id: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_cancel_event(
    transaction_id: &str,
    code: &str,
    reason: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("code");
    w.write_string(code);
    w.write_key("reason");
    w.write_string(reason);
    w.write_end_object();
    w.take_buffer().to_vec()
}
