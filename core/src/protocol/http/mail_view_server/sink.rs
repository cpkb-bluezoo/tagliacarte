/*
 * sink.rs
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
//! The response side of one mail-view request: the handler code writes
//! heads and chunks into a [`ResponseSink`], which hands them to the
//! connection's hopf reactor. Every so often the sink reports a full
//! backlog; the writer then waits for the socket to drain (a callback from
//! the reactor) before writing more, so a slow WebView never makes the
//! server buffer a whole attachment. Nothing here blocks.

use std::io;
use std::sync::Arc;

use hopf_http::{Headers, ServerResponseHandle};

/// Bytes handed to the reactor between two waits for the socket to drain.
const DRAIN_EVERY: usize = 256 * 1024;
/// Piece size for [`ResponseSink::write_all`].
const PIECE: usize = 16 * 1024;

/// One parsed request: method, request-target (path and query) and headers.
#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub method: String,
    /// Path including query string (request-target from the request line).
    pub target: String,
    pub headers: Vec<(String, String)>,
}

impl ParsedRequest {
    pub(crate) fn from_headers(headers: &Headers) -> Self {
        Self {
            method: headers.get(":method").unwrap_or_default().to_string(),
            target: headers.get(":path").unwrap_or("/").to_string(),
            headers: headers
                .iter()
                .filter(|f| !f.name.starts_with(':'))
                .map(|f| (f.name.clone(), f.value.clone()))
                .collect(),
        }
    }
}

/// What [`ResponseSink::chunk`] says about the outbound backlog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backlog {
    /// More may be written right away.
    Room,
    /// Wait for [`ResponseSink::when_drained`] before writing more.
    Full,
}

/// What runs after a continuation-style write: the sink to go on with, or
/// the error that ended the response.
pub type Continue = Box<dyn FnOnce(io::Result<ResponseSink>) + Send>;

/// Where a request handler writes its response.
pub struct ResponseSink {
    handle: ServerResponseHandle,
    unacked: usize,
    started: bool,
    done: bool,
}

/// Request headers hopf frames itself; a handler's copy is dropped.
fn managed_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("transfer-encoding")
}

fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionReset, "client connection closed")
}

/// Run `k` on the connection's reactor once its outbound buffer is empty
/// (`true`), or at once with `false` when the connection is gone.
fn drained(handle: &ServerResponseHandle, k: Box<dyn FnOnce(bool) + Send>) {
    handle.conn_handle().with_endpoint(move |ep| {
        if !ep.is_open() {
            k(false);
            return;
        }
        ep.on_write_ready(Some(Box::new(move |ep| k(ep.is_open()))));
    });
}

impl ResponseSink {
    pub(crate) fn new(handle: ServerResponseHandle) -> Self {
        Self { handle, unacked: 0, started: false, done: false }
    }

    fn check_open(&self) -> io::Result<()> {
        if self.done || !self.handle.conn_handle().is_probably_open() {
            return Err(closed());
        }
        Ok(())
    }

    /// Status line and headers; the body follows with [`Self::chunk`].
    pub fn head(&mut self, status: u16, headers: &[(&str, &str)]) -> io::Result<()> {
        self.check_open()?;
        if self.started {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "response head already sent"));
        }
        let mut h = Headers::new();
        h.status(status);
        for (k, v) in headers {
            if !managed_header(k) {
                h.add(*k, *v);
            }
        }
        self.handle.execute(move |w| {
            w.headers(h);
            w.start_response_body();
        });
        self.started = true;
        Ok(())
    }

    /// Part of the body, queued for the reactor. Every [`DRAIN_EVERY`]
    /// bytes this reports [`Backlog::Full`]: write more only from
    /// [`Self::when_drained`] (or use [`Self::write`]).
    pub fn chunk(&mut self, data: &[u8]) -> io::Result<Backlog> {
        self.check_open()?;
        if !self.started {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "response head not sent"));
        }
        if data.is_empty() {
            return Ok(Backlog::Room);
        }
        let owned = data.to_vec();
        let n = owned.len();
        self.handle.execute(move |w| w.response_body_content(&owned));
        self.unacked += n;
        if self.unacked >= DRAIN_EVERY {
            self.unacked = 0;
            return Ok(Backlog::Full);
        }
        Ok(Backlog::Room)
    }

    /// Run `k` on the connection's reactor once everything queued so far
    /// has been written to the socket (`true`), or at once with `false`
    /// when the connection is gone.
    pub fn when_drained(&self, k: Box<dyn FnOnce(bool) + Send>) {
        drained(&self.handle, k)
    }

    /// End the body and complete the response.
    pub fn end(&mut self) -> io::Result<()> {
        self.check_open()?;
        if !self.started {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "response head not sent"));
        }
        self.handle.execute(|w| {
            w.end_response_body();
            w.complete();
        });
        self.done = true;
        Ok(())
    }

    /// Give up on the response: close the connection. For an error met
    /// after the head went out, when nothing truthful can be sent.
    pub fn abort(mut self) {
        self.done = true;
        self.handle.conn_handle().close();
    }

    /// A complete response with a `Content-Length` body.
    pub fn bytes(&mut self, status: u16, headers: &[(&str, &str)], body: &[u8]) -> io::Result<()> {
        let len = body.len().to_string();
        let mut all: Vec<(&str, &str)> = headers.to_vec();
        all.push(("Content-Length", len.as_str()));
        self.head(status, &all)?;
        self.chunk(body)?;
        self.end()
    }

    /// Queue `data`, then continue with `next`: at once when there is
    /// room, otherwise from the reactor once the backlog has drained.
    pub fn write(mut self, data: &[u8], next: Continue) {
        match self.chunk(data) {
            Err(e) => next(Err(e)),
            Ok(Backlog::Room) => next(Ok(self)),
            Ok(Backlog::Full) => {
                let handle = self.handle.clone();
                drained(&handle, Box::new(move |open| if open { next(Ok(self)) } else { next(Err(closed())) }));
            }
        }
    }

    /// Queue a whole body in pieces, waiting for the backlog to drain as
    /// needed, then continue with `next`.
    pub fn write_all(self, data: Vec<u8>, next: Continue) {
        self.write_from(Arc::new(data), 0, next)
    }

    fn write_from(mut self, data: Arc<Vec<u8>>, mut offset: usize, next: Continue) {
        while offset < data.len() {
            let end = (offset + PIECE).min(data.len());
            match self.chunk(&data[offset..end]) {
                Err(e) => return next(Err(e)),
                Ok(Backlog::Room) => offset = end,
                Ok(Backlog::Full) => {
                    offset = end;
                    let handle = self.handle.clone();
                    return drained(
                        &handle,
                        Box::new(move |open| if open { self.write_from(data, offset, next) } else { next(Err(closed())) }),
                    );
                }
            }
        }
        next(Ok(self))
    }
}
