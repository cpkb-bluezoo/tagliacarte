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

//! POP3 protocol client: connect, CAPA, STLS (RFC 2595), USER/PASS, STAT, UIDL, LIST, RETR, TOP, QUIT.

use crate::net::{connect_implicit_tls, connect_plain, PlainStream, TlsStreamWrapper};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// POP3 client error (network, protocol, auth).
#[derive(Debug)]
pub struct Pop3ClientError {
    pub message: String,
}

impl Pop3ClientError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for Pop3ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Pop3ClientError {}

impl From<io::Error> for Pop3ClientError {
    fn from(e: io::Error) -> Self {
        Self::new(e.to_string())
    }
}

/// Stream for POP3: plain TCP or TLS.
///
/// `Poisoned` exists only transiently during an [`Pop3Session::stls`] upgrade and must not be
/// observable by callers.
pub enum Pop3Stream {
    Plain(PlainStream),
    Tls(TlsStreamWrapper),
    Poisoned,
}

fn poisoned_io() -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        "POP3 stream in invalid internal state",
    )
}

impl Pop3Stream {
    pub async fn connect(host: &str, port: u16, use_tls: bool) -> io::Result<Self> {
        if use_tls {
            let tls = connect_implicit_tls(host, port).await?;
            Ok(Pop3Stream::Tls(tls))
        } else {
            let plain = connect_plain(host, port).await?;
            Ok(Pop3Stream::Plain(plain))
        }
    }
}

impl AsyncRead for Pop3Stream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        match self.get_mut() {
            Pop3Stream::Plain(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            Pop3Stream::Tls(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            Pop3Stream::Poisoned => std::task::Poll::Ready(Err(poisoned_io())),
        }
    }
}

impl AsyncWrite for Pop3Stream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        match self.get_mut() {
            Pop3Stream::Plain(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            Pop3Stream::Tls(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            Pop3Stream::Poisoned => std::task::Poll::Ready(Err(poisoned_io())),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        match self.get_mut() {
            Pop3Stream::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
            Pop3Stream::Tls(s) => std::pin::Pin::new(s).poll_flush(cx),
            Pop3Stream::Poisoned => std::task::Poll::Ready(Err(poisoned_io())),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        match self.get_mut() {
            Pop3Stream::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            Pop3Stream::Tls(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            Pop3Stream::Poisoned => std::task::Poll::Ready(Err(poisoned_io())),
        }
    }
}

/// STAT response: message count and total size in octets.
#[derive(Debug, Clone)]
pub struct StatResponse {
    pub count: u32,
    pub total_size: u64,
}

/// UIDL list entry: message number and unique-id.
#[derive(Debug, Clone)]
pub struct UidlEntry {
    pub msg_no: u32,
    pub uidl: String,
}

/// LIST entry: message number and size in octets.
#[derive(Debug, Clone)]
pub struct ListEntry {
    pub msg_no: u32,
    pub size: u64,
}

async fn read_line<S>(stream: &mut S, buf: &mut Vec<u8>) -> io::Result<String>
where
    S: AsyncRead + Unpin,
{
    buf.clear();
    loop {
        let mut b = [0u8; 1];
        let n = stream.read(&mut b).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ));
        }
        buf.push(b[0]);
        if buf.len() >= 2 && buf[buf.len() - 2..] == *b"\r\n" {
            break;
        }
    }
    let line = String::from_utf8_lossy(&buf[..buf.len() - 2])
        .trim_end()
        .to_string();
    Ok(line)
}

async fn write_line<S>(stream: &mut S, line: &str) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
{
    stream.write_all(line.as_bytes()).await?;
    stream.write_all(b"\r\n").await?;
    stream.flush().await?;
    Ok(())
}

