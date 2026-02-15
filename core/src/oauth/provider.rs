/*
 * provider.rs
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

//! OAuth2 provider abstraction. Concrete providers for Google (Gmail) and Microsoft (Exchange/Outlook).

/// Trait describing an OAuth2 provider (authorization server endpoints, client_id, scopes).
pub trait OAuthProvider: Send + Sync {
    /// Short identifier: "google" or "microsoft".
    fn provider_id(&self) -> &str;
    /// Authorization endpoint URL.
    fn auth_url(&self) -> &str;
    /// Token endpoint URL.
    fn token_url(&self) -> &str;
    /// Scopes to request (space-joined when building the URL).
    fn scopes(&self) -> &[&str];
    /// OAuth2 client_id (public client; embedded in binary, no secret).
    fn client_id(&self) -> &str;
}

/// Google OAuth2 provider for Gmail (IMAP + XOAUTH2).
///
/// Scopes: `https://mail.google.com/` (full IMAP/POP/SMTP access), `openid`, `email`.
/// Auth: `https://accounts.google.com/o/oauth2/v2/auth`
/// Token: `https://oauth2.googleapis.com/token`
pub struct GoogleOAuthProvider {
    client_id: String,
}

impl GoogleOAuthProvider {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }
}

impl OAuthProvider for GoogleOAuthProvider {
    fn provider_id(&self) -> &str {
        "google"
    }

    fn auth_url(&self) -> &str {
        "https://accounts.google.com/o/oauth2/v2/auth"
    }

    fn token_url(&self) -> &str {
        "https://oauth2.googleapis.com/token"
    }

    fn scopes(&self) -> &[&str] {
        &["https://mail.google.com/", "openid", "email"]
    }

    fn client_id(&self) -> &str {
        &self.client_id
    }
}

/// Microsoft OAuth2 provider for Exchange / Outlook (Graph API + IMAP XOAUTH2).
///
/// Scopes: `Mail.ReadWrite`, `Mail.Send`, `IMAP.AccessAsUser.All`, `SMTP.Send`, `offline_access`, `openid`, `email`.
/// Auth: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`
/// Token: `https://login.microsoftonline.com/common/oauth2/v2.0/token`
pub struct MicrosoftOAuthProvider {
    client_id: String,
}

impl MicrosoftOAuthProvider {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }
}

impl OAuthProvider for MicrosoftOAuthProvider {
    fn provider_id(&self) -> &str {
        "microsoft"
    }

    fn auth_url(&self) -> &str {
        "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
    }

    fn token_url(&self) -> &str {
        "https://login.microsoftonline.com/common/oauth2/v2.0/token"
    }

    fn scopes(&self) -> &[&str] {
        &[
            "https://graph.microsoft.com/Mail.ReadWrite",
            "https://graph.microsoft.com/Mail.Send",
            "https://graph.microsoft.com/IMAP.AccessAsUser.All",
            "https://graph.microsoft.com/SMTP.Send",
            "offline_access",
            "openid",
            "email",
        ]
    }

    fn client_id(&self) -> &str {
        &self.client_id
    }
}

/// Look up a provider by id string. Returns None if unknown.
/// `google_client_id` and `microsoft_client_id` are the registered OAuth2 client IDs.
pub fn provider_by_id(
    id: &str,
    google_client_id: &str,
    microsoft_client_id: &str,
) -> Option<Box<dyn OAuthProvider>> {
    match id {
        "google" => Some(Box::new(GoogleOAuthProvider::new(google_client_id))),
        "microsoft" => Some(Box::new(MicrosoftOAuthProvider::new(microsoft_client_id))),
        _ => None,
    }
}
