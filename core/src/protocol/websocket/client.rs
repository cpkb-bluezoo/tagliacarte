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

//! WebSocket client: connect to ws:// or wss:// URL, perform handshake, return WebSocketConnection.

use bytes::BytesMut;
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;
use url::Url;

use crate::mime::base64;
use crate::net::http_client_config;
use crate::protocol::http::HttpStream;
use crate::protocol::http::h1::{ParseState, ResponseParser};
use crate::protocol::websocket::connection::WebSocketConnection;
use crate::protocol::websocket::handshake::{
    build_handshake_request, parse_101_response, verify_accept,
};

/// WebSocket client. Connect with `WebSocketClient::connect(url)`.
pub struct WebSocketClient;

impl WebSocketClient {
    /// Connect to the given WebSocket URL (ws:// or wss://), perform the opening handshake,
    /// and return a `WebSocketConnection`. Call `connected()` on your handler, then use
    /// `conn.run(handler)` to drive the read loop and `conn.send_text()` etc. to send.
    pub async fn connect(url: &str) -> io::Result<WebSocketConnection> {
        let url = Url::parse(url).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
        })?;
        let host = url.host_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "URL has no host")
        })?;
        let port = url.port_or_known_default().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "URL has no port")
        })?;
        let path = if url.path().is_empty() {
            "/"
        } else {
            url.path()
        };
        let use_tls = matches!(url.scheme(), "wss" | "https");
        if !matches!(url.scheme(), "ws" | "wss") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "URL scheme must be ws or wss",
            ));
        }

        let addr = format!("{}:{}", host, port);
        let tcp = TcpStream::connect(&addr).await?;

        let stream = if use_tls {
            let host_static: &'static str = Box::leak(host.to_string().into_boxed_str());
            let server_name: ServerName<'static> = host_static
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid host name"))?;
            let connector = TlsConnector::from(http_client_config());
            let tls = connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;
            HttpStream::Tls(tls)
        } else {
            HttpStream::Plain(tcp)
        };

        // Handshake: 16 random bytes -> base64 key
        let mut key_raw = [0u8; 16];
        getrandom::getrandom(&mut key_raw).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let key_base64 = base64::encode(&key_raw);

        let request = build_handshake_request(host, port, path, &key_base64);
        let mut stream = stream;
        stream.write_all(&request).await?;
        stream.flush().await?;

        let mut read_buf = BytesMut::with_capacity(4096);
        let mut parser = ResponseParser::new();
        loop {
            let mut tmp = [0u8; 4096];
            let n = stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed during handshake",
                ));
            }
            read_buf.extend_from_slice(&tmp[..n]);
            let (status, accept) = parse_101_response(&mut parser, &mut read_buf)?;
            if parser.state() == ParseState::HeadersComplete {
                if status != 101 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("expected 101 Switching Protocols, got {}", status),
                    ));
                }
                verify_accept(accept.as_deref(), &key_raw)?;
                break;
            }
        }

        Ok(WebSocketConnection::new(stream))
    }
}
