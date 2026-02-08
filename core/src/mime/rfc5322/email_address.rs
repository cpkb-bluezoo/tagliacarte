/*
 * email_address.rs
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

//! RFC 5322 email address (mailbox).

#[derive(Debug, Clone)]
pub struct EmailAddress {
    pub display_name: Option<String>,
    pub local_part: String,
    pub domain: String,
}

impl EmailAddress {
    pub fn new(
        display_name: Option<impl Into<String>>,
        local_part: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        Self {
            display_name: display_name.map(|s| s.into()),
            local_part: local_part.into(),
            domain: domain.into(),
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn local_part(&self) -> &str {
        &self.local_part
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Full mailbox address: local-part@domain.
    pub fn address(&self) -> String {
        format!("{}@{}", self.local_part, self.domain)
    }

    /// Envelope address for SMTP (same as address).
    pub fn envelope_address(&self) -> String {
        self.address()
    }
}

impl std::fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref dn) = self.display_name {
            if !dn.is_empty() {
                write!(f, "{} ", dn)?;
            }
        }
        write!(f, "<{}>", self.address())
    }
}

/// Format a mailbox for RFC 5322 headers (same format as EmailAddress Display).
/// Use when building messages; domain may be empty for local-only.
pub fn format_mailbox(display_name: Option<&str>, local_part: &str, domain: &str) -> String {
    let addr = if domain.is_empty() {
        local_part.to_string()
    } else {
        format!("{}@{}", local_part, domain)
    };
    match display_name {
        Some(dn) if !dn.is_empty() => format!("{} <{}>", dn, addr),
        _ => format!("<{}>", addr),
    }
}
