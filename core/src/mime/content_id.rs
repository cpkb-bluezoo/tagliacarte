/*
 * content_id.rs
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

//! Content-ID / Message-ID (RFC 2045, RFC 5322): &lt;local@domain&gt;

#[derive(Debug, Clone)]
pub struct ContentID {
    local_part: String,
    domain: String,
}

impl ContentID {
    pub fn new(local_part: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            local_part: local_part.into(),
            domain: domain.into(),
        }
    }

    pub fn get_local_part(&self) -> &str {
        &self.local_part
    }

    pub fn get_domain(&self) -> &str {
        &self.domain
    }
}

impl std::fmt::Display for ContentID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}@{}>", self.local_part, self.domain)
    }
}

/// Parse a single Content-ID or Message-ID value (with or without angle brackets).
pub fn parse_content_id(value: &str) -> Option<ContentID> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let start = if value.starts_with('<') {
        1
    } else {
        0
    };
    let end = value.len()
        - if value.ends_with('>') {
            1
        } else {
            0
        };
    if start >= end {
        return None;
    }
    let content = &value[start..end];
    let at = content.find('@')?;
    if at < 1 || at >= content.len() - 1 {
        return None;
    }
    let local = content[..at].trim();
    let domain = content[at + 1..].trim();
    if local.is_empty() || domain.is_empty() {
        return None;
    }
    Some(ContentID::new(local, domain))
}
