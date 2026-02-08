/*
 * transport.rs
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

//! Transport trait: send messages (e.g. SMTP). Semantic send via SendPayload or streaming SendSession.

use crate::store::error::StoreError;
use crate::store::kinds::TransportKind;
use crate::store::message::SendPayload;
use crate::store::send_session::SendSession;

/// Transport for sending messages (e.g. SMTP). One per Store when configurable.
/// Supports both one-shot send (payload) and non-blocking streaming send (SendSession).
pub trait Transport: Send + Sync {
    /// Kind of transport (Email, Nostr, Matrix). Used by UI and FFI.
    fn transport_kind(&self) -> TransportKind;

    /// Send a message from structured payload. Blocks until done. Prefer streaming send for UI.
    fn send(&self, payload: &SendPayload) -> Result<(), StoreError>;

    /// Start a streaming send session. Returns a session that accepts metadata, body chunks, and
    /// attachment chunks; call end_send() to finish and get a Future that completes when send is done.
    /// Default returns error (not supported); override for transports that support streaming.
    fn start_send(&self) -> Result<Box<dyn SendSession>, StoreError> {
        Err(StoreError::new("streaming send not supported"))
    }
}
