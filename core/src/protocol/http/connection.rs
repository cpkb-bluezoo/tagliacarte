/*
 * connection.rs
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

//! HTTP connection: one TCP or TLS stream, drives H1 or H2 parser, invokes ResponseHandler.
//! Supports ALPN (h2 / http/1.1) and h2c upgrade.

use bytes::{Bytes, BytesMut};
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream as TokioTlsStream;

use crate::protocol::http::h1::{H1ResponseHandler, ParseState, ResponseParser};
use crate::protocol::http::h2::{
    self, error_to_string, H2FrameHandler, H2Parser, H2Writer,
    SETTINGS_HEADER_TABLE_SIZE, SETTINGS_INITIAL_WINDOW_SIZE,
    SETTINGS_MAX_CONCURRENT_STREAMS, SETTINGS_MAX_FRAME_SIZE, SETTINGS_MAX_HEADER_LIST_SIZE,
};
use crate::protocol::http::hpack::{self, Decoder as HpackDecoder, HeaderHandler};
use crate::protocol::http::request::{Method, RequestBuilder};
use crate::protocol::http::response::Response;
use crate::protocol::http::ResponseHandler;

/// Negotiated protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion {
    Http1_1,
    Http2,
}

/// Unified stream: plain TCP or TLS. Implements AsyncRead + AsyncWrite.
pub enum HttpStream {
    Plain(TcpStream),
    Tls(TokioTlsStream<TcpStream>),
}

impl AsyncRead for HttpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            HttpStream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            HttpStream::Tls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for HttpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            HttpStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            HttpStream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            HttpStream::Plain(s) => Pin::new(s).poll_flush(cx),
            HttpStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            HttpStream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            HttpStream::Tls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

// ── H1 ──────────────────────────────────────────────────────────────────

/// Bridges H1 parser callbacks to the connection state and user's ResponseHandler.
struct H1Driver<'a> {
    h1_status: &'a mut Option<(u16, Option<String>)>,
    h1_headers: &'a mut Vec<(String, String)>,
    handler: &'a mut (dyn ResponseHandler + Send),
}

impl H1ResponseHandler for H1Driver<'_> {
    fn status(&mut self, code: u16, reason: Option<&str>) {
        *self.h1_status = Some((code, reason.map(|s| s.to_string())));
    }

    fn header(&mut self, name: &str, value: &str) {
        self.h1_headers.push((name.to_string(), value.to_string()));
    }

    fn start_body(&mut self) {
        self.handler.start_body();
    }

    fn body_chunk(&mut self, data: &[u8]) {
        self.handler.body_chunk(data);
    }

    fn end_body(&mut self) {
        self.handler.end_body();
    }

    fn trailer(&mut self, name: &str, value: &str) {
        self.handler.header(name, value);
    }

    fn complete(&mut self) {
        self.handler.complete();
    }
}

// ── H2 frame handlers ──────────────────────────────────────────────────

/// Collects SETTINGS and PING frames received during the initial h2 handshake.
/// No external references needed — just records what happened.
struct H2Handshake {
    settings_received: bool,
    server_settings: Vec<(u16, u32)>,
    pings_to_ack: Vec<u64>,
    goaway_error: Option<String>,
}

impl H2Handshake {
    fn new() -> Self {
        Self {
            settings_received: false,
            server_settings: Vec::new(),
            pings_to_ack: Vec::new(),
            goaway_error: None,
        }
    }
}

impl H2FrameHandler for H2Handshake {
    fn settings_frame_received(&mut self, ack: bool, settings: Vec<(u16, u32)>) {
        if !ack {
            self.settings_received = true;
            self.server_settings = settings;
        }
    }
    fn ping_frame_received(&mut self, ack: bool, opaque_data: u64) {
        if !ack {
            self.pings_to_ack.push(opaque_data);
        }
    }
    fn goaway_frame_received(&mut self, _last_stream_id: u32, error_code: u32, _debug_data: Bytes) {
        self.goaway_error = Some(format!("GOAWAY during handshake: {}", error_to_string(error_code)));
    }
    fn window_update_frame_received(&mut self, _stream_id: u32, _increment: u32) {}
    fn data_frame_received(&mut self, _stream_id: u32, _end_stream: bool, _data: Bytes) {}
    fn headers_frame_received(&mut self, _: u32, _: bool, _: bool, _: u32, _: bool, _: u8, _: Bytes) {}
    fn priority_frame_received(&mut self, _: u32, _: u32, _: bool, _: u8) {}
    fn rst_stream_frame_received(&mut self, _: u32, _: u32) {}
    fn push_promise_frame_received(&mut self, _: u32, _: u32, _: bool, _: Bytes) {}
    fn continuation_frame_received(&mut self, _: u32, _: bool, _: Bytes) {}
    fn frame_error(&mut self, _error_code: u32, _stream_id: u32, message: String) {
        self.goaway_error = Some(message);
    }
}

/// Processes h2 frames during the response read loop for a single request.
///
/// Holds mutable references to individual connection fields (split borrows from
/// HttpConnection) so the H2Parser can invoke this as its H2FrameHandler while
/// the parser itself is also borrowed mutably.
struct H2ResponseDriver<'a> {
    target_stream_id: u32,
    hpack_decoder: &'a mut HpackDecoder,
    header_block: &'a mut Option<BytesMut>,
    continuation_stream_id: &'a mut u32,
    handler: &'a mut (dyn ResponseHandler + Send),
    response_started: &'a mut bool,
    body_started: &'a mut bool,
    stream_complete: bool,
    settings_to_ack: bool,
    server_settings: Vec<(u16, u32)>,
    pings_to_ack: Vec<u64>,
    goaway_error: Option<String>,
    rst_error: Option<u32>,
}

/// Collects decoded headers for HPACK processing.
struct VecHeaderCollector(Vec<(String, String)>);

impl HeaderHandler for VecHeaderCollector {
    fn header(&mut self, name: &str, value: &str) {
        self.0.push((name.to_string(), value.to_string()));
    }
}

impl H2ResponseDriver<'_> {
    /// HPACK-decode the accumulated header block, call the ResponseHandler, and reset.
    fn process_headers(&mut self, end_stream: bool) {
        let hb = match self.header_block.take() {
            Some(hb) => hb,
            None => return,
        };
        *self.continuation_stream_id = 0;

        let mut collector = VecHeaderCollector(Vec::new());
        let mut cursor = &hb[..];
        if self.hpack_decoder.decode(&mut cursor, &mut collector).is_err() {
            self.goaway_error = Some("HPACK decompression error".to_string());
            return;
        }

        let status_code = collector
            .0
            .iter()
            .find(|(n, _)| n == ":status")
            .and_then(|(_, v)| v.parse::<u16>().ok())
            .unwrap_or(0);

        if !*self.response_started {
            *self.response_started = true;
            let response = Response::new(status_code);
            if (200..300).contains(&status_code) {
                self.handler.ok(response);
            } else {
                self.handler.error(response);
            }
            for (name, value) in &collector.0 {
                if !name.starts_with(':') {
                    self.handler.header(name, value);
                }
            }
        }

        if end_stream {
            if *self.body_started {
                self.handler.end_body();
            }
            self.handler.complete();
            self.stream_complete = true;
        } else if !*self.body_started {
            self.handler.start_body();
            *self.body_started = true;
        }
    }
}

impl H2FrameHandler for H2ResponseDriver<'_> {
    fn headers_frame_received(
        &mut self,
        stream_id: u32,
        end_stream: bool,
        end_headers: bool,
        _stream_dependency: u32,
        _exclusive: bool,
        _weight: u8,
        header_block_fragment: Bytes,
    ) {
        if stream_id != self.target_stream_id {
            return;
        }
        let hb = self
            .header_block
            .get_or_insert_with(|| BytesMut::with_capacity(header_block_fragment.len()));
        hb.extend_from_slice(&header_block_fragment);

        if end_headers {
            self.process_headers(end_stream);
        } else {
            *self.continuation_stream_id = stream_id;
        }
    }

    fn continuation_frame_received(
        &mut self,
        stream_id: u32,
        end_headers: bool,
        header_block_fragment: Bytes,
    ) {
        if stream_id != self.target_stream_id {
            return;
        }
        if let Some(hb) = self.header_block.as_mut() {
            hb.extend_from_slice(&header_block_fragment);
        }
        if end_headers {
            self.process_headers(false);
        }
    }

    fn data_frame_received(&mut self, stream_id: u32, end_stream: bool, data: Bytes) {
        if stream_id != self.target_stream_id {
            return;
        }
        if !*self.body_started {
            self.handler.start_body();
            *self.body_started = true;
        }
        self.handler.body_chunk(&data);
        if end_stream {
            self.handler.end_body();
            self.handler.complete();
            self.stream_complete = true;
        }
    }

    fn settings_frame_received(&mut self, ack: bool, settings: Vec<(u16, u32)>) {
        if !ack {
            self.settings_to_ack = true;
            self.server_settings = settings;
        }
    }

    fn ping_frame_received(&mut self, ack: bool, opaque_data: u64) {
        if !ack {
            self.pings_to_ack.push(opaque_data);
        }
    }

    fn window_update_frame_received(&mut self, _stream_id: u32, _increment: u32) {}

    fn goaway_frame_received(&mut self, _last_stream_id: u32, error_code: u32, _debug_data: Bytes) {
        self.goaway_error = Some(format!("GOAWAY: {}", error_to_string(error_code)));
    }

    fn rst_stream_frame_received(&mut self, stream_id: u32, error_code: u32) {
        if stream_id == self.target_stream_id {
            self.rst_error = Some(error_code);
        }
    }

    fn push_promise_frame_received(&mut self, _: u32, _: u32, _: bool, _: Bytes) {}
    fn priority_frame_received(&mut self, _: u32, _: u32, _: bool, _: u8) {}

    fn frame_error(&mut self, _error_code: u32, _stream_id: u32, message: String) {
        self.goaway_error = Some(message);
    }
}

// ── HttpConnection ──────────────────────────────────────────────────────

/// HTTP connection: holds stream, version, and drives read loop. Call send() to issue a request.
pub struct HttpConnection {
    stream: HttpStream,
    host: String,
    port: u16,
    secure: bool,
    version: HttpVersion,

    read_buf: BytesMut,

    // HTTP/1.1 state
    h1_parser: ResponseParser,
    h1_status: Option<(u16, Option<String>)>,
    h1_headers: Vec<(String, String)>,
    #[allow(dead_code)]
    h1_handler: Option<Box<dyn ResponseHandler + Send>>,

    // HTTP/2 state
    h2_parser: H2Parser,
    h2_writer: H2Writer,
    hpack_decoder: HpackDecoder,
    next_stream_id: u32,
    #[allow(dead_code)]
    active_streams: HashMap<u32, ()>,
    h2_header_block: Option<BytesMut>,
    h2_continuation_stream_id: u32,
    h2_preface_sent: bool,
    h2_max_frame_size: usize,
}

impl HttpConnection {
    pub fn new(
        stream: HttpStream,
        host: String,
        port: u16,
        secure: bool,
        version: HttpVersion,
    ) -> Self {
        Self {
            stream,
            host,
            port,
            secure,
            version,
            read_buf: BytesMut::with_capacity(8192),
            h1_parser: ResponseParser::new(),
            h1_status: None,
            h1_headers: Vec::new(),
            h1_handler: None,
            h2_parser: H2Parser::new(),
            h2_writer: H2Writer::new(),
            hpack_decoder: HpackDecoder::new(4096),
            next_stream_id: 1,
            active_streams: HashMap::new(),
            h2_header_block: None,
            h2_continuation_stream_id: 0,
            h2_preface_sent: false,
            h2_max_frame_size: h2::DEFAULT_MAX_FRAME_SIZE,
        }
    }

    pub fn version(&self) -> HttpVersion {
        self.version
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn request(&mut self, method: Method, path: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(method, path.into())
    }

    /// Send the request and run the read loop until the response is complete.
    pub async fn send(
        &mut self,
        request: RequestBuilder,
        mut handler: impl ResponseHandler + Send + 'static,
    ) -> io::Result<()> {
        match self.version {
            HttpVersion::Http1_1 => self.send_http1(request, &mut handler).await,
            HttpVersion::Http2 => self.send_http2(request, &mut handler).await,
        }
    }

    // ── HTTP/1.1 ────────────────────────────────────────────────────────

    async fn send_http1(
        &mut self,
        request: RequestBuilder,
        handler: &mut (dyn ResponseHandler + Send),
    ) -> io::Result<()> {
        self.h1_status = None;
        self.h1_headers.clear();
        self.h1_parser.reset();

        self.write_http1_request(&request).await?;

        loop {
            let mut tmp = [0u8; 8192];
            let n = self.stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "HTTP connection closed",
                ));
            }
            self.read_buf.extend_from_slice(&tmp[..n]);

            let parser = &mut self.h1_parser;
            let read_buf = &mut self.read_buf;
            let h1_status = &mut self.h1_status;
            let h1_headers = &mut self.h1_headers;
            let mut driver = H1Driver {
                h1_status,
                h1_headers,
                handler,
            };
            parser.receive(read_buf, &mut driver)?;

            if self.h1_parser.state() == ParseState::HeadersComplete {
                let (code, reason) = self.h1_status.take().unwrap_or((0, None));
                let content_length = self
                    .h1_headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.trim().parse::<u64>().ok());
                let chunked = self
                    .h1_headers
                    .iter()
                    .any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.contains("chunked"));

                let response = match reason {
                    Some(r) => Response::with_reason(code, r),
                    None => Response::new(code),
                };
                if (200..300).contains(&code) {
                    handler.ok(response);
                } else {
                    handler.error(response);
                }
                for (name, value) in &self.h1_headers {
                    handler.header(name, value);
                }
                let has_body = chunked
                    || content_length.map(|cl| cl > 0).unwrap_or(false)
                    || (content_length.is_none() && !chunked && code != 204 && code != 304);
                if has_body {
                    handler.start_body();
                }
                self.h1_parser.set_body_mode(content_length, chunked);
            }

            if self.h1_parser.state() == ParseState::Idle {
                break;
            }
        }
        Ok(())
    }

    async fn write_http1_request(&mut self, request: &RequestBuilder) -> io::Result<()> {
        let host_header = if (self.secure && self.port != 443) || (!self.secure && self.port != 80) {
            format!("{}:{}", self.host, self.port)
        } else {
            self.host.clone()
        };
        let has_body = request.body.is_some();
        let has_content_length = request
            .headers
            .keys()
            .any(|k| k.eq_ignore_ascii_case("Content-Length"));
        let use_chunked = has_body && !has_content_length;

        let mut req = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\n",
            request.method.as_str(),
            request.path,
            host_header
        );
        for (k, v) in &request.headers {
            let lk = k.to_ascii_lowercase();
            if lk == "host" || lk == "transfer-encoding" {
                continue;
            }
            req.push_str(k);
            req.push_str(": ");
            req.push_str(v);
            req.push_str("\r\n");
        }
        if use_chunked {
            req.push_str("Transfer-Encoding: chunked\r\n");
        }
        if !has_body {
            req.push_str("Connection: keep-alive\r\n");
        }
        req.push_str("\r\n");
        self.stream.write_all(req.as_bytes()).await?;
        if let Some(body) = &request.body {
            if use_chunked {
                let hex_len = format!("{:x}\r\n", body.len());
                self.stream.write_all(hex_len.as_bytes()).await?;
                self.stream.write_all(body).await?;
                self.stream.write_all(b"\r\n").await?;
                self.stream.write_all(b"0\r\n\r\n").await?;
            } else {
                self.stream.write_all(body).await?;
            }
        }
        self.stream.flush().await?;
        Ok(())
    }

    // ── HTTP/2 ──────────────────────────────────────────────────────────

    async fn send_http2(
        &mut self,
        request: RequestBuilder,
        handler: &mut (dyn ResponseHandler + Send),
    ) -> io::Result<()> {
        // ── 1. Connection preface + SETTINGS exchange (first request only) ──
        if !self.h2_preface_sent {
            self.h2_preface_sent = true;

            self.stream.write_all(h2::CONNECTION_PREFACE).await?;
            self.h2_writer.write_settings(&[])?;
            let buf = self.h2_writer.take_buffer();
            self.stream.write_all(&buf).await?;
            self.stream.flush().await?;

            loop {
                let mut tmp = [0u8; 8192];
                let n = self.stream.read(&mut tmp).await?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "connection closed during h2 handshake",
                    ));
                }
                self.read_buf.extend_from_slice(&tmp[..n]);

                let mut handshake = H2Handshake::new();
                self.h2_parser
                    .receive(&mut self.read_buf, &mut handshake)?;

                if let Some(err) = handshake.goaway_error {
                    return Err(io::Error::new(io::ErrorKind::ConnectionRefused, err));
                }

                for ping in &handshake.pings_to_ack {
                    self.h2_writer.write_ping(*ping, true)?;
                }

                if handshake.settings_received {
                    self.apply_server_settings(&handshake.server_settings);
                    self.h2_writer.write_settings_ack()?;
                    let ack_buf = self.h2_writer.take_buffer();
                    self.stream.write_all(&ack_buf).await?;
                    self.stream.flush().await?;
                    break;
                }

                if !self.h2_writer.is_empty() {
                    let buf = self.h2_writer.take_buffer();
                    self.stream.write_all(&buf).await?;
                    self.stream.flush().await?;
                }
            }
        }

        // ── 2. Encode and send request ──────────────────────────────────
        let stream_id = self.next_stream_id;
        self.next_stream_id += 2;

        let has_body = request.body.is_some();
        let header_block = self.encode_h2_request_headers(&request)?;
        self.h2_writer
            .write_headers(stream_id, &header_block, !has_body, true)?;
        let headers_buf = self.h2_writer.take_buffer();
        self.stream.write_all(&headers_buf).await?;

        if let Some(body) = &request.body {
            if !body.is_empty() {
                self.write_h2_data(stream_id, body, true)?;
                let data_buf = self.h2_writer.take_buffer();
                self.stream.write_all(&data_buf).await?;
            }
        }
        self.stream.flush().await?;

        // ── 3. Read loop until response is complete ─────────────────────
        let mut response_started = false;
        let mut body_started = false;

        loop {
            let mut tmp = [0u8; 8192];
            let n = self.stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "h2 connection closed before response complete",
                ));
            }
            self.read_buf.extend_from_slice(&tmp[..n]);

            let (stream_complete, settings_to_ack, server_settings, pings, goaway, rst) = {
                let h2_parser = &mut self.h2_parser;
                let read_buf = &mut self.read_buf;
                let hpack_decoder = &mut self.hpack_decoder;
                let h2_header_block = &mut self.h2_header_block;
                let cont_id = &mut self.h2_continuation_stream_id;

                let mut driver = H2ResponseDriver {
                    target_stream_id: stream_id,
                    hpack_decoder,
                    header_block: h2_header_block,
                    continuation_stream_id: cont_id,
                    handler,
                    response_started: &mut response_started,
                    body_started: &mut body_started,
                    stream_complete: false,
                    settings_to_ack: false,
                    server_settings: Vec::new(),
                    pings_to_ack: Vec::new(),
                    goaway_error: None,
                    rst_error: None,
                };

                h2_parser.receive(read_buf, &mut driver)?;

                (
                    driver.stream_complete,
                    driver.settings_to_ack,
                    driver.server_settings,
                    driver.pings_to_ack,
                    driver.goaway_error,
                    driver.rst_error,
                )
            };

            if settings_to_ack {
                self.apply_server_settings(&server_settings);
                self.h2_writer.write_settings_ack()?;
            }
            for ping in &pings {
                self.h2_writer.write_ping(*ping, true)?;
            }
            if !self.h2_writer.is_empty() {
                let buf = self.h2_writer.take_buffer();
                self.stream.write_all(&buf).await?;
                self.stream.flush().await?;
            }

            if stream_complete {
                return Ok(());
            }
            if let Some(err) = goaway {
                return Err(io::Error::new(io::ErrorKind::ConnectionReset, err));
            }
            if let Some(code) = rst {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionReset,
                    format!("RST_STREAM: {}", error_to_string(code)),
                ));
            }
        }
    }

    /// Apply SETTINGS parameters received from the server.
    fn apply_server_settings(&mut self, settings: &[(u16, u32)]) {
        for &(id, value) in settings {
            match id {
                SETTINGS_HEADER_TABLE_SIZE => {
                    self.hpack_decoder.set_header_table_size(value as usize);
                }
                SETTINGS_MAX_FRAME_SIZE => {
                    let size = value as usize;
                    if (h2::MIN_MAX_FRAME_SIZE..=h2::MAX_MAX_FRAME_SIZE).contains(&size) {
                        self.h2_max_frame_size = size;
                        self.h2_parser.set_max_frame_size(size);
                    }
                }
                SETTINGS_MAX_CONCURRENT_STREAMS | SETTINGS_INITIAL_WINDOW_SIZE | SETTINGS_MAX_HEADER_LIST_SIZE => {}
                _ => {}
            }
        }
    }

    /// Encode request headers into an HPACK header block for HTTP/2.
    fn encode_h2_request_headers(&self, request: &RequestBuilder) -> io::Result<Vec<u8>> {
        let authority = if (self.secure && self.port != 443) || (!self.secure && self.port != 80) {
            format!("{}:{}", self.host, self.port)
        } else {
            self.host.clone()
        };
        let scheme = if self.secure { "https" } else { "http" };

        let mut headers: Vec<(&str, &str)> = Vec::new();
        headers.push((":method", request.method.as_str()));
        headers.push((":scheme", scheme));
        headers.push((":authority", &authority));
        headers.push((":path", &request.path));

        // Copy user headers, stripping HTTP/1.1-specific ones.
        let user_headers: Vec<(String, String)> = request
            .headers
            .iter()
            .filter(|(k, _)| {
                let lk = k.to_ascii_lowercase();
                lk != "connection"
                    && lk != "transfer-encoding"
                    && lk != "upgrade"
                    && lk != "host"
            })
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();
        for (k, v) in &user_headers {
            headers.push((k, v));
        }

        let mut out = BytesMut::with_capacity(256);
        hpack::encode_request_headers(&headers, &mut out)?;
        Ok(out.to_vec())
    }

    /// Write DATA frame(s), splitting into chunks that respect the server's max frame size.
    fn write_h2_data(&mut self, stream_id: u32, data: &[u8], end_stream: bool) -> io::Result<()> {
        let max = self.h2_max_frame_size;
        if data.len() <= max {
            self.h2_writer.write_data(stream_id, data, end_stream)?;
        } else {
            let mut offset = 0;
            while offset < data.len() {
                let end = (offset + max).min(data.len());
                let is_last = end == data.len();
                self.h2_writer
                    .write_data(stream_id, &data[offset..end], is_last && end_stream)?;
                offset = end;
            }
        }
        Ok(())
    }
}
