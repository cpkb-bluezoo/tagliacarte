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

//! Store abstraction: Store, Folder, Message, Transport traits and types.

mod error;
mod folder;
mod kinds;
mod message;
mod send_session;
mod store;
mod transport;

pub use error::StoreError;
pub use folder::Folder;
pub use kinds::{StoreKind, TransportKind};
pub use message::{Address, Attachment, ConversationSummary, DateTime, Envelope, Flag, Message, SendPayload};
pub use send_session::SendSession;
pub use store::{OpenFolderEvent, Store};
pub use transport::Transport;

pub use folder::{FolderInfo, ThreadId, ThreadSummary};
