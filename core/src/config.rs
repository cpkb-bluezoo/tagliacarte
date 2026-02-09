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

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Default config directory: ~/.tagliacarte.
pub fn default_config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from).map(|h| h.join(".tagliacarte"))
}

/// Default credentials path: ~/.tagliacarte/credentials. Separate from config.xml so we never
/// overwrite the UI's XML configuration. Content is line-based (uri\tusername\tpassword).
pub fn default_credentials_path() -> Option<std::path::PathBuf> {
    default_config_dir().map(|d| d.join("credentials"))
}

/// Credential entry for one store or transport (username optional for token-only).
#[derive(Debug, Clone, Default)]
pub struct CredentialEntry {
    pub username: String,
    pub password_or_token: String,
}

/// Load all stored credentials from the default path. Returns uri -> (username, password_or_token).
/// File format: one line per credential: "uri\tusername\tpassword" (tabs separate; values must not contain newline or tab).
pub fn load_credentials(path: &Path) -> Result<HashMap<String, CredentialEntry>, String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(e.to_string()),
    };
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

/// Save one credential and merge with existing file. Creates directory and file if needed.
/// Overwrites the line for this URI if present, otherwise appends.
pub fn save_credential(path: &Path, uri: &str, username: &str, password_or_token: &str) -> Result<(), String> {
    if uri.contains('\t') || uri.contains('\n') || username.contains('\t') || username.contains('\n')
        || password_or_token.contains('\t') || password_or_token.contains('\n')
    {
        return Err("credential values must not contain tab or newline".to_string());
    }
    let parent = path.parent().ok_or("no parent dir")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut entries = load_credentials(path).unwrap_or_default();
    entries.insert(
        uri.to_string(),
        CredentialEntry {
            username: username.to_string(),
            password_or_token: password_or_token.to_string(),
        },
    );
    let mut f = fs::File::create(path).map_err(|e| e.to_string())?;
    for (u, e) in &entries {
        writeln!(f, "{}\t{}\t{}", u, e.username, e.password_or_token).map_err(|e| e.to_string())?;
    }
    Ok(())
}