/// Read multi-line response (lines until "." alone). POP3 dot-stuffing: leading "." in content is sent as "..".
async fn read_multiline<S>(stream: &mut S, buf: &mut Vec<u8>) -> Result<Vec<u8>, Pop3ClientError>
where
    S: AsyncRead + Unpin,
{
    let mut out = Vec::new();
    loop {
        let line = read_line(stream, buf).await?;
        if line == "." {
            break;
        }
        let to_append = if line.starts_with("..") {
            &line[1..]
        } else {
            line.as_str()
        };
        out.extend_from_slice(to_append.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    Ok(out)
}

fn check_ok(line: &str) -> Result<(), Pop3ClientError> {
    if line.starts_with("+OK") {
        Ok(())
    } else {
        Err(Pop3ClientError::new(line.to_string()))
    }
}

/// POP3 session (connected stream). Call login, then STAT/UIDL/LIST/RETR/TOP, then quit.
pub struct Pop3Session {
    stream: Pop3Stream,
    read_buf: Vec<u8>,
}

impl Pop3Session {
    pub async fn connect(host: &str, port: u16, use_tls: bool) -> io::Result<Self> {
        let stream = Pop3Stream::connect(host, port, use_tls).await?;
        let read_buf = Vec::with_capacity(4096);
        Ok(Self { stream, read_buf })
    }

    /// Read greeting (+OK ... or -ERR).
    pub async fn read_greeting(&mut self) -> Result<(), Pop3ClientError> {
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)
    }

    /// RFC 2449 `CAPA`: returns uppercased capability tokens (whitespace-separated words per line).
    pub async fn capa(&mut self) -> Result<Vec<String>, Pop3ClientError> {
        write_line(&mut self.stream, "CAPA").await?;
        let first = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&first)?;
        let mut caps = Vec::new();
        loop {
            let line = read_line(&mut self.stream, &mut self.read_buf).await?;
            if line == "." {
                break;
            }
            let to_parse = if line.starts_with("..") {
                &line[1..]
            } else {
                line.as_str()
            };
            for w in to_parse.split_whitespace() {
                caps.push(w.to_uppercase());
            }
        }
        Ok(caps)
    }

    /// RFC 2595: on a plain connection, send `STLS`, perform TLS handshake, then read the
    /// post-upgrade greeting. Call only after [`Self::capa`] confirms the server lists `STLS`.
    pub async fn stls(&mut self, host: &str) -> Result<(), Pop3ClientError> {
        if matches!(self.stream, Pop3Stream::Tls(_)) {
            return Err(Pop3ClientError::new("STLS: connection already uses TLS"));
        }
        if !matches!(self.stream, Pop3Stream::Plain(_)) {
            return Err(Pop3ClientError::new("STLS: invalid stream state"));
        }
        write_line(&mut self.stream, "STLS").await?;
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)?;

        let plain = match std::mem::replace(&mut self.stream, Pop3Stream::Poisoned) {
            Pop3Stream::Plain(p) => p,
            Pop3Stream::Tls(_) | Pop3Stream::Poisoned => {
                return Err(Pop3ClientError::new("STLS: internal stream state error"));
            }
        };
        let tls = plain
            .upgrade_to_tls(host)
            .await
            .map_err(|e| Pop3ClientError::new(e.to_string()))?;
        self.stream = Pop3Stream::Tls(tls);
        self.read_greeting().await
    }

    /// USER then PASS.
    pub async fn login(&mut self, username: &str, password: &str) -> Result<(), Pop3ClientError> {
        write_line(&mut self.stream, &format!("USER {}", username)).await?;
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)?;

        write_line(&mut self.stream, &format!("PASS {}", password)).await?;
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)?;
        Ok(())
    }

    /// STAT -> count and total size.
    pub async fn stat(&mut self) -> Result<StatResponse, Pop3ClientError> {
        write_line(&mut self.stream, "STAT").await?;
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)?;
        // +OK count size
        let rest = line.strip_prefix("+OK").map(|s| s.trim()).unwrap_or("");
        let parts: Vec<&str> = rest.split_whitespace().collect();
        let count = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0u32);
        let total_size = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0u64);
        Ok(StatResponse { count, total_size })
    }

    /// UIDL [msg] -> list of (msg_no, uidl). If msg is None, list all.
    pub async fn uidl(&mut self, msg: Option<u32>) -> Result<Vec<UidlEntry>, Pop3ClientError> {
        let cmd = match msg {
            Some(n) => format!("UIDL {}", n),
            None => "UIDL".to_string(),
        };
        write_line(&mut self.stream, &cmd).await?;
        let first = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&first)?;

        let mut entries = Vec::new();
        loop {
            let line = read_line(&mut self.stream, &mut self.read_buf).await?;
            if line == "." {
                break;
            }
            let mut sp = line.splitn(2, ' ');
            let msg_no: u32 = sp.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let uidl = sp.next().unwrap_or("").to_string();
            if msg_no > 0 {
                entries.push(UidlEntry { msg_no, uidl });
            }
        }
        Ok(entries)
    }

    /// LIST [msg] -> list of (msg_no, size). If msg is None, list all.
    pub async fn list(&mut self, msg: Option<u32>) -> Result<Vec<ListEntry>, Pop3ClientError> {
        let cmd = match msg {
            Some(n) => format!("LIST {}", n),
            None => "LIST".to_string(),
        };
        write_line(&mut self.stream, &cmd).await?;
        let first = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&first)?;

        let mut entries = Vec::new();
        loop {
            let line = read_line(&mut self.stream, &mut self.read_buf).await?;
            if line == "." {
                break;
            }
            let mut sp = line.splitn(2, ' ');
            let msg_no: u32 = sp.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let size: u64 = sp.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            if msg_no > 0 {
                entries.push(ListEntry { msg_no, size });
            }
        }
        Ok(entries)
    }

    /// RETR msg -> full message bytes.
    pub async fn retr(&mut self, msg_no: u32) -> Result<Vec<u8>, Pop3ClientError> {
        write_line(&mut self.stream, &format!("RETR {}", msg_no)).await?;
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)?;
        read_multiline(&mut self.stream, &mut self.read_buf).await
    }

    /// TOP msg n -> headers plus first n lines of body. n=0 for headers only.
    pub async fn top(&mut self, msg_no: u32, n: u32) -> Result<Vec<u8>, Pop3ClientError> {
        write_line(&mut self.stream, &format!("TOP {} {}", msg_no, n)).await?;
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)?;
        read_multiline(&mut self.stream, &mut self.read_buf).await
    }

    /// RETR with streaming: call on_chunk for each piece of the message.
    pub async fn retr_streaming<F>(
        &mut self,
        msg_no: u32,
        mut on_chunk: F,
    ) -> Result<(), Pop3ClientError>
    where
        F: FnMut(&[u8]),
    {
        write_line(&mut self.stream, &format!("RETR {}", msg_no)).await?;
        let line = read_line(&mut self.stream, &mut self.read_buf).await?;
        check_ok(&line)?;
        let mut buf = Vec::new();
        loop {
            let line = read_line(&mut self.stream, &mut self.read_buf).await?;
            if line == "." {
                break;
            }
            let to_append = if line.starts_with("..") {
                &line[1..]
            } else {
                line.as_str()
            };
            buf.extend_from_slice(to_append.as_bytes());
            buf.extend_from_slice(b"\r\n");
        }
        if !buf.is_empty() {
            on_chunk(&buf);
        }
        Ok(())
    }

    /// QUIT.
    pub async fn quit(&mut self) -> Result<(), Pop3ClientError> {
        let _ = write_line(&mut self.stream, "QUIT").await;
        let _ = read_line(&mut self.stream, &mut self.read_buf).await;
        Ok(())
    }
}
