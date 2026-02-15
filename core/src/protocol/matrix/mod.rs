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

//! Matrix backend (Store, Folder, Transport). Folder = one room. Connection reuse over HTTP;
//! semantic send; event-driven; MessageIds matrix://room/event; token refresh/re-login as needed.
//!
//! All trait methods are callback-driven and return immediately.

use crate::message_id::MessageId;
use crate::store::{ConversationSummary, Envelope};
use crate::store::{Folder, FolderInfo, OpenFolderEvent, Store, StoreError, StoreKind};
use crate::store::{SendPayload, Transport, TransportKind};
use std::ops::Range;

/// Matrix store: homeserver + auth (user id, token). list_folders = joined rooms.
pub struct MatrixStore {
    uri: String,
    /// Homeserver URL (e.g. https://matrix.example.org).
    _homeserver: String,
    /// User ID or localpart (e.g. @user:example.org or user).
    _user_id: String,
    /// Access token (refresh handled internally; never logged).
    _access_token: Option<String>,
}

impl MatrixStore {
    /// Create a new Matrix store. access_token: initial token; None = must log in first (stub: not implemented).
    pub fn new(
        homeserver: String,
        user_id: String,
        access_token: Option<String>,
    ) -> Result<Self, StoreError> {
        let uri = crate::uri::matrix_store_uri(&homeserver, &user_id);
        Ok(Self {
            uri,
            _homeserver: homeserver,
            _user_id: user_id,
            _access_token: access_token,
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }
}

impl Store for MatrixStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Matrix
    }

    fn list_folders(
        &self,
        _on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // TODO: sync/rooms, call on_folder per joined room
        on_complete(Ok(()));
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        // name = room id (e.g. !abc:server)
        // TODO: return MatrixFolder for this room
        let _ = name;
        on_complete(Err(StoreError::new("Matrix open_folder not yet implemented")));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        None
    }

    fn default_folder(&self) -> Option<&str> {
        None
    }
}

/// Folder = one Matrix room. Messages = room events (m.room.message).
#[allow(dead_code)]
struct MatrixFolder {
    _room_id: String,
    _store_uri: String,
}

impl Folder for MatrixFolder {
    fn list_conversations(
        &self,
        _range: Range<u64>,
        _on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // TODO: paginate room timeline, call on_summary per event
        on_complete(Ok(()));
    }

    fn message_count(
        &self,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    ) {
        on_complete(Ok(0));
    }

    fn get_message(
        &self,
        id: &MessageId,
        _on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        _on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let _ = id;
        // TODO: fetch event by id, call on_metadata + on_content_chunk
        on_complete(Err(StoreError::new("Matrix get_message not yet implemented")));
    }
}

/// Matrix transport: send to room or user. Same account as store.
pub struct MatrixTransport {
    uri: String,
    _homeserver: String,
    _access_token: Option<String>,
}

impl MatrixTransport {
    pub fn new(
        homeserver: String,
        user_id: String,
        access_token: Option<String>,
    ) -> Result<Self, StoreError> {
        let uri = crate::uri::matrix_transport_uri(&homeserver, &user_id);
        Ok(Self {
            uri,
            _homeserver: homeserver,
            _access_token: access_token,
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }
}

impl Transport for MatrixTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Matrix
    }

    fn send(
        &self,
        _payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        // TODO: map SendPayload to room/DM, send m.room.message; token refresh if 401
        on_complete(Err(StoreError::new("Matrix send not yet implemented")));
    }
}
