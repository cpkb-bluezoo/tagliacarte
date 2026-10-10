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
//! A live WebSocket connection on hopf: frames are written here and handed
//! to the connection's reactor, from any thread.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use hopf_core::{ConnHandle, TimerHandle};
use hopf_websocket::{write_binary, write_close, write_close_empty, write_ping, write_pong, write_text, WsRole};

/// Handle to an open WebSocket connection. Cheap to clone; sending from a
/// handler callback or from another thread is the same call. Dropping
/// the handles does not close the connection: call [`Self::close`].
#[derive(Clone)]
pub struct WebSocketSession {
    conn: ConnHandle,
    open: Arc<AtomicBool>,
}

impl WebSocketSession {
    pub(crate) fn new(conn: ConnHandle) -> Self {
        Self { conn, open: Arc::new(AtomicBool::new(true)) }
    }

    pub(crate) fn mark_closed(&self) {
        self.open.store(false, Ordering::Release);
    }

    /// Whether the connection is still open as far as we know.
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire) && self.conn.is_probably_open()
    }

    fn send_raw(&self, frame: Vec<u8>) -> io::Result<()> {
        if !self.is_open() {
            return Err(io::Error::new(io::ErrorKind::ConnectionReset, "WebSocket connection closed"));
        }
        self.conn.send(frame);
        Ok(())
    }

    /// Send a text frame.
    pub fn send_text(&self, text: &str) -> io::Result<()> {
        let mut out = Vec::with_capacity(text.len() + 14);
        write_text(&mut out, WsRole::Client, text);
        self.send_raw(out)
    }

    /// Send a binary frame.
    pub fn send_binary(&self, data: &[u8]) -> io::Result<()> {
        let mut out = Vec::with_capacity(data.len() + 14);
        write_binary(&mut out, WsRole::Client, data);
        self.send_raw(out)
    }

    /// Send a ping frame.
    pub fn send_ping(&self, data: &[u8]) -> io::Result<()> {
        if data.len() > 125 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "ping payload max 125 bytes"));
        }
        let mut out = Vec::new();
        write_ping(&mut out, WsRole::Client, data);
        self.send_raw(out)
    }

    /// Send a pong frame.
    pub fn send_pong(&self, data: &[u8]) -> io::Result<()> {
        if data.len() > 125 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "pong payload max 125 bytes"));
        }
        let mut out = Vec::new();
        write_pong(&mut out, WsRole::Client, data);
        self.send_raw(out)
    }

    /// Send a close frame. Reason is UTF-8; code is optional (e.g. 1000 = normal).
    pub fn send_close(&self, code: Option<u16>, reason: &str) -> io::Result<()> {
        if reason.len() + 2 > 125 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "close payload max 125 bytes"));
        }
        let mut out = Vec::new();
        match code {
            Some(c) => write_close(&mut out, WsRole::Client, c, reason),
            None => write_close_empty(&mut out, WsRole::Client),
        }
        self.send_raw(out)
    }

    /// Tear the connection down. The handler hears `failed` with
    /// `UnexpectedEof` once hopf has closed it.
    pub fn close(&self) {
        self.mark_closed();
        self.conn.close();
    }

    /// Run `callback` on this connection's reactor after `delay`.
    pub fn schedule_timer(&self, delay: Duration, callback: Box<dyn FnOnce() + Send>) -> TimerHandle {
        self.conn.schedule_timer(delay, callback)
    }
}
