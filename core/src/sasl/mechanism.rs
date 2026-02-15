/*
 * mechanism.rs
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

//! SASL mechanism names and metadata (gumdrop SASLMechanism).

/// Supported SASL mechanisms (client-side).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslMechanism {
    /// PLAIN (RFC 4616) – requires TLS.
    Plain,
    /// Legacy LOGIN – requires TLS.
    Login,
    /// CRAM-MD5 (RFC 2195) – challenge-response.
    CramMd5,
    /// SCRAM-SHA-256 (RFC 5802, 7677) – challenge-response.
    ScramSha256,
    /// XOAUTH2 – OAuth2 bearer token (Gmail, Outlook). Single-shot, no challenge.
    XOAuth2,
}

impl SaslMechanism {
    pub fn name(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::Login => "LOGIN",
            SaslMechanism::CramMd5 => "CRAM-MD5",
            SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
            SaslMechanism::XOAuth2 => "XOAUTH2",
        }
    }

    pub fn requires_tls(&self) -> bool {
        matches!(self, SaslMechanism::Plain | SaslMechanism::Login | SaslMechanism::XOAuth2)
    }

    pub fn is_challenge_response(&self) -> bool {
        matches!(self, SaslMechanism::CramMd5 | SaslMechanism::ScramSha256)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_uppercase().as_str() {
            "PLAIN" => Some(SaslMechanism::Plain),
            "LOGIN" => Some(SaslMechanism::Login),
            "CRAM-MD5" => Some(SaslMechanism::CramMd5),
            "SCRAM-SHA-256" => Some(SaslMechanism::ScramSha256),
            "XOAUTH2" => Some(SaslMechanism::XOAuth2),
            _ => None,
        }
    }
}

impl std::fmt::Display for SaslMechanism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
