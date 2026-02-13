/*
 * connection.rs
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

//! WebSocket connection: owns stream after handshake, drives frame parser, exposes send/run.

use bytes::BytesMut;
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::protocol::http::HttpStream;
use crate::protocol::websocket::frame::{encode_frame, FrameHandler, FrameParser, OP_BINARY, OP_CLOSE, OP_PING, OP_PONG, OP_TEXT};
use crate::protocol::websocket::WebSocketHandler;

/// WebSocket connection after successful handshake. Use run() to drive the read loop with a handler;
/// use send_text/send_binary/send_ping/send_close to send frames.
pub struct WebSocketConnection {
    stream: HttpStream,
    read_buf: BytesMut,
    frame_parser: FrameParser,
}

impl WebSocketConnection {
    pub(crate) fn new(stream: HttpStream) -> Self {
        Self {
            stream,
            read_buf: BytesMut::with_capacity(8192),
            frame_parser: FrameParser::new(),
        }
    }

    /// Run the read loop, calling the handler for each frame. Returns when the connection closes,
    /// an error occurs (handler.failed is called before return), or handler.should_stop() is true.
    pub async fn run(&mut self, handler: &mut dyn WebSocketHandler) -> io::Result<()> {
        loop {
            {
                let mut adapter = FrameToHandlerAdapter { handler };
                let mut tmp = [0u8; 8192];
                let n = match self.stream.read(&mut tmp).await {
                    Ok(0) => {
                        return Ok(());
                    }
                    Ok(n) => n,
                    Err(e) => {
                        handler.failed(&e);
                        return Err(e);
                    }
                };
                self.read_buf.extend_from_slice(&tmp[..n]);
                if let Err(e) = self.frame_parser.receive(&mut self.read_buf, &mut adapter) {
                    handler.failed(&e);
                    return Err(e);
                }
            }
            if handler.should_stop() {
                return Ok(());
            }
        }
    }

    /// Send a text frame.
    pub async fn send_text(&mut self, data: &[u8]) -> io::Result<()> {
        self.send_frame(OP_TEXT, data).await
    }

    /// Send a binary frame.
    pub async fn send_binary(&mut self, data: &[u8]) -> io::Result<()> {
        self.send_frame(OP_BINARY, data).await
    }

    /// Send a ping frame.
    pub async fn send_ping(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() > 125 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ping payload max 125 bytes",
            ));
        }
        self.send_frame(OP_PING, data).await
    }

    /// Send a pong frame (e.g. in response to ping).
    pub async fn send_pong(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() > 125 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "pong payload max 125 bytes",
            ));
        }
        self.send_frame(OP_PONG, data).await
    }

    /// Send a close frame. Reason is UTF-8; code is optional (e.g. 1000 = normal).
    pub async fn send_close(&mut self, code: Option<u16>, reason: &str) -> io::Result<()> {
        let mut payload = Vec::new();
        if let Some(c) = code {
            payload.extend_from_slice(&c.to_be_bytes());
        }
        payload.extend_from_slice(reason.as_bytes());
        if payload.len() > 125 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "close payload max 125 bytes",
            ));
        }
        self.send_frame(OP_CLOSE, &payload).await
    }

    async fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        let mut mask_key = [0u8; 4];
        getrandom::getrandom(&mut mask_key).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        let mut out = BytesMut::with_capacity(14 + payload.len());
        encode_frame(opcode, payload, &mask_key, &mut out)?;
        self.stream.write_all(&out).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

/// Adapts FrameHandler callbacks to WebSocketHandler.
struct FrameToHandlerAdapter<'a> {
    handler: &'a mut dyn WebSocketHandler,
}

impl FrameHandler for FrameToHandlerAdapter<'_> {
    fn frame(&mut self, opcode: u8, _fin: bool, data: &[u8]) {
        match opcode {
            OP_TEXT => self.handler.text_frame(data),
            OP_BINARY => self.handler.binary_frame(data),
            OP_CLOSE => {
                let (code, reason) = if data.len() >= 2 {
                    let code = u16::from_be_bytes([data[0], data[1]]);
                    let reason = std::str::from_utf8(&data[2..]).unwrap_or("").to_string();
                    (Some(code), reason)
                } else {
                    (None, String::new())
                };
                self.handler.close(code, &reason);
            }
            OP_PING => self.handler.ping(data),
            OP_PONG => self.handler.pong(data),
            _ => {}
        }
    }
}
