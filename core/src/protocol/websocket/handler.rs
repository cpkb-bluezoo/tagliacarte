/*
 * client.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * This file is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */
//! WebSocket handler trait: the connection drives it from hopf's reactor
//! as frames arrive, handing it the session to reply on.

use crate::protocol::websocket::WebSocketSession;

/// Handler for WebSocket events (push model). Every method runs on the
/// connection's reactor thread; `ws` sends frames back or closes.
pub trait WebSocketHandler: Send {
    /// Handshake succeeded; the connection is in WebSocket frame mode.
    fn connected(&mut self, ws: &WebSocketSession);
    /// Text frame payload. Data is valid only for the duration of the call.
    fn text_frame(&mut self, ws: &WebSocketSession, data: &[u8]);
    /// Binary frame payload.
    fn binary_frame(&mut self, ws: &WebSocketSession, data: &[u8]);
    /// Close frame (optional code + reason). The connection closes after
    /// this; nothing else is delivered.
    fn close(&mut self, code: Option<u16>, reason: &str);
    /// Ping received; the pong has already been sent.
    fn ping(&mut self, ws: &WebSocketSession, data: &[u8]) {
        let _ = (ws, data);
    }
    /// Pong received (e.g. in response to our ping).
    fn pong(&mut self, ws: &WebSocketSession, data: &[u8]) {
        let _ = (ws, data);
    }
    /// The dial, the handshake or the connection failed; nothing else is
    /// delivered. A transport that went away without a Close frame
    /// arrives here as `UnexpectedEof`.
    fn failed(&mut self, error: &std::io::Error);
}
