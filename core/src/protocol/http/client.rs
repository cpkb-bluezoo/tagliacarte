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

//! HTTP client: connect to a host, then use the connection to send requests with a callback handler.

use std::io;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;

use crate::net::http_client_config;
use crate::protocol::http::connection::{HttpConnection, HttpStream, HttpVersion};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// HTTP client. Create with `HttpClient::connect(host, port, use_tls)` then use the returned connection to build requests and send with a handler.
pub struct HttpClient;

impl HttpClient {
    /// Connect to the given host and port. If `use_tls` is true, performs TLS handshake with ALPN (h2, http/1.1).
    /// Returns an `HttpConnection` that can be used to issue requests. The negotiated protocol (HTTP/1.1 or HTTP/2) is set from ALPN when using TLS; plain TCP uses HTTP/1.1.
    pub async fn connect(
        host: &str,
        port: u16,
        use_tls: bool,
    ) -> io::Result<HttpConnection> {
        let addr = format!("{}:{}", host, port);
        let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timed out"))??;

        if use_tls {
            let host_static: &'static str = Box::leak(host.to_string().into_boxed_str());
            let server_name: ServerName<'static> = host_static
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid host name"))?;
            let connector = TlsConnector::from(http_client_config());
            let tls = connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;
            // ALPN: if negotiated protocol is "h2" we use Http2, else Http1_1
            let version = tls
                .get_ref()
                .1
                .alpn_protocol()
                .map(|p| {
                    if p == b"h2" {
                        HttpVersion::Http2
                    } else {
                        HttpVersion::Http1_1
                    }
                })
                .unwrap_or(HttpVersion::Http1_1);
            let stream = HttpStream::Tls(tls);
            Ok(HttpConnection::new(
                stream,
                host.to_string(),
                port,
                true,
                version,
            ))
        } else {
            let stream = HttpStream::Plain(tcp);
            Ok(HttpConnection::new(
                stream,
                host.to_string(),
                port,
                false,
                HttpVersion::Http1_1,
            ))
        }
    }
}
