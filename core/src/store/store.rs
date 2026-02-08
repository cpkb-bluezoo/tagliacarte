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
pub trait Store: Send + Sync {
    /// Kind of store (Email, Nostr, Matrix). Used by UI and FFI.
    fn store_kind(&self) -> StoreKind;

    /// List folders in this store (e.g. INBOX, Sent, Drafts).
    fn list_folders(&self) -> Result<Vec<FolderInfo>, StoreError>;

    /// Refresh folder list with streaming: call `on_folder` for each folder as it is discovered (e.g. per IMAP * LIST line).
    /// Call `on_complete(result)` when done. Returns immediately; callbacks may run on a background thread.
    /// Default implementation uses `list_folders()` and invokes callbacks (batch). Override for protocol-level streaming.
    fn refresh_folders_streaming(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        match self.list_folders() {
            Ok(folders) => {
                for f in folders {
                    on_folder(f);
                }
                on_complete(Ok(()));
            }
            Err(e) => {
                on_complete(Err(e));
            }
        }
        Ok(())
    }

    /// Open a folder by name. Returns a boxed Folder. Blocks until SELECT/open is done.
    fn open_folder(&self, name: &str) -> Result<Box<dyn Folder>, StoreError>;

    /// Start opening a folder with streaming: send SELECT (or equivalent), return immediately; call `on_event` for each response item, then `on_complete(Ok(folder))` or `on_complete(Err)`. Default returns error (not supported).
    fn start_open_folder_streaming(
        &self,
        _name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        _on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) -> Result<(), StoreError> {
        Err(StoreError::new("streaming open folder not supported"))
    }

    /// Hierarchy delimiter used in folder names (e.g. '/' or '.').
    fn hierarchy_delimiter(&self) -> Option<char>;

    /// Default folder name (e.g. "INBOX").
    fn default_folder(&self) -> Option<&str> {
        None
    }
}
