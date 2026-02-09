/*
 * error.rs
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

//! Store and protocol errors.

use std::fmt;

/// Errors from Store, Folder, Transport, or protocol operations.
#[derive(Debug)]
pub enum StoreError {
    /// Generic error message.
    Message(String),
    /// Credential required before connect; UI should prompt and call credential_provide or credential_cancel.
    NeedsCredential {
        username: String,
        is_plaintext: bool,
    },
}

impl StoreError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Message(m) => write!(f, "{}", m),
            StoreError::NeedsCredential { username, .. } => {
                write!(f, "credential required for {}", username)
            }
        }
    }
}

impl std::error::Error for StoreError {}
