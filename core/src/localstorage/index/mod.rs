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

//! Index keyed by MessageId: metadata, full-text, windowed list_conversations. Part of local storage only.

use crate::message_id::MessageId;
use crate::store::ConversationSummary;
use std::ops::Range;

/// Index for a local folder (keyed by MessageId). Used by Maildir/mbox Store implementations.
pub struct FolderIndex {
    _stub: (),
}

impl FolderIndex {
    pub fn open(_path: &std::path::Path) -> Result<Self, crate::store::StoreError> {
        Ok(Self { _stub: () })
    }

    pub fn list_conversations(&self, _range: Range<u64>) -> Result<Vec<ConversationSummary>, crate::store::StoreError> {
        Ok(Vec::new())
    }

    pub fn get_message_id_by_offset(&self, _offset: u64) -> Result<Option<MessageId>, crate::store::StoreError> {
        Ok(None)
    }
}
