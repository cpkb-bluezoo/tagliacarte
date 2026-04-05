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
//! overwrite the UI's `config.xml` (accounts, display names, etc. in XML).
//! All XML read/write uses the quick_xml parser/writer; no regex or hand parsing.
//! When key-file encryption is used, the credentials file is encrypted with XChaCha20-Poly1305
//! using a key stored next to credentials as `.key` (mode 0o600).

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::XChaCha20Poly1305;
use keyring::Entry;
use quick_xml::events::Event;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Magic header for encrypted credentials file (5 bytes).
const ENCRYPTED_MAGIC: &[u8] = b"TCENC";
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

/// Service name for keyring entries (one entry per store/transport URI).
const KEYRING_SERVICE: &str = "tagliacarte";

/// Absolute path to the active `config.xml` (set by FRB / TUI when loading config).
static ACTIVE_CONFIG_XML_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Record which `config.xml` is in use so [`resolve_credentials_file_path`] can load `credentials`
/// from the **same directory** (Flutter and TUI may use a path that does not match
/// [`tagliacarte_data_dir`] alone, e.g. explicit argv or a different resolver).
pub fn set_active_config_xml_path(path: impl AsRef<Path>) {
    let p = path.as_ref();
    if p.as_os_str().is_empty() {
        return;
    }
    if let Ok(mut g) = ACTIVE_CONFIG_XML_PATH.lock() {
        *g = Some(p.to_path_buf());
    }
}

fn credentials_beside_active_config_xml() -> Option<PathBuf> {
    let g = ACTIVE_CONFIG_XML_PATH.lock().ok()?;
    let cfg = g.as_ref()?;
    let parent = cfg.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    Some(parent.join("credentials"))
}

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
    Some(CredentialEntry {
        username,
        password_or_token: password,
    })
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

/// Application id: keep in sync with Flutter native IDs:
/// `flutter_ui/macos/Runner/Configs/AppInfo.xcconfig` (`PRODUCT_BUNDLE_IDENTIFIER`),
/// `flutter_ui/linux/CMakeLists.txt` (`APPLICATION_ID`).
///
/// Flutter resolves `config.xml` as `{getApplicationSupportDirectory()}/tagliacarte/config.xml`.
pub const TAGLIACARTE_APPLICATION_ID: &str = "org.bluezoo.tagliacarte";

/// Directory segment under the OS application-support root (before `config.xml`), matching Dart.
const APP_DATA_SUBDIR: &str = "tagliacarte";

fn home_or_userprofile() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Legacy data root (`~/.tagliacarte` / `%USERPROFILE%\.tagliacarte`) for migration only.
fn legacy_dot_tagliacarte_dir() -> Option<PathBuf> {
    home_or_userprofile().map(|h| h.join(".tagliacarte"))
}

/// Canonical application data directory (Flutter GUI, TUI, and `flutter_rust_bridge` use the same tree).
///
/// Contains `config.xml`, `credentials`, `tui.log`, caches, etc. Override with **`TAGLIACARTE_DATA_DIR`**
/// or **`TAGLIACARTE_CONFIG_DIR`** (same meaning: path to the directory that holds `config.xml`).
///
/// Defaults match Flutter `path_provider` + `tagliacarte_api.dart` `_configPath`:
/// - **macOS:** `~/Library/Application Support/{bundle id}/tagliacarte`
/// - **Linux:** `$XDG_DATA_HOME|~/.local/share/{APPLICATION_ID}/tagliacarte`
/// - **Windows:** `%APPDATA%\org.bluezoo\Tagliacarte\tagliacarte`
pub fn tagliacarte_data_dir() -> Option<PathBuf> {
    for key in ["TAGLIACARTE_DATA_DIR", "TAGLIACARTE_CONFIG_DIR"] {
        if let Ok(d) = std::env::var(key) {
            let p = PathBuf::from(d.trim());
            if !p.as_os_str().is_empty() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = macos_real_user_home_dir().or_else(home_or_userprofile)?;
        return Some(
            home.join("Library/Application Support")
                .join(TAGLIACARTE_APPLICATION_ID)
                .join(APP_DATA_SUBDIR),
        );
    }

    #[cfg(all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let xdg_data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".local/share"));
        return Some(
            xdg_data
                .join(TAGLIACARTE_APPLICATION_ID)
                .join(APP_DATA_SUBDIR),
        );
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from)?;
        return Some(
            appdata
                .join("org.bluezoo")
                .join("Tagliacarte")
                .join(APP_DATA_SUBDIR),
        );
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        legacy_dot_tagliacarte_dir()
    }
}

/// Same as [`tagliacarte_data_dir`] — application data root (not only “config” in the narrow sense).
///
/// On **macOS App Sandbox**, prefer [`macos_real_user_home_dir`] inside [`tagliacarte_data_dir`] for
/// non-sandboxed CLIs; sandboxed Flutter still uses the container’s application support.
pub fn default_config_dir() -> Option<PathBuf> {
    tagliacarte_data_dir()
}

