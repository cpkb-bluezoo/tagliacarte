/*
 * server.rs
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

//! The loopback HTTPS listener on hopf: one `ServerHandler` per request
//! parses the head and hands a [`ResponseSink`] to the application's
//! request handler, which answers from the reactor through callbacks and
//! must not block it.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use hopf_core::{ProtocolHandler, SharedTlsAcceptor, TcpListenerConfig};
use hopf_http::{AlpnHttpEndpoint, Headers, HttpLimits, ServerHandler, ServerHandlerFactory, ServerWriter};

use super::sink::{ParsedRequest, ResponseSink};
use crate::net::hopf_runtime;

/// Called once per request; must not block the reactor.
pub type RequestHandler = Arc<dyn Fn(ParsedRequest, ResponseSink) + Send + Sync>;

/// Largest outbound backlog hopf keeps for one WebView connection before
/// closing it; [`ResponseSink`] waits for drains well under this.
const MAX_NET_OUT: usize = 8 * 1024 * 1024;

/// Start listening on an ephemeral loopback port.
pub fn start_mail_view_server(acceptor: SharedTlsAcceptor, handler: RequestHandler) -> io::Result<SocketAddr> {
    let factory: Arc<dyn ServerHandlerFactory> = Arc::new(Factory { handler });
    let limits = HttpLimits::default();
    let (addr, _) = hopf_runtime().add_tcp_listener(
        TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
            Box::new(AlpnHttpEndpoint::new(Arc::clone(&factory), limits)) as Box<dyn ProtocolHandler>
        })
        .max_net_out(MAX_NET_OUT)
        .with_tls(acceptor),
    )?;
    Ok(addr)
}

struct Factory {
    handler: RequestHandler,
}

impl ServerHandlerFactory for Factory {
    fn create_handler(&self) -> Box<dyn ServerHandler> {
        Box::new(Request { handler: Arc::clone(&self.handler), parsed: None })
    }
}

struct Request {
    handler: RequestHandler,
    parsed: Option<ParsedRequest>,
}

impl ServerHandler for Request {
    fn headers(&mut self, _response: &mut dyn ServerWriter, headers: &Headers) {
        self.parsed = Some(ParsedRequest::from_headers(headers));
    }

    fn request_complete(&mut self, response: &mut dyn ServerWriter) {
        if let Some(req) = self.parsed.take() {
            let sink = ResponseSink::new(response.response_handle());
            (self.handler)(req, sink);
        }
    }
}
