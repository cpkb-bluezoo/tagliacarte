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

use bytes::BytesMut;
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream as TokioTlsStream;

use crate::protocol::http::h1::{H1ResponseHandler, ParseState, ResponseParser};
use crate::protocol::http::h2::{H2Parser, H2Writer};
use crate::protocol::http::hpack::Decoder as HpackDecoder;
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

/// Active request state for HTTP/2: stream id and optional handler (until response completes).
#[allow(dead_code)]
struct H2StreamState {
    stream_id: u32,
    handler: Option<Box<dyn ResponseHandler + Send>>,
}

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

/// HTTP connection: holds stream, version, and drives read loop. Call send() to issue a request.
#[allow(dead_code)]
pub struct HttpConnection {
    stream: HttpStream,
    host: String,
    port: u16,
    secure: bool,
    version: HttpVersion,

    // Read buffer (shared by H1 and H2)
    read_buf: BytesMut,

    // HTTP/1.1 state
    h1_parser: ResponseParser,
    h1_status: Option<(u16, Option<String>)>,
    h1_headers: Vec<(String, String)>,
    h1_handler: Option<Box<dyn ResponseHandler + Send>>,

    // HTTP/2 state (when version == Http2)
    h2_parser: H2Parser,
    h2_writer: H2Writer,
    hpack_decoder: HpackDecoder,
    next_stream_id: u32,
    active_streams: HashMap<u32, H2StreamState>,
    /// Accumulated header block fragment for current stream (until end_headers).
    h2_header_block: Option<BytesMut>,
    h2_continuation_stream_id: u32,
}

impl HttpConnection {
    /// Create from an already-connected stream and negotiated version. Used by HttpClient::connect().
    pub fn new(
        stream: HttpStream,
        host: String,
        port: u16,
        secure: bool,
        version: HttpVersion,
    ) -> Self {
        let h2_parser = H2Parser::new();
        let mut h2_writer = H2Writer::new();
        if version == HttpVersion::Http2 {
            h2_writer.write_settings(&[]).ok();
        }
        Self {
            stream,
            host: host.clone(),
            port,
            secure,
            version,
            read_buf: BytesMut::with_capacity(8192),
            h1_parser: ResponseParser::new(),
            h1_status: None,
            h1_headers: Vec::new(),
            h1_handler: None,
            h2_parser,
            h2_writer,
            hpack_decoder: HpackDecoder::new(4096),
            next_stream_id: 1,
            active_streams: HashMap::new(),
            h2_header_block: None,
            h2_continuation_stream_id: 0,
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

    /// Build a request (method, path). Use send() to execute it with a handler.
    pub fn request(&mut self, method: Method, path: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(method, path.into())
    }

    /// Send the request and run the read loop until the response is complete. Handler is invoked as data arrives.
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
        let use_chunked = request.body.is_some()
            && !request.headers.contains_key("Content-Length")
            && !request.headers.contains_key("Transfer-Encoding");
        let mut req = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\n",
            request.method.as_str(),
            request.path,
            host_header
        );
        for (k, v) in &request.headers {
            req.push_str(k);
            req.push_str(": ");
            req.push_str(v);
            req.push_str("\r\n");
        }
        if request.body.is_none() {
            req.push_str("Connection: keep-alive\r\n");
        } else if use_chunked {
            req.push_str("Transfer-Encoding: chunked\r\n");
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

    async fn send_http2(
        &mut self,
        _request: RequestBuilder,
        _handler: &mut (dyn ResponseHandler + Send),
    ) -> io::Result<()> {
        // TODO: implement HTTP/2 send (encode request with HPACK, write HEADERS+DATA, drive H2 parser, HPACK decode, forward to handler)
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "HTTP/2 send not yet implemented",
        ))
    }
}
