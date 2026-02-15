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

//! Folder trait: contains Messages (conversations).
//!
//! All methods are callback-driven and return immediately (never block).
//! For network protocols the callbacks fire asynchronously from the pipeline task.
//! For file-based backends the callbacks fire inline before the method returns.

use crate::message_id::MessageId;
use crate::store::error::StoreError;
use crate::store::message::{ConversationSummary, Flag};
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
///
/// All operations are non-blocking: methods accept callbacks and return immediately.
/// Results are delivered via the callbacks, which may fire from a background task
/// (network protocols) or inline before the method returns (file-based backends).
pub trait Folder: Send + Sync {
    /// List message summaries in range (flat view).
    /// Calls `on_summary` for each message, then `on_complete` when done.
    /// Default delegates to `list_conversations`.
    fn list_messages(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        self.list_conversations(range, on_summary, on_complete);
    }

    /// List conversation summaries in range.
    /// Calls `on_summary` for each conversation, then `on_complete` when done.
    fn list_conversations(
        &self,
        range: Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    );

    /// Total message count in this folder.
    /// Calls `on_complete` with the count or an error.
    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    );

    /// Get a single message by stable id.
    /// Calls `on_metadata` with the envelope when available,
    /// `on_content_chunk` for each chunk of raw message data,
    /// then `on_complete` when done.
    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(crate::store::Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    );

    /// Delete a message by id. Calls `on_complete` when done.
    fn delete_message(
        &self,
        _id: &MessageId,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("delete not supported for this folder")));
    }

    /// List threads in range (email: group by subject + References/In-Reply-To).
    /// Calls `on_thread` for each thread, then `on_complete`.
    /// Default calls `on_complete(Ok(()))` immediately (non-email backends).
    fn list_threads(
        &self,
        _range: Range<u64>,
        _on_thread: Box<dyn Fn(ThreadSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Ok(()));
    }

    /// List message summaries in a thread.
    /// Calls `on_summary` for each message, then `on_complete`.
    /// Default calls `on_complete(Ok(()))` immediately (non-email backends).
    fn list_messages_in_thread(
        &self,
        _thread_id: &ThreadId,
        _range: Range<u64>,
        _on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Ok(()));
    }

    /// Append raw message bytes (e.g. from .eml file) to this folder.
    /// Calls `on_complete` when done.
    fn append_message(
        &self,
        _data: &[u8],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("append not supported for this folder")));
    }

    /// Copy messages to another folder within the same store. `ids` are raw id strings
    /// (UIDs for IMAP, paths for Maildir). Default: not supported.
    fn copy_messages_to(
        &self,
        _ids: &[&str],
        _dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("copy not supported for this folder")));
    }

    /// Move messages to another folder within the same store. Default: not supported.
    fn move_messages_to(
        &self,
        _ids: &[&str],
        _dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("move not supported for this folder")));
    }

    /// Change flags on messages. `add` flags are set, `remove` flags are cleared.
    /// `ids` are raw id strings. Default: not supported.
    fn store_flags(
        &self,
        _ids: &[&str],
        _add: &[Flag],
        _remove: &[Flag],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("flags not supported for this folder")));
    }

    /// Expunge all messages marked \Deleted from this folder. Default: not supported.
    fn expunge(
        &self,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("expunge not supported for this folder")));
    }
}