/// Default `config.xml` path: active Flutter/TUI location if the file exists, else legacy `~/.tagliacarte/config.xml`,
/// else the path where a new install should create the file ([`tagliacarte_data_dir`]/`config.xml`).
pub fn default_config_xml_path() -> Option<PathBuf> {
    let modern = tagliacarte_data_dir()?.join("config.xml");
    if modern.is_file() {
        return Some(modern);
    }
    let legacy = legacy_dot_tagliacarte_dir()?.join("config.xml");
    if legacy.is_file() {
        return Some(legacy);
    }
    Some(modern)
}

/// Login home directory from `getpwuid(getuid())` (passwd database).
///
/// Unlike `$HOME` / [`default_config_dir`], this is the real user path
/// (e.g. `/Users/you`) even when the app runs in the macOS App Sandbox.
/// Prefer `credentials` in the same directory as the active `config.xml`, then env / app data / legacy.
pub fn resolve_credentials_file_path() -> Option<std::path::PathBuf> {
    if let Some(c) = credentials_beside_active_config_xml() {
        if c.is_file() {
            return Some(c);
        }
    }

    if let Ok(s) = std::env::var("TAGLIACARTE_CONFIG") {
        let t = s.trim();
        if !t.is_empty() {
            let c = Path::new(t).parent()?.join("credentials");
            if c.is_file() {
                return Some(c);
            }
        }
    }

    let modern = default_credentials_path()?;
    if modern.is_file() {
        return Some(modern);
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            macos_real_user_home_dir().map(|h| h.join(".tagliacarte").join("credentials")),
            home_or_userprofile().map(|h| h.join(".tagliacarte").join("credentials")),
        ];
        for c in candidates.into_iter().flatten() {
            if c.is_file() {
                return Some(c);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let legacy = legacy_dot_tagliacarte_dir()?.join("credentials");
        if legacy.is_file() {
            return Some(legacy);
        }
    }

    if let Some(c) = credentials_beside_active_config_xml() {
        return Some(c);
    }

    if let Ok(s) = std::env::var("TAGLIACARTE_CONFIG") {
        let t = s.trim();
        if !t.is_empty() {
            if let Some(parent) = Path::new(t).parent() {
                if !parent.as_os_str().is_empty() {
                    return Some(parent.join("credentials"));
                }
            }
        }
    }

    Some(modern)
}

#[cfg(target_os = "macos")]
pub fn macos_real_user_home_dir() -> Option<std::path::PathBuf> {
    use std::ffi::CStr;
    use std::os::raw::c_char;

    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() {
            return None;
        }
        let dir = (*pw).pw_dir;
        if dir.is_null() {
            return None;
        }
        CStr::from_ptr(dir.cast::<c_char>())
            .to_str()
            .ok()
            .map(std::path::PathBuf::from)
    }
}

/// Default credentials path: `{[`tagliacarte_data_dir`]}/credentials`. Separate from `config.xml`.
/// Content is XML (root \<credentials\>, \<credential\> with \<uri\>, \<username\>, \<password\>).
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
    drop(fs::set_permissions(
        key_path,
        PermissionsExt::from_mode(0o600),
    ));
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
pub fn load_credentials(
    path: &Path,
    uri_for_keychain: Option<&str>,
) -> Result<HashMap<String, CredentialEntry>, String> {
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
            out.insert(
                uri,
                CredentialEntry {
                    username,
                    password_or_token,
                },
            );
        }
    }
    Ok(out)
}

/// Parse XML credentials using quick_xml. Expects <credentials><credential><uri>...</uri><username>...</username><password>...</password></credential>...</credentials>.
fn load_credentials_xml(content: &str) -> Result<HashMap<String, CredentialEntry>, String> {
    let mut reader = Reader::from_str(content);
    // Do not trim text nodes: passwords (and rare usernames) can legitimately start/end
    // with whitespace; trimming corrupts them when loading from the encrypted XML file.
    reader.config_mut().trim_text(false);
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
                } else if in_credential
                    && (name == b"uri" || name == b"username" || name == b"password")
                {
                    element_name.clear();
                    element_name.extend_from_slice(name);
                }
            }
            Ok(Event::Text(e)) => {
                if !in_credential || element_name.is_empty() {
                    continue;
                }
                let raw = e.unescape().map_err(|e| e.to_string())?;
                if element_name == b"uri" {
                    current_uri = raw.trim().to_string();
                } else if element_name == b"username" {
                    current_username = raw.into_owned();
                } else if element_name == b"password" {
                    current_password = raw.into_owned();
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
pub fn save_credential(
    path: &Path,
    uri: &str,
    username: &str,
    password_or_token: &str,
) -> Result<(), String> {
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
fn write_credentials_encrypted(
    path: &Path,
    entries: &HashMap<String, CredentialEntry>,
) -> Result<(), String> {
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
