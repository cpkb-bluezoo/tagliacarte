/*
 * client.rs
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
//! HTTP client: dial a host on hopf and hand the connection to a
//! continuation; requests then go out with a callback handler.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_http::{HttpClientSessionHandle, HttpClientTimeouts, HttpConnectionHandler, HttpLimits};

use crate::net::{hopf_http_tls_connector, hopf_runtime};
use crate::protocol::http::connection::{HttpConnection, HttpVersion};

const DNS_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Quiet-time budget for one request once it is on the wire; hopf re-arms
/// it on every byte in either direction, so a slow but live download is
/// not cut off.
const STAGE_TIMEOUT: Duration = Duration::from_secs(120);

/// Hears the outcome of a dial, once: from hopf's reactor when the
/// connection (and TLS handshake) is up or has failed, or from the calling
/// thread when the dial is refused outright.
pub type OnConnect = Box<dyn FnOnce(io::Result<HttpConnection>) + Send>;

/// HTTP client. Dial with [`HttpClient::connect`]; the continuation gets an
/// [`HttpConnection`] to build requests on and send with a handler.
pub struct HttpClient;

struct DialHandler {
    reply: Arc<Mutex<Option<OnConnect>>>,
    host: String,
    port: u16,
    secure: bool,
    alive: Arc<AtomicBool>,
}

impl DialHandler {
    fn reply(&self, r: io::Result<HttpConnection>) {
        let k = self.reply.lock().unwrap().take();
        if let Some(k) = k {
            k(r);
        }
    }
}

impl HttpConnectionHandler for DialHandler {
    fn on_connected(&mut self, session: &mut HttpClientSessionHandle) {
        let version = match session.version() {
            hopf_http::HttpVersion::Http2 => HttpVersion::Http2,
            _ => HttpVersion::Http1_1,
        };
        self.reply(Ok(HttpConnection::new(
            session.clone(),
            self.host.clone(),
            self.port,
            self.secure,
            version,
            Arc::clone(&self.alive),
        )));
    }

    fn on_disconnected(&mut self) {
        self.alive.store(false, Ordering::Release);
        self.reply(Err(io::Error::new(io::ErrorKind::ConnectionReset, "connection closed")));
    }

    fn on_error(&mut self, err: &io::Error) {
        self.alive.store(false, Ordering::Release);
        self.reply(Err(io::Error::new(err.kind(), err.to_string())));
    }
}

impl HttpClient {
    /// Connect to the given host and port. If `use_tls` is true, performs a
    /// TLS handshake with ALPN (h2, http/1.1). `on_connect` gets an
    /// `HttpConnection` to issue requests on. The negotiated protocol
    /// (HTTP/1.1 or HTTP/2) is set from ALPN when using TLS; plain TCP uses
    /// HTTP/1.1.
    pub fn connect(host: &str, port: u16, use_tls: bool, on_connect: OnConnect) {
        Self::dial(host, port, use_tls, false, on_connect)
    }

    /// Cleartext HTTP/2 with prior knowledge (RFC 9113 §3.3), for servers
    /// known to speak it without TLS, such as a local one in tests.
    pub fn connect_h2c(host: &str, port: u16, on_connect: OnConnect) {
        Self::dial(host, port, false, true, on_connect)
    }

    fn dial(host: &str, port: u16, use_tls: bool, h2c: bool, on_connect: OnConnect) {
        let limits = HttpLimits {
            // Attachments come through here whole; no cap on what a decoded
            // response may grow to (the consumer streams it anyway).
            max_decoded_body: usize::MAX,
            ..HttpLimits::default()
        };
        let mut client = hopf_http::HttpClient::new(host, port)
            .timeouts(HttpClientTimeouts { dns: DNS_TIMEOUT, connect: CONNECT_TIMEOUT, stage: STAGE_TIMEOUT })
            .limits(limits)
            .resolver(crate::net::hopf_dns_resolver())
            .h2_prior_knowledge(h2c);
        if use_tls {
            client = client.tls(hopf_http_tls_connector(), host);
        }
        let reply = Arc::new(Mutex::new(Some(on_connect)));
        let handler = DialHandler {
            reply: Arc::clone(&reply),
            host: host.to_string(),
            port,
            secure: use_tls,
            alive: Arc::new(AtomicBool::new(true)),
        };
        if let Err(e) = client.connect(&hopf_runtime(), Box::new(handler)) {
            let k = reply.lock().unwrap().take();
            if let Some(k) = k {
                k(Err(e));
            }
        }
    }
}
