/*
 * fetch.rs
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

//! One-shot requests by URL: parse the URL, dial on hopf, collect the
//! response. For the small REST and DAV clients that want a status and a
//! body and nothing else.

use std::io;

use crate::protocol::http::client::OnConnect;
use crate::protocol::http::{HttpClient, HttpConnection, Method, Response, ResponseHandler};

/// The parts of an `http://` or `https://` URL a dial needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpUrl {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    /// Path and query, always starting with `/`.
    pub target: String,
}

impl HttpUrl {
    /// Parse an absolute `http` or `https` URL.
    pub fn parse(url: &str) -> io::Result<Self> {
        let url = url.trim();
        let (tls, rest) = if let Some(r) = url.strip_prefix("https://") {
            (true, r)
        } else if let Some(r) = url.strip_prefix("http://") {
            (false, r)
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("not an http(s) URL: {url}")));
        };
        let (authority, target) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        // Credentials in the URL are not supported; strip them rather than
        // dialling the wrong host.
        let authority = authority.rsplit('@').next().unwrap_or(authority);
        let default_port = if tls { 443 } else { 80 };
        let (host, port) = if let Some(end) = authority.strip_prefix('[').and_then(|a| a.find(']')) {
            let host = &authority[1..=end];
            let after = &authority[end + 2..];
            let port = match after.strip_prefix(':') {
                Some(p) => p.parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid port"))?,
                None => default_port,
            };
            (host.to_string(), port)
        } else {
            match authority.rfind(':') {
                Some(i) => (
                    authority[..i].to_string(),
                    authority[i + 1..].parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid port"))?,
                ),
                None => (authority.to_string(), default_port),
            }
        };
        if host.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "URL has no host"));
        }
        Ok(Self { host, port, tls, target: target.to_string() })
    }

    /// Same host, port and scheme: a request for it can reuse a connection.
    pub fn same_origin(&self, other: &HttpUrl) -> bool {
        self.host.eq_ignore_ascii_case(&other.host) && self.port == other.port && self.tls == other.tls
    }

    /// Dial this origin.
    pub fn connect(&self, on_connect: OnConnect) {
        HttpClient::connect(&self.host, self.port, self.tls, on_connect)
    }

}

/// A whole response, collected.
#[derive(Debug, Clone, Default)]
pub struct Collected {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Collected {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Hears one collected response.
pub type OnCollected = Box<dyn FnOnce(io::Result<Collected>) + Send>;

/// Collects everything, then hands it on.
struct Collector {
    out: Collected,
    done: Option<OnCollected>,
}

impl ResponseHandler for Collector {
    fn ok(&mut self, r: Response) {
        self.out.status = r.code;
    }
    fn error(&mut self, r: Response) {
        self.out.status = r.code;
    }
    fn header(&mut self, name: &str, value: &str) {
        self.out.headers.push((name.to_string(), value.to_string()));
    }
    fn start_body(&mut self) {}
    fn body_chunk(&mut self, data: &[u8]) {
        self.out.body.extend_from_slice(data);
    }
    fn end_body(&mut self) {}
    fn complete(&mut self) {
        if let Some(k) = self.done.take() {
            k(Ok(std::mem::take(&mut self.out)));
        }
    }
    fn failed(&mut self, error: &io::Error) {
        if let Some(k) = self.done.take() {
            k(Err(io::Error::new(error.kind(), error.to_string())));
        }
    }
}

/// Send one request on `conn` and collect the response. `on_done` hears
/// the outcome exactly once.
pub fn send_collect(
    conn: &HttpConnection,
    method: Method,
    target: &str,
    headers: &[(&str, &str)],
    body: Option<Vec<u8>>,
    on_done: OnCollected,
) {
    let mut req = conn.request(method, target);
    for (k, v) in headers {
        req.header(*k, *v);
    }
    if let Some(b) = body {
        req.header("Content-Length", b.len().to_string());
        req.body(b);
    }
    conn.send(req, Collector { out: Collected::default(), done: Some(on_done) });
}

/// Dial `url` and perform one request; the connection lives until the
/// response has been collected.
pub fn fetch(method: Method, url: &str, headers: &[(&str, &str)], body: Option<Vec<u8>>, on_done: OnCollected) {
    let u = match HttpUrl::parse(url) {
        Ok(u) => u,
        Err(e) => {
            on_done(Err(e));
            return;
        }
    };
    let headers: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let target = u.target.clone();
    u.connect(Box::new(move |conn| {
        let conn = match conn {
            Ok(c) => c,
            Err(e) => {
                on_done(Err(e));
                return;
            }
        };
        let keep = conn.clone();
        let borrowed: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        send_collect(
            &conn,
            method,
            &target,
            &borrowed,
            body,
            Box::new(move |r| {
                drop(keep);
                on_done(r);
            }),
        );
    }));
}

// ── blocking forms for batch jobs (never from a hopf reactor thread) ──

/// Longer than hopf's own quiet-time budget for a request, so a stalled
/// connection is reported by hopf first; this only guards against a
/// handler that was dropped without a word.
const BLOCKING_WAIT: std::time::Duration = std::time::Duration::from_secs(180);

/// Wait on this thread for a callback-reported outcome. For code that runs
/// a sequence of requests from an ordinary thread (sync jobs, FFI entry
/// points), not from a hopf reactor thread, which must never block.
pub(crate) fn wait_for<T: Send + 'static>(start: impl FnOnce(Box<dyn FnOnce(io::Result<T>) + Send>)) -> io::Result<T> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<io::Result<T>>(1);
    start(Box::new(move |r| {
        let _ = tx.send(r);
    }));
    match rx.recv_timeout(BLOCKING_WAIT) {
        Ok(r) => r,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(io::Error::new(io::ErrorKind::TimedOut, "no reply from the HTTP session"))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(io::Error::new(io::ErrorKind::ConnectionReset, "connection closed"))
        }
    }
}

impl HttpUrl {
    /// [`Self::connect`] waited for on this thread.
    pub fn connect_blocking(&self) -> io::Result<HttpConnection> {
        wait_for(|k| self.connect(k))
    }
}

/// [`send_collect`] waited for on this thread.
pub fn send_collect_blocking(
    conn: &HttpConnection,
    method: Method,
    target: &str,
    headers: &[(&str, &str)],
    body: Option<Vec<u8>>,
) -> io::Result<Collected> {
    wait_for(|k| send_collect(conn, method, target, headers, body, k))
}

/// [`fetch`] waited for on this thread.
pub fn fetch_blocking(method: Method, url: &str, headers: &[(&str, &str)], body: Option<Vec<u8>>) -> io::Result<Collected> {
    wait_for(|k| fetch(method, url, headers, body, k))
}
