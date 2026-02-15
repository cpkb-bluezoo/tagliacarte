/*
 * token_store.rs
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

//! OAuth2 token storage and automatic refresh.
//!
//! Tokens are stored in the same encrypted credentials file (or keychain) used
//! by password-based authentication, using the existing `config.rs` infrastructure.
//! The "password" field stores a JSON blob with the token data.
//!
//! Token refresh is transparent: `get_valid_access_token` checks expiry and
//! refreshes automatically if within the threshold.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::BytesMut;

use crate::config::{load_credentials, save_credential};
use crate::json::{JsonContentHandler, JsonNumber, JsonParser, JsonWriter};
use crate::oauth::provider::OAuthProvider;
use crate::oauth::flow::{refresh_access_token, OAuthTokens};

/// Threshold in seconds: refresh the token if it expires within this window.
const REFRESH_THRESHOLD_SECS: i64 = 300; // 5 minutes

/// Stored OAuth2 token entry (serialized as JSON in the credential store).
#[derive(Debug, Clone)]
pub struct OAuthTokenEntry {
    /// Provider id: "google" or "microsoft".
    pub provider: String,
    /// Current access token.
    pub access_token: String,
    /// Refresh token (long-lived; used to obtain new access tokens).
    pub refresh_token: String,
    /// Unix timestamp (seconds) when the access token expires.
    pub expires_at: i64,
    /// Space-separated scopes.
    pub scopes: String,
}

impl OAuthTokenEntry {
    /// Create from OAuth flow result.
    pub fn from_tokens(provider_id: &str, tokens: &OAuthTokens, scopes: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let expires_at = now + tokens.expires_in.unwrap_or(3600) as i64;
        Self {
            provider: provider_id.to_string(),
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone().unwrap_or_default(),
            expires_at,
            scopes: scopes.to_string(),
        }
    }

    /// Returns true if the access token is expired or will expire within the threshold.
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now >= (self.expires_at - REFRESH_THRESHOLD_SECS)
    }

    /// Serialize to JSON string for storage.
    pub fn to_json(&self) -> String {
        let mut w = JsonWriter::new();
        w.write_start_object();
        w.write_key("provider");
        w.write_string(&self.provider);
        w.write_key("access_token");
        w.write_string(&self.access_token);
        w.write_key("refresh_token");
        w.write_string(&self.refresh_token);
        w.write_key("expires_at");
        w.write_number(JsonNumber::I64(self.expires_at));
        w.write_key("scopes");
        w.write_string(&self.scopes);
        w.write_end_object();
        String::from_utf8(w.take_buffer().to_vec()).unwrap_or_default()
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Option<Self> {
        let mut handler = TokenEntryHandler::default();
        let mut parser = JsonParser::new();
        let mut buf = BytesMut::from(json.as_bytes());
        parser.receive(&mut buf, &mut handler).ok()?;
        parser.close(&mut handler).ok()?;
        // provider and access_token are required; others have defaults.
        let provider = handler.provider?;
        let access_token = handler.access_token?;
        Some(Self {
            provider,
            access_token,
            refresh_token: handler.refresh_token.unwrap_or_default(),
            expires_at: handler.expires_at?,
            scopes: handler.scopes.unwrap_or_default(),
        })
    }
}

/// Build the credential store key for an OAuth token.
/// Format: `oauth:{provider}:{uri}` to avoid collision with password credentials.
fn oauth_credential_key(provider_id: &str, uri: &str) -> String {
    format!("oauth:{}:{}", provider_id, uri)
}

/// Load an OAuth token entry from the credential store.
///
/// `credentials_path`: path to the encrypted credentials file.
/// `provider_id`: "google" or "microsoft".
/// `uri`: the store or transport URI this token is associated with.
pub fn load_oauth_token(
    credentials_path: &Path,
    provider_id: &str,
    uri: &str,
) -> Option<OAuthTokenEntry> {
    let key = oauth_credential_key(provider_id, uri);
    let uri_for_keychain = Some(key.as_str());
    let creds = load_credentials(credentials_path, uri_for_keychain).ok()?;
    let entry = creds.get(&key)?;
    // The password_or_token field holds the JSON-encoded token data.
    OAuthTokenEntry::from_json(&entry.password_or_token)
}

/// Save an OAuth token entry to the credential store.
pub fn save_oauth_token(
    credentials_path: &Path,
    provider_id: &str,
    uri: &str,
    token_entry: &OAuthTokenEntry,
) -> Result<(), String> {
    let key = oauth_credential_key(provider_id, uri);
    let json = token_entry.to_json();
    // Store with username = provider_id, password = JSON blob.
    save_credential(credentials_path, &key, provider_id, &json)
}

/// Get a valid (non-expired) access token, refreshing if necessary.
///
/// If the stored token is expired (or near-expiry), this function refreshes it
/// using the refresh token, updates the store, and returns the new access token.
///
/// `runtime_handle`: tokio runtime handle for running the async refresh.
pub fn get_valid_access_token(
    credentials_path: &Path,
    provider: &dyn OAuthProvider,
    uri: &str,
    runtime_handle: &tokio::runtime::Handle,
) -> Result<String, String> {
    let mut entry = load_oauth_token(credentials_path, provider.provider_id(), uri)
        .ok_or_else(|| format!("no OAuth token stored for {} ({})", provider.provider_id(), uri))?;

    if !entry.is_expired() {
        return Ok(entry.access_token.clone());
    }

    // Token is expired or near-expiry; refresh it.
    if entry.refresh_token.is_empty() {
        return Err("access token expired and no refresh token available".to_string());
    }

    let tokens = runtime_handle.block_on(refresh_access_token(provider, &entry.refresh_token))?;

    // Update the entry with the new tokens.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    entry.access_token = tokens.access_token.clone();
    entry.expires_at = now + tokens.expires_in.unwrap_or(3600) as i64;
    if let Some(ref rt) = tokens.refresh_token {
        entry.refresh_token = rt.clone();
    }

    // Persist the updated token.
    save_oauth_token(credentials_path, provider.provider_id(), uri, &entry)?;

    Ok(entry.access_token)
}

// ── JSON push-parser handler for OAuthTokenEntry ──────────────────────

/// Push-parser handler that extracts the 5 fields of an OAuthTokenEntry.
#[derive(Default)]
struct TokenEntryHandler {
    current_key: Option<String>,
    provider: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
    scopes: Option<String>,
}

impl JsonContentHandler for TokenEntryHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        match self.current_key.as_deref() {
            Some("provider") => self.provider = Some(value.to_string()),
            Some("access_token") => self.access_token = Some(value.to_string()),
            Some("refresh_token") => self.refresh_token = Some(value.to_string()),
            Some("scopes") => self.scopes = Some(value.to_string()),
            _ => {}
        }
        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.current_key.as_deref() == Some("expires_at") {
            self.expires_at = number.as_i64();
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, _value: bool) {
        self.current_key = None;
    }

    fn null_value(&mut self) {
        self.current_key = None;
    }
}
