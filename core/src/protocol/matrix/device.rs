/*
 * device.rs
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

//! Device tracking and key management for Matrix E2EE.
//!
//! Tracks known devices per user from `/keys/query`, verifies Ed25519
//! signatures on device keys, and manages key claim requests.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use vodozemac::{Curve25519PublicKey, Ed25519PublicKey, Ed25519Signature};

use crate::json::JsonWriter;

/// Public key material for a single device.
#[derive(Debug, Clone)]
pub struct DeviceKeys {
    pub user_id: String,
    pub device_id: String,
    pub algorithms: Vec<String>,
    pub ed25519_key: Option<Ed25519PublicKey>,
    pub curve25519_key: Option<Curve25519PublicKey>,
    pub verified: bool,
}

/// Tracks devices for users we communicate with.
pub struct DeviceTracker {
    devices: RwLock<HashMap<String, HashMap<String, DeviceKeys>>>,
    dirty_users: RwLock<HashSet<String>>,
}

impl DeviceTracker {
    pub fn new() -> Self {
        Self {
            devices: RwLock::new(HashMap::new()),
            dirty_users: RwLock::new(HashSet::new()),
        }
    }

    pub fn mark_users_dirty(&self, user_ids: &[String]) {
        let mut dirty = self.dirty_users.write().unwrap();
        for uid in user_ids {
            dirty.insert(uid.clone());
        }
    }

    pub fn dirty_users(&self) -> Vec<String> {
        self.dirty_users.read().unwrap().iter().cloned().collect()
    }

    pub fn clear_dirty(&self, user_ids: &[String]) {
        let mut dirty = self.dirty_users.write().unwrap();
        for uid in user_ids {
            dirty.remove(uid);
        }
    }

    pub fn update_devices(&self, user_id: &str, new_devices: HashMap<String, DeviceKeys>) {
        let mut devices = self.devices.write().unwrap();
        devices.insert(user_id.to_string(), new_devices);
    }

    pub fn get_devices(&self, user_id: &str) -> Option<HashMap<String, DeviceKeys>> {
        self.devices.read().unwrap().get(user_id).cloned()
    }

    pub fn get_device(&self, user_id: &str, device_id: &str) -> Option<DeviceKeys> {
        self.devices.read().unwrap()
            .get(user_id)
            .and_then(|devs| devs.get(device_id).cloned())
    }

    pub fn get_devices_for_users(&self, user_ids: &[String]) -> Vec<(String, DeviceKeys)> {
        let devices = self.devices.read().unwrap();
        let mut result = Vec::new();
        for uid in user_ids {
            if let Some(devs) = devices.get(uid) {
                for dk in devs.values() {
                    result.push((uid.clone(), dk.clone()));
                }
            }
        }
        result
    }

    pub fn users_needing_query(&self, user_ids: &[String]) -> Vec<String> {
        let devices = self.devices.read().unwrap();
        let dirty = self.dirty_users.read().unwrap();
        user_ids.iter()
            .filter(|uid| !devices.contains_key(*uid) || dirty.contains(*uid))
            .cloned()
            .collect()
    }
}

// ── JSON request builders ────────────────────────────────────────────

pub fn build_keys_query_body(user_ids: &[String]) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("device_keys");
    w.write_start_object();
    for uid in user_ids {
        w.write_key(uid);
        w.write_start_array();
        w.write_end_array();
    }
    w.write_end_object();
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_keys_claim_body(claims: &[(String, String)]) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("one_time_keys");
    w.write_start_object();

    let mut by_user: HashMap<&str, Vec<&str>> = HashMap::new();
    for (uid, did) in claims {
        by_user.entry(uid.as_str()).or_default().push(did.as_str());
    }

    for (uid, dids) in &by_user {
        w.write_key(uid);
        w.write_start_object();
        for did in dids {
            w.write_key(did);
            w.write_string("signed_curve25519");
        }
        w.write_end_object();
    }

    w.write_end_object();
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn verify_device_signature(
    signing_key: &Ed25519PublicKey,
    canonical_json: &str,
    signature_b64: &str,
) -> bool {
    let sig = match Ed25519Signature::from_base64(signature_b64) {
        Ok(s) => s,
        Err(_) => return false,
    };
    signing_key.verify(canonical_json.as_bytes(), &sig).is_ok()
}

/// Build `/keys/upload` body, embedding pre-built device_keys and one_time_keys
/// JSON objects as raw values.
pub fn build_keys_upload_body(
    device_keys: Option<&[u8]>,
    one_time_keys: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(1024);
    out.push(b'{');
    let mut need_comma = false;

    if let Some(dk) = device_keys {
        out.extend_from_slice(b"\"device_keys\":");
        out.extend_from_slice(dk);
        need_comma = true;
    }

    if let Some(otk) = one_time_keys {
        if need_comma {
            out.push(b',');
        }
        out.extend_from_slice(b"\"one_time_keys\":");
        out.extend_from_slice(otk);
    }

    out.push(b'}');
    out
}

/// Build `PUT /sendToDevice/{eventType}/{txnId}` body.
/// `messages` maps user_id -> { device_id -> content_json_bytes }.
pub fn build_send_to_device_body(
    messages: &HashMap<String, HashMap<String, Vec<u8>>>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(4096);
    out.extend_from_slice(b"{\"messages\":{");

    let mut first_user = true;
    for (user_id, devices) in messages {
        if !first_user { out.push(b','); }
        first_user = false;
        write_json_string(&mut out, user_id);
        out.push(b':');
        out.push(b'{');

        let mut first_device = true;
        for (device_id, content) in devices {
            if !first_device { out.push(b','); }
            first_device = false;
            write_json_string(&mut out, device_id);
            out.push(b':');
            out.extend_from_slice(content);
        }

        out.push(b'}');
    }

    out.extend_from_slice(b"}}");
    out
}

fn write_json_string(out: &mut Vec<u8>, s: &str) {
    out.push(b'"');
    for ch in s.bytes() {
        match ch {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            c if c < 0x20 => {
                out.extend_from_slice(b"\\u00");
                out.push(HEX[(c >> 4) as usize]);
                out.push(HEX[(c & 0x0f) as usize]);
            }
            c => out.push(c),
        }
    }
    out.push(b'"');
}

const HEX: [u8; 16] = *b"0123456789abcdef";
