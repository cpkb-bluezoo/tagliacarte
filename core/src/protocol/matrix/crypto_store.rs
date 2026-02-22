/*
 * crypto_store.rs
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

//! Persistent storage for Matrix E2EE crypto state.
//!
//! Stores pickled vodozemac objects (Olm account, Olm sessions, Megolm sessions,
//! device keys) as JSON files encrypted with XChaCha20-Poly1305 under
//! `~/.tagliacarte/matrix/<user_hash>/`. The encryption key is derived via
//! HKDF-SHA-256 from the access token plus a persisted random salt.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::XChaCha20Poly1305;
use hkdf::Hkdf;
use sha2::Sha256;

use vodozemac::olm::{Account, AccountPickle, Session, SessionPickle};
use vodozemac::megolm::{
    GroupSession, GroupSessionPickle,
    InboundGroupSession, InboundGroupSessionPickle,
};

use crate::store::StoreError;

const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const HKDF_INFO: &[u8] = b"tagliacarte-matrix-crypto-store";

/// Persistent storage for all crypto state associated with one Matrix account.
pub struct CryptoStore {
    base_dir: PathBuf,
    cipher_key: [u8; KEY_LEN],
}

impl CryptoStore {
    /// Open (or create) a store rooted at `~/.tagliacarte/matrix/<user_hash>/`.
    /// The encryption key is derived from a persisted random secret (salt file),
    /// independent of the access token so re-login doesn't invalidate the store.
    pub fn open(user_id: &str, _access_token: &str) -> Result<Self, StoreError> {
        let base = config_matrix_dir(user_id)?;
        fs::create_dir_all(&base)
            .map_err(|e| StoreError::new(format!("crypto_store mkdir: {}", e)))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&base, PermissionsExt::from_mode(0o700));
        }

        let salt = get_or_create_salt(&base)?;
        let cipher_key = derive_key(&salt, &salt);

        let store = Self { base_dir: base, cipher_key };
        // One-time migration: if an account pickle exists but can't be
        // decrypted, it was created under the old access-token-based key.
        // Wipe all pickles so we start clean under the new derivation.
        if store.base_dir.join("account.pickle").exists() {
            if store.read_encrypted("account.pickle").is_err() {
                eprintln!("[matrix] crypto store: migrating from old key derivation, wiping stale pickles");
                store.wipe_pickles();
            }
        }
        Ok(store)
    }

    /// Remove all encrypted pickle files (account, sessions, keys) but keep
    /// the salt. Used when the key derivation scheme has changed.
    fn wipe_pickles(&self) {
        let _ = fs::remove_file(self.base_dir.join("account.pickle"));
        let _ = fs::remove_dir_all(self.base_dir.join("olm_sessions"));
        let _ = fs::remove_dir_all(self.base_dir.join("megolm_inbound"));
        let _ = fs::remove_dir_all(self.base_dir.join("megolm_outbound"));
        let _ = fs::remove_dir_all(self.base_dir.join("device_keys"));
    }

    // ── Account ──────────────────────────────────────────────────────

    pub fn save_account(&self, account: &Account) -> Result<(), StoreError> {
        let pickle = account.pickle();
        let json = serde_json::to_vec(&pickle)
            .map_err(|e| StoreError::new(format!("pickle account: {}", e)))?;
        self.write_encrypted("account.pickle", &json)
    }

    pub fn load_account(&self) -> Result<Option<Account>, StoreError> {
        match self.read_encrypted("account.pickle")? {
            None => Ok(None),
            Some(json) => {
                let pickle: AccountPickle = serde_json::from_slice(&json)
                    .map_err(|e| StoreError::new(format!("unpickle account: {}", e)))?;
                Ok(Some(Account::from_pickle(pickle)))
            }
        }
    }

    // ── Olm sessions ────────────────────────────────────────────────

    pub fn save_olm_session(&self, sender_key: &str, session: &Session) -> Result<(), StoreError> {
        let dir = self.base_dir.join("olm_sessions");
        fs::create_dir_all(&dir)
            .map_err(|e| StoreError::new(format!("mkdir olm_sessions: {}", e)))?;

        let filename = format!("{}_{}.pickle", hex_hash(sender_key), session.session_id());
        let pickle = session.pickle();
        let json = serde_json::to_vec(&pickle)
            .map_err(|e| StoreError::new(format!("pickle olm session: {}", e)))?;
        self.write_encrypted_to(&dir.join(&filename), &json)
    }

    pub fn load_olm_sessions(&self, sender_key: &str) -> Result<Vec<Session>, StoreError> {
        let dir = self.base_dir.join("olm_sessions");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let prefix = hex_hash(sender_key);
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| StoreError::new(e.to_string()))? {
            let entry = entry.map_err(|e| StoreError::new(e.to_string()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && name.ends_with(".pickle") {
                if let Some(json) = self.read_encrypted_from(&entry.path())? {
                    let pickle: SessionPickle = serde_json::from_slice(&json)
                        .map_err(|e| StoreError::new(format!("unpickle olm session: {}", e)))?;
                    sessions.push(Session::from_pickle(pickle));
                }
            }
        }
        Ok(sessions)
    }

    // ── Inbound Megolm sessions ─────────────────────────────────────

    pub fn save_inbound_group_session(
        &self,
        room_id: &str,
        session_id: &str,
        session: &InboundGroupSession,
    ) -> Result<(), StoreError> {
        let dir = self.base_dir.join("megolm_inbound");
        fs::create_dir_all(&dir)
            .map_err(|e| StoreError::new(format!("mkdir megolm_inbound: {}", e)))?;

        let filename = format!("{}_{}.pickle", hex_hash(room_id), hex_hash(session_id));
        let pickle = session.pickle();
        let json = serde_json::to_vec(&pickle)
            .map_err(|e| StoreError::new(format!("pickle inbound megolm: {}", e)))?;
        self.write_encrypted_to(&dir.join(&filename), &json)
    }

    pub fn load_inbound_group_session(
        &self,
        room_id: &str,
        session_id: &str,
    ) -> Result<Option<InboundGroupSession>, StoreError> {
        let dir = self.base_dir.join("megolm_inbound");
        let filename = format!("{}_{}.pickle", hex_hash(room_id), hex_hash(session_id));
        let path = dir.join(&filename);
        match self.read_encrypted_from(&path)? {
            None => Ok(None),
            Some(json) => {
                let pickle: InboundGroupSessionPickle = serde_json::from_slice(&json)
                    .map_err(|e| StoreError::new(format!("unpickle inbound megolm: {}", e)))?;
                Ok(Some(InboundGroupSession::from_pickle(pickle)))
            }
        }
    }

    /// Load all inbound group sessions for a room.
    pub fn load_inbound_group_sessions_for_room(
        &self,
        room_id: &str,
    ) -> Result<Vec<InboundGroupSession>, StoreError> {
        let dir = self.base_dir.join("megolm_inbound");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let prefix = hex_hash(room_id);
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| StoreError::new(e.to_string()))? {
            let entry = entry.map_err(|e| StoreError::new(e.to_string()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && name.ends_with(".pickle") {
                if let Some(json) = self.read_encrypted_from(&entry.path())? {
                    let pickle: InboundGroupSessionPickle = serde_json::from_slice(&json)
                        .map_err(|e| StoreError::new(format!("unpickle inbound megolm: {}", e)))?;
                    sessions.push(InboundGroupSession::from_pickle(pickle));
                }
            }
        }
        Ok(sessions)
    }

    // ── Outbound Megolm sessions ────────────────────────────────────

    pub fn save_outbound_group_session(
        &self,
        room_id: &str,
        session: &GroupSession,
    ) -> Result<(), StoreError> {
        let dir = self.base_dir.join("megolm_outbound");
        fs::create_dir_all(&dir)
            .map_err(|e| StoreError::new(format!("mkdir megolm_outbound: {}", e)))?;

        let filename = format!("{}.pickle", hex_hash(room_id));
        let pickle = session.pickle();
        let json = serde_json::to_vec(&pickle)
            .map_err(|e| StoreError::new(format!("pickle outbound megolm: {}", e)))?;
        self.write_encrypted_to(&dir.join(&filename), &json)
    }

    pub fn load_outbound_group_session(
        &self,
        room_id: &str,
    ) -> Result<Option<GroupSession>, StoreError> {
        let dir = self.base_dir.join("megolm_outbound");
        let filename = format!("{}.pickle", hex_hash(room_id));
        let path = dir.join(&filename);
        match self.read_encrypted_from(&path)? {
            None => Ok(None),
            Some(json) => {
                let pickle: GroupSessionPickle = serde_json::from_slice(&json)
                    .map_err(|e| StoreError::new(format!("unpickle outbound megolm: {}", e)))?;
                Ok(Some(GroupSession::from_pickle(pickle)))
            }
        }
    }

    // ── Device keys cache ───────────────────────────────────────────

    /// Save device keys for a user. `keys` maps `device_id` to the JSON-encoded
    /// device_keys object received from `/keys/query`.
    pub fn save_device_keys(
        &self,
        user_id: &str,
        keys: &HashMap<String, Vec<u8>>,
    ) -> Result<(), StoreError> {
        let dir = self.base_dir.join("device_keys");
        fs::create_dir_all(&dir)
            .map_err(|e| StoreError::new(format!("mkdir device_keys: {}", e)))?;

        let filename = format!("{}.json", hex_hash(user_id));
        let json = serde_json::to_vec(keys)
            .map_err(|e| StoreError::new(format!("serialize device keys: {}", e)))?;
        self.write_encrypted_to(&dir.join(&filename), &json)
    }

    pub fn load_device_keys(
        &self,
        user_id: &str,
    ) -> Result<Option<HashMap<String, Vec<u8>>>, StoreError> {
        let dir = self.base_dir.join("device_keys");
        let filename = format!("{}.json", hex_hash(user_id));
        let path = dir.join(&filename);
        match self.read_encrypted_from(&path)? {
            None => Ok(None),
            Some(json) => {
                let keys: HashMap<String, Vec<u8>> = serde_json::from_slice(&json)
                    .map_err(|e| StoreError::new(format!("deserialize device keys: {}", e)))?;
                Ok(Some(keys))
            }
        }
    }

    // ── Cross-signing keys ──────────────────────────────────────────

    pub fn save_cross_signing_keys(&self, data: &[u8]) -> Result<(), StoreError> {
        self.write_encrypted("cross_signing.pickle", data)
    }

    pub fn load_cross_signing_keys(&self) -> Result<Option<Vec<u8>>, StoreError> {
        self.read_encrypted("cross_signing.pickle")
    }

    // ── Encrypted file I/O ──────────────────────────────────────────

    fn write_encrypted(&self, filename: &str, plaintext: &[u8]) -> Result<(), StoreError> {
        let path = self.base_dir.join(filename);
        self.write_encrypted_to(&path, plaintext)
    }

    fn write_encrypted_to(&self, path: &Path, plaintext: &[u8]) -> Result<(), StoreError> {
        let cipher = XChaCha20Poly1305::new((&self.cipher_key).into());
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, plaintext)
            .map_err(|e| StoreError::new(format!("encrypt pickle: {}", e)))?;

        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);

        let mut file = fs::File::create(path)
            .map_err(|e| StoreError::new(format!("create pickle file: {}", e)))?;
        file.write_all(&out)
            .map_err(|e| StoreError::new(format!("write pickle file: {}", e)))?;
        Ok(())
    }

    fn read_encrypted(&self, filename: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.base_dir.join(filename);
        self.read_encrypted_from(&path)
    }

    fn read_encrypted_from(&self, path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
        let data = match fs::read(path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(StoreError::new(format!("read pickle file: {}", e))),
        };
        if data.len() < NONCE_LEN {
            return Err(StoreError::new("pickle file too short"));
        }
        let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
        let cipher = XChaCha20Poly1305::new((&self.cipher_key).into());
        let nonce = chacha20poly1305::XNonce::from_slice(nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| StoreError::new("decrypt pickle failed (wrong key or corrupt)"))?;
        Ok(Some(plaintext))
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn config_matrix_dir(user_id: &str) -> Result<PathBuf, StoreError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| StoreError::new("HOME not set"))?;
    Ok(PathBuf::from(home)
        .join(".tagliacarte")
        .join("matrix")
        .join(hex_hash(user_id)))
}

fn get_or_create_salt(dir: &Path) -> Result<[u8; SALT_LEN], StoreError> {
    let salt_path = dir.join(".salt");
    match fs::read(&salt_path) {
        Ok(data) if data.len() == SALT_LEN => {
            let mut salt = [0u8; SALT_LEN];
            salt.copy_from_slice(&data);
            Ok(salt)
        }
        Ok(_) => Err(StoreError::new("salt file has wrong length")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut salt = [0u8; SALT_LEN];
            getrandom::getrandom(&mut salt)
                .map_err(|e| StoreError::new(format!("getrandom: {}", e)))?;
            let mut f = fs::File::create(&salt_path)
                .map_err(|e| StoreError::new(format!("create salt: {}", e)))?;
            f.write_all(&salt)
                .map_err(|e| StoreError::new(format!("write salt: {}", e)))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&salt_path, PermissionsExt::from_mode(0o600));
            }
            Ok(salt)
        }
        Err(e) => Err(StoreError::new(format!("read salt: {}", e))),
    }
}

fn derive_key(ikm: &[u8], salt: &[u8]) -> [u8; KEY_LEN] {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut key = [0u8; KEY_LEN];
    hk.expand(HKDF_INFO, &mut key).expect("HKDF expand");
    key
}

/// SHA-256 hex digest truncated to 16 chars — short, filesystem-safe identifier.
fn hex_hash(input: &str) -> String {
    use sha2::Digest;
    let hash = Sha256::digest(input.as_bytes());
    let mut out = String::with_capacity(16);
    for &b in &hash[..8] {
        out.push(HEX_LOWER[(b >> 4) as usize] as char);
        out.push(HEX_LOWER[(b & 0x0f) as usize] as char);
    }
    out
}

const HEX_LOWER: [u8; 16] = *b"0123456789abcdef";
