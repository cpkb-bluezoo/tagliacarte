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
//! WebSocket client: connect to a ws:// or wss:// URL on hopf, perform the
//! opening handshake, and drive a [`WebSocketHandler`] from the reactor.
//!
//! The dial uses hopf-http's `ClientHandler` SPI: the HTTP/1.1 `Upgrade`
//! request goes out from `start`, the `101` is validated in
//! `switching_protocols`, and hopf-websocket's `WsUpgradeHandler` then
//! owns the connection, forwarding every frame to the handler.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_core::ConnHandle;
use hopf_http::{
    connect_http, connect_https, ClientHandler, ClientHandlerFactory, ClientWriter, DialSettings,
    Headers, HttpClientTimeouts, HttpLimits,
};
use hopf_websocket::{WebSocketOpening, WsEventHandler, WsFrameError, WsSession, WsUpgradeHandler};

use crate::net::{hopf_runtime, hopf_tls_connector};
use crate::protocol::websocket::{WebSocketHandler, WebSocketSession};

/// Largest message a relay may send us before the frame parser gives up.
const MAX_PAYLOAD: usize = 16 * 1024 * 1024;
const DNS_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Default upper bound on the whole dial plus handshake. A refused TCP
/// connect or a DNS failure is not reported through the `ClientHandler`
/// SPI, so the wait has to end on its own.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(25);

/// Parsed components of a WebSocket URL.
struct WsUrl<'a> {
    scheme: &'a str,
    host: &'a str,
    port: u16,
    path: &'a str,
}

/// Parse a ws:// or wss:// URL into its components.
fn parse_ws_url(url: &str) -> io::Result<WsUrl<'_>> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("wss://") {
        ("wss", r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        ("ws", r)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "URL scheme must be ws or wss",
        ));
    };
    let default_port: u16 = if scheme == "wss" { 443 } else { 80 };

    // Split host+port from path
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    // Split host and optional port
    // Handle IPv6 literal [::1]:port
    let (host, port) = if authority.starts_with('[') {
        // IPv6 literal
        match authority.find(']') {
            Some(end) => {
                let h = &authority[1..end];
                let after = &authority[end + 1..];
                let p = if let Some(port_str) = after.strip_prefix(':') {
                    port_str
                        .parse::<u16>()
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid port"))?
                } else {
                    default_port
                };
                (h, p)
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unterminated IPv6 bracket",
                ));
            }
        }
    } else {
        match authority.rfind(':') {
            Some(i) => {
                let h = &authority[..i];
                let p = authority[i + 1..]
                    .parse::<u16>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid port"))?;
                (h, p)
            }
            None => (authority, default_port),
        }
    };

    if host.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "URL has no host",
        ));
    }

    Ok(WsUrl {
        scheme,
        host,
        port,
        path,
    })
}

/// WebSocket client. Connect with [`WebSocketClient::connect`].
pub struct WebSocketClient;

/// The handler until the connection is up, and the session afterwards.
/// Whoever ends the dial first (the handshake, a failure, or the timer)
/// takes the handler; the others find it gone.
struct Dial {
    handler: Option<Box<dyn WebSocketHandler>>,
    session: Option<WebSocketSession>,
}

type Shared = Arc<Mutex<Dial>>;

fn fail(shared: &Shared, err: io::Error) {
    let handler = shared.lock().unwrap().handler.take();
    if let Some(mut h) = handler {
        h.failed(&err);
    }
}

/// Frames arriving on the reactor, delivered to the handler.
struct Events {
    shared: Shared,
}

impl Events {
    fn with(&self, f: impl FnOnce(&mut dyn WebSocketHandler, &WebSocketSession)) {
        let mut g = self.shared.lock().unwrap();
        let Dial { handler, session } = &mut *g;
        if let (Some(h), Some(ws)) = (handler.as_deref_mut(), session.as_ref()) {
            f(h, ws);
        }
    }

    /// The connection is over: the handler hears about it once and is
    /// dropped.
    fn end(&self, f: impl FnOnce(&mut dyn WebSocketHandler)) {
        let (handler, session) = {
            let mut g = self.shared.lock().unwrap();
            (g.handler.take(), g.session.take())
        };
        if let Some(ws) = session {
            ws.mark_closed();
        }
        if let Some(mut h) = handler {
            f(&mut *h);
        }
    }
}

impl WsEventHandler for Events {
    fn opened(&mut self, _session: &mut WsSession<'_>, conn: &ConnHandle) {
        let ws = WebSocketSession::new(conn.clone());
        let mut g = self.shared.lock().unwrap();
        if g.handler.is_none() {
            // The dial timed out before the handshake finished.
            drop(g);
            ws.close();
            return;
        }
        g.session = Some(ws.clone());
        if let Some(h) = g.handler.as_deref_mut() {
            h.connected(&ws);
        }
    }

    fn text_message(&mut self, _session: &mut WsSession<'_>, text: &str) {
        self.with(|h, ws| h.text_frame(ws, text.as_bytes()));
    }

    fn binary_message(&mut self, _session: &mut WsSession<'_>, data: &[u8]) {
        self.with(|h, ws| h.binary_frame(ws, data));
    }

    fn ping(&mut self, session: &mut WsSession<'_>, payload: &[u8]) {
        session.send_pong(payload);
        self.with(|h, ws| h.ping(ws, payload));
    }

    fn pong(&mut self, _session: &mut WsSession<'_>, payload: &[u8]) {
        self.with(|h, ws| h.pong(ws, payload));
    }

