/*
 * config.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
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

//! Credential storage: load/save per store or transport URI in a separate file so we do not
//! overwrite the UI's ~/.tagliacarte/config.xml (which holds accounts, display names, etc. in XML).
//! All XML read/write uses the quick_xml parser/writer; no regex or hand parsing.
//! When key-file encryption is used, the credentials file is encrypted with XChaCha20-Poly1305
//! using a key stored in ~/.tagliacarte/.key (mode 0o600).

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::XChaCha20Poly1305;
use keyring::Entry;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Magic header for encrypted credentials file (5 bytes).
const ENCRYPTED_MAGIC: &[u8] = b"TCENC";
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

/// Service name for keyring entries (one entry per store/transport URI).
const KEYRING_SERVICE: &str = "tagliacarte";

/// When true, credentials are read/written via the system keychain; when false, via the encrypted file.
static USE_KEYCHAIN: AtomicBool = AtomicBool::new(false);

/// Set whether to use the system keychain (true) or the encrypted file (false) for credentials.
pub fn set_credentials_backend(use_keychain: bool) {
    USE_KEYCHAIN.store(use_keychain, Ordering::SeqCst);
}

/// Return true if the credentials backend is the system keychain.
pub fn credentials_use_keychain() -> bool {
    USE_KEYCHAIN.load(Ordering::SeqCst)
}

/// Probe: try to create and delete a dummy keyring entry. Returns true if the system keychain is available.
pub fn keychain_available() -> bool {
    let entry = match Entry::new(KEYRING_SERVICE, "__tagliacarte_probe__") {
        Ok(e) => e,
        Err(_) => return false,
    };
    let _ = entry.set_password("probe");
    let _ = entry.delete_credential();
    true
}

/// Encode (username, password) as: 4-byte LE username length + username UTF-8 + password UTF-8.
fn encode_credential_secret(username: &str, password: &str) -> Vec<u8> {
    let u = username.as_bytes();
    let p = password.as_bytes();
    let mut out = Vec::with_capacity(4 + u.len() + p.len());
    out.extend_from_slice(&(u.len() as u32).to_le_bytes());
    out.extend_from_slice(u);
    out.extend_from_slice(p);
    out
}

/// Decode secret bytes into (username, password). Returns None if format is invalid.
fn decode_credential_secret(secret: &[u8]) -> Option<CredentialEntry> {
    if secret.len() < 4 {
        return None;
    }
    let len = u32::from_le_bytes([secret[0], secret[1], secret[2], secret[3]]) as usize;
    if 4 + len > secret.len() {
        return None;
    }
    let username = std::str::from_utf8(&secret[4..4 + len]).ok()?.to_string();
    let password = std::str::from_utf8(&secret[4 + len..]).ok()?.to_string();
    Some(CredentialEntry { username, password_or_token: password })
}

fn get_credential_keychain(uri: &str) -> Option<CredentialEntry> {
    let entry = Entry::new(KEYRING_SERVICE, uri).ok()?;
    let secret = entry.get_secret().ok()?;
    decode_credential_secret(&secret)
}

fn set_credential_keychain(uri: &str, username: &str, password: &str) -> Result<(), String> {
    let entry = Entry::new(KEYRING_SERVICE, uri).map_err(|e| e.to_string())?;
    let secret = encode_credential_secret(username, password);
    entry.set_secret(&secret).map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove one credential from the system keychain. No-op if the entry does not exist.
pub fn delete_credential_keychain(uri: &str) -> Result<(), String> {
    let entry = Entry::new(KEYRING_SERVICE, uri).map_err(|e| e.to_string())?;
    let _ = entry.delete_credential();
    Ok(())
}

/// Default config directory: ~/.tagliacarte.
pub fn default_config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from).map(|h| h.join(".tagliacarte"))
}

/// Default credentials path: ~/.tagliacarte/credentials. Separate from config.xml so we never
/// overwrite the UI's XML configuration. Content is XML (root \<credentials\>, \<credential\> with \<uri\>, \<username\>, \<password\>).
/// When encrypted, file format is "TCENC" + 24-byte nonce + XChaCha20-Poly1305 ciphertext (with tag).
pub fn default_credentials_path() -> Option<std::path::PathBuf> {
    default_config_dir().map(|d| d.join("credentials"))
}

/// Path to the key file for credentials encryption: same directory as credentials, file `.key`.
fn key_path(credentials_path: &Path) -> Option<std::path::PathBuf> {
    credentials_path.parent().map(|p| p.join(".key"))
}

/// Read the key file (32 bytes). Returns error if missing or wrong length.
fn read_key(key_path: &Path) -> Result<[u8; KEY_LEN], String> {
    let buf = fs::read(key_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "encrypted credentials file but key file not found".to_string()
        } else {
            e.to_string()
        }
    })?;
    if buf.len() != KEY_LEN {
        return Err("key file has wrong length".to_string());
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&buf[..KEY_LEN]);
    Ok(key)
}

/// Ensure the key file exists: read 32 bytes if present, otherwise generate with getrandom and write (mode 0o600).
fn get_or_create_key(key_path: &Path, parent_dir: &Path) -> Result<[u8; KEY_LEN], String> {
    match read_key(key_path) {
        Ok(key) => return Ok(key),
        Err(e) if e.contains("not found") => {}
        Err(e) => return Err(e),
    }
    fs::create_dir_all(parent_dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    if let Err(e) = fs::set_permissions(parent_dir, PermissionsExt::from_mode(0o700)) {
        let _ = e;
    }
    let mut key = [0u8; KEY_LEN];
    getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
    let mut f = open_key_file_for_write(key_path).map_err(|e| e.to_string())?;
    f.write_all(&key).map_err(|e| e.to_string())?;
    f.flush().map_err(|e| e.to_string())?;
    #[cfg(unix)]
    drop(fs::set_permissions(key_path, PermissionsExt::from_mode(0o600)));
    Ok(key)
}

/// Credential entry for one store or transport (username optional for token-only).
#[derive(Debug, Clone, Default)]
pub struct CredentialEntry {
    pub username: String,
    pub password_or_token: String,
}

/// Reject NUL (U+0000) since XML cannot represent it.
fn contains_nul(s: &str) -> bool {
    s.contains('\0')
}

/// Load credentials. When the backend is keychain, pass the store/transport URI to look up (returns 0 or 1 entry);
/// when the backend is file, `uri_for_keychain` is ignored and the full file is loaded.
/// If the file does not exist, returns empty. When keychain and `uri_for_keychain` is None, returns empty.
pub fn load_credentials(path: &Path, uri_for_keychain: Option<&str>) -> Result<HashMap<String, CredentialEntry>, String> {
    if credentials_use_keychain() {
        let uri = match uri_for_keychain {
            Some(u) => u,
            None => return Ok(HashMap::new()),
        };
        let mut out = HashMap::new();
        if let Some(entry) = get_credential_keychain(uri) {
            out.insert(uri.to_string(), entry);
        }
        return Ok(out);
    }
    load_credentials_from_file(path)
}

/// Load credentials from the encrypted or plaintext file (used when backend is file).
fn load_credentials_from_file(path: &Path) -> Result<HashMap<String, CredentialEntry>, String> {
    let raw = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(e.to_string()),
    };
    let content = if raw.len() >= ENCRYPTED_MAGIC.len() && raw.starts_with(ENCRYPTED_MAGIC) {
        if raw.len() < ENCRYPTED_MAGIC.len() + NONCE_LEN + 16 {
            return Err("encrypted credentials file too short".to_string());
        }
        let key_path = key_path(path).ok_or("no parent for credentials path")?;
        let key = read_key(&key_path)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
        let nonce_slice = &raw[ENCRYPTED_MAGIC.len()..ENCRYPTED_MAGIC.len() + NONCE_LEN];
        let nonce = chacha20poly1305::XNonce::from_slice(nonce_slice);
        let ciphertext = &raw[ENCRYPTED_MAGIC.len() + NONCE_LEN..];
        let plain = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "decryption failed (wrong key or tampered file)".to_string())?;
        String::from_utf8(plain).map_err(|e| format!("decrypted content not UTF-8: {}", e))?
    } else {
        String::from_utf8(raw).map_err(|e| format!("credentials file not valid UTF-8: {}", e))?
    };
    let trimmed = content.trim_start();
    if !trimmed.starts_with('<') {
        return load_credentials_legacy(&content);
    }
    load_credentials_xml(trimmed)
}

/// Parse legacy tab-separated format (one line per credential: uri\tusername\tpassword).
fn load_credentials_legacy(content: &str) -> Result<HashMap<String, CredentialEntry>, String> {
    let mut out = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() >= 3 {
            let uri = parts[0].to_string();
            let username = parts[1].to_string();
            let password_or_token = parts[2].to_string();
            out.insert(uri, CredentialEntry { username, password_or_token });
        }
    }
    Ok(out)
}

/// Parse XML credentials using quick_xml. Expects <credentials><credential><uri>...</uri><username>...</username><password>...</password></credential>...</credentials>.
fn load_credentials_xml(content: &str) -> Result<HashMap<String, CredentialEntry>, String> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = HashMap::new();
    let mut current_uri = String::new();
    let mut current_username = String::new();
    let mut current_password = String::new();
    let mut in_credential = false;
    let mut element_name = Vec::<u8>::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("XML parse error: {}", e)),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = e.name();
                let name = name.as_ref();
                if name == b"credential" {
                    in_credential = true;
                    current_uri.clear();
                    current_username.clear();
                    current_password.clear();
                } else if in_credential && (name == b"uri" || name == b"username" || name == b"password") {
                    element_name.clear();
                    element_name.extend_from_slice(name);
                }
            }
            Ok(Event::Text(e)) => {
                if !in_credential || element_name.is_empty() {
                    continue;
                }
                let text = e.unescape().map_err(|e| e.to_string())?.trim().to_string();
                if element_name == b"uri" {
                    current_uri = text;
                } else if element_name == b"username" {
                    current_username = text;
                } else if element_name == b"password" {
                    current_password = text;
                }
                element_name.clear();
            }
            Ok(Event::End(e)) => {
                let end_name = e.name();
                if end_name.as_ref() == b"credential" && !current_uri.is_empty() {
                    out.insert(
                        std::mem::take(&mut current_uri),
                        CredentialEntry {
                            username: std::mem::take(&mut current_username),
                            password_or_token: std::mem::take(&mut current_password),
                        },
                    );
                    in_credential = false;
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// Save one credential. When the backend is keychain, writes to the system keychain; when file, merges with existing and writes encrypted file.
/// Rejects U+0000 in any value.
pub fn save_credential(path: &Path, uri: &str, username: &str, password_or_token: &str) -> Result<(), String> {
    if contains_nul(uri) || contains_nul(username) || contains_nul(password_or_token) {
        return Err("credential values must not contain NUL (U+0000)".to_string());
    }
    if credentials_use_keychain() {
        return set_credential_keychain(uri, username, password_or_token);
    }
    let parent = path.parent().ok_or("no parent dir")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    if let Err(e) = fs::set_permissions(parent, PermissionsExt::from_mode(0o700)) {
        let _ = e;
    }
    let mut entries = load_credentials_from_file(path).unwrap_or_default();
    entries.insert(
        uri.to_string(),
        CredentialEntry {
            username: username.to_string(),
            password_or_token: password_or_token.to_string(),
        },
    );
    write_credentials_encrypted(path, &entries)?;
    #[cfg(unix)]
    drop(fs::set_permissions(path, PermissionsExt::from_mode(0o600)));
    Ok(())
}

/// Build credentials XML into a byte vector (UTF-8).
fn credentials_xml_to_bytes(entries: &HashMap<String, CredentialEntry>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut writer = Writer::new(&mut out);
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| e.to_string())?;
    writer
        .write_event(Event::Start(BytesStart::new("credentials")))
        .map_err(|e| e.to_string())?;
    for (uri, e) in entries {
        writer
            .write_event(Event::Start(BytesStart::new("credential")))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::Start(BytesStart::new("uri")))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::Text(BytesText::new(uri.as_str())))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::End(BytesEnd::new("uri")))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::Start(BytesStart::new("username")))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::Text(BytesText::new(e.username.as_str())))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::End(BytesEnd::new("username")))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::Start(BytesStart::new("password")))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::Text(BytesText::new(e.password_or_token.as_str())))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::End(BytesEnd::new("password")))
            .map_err(|e| e.to_string())?;
        writer
            .write_event(Event::End(BytesEnd::new("credential")))
            .map_err(|e| e.to_string())?;
    }
    writer
        .write_event(Event::End(BytesEnd::new("credentials")))
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Write credentials encrypted with XChaCha20-Poly1305. Key is in .key (created if missing). File format: "TCENC" + nonce (24) + ciphertext.
fn write_credentials_encrypted(path: &Path, entries: &HashMap<String, CredentialEntry>) -> Result<(), String> {
    let plain = credentials_xml_to_bytes(entries)?;
    let key_path = key_path(path).ok_or("no parent for credentials path")?;
    let parent = path.parent().ok_or("no parent dir")?;
    let key = get_or_create_key(&key_path, parent)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plain.as_ref())
        .map_err(|e| e.to_string())?;
    let mut f = open_credentials_file_for_write(path)?;
    f.write_all(ENCRYPTED_MAGIC).map_err(|e| e.to_string())?;
    f.write_all(nonce.as_slice()).map_err(|e| e.to_string())?;
    f.write_all(&ciphertext).map_err(|e| e.to_string())?;
    f.flush().map_err(|e| e.to_string())?;
    #[cfg(unix)]
    drop(fs::set_permissions(path, PermissionsExt::from_mode(0o600)));
    Ok(())
}

/// Migrate credentials from the encrypted file to the system keychain. Call after set_credentials_backend(true).
/// Loads from file, writes each to keyring, then removes the credentials file and .key. No-op if file does not exist.
pub fn migrate_credentials_to_keychain(path: &Path) -> Result<(), String> {
    let entries = load_credentials_from_file(path)?;
    if entries.is_empty() {
        let _ = fs::remove_file(path);
        if let Some(kp) = key_path(path) {
            let _ = fs::remove_file(&kp);
        }
        return Ok(());
    }
    for (uri, entry) in &entries {
        set_credential_keychain(uri, &entry.username, &entry.password_or_token)?;
    }
    fs::remove_file(path).map_err(|e| e.to_string())?;
    if let Some(kp) = key_path(path) {
        let _ = fs::remove_file(&kp);
    }
    Ok(())
}

/// Migrate credentials from the system keychain to the encrypted file for the given URIs. Call after set_credentials_backend(false).
/// Looks up each URI in the keyring, builds the credentials map, writes the encrypted file.
pub fn migrate_credentials_to_file(path: &Path, uris: &[String]) -> Result<(), String> {
    let mut entries = HashMap::new();
    for uri in uris {
        if let Some(entry) = get_credential_keychain(uri) {
            entries.insert(uri.clone(), entry);
        }
    }
    if entries.is_empty() {
        return Ok(());
    }
    write_credentials_encrypted(path, &entries)?;
    #[cfg(unix)]
    if let Ok(_meta) = path.metadata() {
        let _ = fs::set_permissions(path, PermissionsExt::from_mode(0o600));
    }
    Ok(())
}

/// Open the credentials file for writing. On Unix, creates it with mode 0o600 (owner read/write only).
fn open_credentials_file_for_write(path: &Path) -> Result<File, String> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    {
        fs::File::create(path).map_err(|e| e.to_string())
    }
}

/// Open the key file for writing. On Unix, creates it with mode 0o600.
fn open_key_file_for_write(path: &Path) -> Result<File, std::io::Error> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .mode(0o600)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        fs::File::create(path)
    }
}
