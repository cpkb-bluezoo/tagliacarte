/*
 * mod.rs
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

//! POP3 client (Store with single INBOX folder).

use crate::store::{Folder, FolderInfo, Store, StoreError, StoreKind};

/// POP3 store (single folder INBOX).
pub struct Pop3Store {
    _host: String,
    _port: u16,
}

impl Pop3Store {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            _host: host.into(),
            _port: port,
        }
    }
}

impl Store for Pop3Store {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn list_folders(&self) -> Result<Vec<FolderInfo>, StoreError> {
        // POP3 has only INBOX.
        Ok(vec![FolderInfo {
            name: "INBOX".to_string(),
            delimiter: None,
            attributes: vec![],
        }])
    }

    fn open_folder(&self, _name: &str) -> Result<Box<dyn Folder>, StoreError> {
        Err(StoreError::new("POP3 not yet implemented"))
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        Some("INBOX")
    }
}