    fn closed(&mut self, _session: &mut WsSession<'_>, code: u16, reason: &str) {
        match code {
            // 1006 is hopf's word for "the transport went away" (no Close frame).
            1006 => self.end(|h| h.failed(&io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"))),
            1005 => self.end(|h| h.close(None, "")),
            c => self.end(|h| h.close(Some(c), reason)),
        }
    }

    fn error(&mut self, err: WsFrameError) {
        self.end(|h| h.failed(&io::Error::new(io::ErrorKind::InvalidData, format!("{err:?}"))));
    }
}

/// Drives the opening handshake on the reactor.
struct Opening {
    opening: WebSocketOpening,
    path: String,
    host_header: String,
    shared: Shared,
    upgraded: bool,
}

impl Opening {
    fn fail(&self, msg: impl Into<String>) {
        fail(&self.shared, io::Error::new(io::ErrorKind::ConnectionRefused, msg.into()));
    }
}

impl ClientHandler for Opening {
    fn start(&mut self, request: &mut dyn ClientWriter) {
        self.opening.write_request(request, &self.path, &self.host_header);
    }

    fn switching_protocols(&mut self, request: &mut dyn ClientWriter, headers: &Headers) {
        if let Err(e) = self.opening.validate_response(headers) {
            self.fail(format!("WebSocket handshake rejected: {e}"));
            return;
        }
        let events = Events { shared: Arc::clone(&self.shared) };
        let conn = request.conn_handle();
        if request.upgrade(Box::new(WsUpgradeHandler::client(Box::new(events), MAX_PAYLOAD, conn))) {
            self.upgraded = true;
        } else {
            self.fail("WebSocket upgrade refused by the HTTP layer");
        }
    }

    fn response_headers(&mut self, _request: &mut dyn ClientWriter, headers: &Headers) {
        self.fail(format!("expected 101 Switching Protocols, got {}", headers.status_code()));
    }

    fn response_complete(&mut self, _request: &mut dyn ClientWriter) {}

    fn request_failed(&mut self, _request: &mut dyn ClientWriter, err: &io::Error) {
        if !self.upgraded {
            fail(&self.shared, io::Error::new(err.kind(), err.to_string()));
        }
    }
}

/// Hands the one prepared handshake to the one connection the dial makes.
struct OpeningFactory(Mutex<Option<Opening>>);

impl ClientHandlerFactory for OpeningFactory {
    fn create_handler(&self) -> Box<dyn ClientHandler> {
        match self.0.lock().unwrap().take() {
            Some(o) => Box::new(o),
            None => Box::new(Opening {
                opening: WebSocketOpening::new(None),
                path: "/".into(),
                host_header: String::new(),
                shared: Arc::new(Mutex::new(Dial { handler: None, session: None })),
                upgraded: false,
            }),
        }
    }
}

static TIMER_WORKER: AtomicUsize = AtomicUsize::new(0);

impl WebSocketClient {
    /// Connect to the given WebSocket URL (ws:// or wss://) and perform the
    /// opening handshake. `handler` hears `connected` with the session to
    /// send on, then every frame, then `close` or `failed`, all on the
    /// connection's reactor thread; or `failed` alone when the dial or the
    /// handshake does not succeed within [`HANDSHAKE_TIMEOUT`].
    pub fn connect(url: &str, handler: Box<dyn WebSocketHandler>) {
        Self::connect_with_timeout(url, HANDSHAKE_TIMEOUT, handler)
    }

    /// [`Self::connect`] with a caller-chosen bound on the dial plus handshake.
    pub fn connect_with_timeout(url: &str, handshake_timeout: Duration, mut handler: Box<dyn WebSocketHandler>) {
        let parsed = match parse_ws_url(url) {
            Ok(p) => p,
            Err(e) => {
                handler.failed(&e);
                return;
            }
        };
        let use_tls = parsed.scheme == "wss";
        let default_port = if use_tls { 443 } else { 80 };
        let host_header = if parsed.port == default_port {
            parsed.host.to_string()
        } else {
            format!("{}:{}", parsed.host, parsed.port)
        };
        let shared: Shared = Arc::new(Mutex::new(Dial { handler: Some(handler), session: None }));
        let opening = Opening {
            opening: WebSocketOpening::new(None),
            path: parsed.path.to_string(),
            host_header,
            shared: Arc::clone(&shared),
            upgraded: false,
        };
        let factory: Arc<dyn ClientHandlerFactory> = Arc::new(OpeningFactory(Mutex::new(Some(opening))));
        let settings = DialSettings {
            limits: HttpLimits::default(),
            timeouts: HttpClientTimeouts { dns: DNS_TIMEOUT, connect: CONNECT_TIMEOUT, stage: Duration::ZERO },
            resolver: Some(crate::net::hopf_dns_resolver()),
        };
        let rt = hopf_runtime();
        let dialled = if use_tls {
            connect_https(&rt, parsed.host, parsed.port, factory, hopf_tls_connector(), parsed.host, settings)
        } else {
            connect_http(&rt, parsed.host, parsed.port, factory, false, settings)
        };
        if let Err(e) = dialled {
            fail(&shared, e);
            return;
        }
        // A dial that never produces a connection is not reported by hopf;
        // the timer ends the wait.
        let worker = TIMER_WORKER.fetch_add(1, Ordering::Relaxed) % rt.worker_count().max(1);
        let timed = Arc::clone(&shared);
        let _ = rt.schedule_on_worker(
            worker,
            handshake_timeout,
            Box::new(move || {
                let waiting = {
                    let g = timed.lock().unwrap();
                    g.handler.is_some() && g.session.is_none()
                };
                if waiting {
                    fail(&timed, io::Error::new(io::ErrorKind::TimedOut, "WebSocket connect timed out"));
                }
            }),
        );
    }
}
