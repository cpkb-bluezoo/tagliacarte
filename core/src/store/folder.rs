/*
 * folder.rs
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

//! Folder trait: contains Messages (conversations). Email folders support flat (list_messages) and thread view (list_threads, list_messages_in_thread).

use crate::message_id::MessageId;
use crate::store::error::StoreError;
use crate::store::message::{ConversationSummary, Message};
use std::ops::Range;

/// Opaque thread identifier (e.g. root Message-ID for email).
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ThreadId(pub String);

impl ThreadId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Summary of a thread for list view (email: subject + message count).
#[derive(Debug, Clone)]
pub struct ThreadSummary {
    pub id: ThreadId,
    pub subject: Option<String>,
    pub message_count: u64,
}

/// Metadata for a folder in a Store.
#[derive(Debug, Clone)]
pub struct FolderInfo {
    pub name: String,
    pub delimiter: Option<char>,
    pub attributes: Vec<String>,
}

/// A Folder contains Messages (e.g. IMAP mailbox, Maildir directory).
pub trait Folder: Send + Sync {
    /// List message summaries in range (flat view). Default delegates to list_conversations.
    fn list_messages(&self, range: Range<u64>) -> Result<Vec<ConversationSummary>, StoreError> {
        self.list_conversations(range)
    }

    /// List conversation summaries (same as list_messages; name retained for compatibility).
    fn list_conversations(&self, range: Range<u64>) -> Result<Vec<ConversationSummary>, StoreError>;

    /// Total message count in this folder.
    fn message_count(&self) -> Result<u64, StoreError>;

    /// Get a single message by stable id.
    fn get_message(&self, id: &MessageId) -> Result<Option<Message>, StoreError>;

    /// Request message list streaming: return immediately; call `on_summary` for each message as it is received, then `on_complete(result)`. Default uses `list_conversations` and invokes callbacks (batch).
    fn request_message_list_streaming(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        match self.list_conversations(range) {
            Ok(summaries) => {
                for s in summaries {
                    on_summary(s);
                }
                on_complete(Ok(()));
            }
            Err(e) => on_complete(Err(e)),
        }
        Ok(())
    }

    /// Request message streaming: return immediately; call `on_metadata(envelope)` when envelope is ready, `on_content_chunk(data)` for each chunk of body data, then `on_complete(result)`. Default uses `get_message` and invokes callbacks (batch).
    fn request_message_streaming(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(crate::store::Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        match self.get_message(id) {
            Ok(Some(msg)) => {
                on_metadata(msg.envelope);
                if let Some(ref b) = msg.body_plain {
                    on_content_chunk(b.as_bytes());
                }
                if let Some(ref b) = msg.body_html {
                    on_content_chunk(b.as_bytes());
                }
                on_complete(Ok(()));
            }
            Ok(None) => on_complete(Err(StoreError::new("message not found"))),
            Err(e) => on_complete(Err(e)),
        }
        Ok(())
    }

    /// List threads in range (email: group by subject + References/In-Reply-To). Default returns empty (non-email backends).
    fn list_threads(&self, _range: Range<u64>) -> Result<Vec<ThreadSummary>, StoreError> {
        Ok(Vec::new())
    }

    /// List message summaries in a thread. Default returns empty (non-email backends).
    fn list_messages_in_thread(
        &self,
        _thread_id: &ThreadId,
        _range: Range<u64>,
    ) -> Result<Vec<ConversationSummary>, StoreError> {
        Ok(Vec::new())
    }
}
