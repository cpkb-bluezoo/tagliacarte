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

use crate::message_id::MessageId;
use crate::store::{ConversationSummary, Message};
use crate::store::{Folder, FolderInfo, Store, StoreError, StoreKind};
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

    fn list_folders(&self) -> Result<Vec<FolderInfo>, StoreError> {
        // TODO: sync/rooms, return one FolderInfo per joined room
        Ok(Vec::new())
    }

    fn open_folder(&self, name: &str) -> Result<Box<dyn Folder>, StoreError> {
        // name = room id (e.g. !abc:server)
        // TODO: return MatrixFolder for this room
        let _ = name;
        Err(StoreError::new("Matrix open_folder not yet implemented"))
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
    fn list_conversations(&self, range: Range<u64>) -> Result<Vec<ConversationSummary>, StoreError> {
        let _ = range;
        // TODO: paginate room timeline, map to ConversationSummary
        Ok(Vec::new())
    }

    fn message_count(&self) -> Result<u64, StoreError> {
        Ok(0)
    }

    fn get_message(&self, id: &MessageId) -> Result<Option<Message>, StoreError> {
        let _ = id;
        // TODO: fetch event by id, build Message
        Ok(None)
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

    fn send(&self, _payload: &SendPayload) -> Result<(), StoreError> {
        // TODO: map SendPayload to room/DM, send m.room.message; token refresh if 401
        Err(StoreError::new("Matrix send not yet implemented"))
    }
}
