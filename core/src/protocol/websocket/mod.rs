/*
 * mod.rs
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

//! WebSocket client (RFC 6455) on hopf: the HTTP/1.1 upgrade and framing
//! come from hopf-http and hopf-websocket. A [`WebSocketHandler`] is driven
//! from the connection's reactor; a [`WebSocketSession`] sends from
//! anywhere.

mod client;
mod connection;
mod handler;

pub use client::{WebSocketClient, HANDSHAKE_TIMEOUT};
pub use connection::WebSocketSession;
pub use handler::WebSocketHandler;
