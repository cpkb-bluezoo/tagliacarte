/*
 * handshake.rs
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

//! WebSocket opening handshake (RFC 6455 §4): GET with Upgrade, parse 101, verify Sec-WebSocket-Accept.

use bytes::BytesMut;
use std::io;

use crate::mime::base64;
use crate::protocol::http::h1::{H1ResponseHandler, ParseState, ResponseParser};

/// Magic string for Sec-WebSocket-Accept (RFC 6455 §4.2.2).
const WS_ACCEPT_MAGIC: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Captures status and Sec-WebSocket-Accept from the 101 response.
struct HandshakeHandler {
    status: Option<u16>,
    accept: Option<String>,
}

impl H1ResponseHandler for HandshakeHandler {
    fn status(&mut self, code: u16, _reason: Option<&str>) {
        self.status = Some(code);
    }

    fn header(&mut self, name: &str, value: &str) {
        if name.eq_ignore_ascii_case("Sec-WebSocket-Accept") {
            self.accept = Some(value.trim().to_string());
        }
    }

    fn start_body(&mut self) {}
    fn body_chunk(&mut self, _data: &[u8]) {}
    fn end_body(&mut self) {}
    fn trailer(&mut self, _name: &str, _value: &str) {}
    fn complete(&mut self) {}
}

/// Build the HTTP GET request for the WebSocket handshake. Caller writes this to the stream.
pub fn build_handshake_request(host: &str, port: u16, path: &str, key_base64: &[u8]) -> Vec<u8> {
    let host_header = if port == 80 || port == 443 {
        host.to_string()
    } else {
        format!("{}:{}", host, port)
    };
    let mut req = Vec::new();
    req.extend_from_slice(b"GET ");
    req.extend_from_slice(path.as_bytes());
    req.extend_from_slice(b" HTTP/1.1\r\nHost: ");
    req.extend_from_slice(host_header.as_bytes());
    req.extend_from_slice(b"\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ");
    req.extend_from_slice(key_base64);
    req.extend_from_slice(b"\r\nSec-WebSocket-Version: 13\r\n\r\n");
    req
}

/// Compute expected Sec-WebSocket-Accept from the base64-encoded key we sent in Sec-WebSocket-Key.
/// Per RFC 6455 §4.2.2: SHA-1(key_base64 + MAGIC_GUID), then base64-encode the result.
pub fn compute_expected_accept(key_base64: &[u8]) -> Vec<u8> {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(key_base64);
    hasher.update(WS_ACCEPT_MAGIC);
    let digest = hasher.finalize();
    base64::encode(digest.as_ref())
}

/// Parse the 101 response from the buffer using the H1 parser. Stops at HeadersComplete.
/// Returns (status, accept_header) or error. Does not read body.
pub fn parse_101_response(
    parser: &mut ResponseParser,
    buf: &mut BytesMut,
) -> Result<(u16, Option<String>), io::Error> {
    let mut handler = HandshakeHandler {
        status: None,
        accept: None,
    };
    parser.receive(buf, &mut handler)?;
    if parser.state() != ParseState::HeadersComplete {
        return Ok((handler.status.unwrap_or(0), handler.accept));
    }
    Ok((
        handler.status.unwrap_or(0),
        handler.accept,
    ))
}

/// Verify the server's Sec-WebSocket-Accept header matches our key (base64-encoded).
pub fn verify_accept(accept_header: Option<&str>, key_base64: &[u8]) -> Result<(), io::Error> {
    let expected = compute_expected_accept(key_base64);
    let expected_str = std::str::from_utf8(&expected).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidData, "invalid expected accept base64")
    })?;
    match accept_header {
        Some(h) if h.trim() == expected_str => Ok(()),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Sec-WebSocket-Accept mismatch",
        )),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing Sec-WebSocket-Accept",
        )),
    }
}
