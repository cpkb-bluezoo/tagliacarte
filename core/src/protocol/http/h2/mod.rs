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

//! HTTP/2: frame parser, frame handler, frame writer (no external h2 crate).

mod frame;
mod handler;
mod parser;
mod writer;

pub use frame::{
    error_to_string, DEFAULT_MAX_FRAME_SIZE, FLAG_ACK, FLAG_END_HEADERS, FLAG_END_STREAM,
    FLAG_PADDED, FLAG_PRIORITY, FRAME_HEADER_LENGTH, MAX_MAX_FRAME_SIZE, MIN_MAX_FRAME_SIZE,
    SETTINGS_HEADER_TABLE_SIZE, SETTINGS_INITIAL_WINDOW_SIZE, SETTINGS_MAX_CONCURRENT_STREAMS,
    SETTINGS_MAX_FRAME_SIZE, SETTINGS_MAX_HEADER_LIST_SIZE, TYPE_CONTINUATION, TYPE_DATA,
    TYPE_GOAWAY, TYPE_HEADERS, TYPE_PING, TYPE_PRIORITY, TYPE_PUSH_PROMISE, TYPE_RST_STREAM,
    TYPE_SETTINGS, TYPE_WINDOW_UPDATE,
};
pub use handler::H2FrameHandler;
pub use parser::H2Parser;
pub use writer::H2Writer;

/// HTTP/2 connection preface (PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n).
pub const CONNECTION_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
