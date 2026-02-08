/*
 * mime_version.rs
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

//! MIME-Version header (RFC 2045).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MimeVersion {
    Version1_0,
}

impl MimeVersion {
    pub fn as_str(&self) -> &'static str {
        "1.0"
    }

    pub fn parse(s: &str) -> Option<Self> {
        if s.trim() == "1.0" {
            Some(MimeVersion::Version1_0)
        } else {
            None
        }
    }
}

impl std::fmt::Display for MimeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
