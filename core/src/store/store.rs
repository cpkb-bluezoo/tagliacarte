/*
 * store.rs
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

//! Store trait: contains hierarchically organised Folders.
//!
//! All methods are callback-driven and return immediately (never block).
//! For network protocols the callbacks fire asynchronously from the pipeline task.
//! For file-based backends the callbacks fire inline before the method returns.

use crate::store::error::StoreError;
use crate::store::folder::Folder;
use crate::store::kinds::StoreKind;
use crate::store::FolderInfo;

/// Event emitted during streaming open folder (e.g. IMAP SELECT response items).
#[derive(Debug, Clone)]
pub enum OpenFolderEvent {
    Exists(u32),
    Recent(u32),
    Flags(Vec<String>),
    UidValidity(u32),
    UidNext(u32),
    Other(String),
}

/// A Store contains (potentially) hierarchically organised Folders (e.g. IMAP mailboxes, local Maildir tree).
///
/// All operations are non-blocking: methods accept callbacks and return immediately.
/// Results are delivered via the callbacks, which may fire from a background task
/// (network protocols) or inline before the method returns (file-based backends).
pub trait Store: Send + Sync {
    /// Kind of store (Email, Nostr, Matrix). Used by UI and FFI.
    fn store_kind(&self) -> StoreKind;

    /// Set credential (password or token) for this store. Used after UI provides credential via FFI.
    /// No-op for stores that do not use password auth (e.g. Maildir).
    fn set_credential(&self, _username: Option<&str>, _password: &str) {
        // Default: no-op
    }

    /// List folders in this store. Calls `on_folder` for each folder as it is discovered
    /// (e.g. per IMAP `* LIST` line), then `on_complete` when done. Returns immediately.
    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    );

    /// Open a folder by name. Calls `on_event` for each status event (e.g. IMAP SELECT items),
    /// then `on_complete` with the opened Folder or an error. Returns immediately.
    fn open_folder(
        &self,
        name: &str,
        on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    );

    /// Hierarchy delimiter used in folder names (e.g. '/' or '.').
    fn hierarchy_delimiter(&self) -> Option<char>;

    /// Default folder name (e.g. "INBOX").
    fn default_folder(&self) -> Option<&str> {
        None
    }

    /// Create a folder. Returns immediately.
    /// On success: triggers on_folder_found callback with new folder info.
    /// on_complete called with Ok(()) or Err on failure.
    fn create_folder(
        &self,
        _name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("create folder not supported")));
    }

    /// Rename a folder. Returns immediately.
    /// On success: triggers on_folder_removed for old name, on_folder_found for new name.
    fn rename_folder(
        &self,
        _old_name: &str,
        _new_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("rename folder not supported")));
    }

    /// Delete a folder. Returns immediately.
    /// On success: triggers on_folder_removed callback.
    fn delete_folder(
        &self,
        _name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        on_complete(Err(StoreError::new("delete folder not supported")));
    }

    /// Configure delete semantics for this store (IMAP-specific). No-op for other backends.
    /// `mode`: 0 = mark \Deleted, 1 = move to trash.
    fn set_delete_config(&self, _mode: i32, _trash_folder: &str) {
        // No-op by default
    }
}
