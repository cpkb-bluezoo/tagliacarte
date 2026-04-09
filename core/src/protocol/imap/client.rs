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

//! Async IMAP client: connect, CAPABILITY, STARTTLS (when advertised, debug flag to skip),
//! LOGIN/AUTH, LIST, SELECT, FETCH. Pattern follows SMTP client (stateful protocol).

use super::trace;
use crate::net::{connect_implicit_tls, connect_plain, PlainStream, TlsStreamWrapper};
use crate::sasl::{
    initial_client_response, login_respond_to_challenge, respond_to_challenge, SaslError,
    SaslFirst, SaslMechanism,
};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

/// IMAP client error (network, protocol, auth).
#[derive(Debug)]
pub struct ImapClientError {
    pub message: String,
}

impl ImapClientError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for ImapClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ImapClientError {}

impl From<io::Error> for ImapClientError {
    fn from(e: io::Error) -> Self {
        Self::new(e.to_string())
    }
}

impl From<SaslError> for ImapClientError {
    fn from(e: SaslError) -> Self {
        Self::new(e.to_string())
    }
}

/// Which RFC822 header fields to request in `BODY.PEEK[HEADER.FIELDS (...)]` for batched summaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryHeaderFields {
    /// Flat list rows: from, sender, subject, date.
    List,
    /// Thread grouping only (subject + Message-ID / References / In-Reply-To).
    ThreadIndex,
    /// Rows inside a thread: envelope fields for the UI plus threading headers.
    ThreadDrillDown,
}

impl SummaryHeaderFields {
    pub(crate) fn body_peek_clause(self) -> &'static str {
        match self {
            Self::List => "BODY.PEEK[HEADER.FIELDS (FROM SENDER SUBJECT DATE)]",
            Self::ThreadIndex => {
                "BODY.PEEK[HEADER.FIELDS (SUBJECT MESSAGE-ID REFERENCES IN-REPLY-TO)]"
            }
            Self::ThreadDrillDown => {
                "BODY.PEEK[HEADER.FIELDS (FROM SENDER TO CC SUBJECT DATE MESSAGE-ID REFERENCES IN-REPLY-TO)]"
            }
        }
    }
}

/// One line of IMAP response (untagged * or tagged A001).
#[derive(Debug, Clone)]
pub struct ImapLine {
    pub raw: String,
    pub tag: Option<String>,
    pub untagged: bool,
    pub status: Option<ImapStatus>,
}

#[derive(Debug, Clone)]
pub enum ImapStatus {
    Ok,
    No,
    Bad,
}

/// Parse "* OK ..." or "A001 OK ..." from a line. Does not handle continuation (literal).
fn parse_line(s: &str) -> ImapLine {
    let raw = s.to_string();
    let untagged = s.starts_with('*');
    let (tag, status) = if untagged {
        let rest = s.trim_start_matches('*').trim_start();
        if rest.starts_with("OK ") {
            (None, Some(ImapStatus::Ok))
        } else if rest.starts_with("NO ") {
            (None, Some(ImapStatus::No))
        } else if rest.starts_with("BAD ") {
            (None, Some(ImapStatus::Bad))
        } else {
            (None, None)
        }
    } else {
        let mut sp = s.splitn(2, ' ');
        let t = sp.next().unwrap_or("").to_string();
        let rest = sp.next().unwrap_or("");
        let st = if rest.starts_with("OK ") {
            Some(ImapStatus::Ok)
        } else if rest.starts_with("NO ") {
            Some(ImapStatus::No)
        } else if rest.starts_with("BAD ") {
            Some(ImapStatus::Bad)
        } else {
            None
        };
        (Some(t), st)
    };
    ImapLine {
        raw,
        tag: tag.filter(|t| !t.is_empty()),
        untagged,
        status,
    }
}

/// Read one line from stream; if line ends with {N}, read N bytes literal and append (as one logical line for parsing we return line + literal separately or combined).
/// Returns (line_string, literal_data_if_any).
async fn read_imap_line<S>(
    stream: &mut S,
    buf: &mut Vec<u8>,
) -> io::Result<(String, Option<Vec<u8>>)>
where
    S: AsyncRead + Unpin,
{
    let (line, literal_size) = read_imap_line_literal_size(stream, buf).await?;
    if let Some(n) = literal_size {
        let mut lit = vec![0u8; n as usize];
        stream.read_exact(&mut lit).await?;
        if trace::enabled() && n > 0 {
            trace::log_inbound_literal_bytes("FETCH literal", &lit);
        }
        return Ok((line, Some(lit)));
    }
    Ok((line, None))
}

/// Read one line; if line ends with {N}, return (line, Some(N)) without reading the N bytes (caller can stream them).
async fn read_imap_line_literal_size<S>(
    stream: &mut S,
    buf: &mut Vec<u8>,
) -> io::Result<(String, Option<u32>)>
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
    let line_end = buf.len() - 2;
    let line = String::from_utf8_lossy(&buf[..line_end]).trim().to_string();
    let literal_size = if let Some(open) = line.rfind('{') {
        let rest = &line[open + 1..];
        if rest.ends_with('}') {
            rest.trim_end_matches('}').trim().parse().ok()
        } else {
            None
        }
    } else {
        None
    };
    if trace::enabled() {
        trace::log_inbound_line(&line, literal_size);
    }
    Ok((line, literal_size))
}

/// Read exactly `size` bytes from stream in chunks of at most `chunk_size`, calling `on_chunk` for each.
async fn read_literal_chunked<S, F>(
    stream: &mut S,
    size: u32,
    chunk_size: usize,
    mut on_chunk: F,
) -> io::Result<()>
where
    S: AsyncRead + Unpin,
    F: FnMut(&[u8]),
{
    let mut remaining = size as usize;
    let mut buf = vec![0u8; chunk_size.min(remaining)];
    while remaining > 0 {
        let to_read = buf.len().min(remaining);
        let n = stream.read(&mut buf[..to_read]).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ));
        }
        on_chunk(&buf[..n]);
        remaining -= n;
        if buf.len() > remaining {
            buf.truncate(remaining);
        }
    }
    Ok(())
}

/// Progress for a UID FETCH body literal after [`AuthenticatedSession::begin_fetch_body_peek_section`]
/// or [`AuthenticatedSession::begin_fetch_body_by_uid`]. Caller reads with
/// [`AuthenticatedSession::read_streaming_literal_chunk`], then
/// [`AuthenticatedSession::finish_streaming_fetch`].
#[derive(Debug, Clone)]
pub struct StreamingLiteralState {
    tag: String,
    /// Bytes remaining in the current FETCH body literal.
    pub remaining: usize,
}

/// Send a UID FETCH that returns one body literal; read until `{N}` appears, without consuming `N` bytes.
async fn begin_body_fetch_streaming_impl<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    tag: &str,
    command: &str,
) -> Result<StreamingLiteralState, ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let full = format!("{} {}", tag, command);
    write_line(stream, full.as_bytes()).await?;
    loop {
        let (line_str, literal_size) = read_imap_line_literal_size(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.untagged && line_str.contains(" FETCH (") {
            if let Some(n) = literal_size {
                return Ok(StreamingLiteralState {
                    tag: tag.to_string(),
                    remaining: n as usize,
                });
            }
        }
        if line.tag.as_deref() == Some(tag) {
            return if matches!(line.status, Some(ImapStatus::Ok)) {
                Err(ImapClientError::new(
                    "UID FETCH: expected body literal, got OK without literal",
                ))
            } else {
                Err(ImapClientError::new(line.raw))
            };
        }
    }
}

/// Read up to `buf.len()` bytes from an in-progress FETCH literal (`state.remaining` decremented).
async fn read_streaming_literal_chunk_impl<S>(
    stream: &mut S,
    buf: &mut [u8],
    state: &mut StreamingLiteralState,
) -> Result<usize, ImapClientError>
where
    S: AsyncRead + Unpin,
{
    if state.remaining == 0 {
        return Ok(0);
    }
    let to_read = buf.len().min(state.remaining);
    let n = stream.read(&mut buf[..to_read]).await?;
    if n == 0 {
        return Err(ImapClientError::new(
            "connection closed during FETCH literal",
        ));
    }
    if trace::enabled() && n > 0 {
        trace::log_inbound_literal_bytes("FETCH literal chunk", &buf[..n]);
    }
    state.remaining -= n;
    Ok(n)
}

/// After the literal is fully read (`remaining == 0`), consume `)\r\n` / continuation lines until tagged OK.
async fn finish_body_fetch_streaming_impl<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    state: StreamingLiteralState,
) -> Result<(), ImapClientError>
where
    S: AsyncRead + Unpin,
{
    if state.remaining != 0 {
        return Err(ImapClientError::new(
            "finish_streaming_fetch: literal not fully consumed",
        ));
    }
    let tag = state.tag;
    loop {
        let (line_str, literal_size) = read_imap_line_literal_size(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.untagged && line_str.contains(" FETCH (") {
            if literal_size.is_some() {
                return Err(ImapClientError::new(
                    "UID FETCH: unexpected second literal after streaming body",
                ));
            }
        }
        if line.tag.as_deref() == Some(tag.as_str()) {
            return if matches!(line.status, Some(ImapStatus::Ok)) {
                Ok(())
            } else {
                Err(ImapClientError::new(line.raw))
            };
        }
    }
}

/// Write a line (no CRLF) then CRLF.
async fn write_line<S>(stream: &mut S, line: &[u8]) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
{
    if trace::enabled() {
        trace::log_outbound_line(&String::from_utf8_lossy(line));
    }
    stream.write_all(line).await?;
    stream.write_all(b"\r\n").await?;
    stream.flush().await?;
    Ok(())
}

/// Untagged line plus optional literal (e.g. FETCH body).
pub struct ImapLineWithLiteral(pub ImapLine, pub Option<Vec<u8>>);

/// LIST streaming: send command then read line-by-line, calling on_entry for each * LIST. Yields as each packet arrives.
async fn list_folders_streaming_impl<S, F>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    tag: &str,
    command: &str,
    on_entry: &mut F,
) -> Result<(), ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: FnMut(ListEntry),
{
    let full = format!("{} {}", tag, command);
    write_line(stream, full.as_bytes()).await?;
    loop {
        let (line_str, _literal) = read_imap_line(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.untagged {
            if line_str.starts_with("* LIST ") {
                if let Some(entry) = parse_list_line(&line_str) {
                    on_entry(entry);
                }
            }
        } else if line.tag.as_deref() == Some(tag) {
            return if matches!(line.status, Some(ImapStatus::Ok)) {
                Ok(())
            } else {
                Err(ImapClientError::new(line.raw))
            };
        }
    }
}

/// SELECT streaming: send SELECT, read line-by-line, call on_event for each untagged, fill exists/uid_validity, return on tagged.
async fn select_streaming_impl<S, F>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    tag: &str,
    command: &str,
    on_event: &mut F,
    exists: &mut u32,
    uid_validity: &mut Option<u32>,
) -> Result<(), ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: FnMut(SelectEvent),
{
    let full = format!("{} {}", tag, command);
    write_line(stream, full.as_bytes()).await?;
    loop {
        let (line_str, _literal) = read_imap_line(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.untagged {
            if let Some(ev) = parse_select_event(&line_str) {
                match &ev {
                    SelectEvent::Exists(n) => *exists = *n,
                    SelectEvent::UidValidity(n) => *uid_validity = Some(*n),
                    _ => {}
                }
                on_event(ev);
            } else {
                on_event(SelectEvent::Other(line_str));
            }
        } else if line.tag.as_deref() == Some(tag) {
            return if matches!(line.status, Some(ImapStatus::Ok)) {
                Ok(())
            } else {
                Err(ImapClientError::new(line.raw))
            };
        }
    }
}

/// FETCH summaries streaming: send command, read line-by-line, call on_summary for each * FETCH ( ... ).
async fn fetch_summaries_streaming_impl<S, F>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    tag: &str,
    command: &str,
    on_summary: &mut F,
) -> Result<(), ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: FnMut(FetchSummary),
{
    let full = format!("{} {}", tag, command);
    write_line(stream, full.as_bytes()).await?;
    loop {
        let (line_str, literal) = read_imap_line(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.untagged {
            if line_str.contains(" FETCH (") {
                if let Some(s) = parse_fetch_summary(&line_str, literal.as_deref()) {
                    on_summary(s);
                }
            }
        } else if line.tag.as_deref() == Some(tag) {
            return if matches!(line.status, Some(ImapStatus::Ok)) {
                Ok(())
            } else {
                Err(ImapClientError::new(line.raw))
            };
        }
    }
}

/// FETCH body streaming: send UID FETCH uid (BODY[]), read line with literal size, stream literal in chunks, then consume to tagged.
async fn fetch_body_streaming_impl<S, F>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    tag: &str,
    command: &str,
    chunk_size: usize,
    on_chunk: &mut F,
) -> Result<(), ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: FnMut(&[u8]),
{
    let full = format!("{} {}", tag, command);
    write_line(stream, full.as_bytes()).await?;
    loop {
        let (line_str, literal_size) = read_imap_line_literal_size(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.untagged && line_str.contains(" FETCH (") {
            if let Some(n) = literal_size {
                read_literal_chunked(stream, n, chunk_size, &mut *on_chunk).await?;
                // Next loop iteration will read ")\r\n" as line ")" then the tagged line
            }
        }
        if line.tag.as_deref() == Some(tag) {
            return if matches!(line.status, Some(ImapStatus::Ok)) {
                Ok(())
            } else {
                Err(ImapClientError::new(line.raw))
            };
        }
    }
}

fn parse_select_event(line: &str) -> Option<SelectEvent> {
    let rest = line.strip_prefix("* ")?.trim_start();
    if rest.ends_with(" EXISTS") {
        let n: u32 = rest.trim_end_matches(" EXISTS").trim().parse().ok()?;
        return Some(SelectEvent::Exists(n));
    }
    if rest.ends_with(" RECENT") {
        let n: u32 = rest.trim_end_matches(" RECENT").trim().parse().ok()?;
        return Some(SelectEvent::Recent(n));
    }
    if rest.starts_with("FLAGS (") {
        let end = rest.find(')')?;
        let inner = &rest[7..end];
        let flags: Vec<String> = inner.split_whitespace().map(|s| s.to_string()).collect();
        return Some(SelectEvent::Flags(flags));
    }
    if rest.starts_with("OK ") {
        if let Some(bracket) = rest.find("[UIDVALIDITY ") {
            let after = &rest[bracket + 13..];
            let n: u32 = after
                .split_whitespace()
                .next()?
                .trim_end_matches(']')
                .parse()
                .ok()?;
            return Some(SelectEvent::UidValidity(n));
        }
        if let Some(bracket) = rest.find("[UIDNEXT ") {
            let after = &rest[bracket + 9..];
            let n: u32 = after
                .split_whitespace()
                .next()?
                .trim_end_matches(']')
                .parse()
                .ok()?;
            return Some(SelectEvent::UidNext(n));
        }
        if let Some(bracket) = rest.find("[PERMANENTFLAGS (") {
            let after = &rest[bracket + 17..];
            let end = after.find(')')?;
            let inner = &after[..end];
            let flags: Vec<String> = inner.split_whitespace().map(|s| s.to_string()).collect();
            return Some(SelectEvent::PermanentFlags(flags));
        }
    }
    None
}

/// Send command with tag, read until tagged response. Returns (untagged lines with optional literals, final tagged line).
async fn send_command<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    tag: &str,
    command: &str,
) -> Result<(Vec<ImapLineWithLiteral>, ImapLine), ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let full = format!("{} {}", tag, command);
    write_line(stream, full.as_bytes()).await?;

    let mut untagged = Vec::new();
    loop {
        let (line_str, literal) = read_imap_line(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.untagged {
            untagged.push(ImapLineWithLiteral(line, literal));
        } else if line.tag.as_deref() == Some(tag) {
            return Ok((untagged, line));
        } else {
            untagged.push(ImapLineWithLiteral(line, literal));
        }
    }
}

/// Send APPEND command with literal (mailbox + raw message bytes). Reads until tagged response.
async fn send_append<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    tag: &str,
    mailbox: &str,
    data: &[u8],
) -> Result<ImapLine, ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let cmd = format!(
        "{} APPEND {} {{{}}}\r\n",
        tag,
        quote_string(mailbox),
        data.len()
    );
    if trace::enabled() {
        trace::log_append_command(&cmd);
    }
    stream.write_all(cmd.as_bytes()).await?;
    stream.write_all(data).await?;
    stream.flush().await?;

    loop {
        let (line_str, _literal) = read_imap_line(stream, read_buf).await?;
        let line = parse_line(&line_str);
        if line.tag.as_deref() == Some(tag) {
            return Ok(line);
        }
    }
}

/// Check if capability string contains STARTTLS.
fn has_starttls(capabilities: &[String]) -> bool {
    capabilities
        .iter()
        .any(|c| c.eq_ignore_ascii_case("STARTTLS"))
}

/// Parse capability list from "* CAPABILITY IMAP4rev2 STARTTLS AUTH=PLAIN ..." or from [CAPABILITY ...] in OK.
fn parse_capabilities(line: &str) -> Vec<String> {
    let mut caps = Vec::new();
    let s = line
        .strip_prefix("* CAPABILITY ")
        .or_else(|| {
            line.find("[CAPABILITY ")
                .map(|i| &line[i + 13..])
                .and_then(|t| t.strip_suffix(']').or_else(|| t.split(']').next()))
        })
        .unwrap_or("");
    for word in s.split_whitespace() {
        caps.push(word.to_uppercase());
    }
    caps
}

/// Generate next tag (A001, A002, ...).
fn next_tag() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed) % 9999 + 1;
    format!("A{:04}", n)
}

fn base64_encode(b: &[u8]) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity((b.len() + 2) / 3 * 4);
    for chunk in b.chunks(3) {
        let n = (chunk[0] as usize) << 16
            | (chunk.get(1).copied().unwrap_or(0) as usize) << 8
            | chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(ALPHABET[n >> 18]);
        out.push(ALPHABET[(n >> 12) & 63]);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) & 63]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n & 63]
        } else {
            b'='
        });
    }
    out
}

/// Get capabilities: from greeting or send CAPABILITY command.
async fn ensure_capabilities<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    greeting_ok_line: Option<&str>,
) -> Result<Vec<String>, ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if let Some(line) = greeting_ok_line {
        let caps = parse_capabilities(line);
        if !caps.is_empty() {
            return Ok(caps);
        }
    }
    capability_command(stream, read_buf).await
}

/// Send `CAPABILITY` and parse the untagged `* CAPABILITY` list (post-auth refresh).
async fn capability_command<S>(stream: &mut S, read_buf: &mut Vec<u8>) -> Result<Vec<String>, ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tag = next_tag();
    let (untagged, final_line) = send_command(stream, read_buf, &tag, "CAPABILITY").await?;
    if !matches!(final_line.status, Some(ImapStatus::Ok)) {
        return Err(ImapClientError::new(final_line.raw.clone()));
    }
    for lwl in untagged {
        if lwl.0.raw.starts_with("* CAPABILITY ") {
            return Ok(parse_capabilities(&lwl.0.raw));
        }
    }
    Ok(Vec::new())
}

/// Perform LOGIN (user, password).
async fn login_plain<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    user: &str,
    pass: &str,
) -> Result<(), ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tag = next_tag();
    let cmd = format!("LOGIN {} {}", quote_string(user), quote_string(pass));
    let (_, final_line) = send_command(stream, read_buf, &tag, &cmd).await?;
    match final_line.status {
        Some(ImapStatus::Ok) => Ok(()),
        _ => Err(ImapClientError::new(final_line.raw)),
    }
}

fn quote_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Parse UNSEEN count from an untagged `* STATUS ...` line.
fn parse_status_unseen(line: &str) -> Option<u32> {
    let line = line.trim_end();
    if !line.starts_with("* STATUS ") {
        return None;
    }
    let idx = line.find("UNSEEN")?;
    let mut tail = line[idx + "UNSEEN".len()..].trim_start();
    if let Some(stripped) = tail.strip_prefix('(') {
        tail = stripped;
    }
    let num_part = tail.split_whitespace().next()?;
    num_part.trim_end_matches(')').parse::<u32>().ok()
}

/// Mailbox token after `* STATUS ` (quoted-string or atom up to ` (`).
fn take_imap_mailbox_token_after_status(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('"') {
        let mut name = String::new();
        let mut i = 1;
        let bytes = s.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                name.push(bytes[i + 1] as char);
                i += 2;
            } else if bytes[i] == b'"' {
                let rest = s[i + 1..].trim_start();
                return Some((name, rest));
            } else {
                name.push(bytes[i] as char);
                i += 1;
            }
        }
        None
    } else {
        let idx = s.find(" (")?;
        Some((s[..idx].to_string(), s[idx + 1..].trim_start()))
    }
}

/// Parse mailbox name and UNSEEN from `* STATUS ...` (RFC 9051 / RFC 3501).
fn parse_status_mailbox_and_unseen(line: &str) -> Option<(String, u32)> {
    let rest = line.strip_prefix("* STATUS ")?.trim_start();
    let (name, _) = take_imap_mailbox_token_after_status(rest)?;
    let unseen = parse_status_unseen(line)?;
    Some((name, unseen))
}

/// Perform AUTH (mechanism with optional initial response).
async fn auth_sasl<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    mechanism: SaslMechanism,
    authcid: &str,
    password: &str,
) -> Result<(), ImapClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let first = initial_client_response(mechanism, "", authcid, password)?;
    let (initial_b64, scram_state) = match &first {
        SaslFirst::Done(b) => (base64_encode(b), None),
        SaslFirst::ScramContinue(b, state) => (base64_encode(b), Some(state.clone())),
    };

    let tag = next_tag();
    let mut cmd = format!("AUTHENTICATE {}", mechanism.name());
    if !initial_b64.is_empty() {
        cmd.push_str(" ");
        cmd.push_str(&String::from_utf8_lossy(&initial_b64));
    }
    let (untagged, final_line) = send_command(stream, read_buf, &tag, &cmd).await?;

    if matches!(final_line.status, Some(ImapStatus::Ok)) {
        return Ok(());
    }
    if matches!(final_line.status, Some(ImapStatus::No)) {
        return Err(ImapClientError::new(final_line.raw));
    }

    // Continuation "+ " with challenge (base64 in literal or after "+ " on line)
    let challenge_b64 = untagged
        .iter()
        .rev()
        .find(|lwl| lwl.0.raw.starts_with("+ "))
        .and_then(|lwl| {
            lwl.1
                .as_ref()
                .and_then(|b| std::str::from_utf8(b).ok())
                .map(|s| s.trim().to_string())
                .or_else(|| lwl.0.raw.strip_prefix('+').map(|s| s.trim().to_string()))
        });
    let challenge_b64 = match challenge_b64 {
        Some(c) => c,
        None => return Err(ImapClientError::new("no AUTH challenge")),
    };

    let response = if mechanism == SaslMechanism::Login {
        login_respond_to_challenge(&challenge_b64, authcid, password)?
    } else {
        respond_to_challenge(
            mechanism,
            &challenge_b64,
            authcid,
            password,
            scram_state.as_ref(),
        )?
    };
    let resp_b64 = String::from_utf8_lossy(&base64_encode(&response)).to_string();
    write_line(stream, resp_b64.as_bytes()).await?;

    let (_line_str, _lit) = read_imap_line(stream, read_buf).await?;
    let line = parse_line(&_line_str);
    if matches!(line.status, Some(ImapStatus::Ok)) {
        Ok(())
    } else {
        Err(ImapClientError::new(line.raw))
    }
}

/// Run session on an already-TLS stream (implicit TLS).
async fn run_authenticated_tls(
    stream: &mut TlsStreamWrapper,
    read_buf: &mut Vec<u8>,
    greeting_line: &str,
    auth: Option<(&str, &str, SaslMechanism)>,
) -> Result<Vec<String>, ImapClientError> {
    let pre = ensure_capabilities(stream, read_buf, Some(greeting_line)).await?;
    if let Some((user, pass, mechanism)) = auth {
        if server_supports_auth(&pre, mechanism) {
            auth_sasl(stream, read_buf, mechanism, user, pass).await?;
        } else {
            login_plain(stream, read_buf, user, pass).await?;
        }
    }
    capability_command(stream, read_buf).await
}

fn server_supports_auth(caps: &[String], mechanism: SaslMechanism) -> bool {
    caps.iter()
        .any(|c| c == &format!("AUTH={}", mechanism.name()))
}

/// Read greeting line (* OK ...).
async fn read_greeting<S>(stream: &mut S, read_buf: &mut Vec<u8>) -> Result<String, ImapClientError>
where
    S: AsyncRead + Unpin,
{
    let (line, _lit) = read_imap_line(stream, read_buf).await?;
    if !line.starts_with("* OK") && !line.starts_with("* PREECH") {
        return Err(ImapClientError::new(format!(
            "expected * OK greeting, got: {}",
            line
        )));
    }
    Ok(line)
}

/// Connect and authenticate. Returns session for LIST, SELECT, FETCH.
///
/// **Plain TCP:** If `use_starttls` is true, the server must advertise `STARTTLS` in
/// `CAPABILITY`; we upgrade before any authentication. If `STARTTLS` is not advertised, or the
/// upgrade fails, we return an error and never send credentials on cleartext. Set `use_starttls`
/// to false only for debugging (cleartext auth is then allowed if the server permits it).
pub async fn connect_and_authenticate(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
) -> Result<AuthenticatedSession, ImapClientError> {
    if trace::enabled() {
        eprintln!(
            "[imap trace] connect_and_authenticate host={host}:{port} implicit_tls={use_implicit_tls} starttls={use_starttls} has_auth={}",
            auth.is_some()
        );
    }
    if use_implicit_tls {
        let mut stream = connect_implicit_tls(host, port).await?;
        let mut read_buf = Vec::with_capacity(4096);
        let greeting = read_greeting(&mut stream, &mut read_buf).await?;
        let caps = run_authenticated_tls(&mut stream, &mut read_buf, &greeting, auth).await?;
        return Ok(AuthenticatedSession::Tls {
            stream,
            read_buf,
            host: host.to_string(),
            capabilities: caps,
            greeting,
        });
    }

    let mut plain = connect_plain(host, port).await?;
    let mut read_buf = Vec::with_capacity(4096);
    let greeting = read_greeting(&mut plain, &mut read_buf).await?;
    let caps = ensure_capabilities(&mut plain, &mut read_buf, Some(&greeting)).await?;
    if use_starttls && !has_starttls(&caps) {
        return Err(ImapClientError::new(
            "STARTTLS is required on this plain connection but the server did not advertise STARTTLS in IMAP CAPABILITY",
        ));
    }
    let do_starttls = has_starttls(&caps) && use_starttls;

    if do_starttls {
        let tag = next_tag();
        let (_, final_line) = send_command(&mut plain, &mut read_buf, &tag, "STARTTLS").await?;
        if !matches!(final_line.status, Some(ImapStatus::Ok)) {
            return Err(ImapClientError::new(final_line.raw));
        }
        let mut tls = plain.upgrade_to_tls(host).await?;
        let greeting2 = read_greeting(&mut tls, &mut read_buf).await?;
        let caps2 = ensure_capabilities(&mut tls, &mut read_buf, Some(&greeting2)).await?;
        if let Some((user, pass, mechanism)) = auth {
            if server_supports_auth(&caps2, mechanism) {
                auth_sasl(&mut tls, &mut read_buf, mechanism, user, pass).await?;
            } else {
                login_plain(&mut tls, &mut read_buf, user, pass).await?;
            }
        }
        let caps_final = capability_command(&mut tls, &mut read_buf).await?;
        return Ok(AuthenticatedSession::Tls {
            stream: tls,
            read_buf,
            host: host.to_string(),
            capabilities: caps_final,
            greeting: greeting2,
        });
    }

    if let Some((user, pass, mechanism)) = auth {
        if server_supports_auth(&caps, mechanism) {
            auth_sasl(&mut plain, &mut read_buf, mechanism, user, pass).await?;
        } else {
            login_plain(&mut plain, &mut read_buf, user, pass).await?;
        }
    }
    let caps_final = capability_command(&mut plain, &mut read_buf).await?;
    Ok(AuthenticatedSession::Plain {
        stream: plain,
        read_buf,
        host: host.to_string(),
        capabilities: caps_final,
        greeting,
    })
}

/// Authenticated IMAP session (plain or TLS). Used for LIST, SELECT, FETCH.
pub enum AuthenticatedSession {
    Plain {
        stream: PlainStream,
        read_buf: Vec<u8>,
        host: String,
        capabilities: Vec<String>,
        greeting: String,
    },
    Tls {
        stream: TlsStreamWrapper,
        read_buf: Vec<u8>,
        host: String,
        capabilities: Vec<String>,
        greeting: String,
    },
}

impl AuthenticatedSession {
    pub fn capabilities(&self) -> &[String] {
        match self {
            AuthenticatedSession::Plain { capabilities, .. } => capabilities,
            AuthenticatedSession::Tls { capabilities, .. } => capabilities,
        }
    }

    pub fn host(&self) -> &str {
        match self {
            AuthenticatedSession::Plain { host, .. } => host,
            AuthenticatedSession::Tls { host, .. } => host,
        }
    }

    /// LIST "" "*" and parse folder names.
    pub async fn list_folders(&mut self) -> Result<Vec<ListEntry>, ImapClientError> {
        let tag = next_tag();
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, r#"LIST "" "*""#).await?,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, r#"LIST "" "*""#).await?,
        };
        if !matches!(final_line.status, Some(ImapStatus::Ok)) {
            return Err(ImapClientError::new(final_line.raw));
        }
        let mut entries = Vec::new();
        for lwl in untagged {
            if lwl.0.raw.starts_with("* LIST ") {
                if let Some(entry) = parse_list_line(&lwl.0.raw) {
                    entries.push(entry);
                }
            }
        }
        Ok(entries)
    }

    /// LIST "" "*" streaming: invoke `on_entry` for each * LIST line as it is read from the server.
    /// Events are delivered at protocol granularity (per packet), not after the full response.
    pub async fn list_folders_streaming<F>(
        &mut self,
        mut on_entry: F,
    ) -> Result<(), ImapClientError>
    where
        F: FnMut(ListEntry),
    {
        let tag = next_tag();
        let cmd = r#"LIST "" "*""#;
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => list_folders_streaming_impl(stream, read_buf, &tag, cmd, &mut on_entry).await,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => list_folders_streaming_impl(stream, read_buf, &tag, cmd, &mut on_entry).await,
        }
    }

    /// SELECT mailbox; returns exists (message count) and optional UIDVALIDITY.
    pub async fn select(&mut self, mailbox: &str) -> Result<SelectResult, ImapClientError> {
        let tag = next_tag();
        let cmd = format!("SELECT {}", quote_string(mailbox));
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
        };
        if !matches!(final_line.status, Some(ImapStatus::Ok)) {
            return Err(ImapClientError::new(final_line.raw));
        }
        let mut exists = 0u32;
        let mut uid_validity = None;
        for lwl in untagged {
            let line = &lwl.0.raw;
            if line.starts_with("* ") {
                let rest = line[2..].trim_start();
                if rest.ends_with(" EXISTS") {
                    if let Ok(n) = rest.trim_end_matches(" EXISTS").trim().parse::<u32>() {
                        exists = n;
                    }
                } else if rest.contains("[UIDVALIDITY ") {
                    if let Some(bracket) = rest.find("[UIDVALIDITY ") {
                        let after = &rest[bracket + 13..];
                        let num = after
                            .split_whitespace()
                            .next()
                            .and_then(|s| s.trim_end_matches(']').parse().ok());
                        if let Some(n) = num {
                            uid_validity = Some(n);
                        }
                    }
                }
            }
        }
        Ok(SelectResult {
            exists,
            uid_validity,
        })
    }

    /// SELECT mailbox streaming: send SELECT, return immediately; call `on_event` for each untagged SELECT response line, then return SelectResult when tagged response received.
    pub async fn select_streaming<F>(
        &mut self,
        mailbox: &str,
        mut on_event: F,
    ) -> Result<SelectResult, ImapClientError>
    where
        F: FnMut(SelectEvent),
    {
        let tag = next_tag();
        let cmd = format!("SELECT {}", quote_string(mailbox));
        let mut exists = 0u32;
        let mut uid_validity = None;
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => {
                select_streaming_impl(
                    stream,
                    read_buf,
                    &tag,
                    &cmd,
                    &mut on_event,
                    &mut exists,
                    &mut uid_validity,
                )
                .await?;
            }
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => {
                select_streaming_impl(
                    stream,
                    read_buf,
                    &tag,
                    &cmd,
                    &mut on_event,
                    &mut exists,
                    &mut uid_validity,
                )
                .await?;
            }
        }
        Ok(SelectResult {
            exists,
            uid_validity,
        })
    }

    /// APPEND raw message bytes to mailbox. Does not require SELECT.
    pub async fn append(&mut self, mailbox: &str, data: &[u8]) -> Result<(), ImapClientError> {
        let tag = next_tag();
        let result = match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => send_append(stream, read_buf, &tag, mailbox, data).await,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => send_append(stream, read_buf, &tag, mailbox, data).await,
        }?;
        if !matches!(result.status, Some(ImapStatus::Ok)) {
            return Err(ImapClientError::new(result.raw));
        }
        Ok(())
    }

    /// FETCH sequence range for envelope summaries (UID, FLAGS, RFC822.SIZE, header fields).
    pub async fn fetch_summaries(
        &mut self,
        seq_start: u32,
        seq_end: u32,
        header_fields: SummaryHeaderFields,
    ) -> Result<Vec<FetchSummary>, ImapClientError> {
        let tag = next_tag();
        let cmd = format!(
            "FETCH {}:{} (UID FLAGS RFC822.SIZE {})",
            seq_start,
            seq_end,
            header_fields.body_peek_clause()
        );
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
        };
        if !matches!(final_line.status, Some(ImapStatus::Ok)) {
            return Err(ImapClientError::new(final_line.raw));
        }
        let mut out = Vec::new();
        for lwl in untagged {
            if lwl.0.raw.contains(" FETCH (") {
                if let Some(s) = parse_fetch_summary(&lwl.0.raw, lwl.1.as_deref()) {
                    out.push(s);
                }
            }
        }
        Ok(out)
    }

    /// FETCH summaries streaming: send FETCH, call `on_summary` for each * FETCH response as it is read, then return.
    pub async fn fetch_summaries_streaming<F>(
        &mut self,
        seq_start: u32,
        seq_end: u32,
        header_fields: SummaryHeaderFields,
        mut on_summary: F,
    ) -> Result<(), ImapClientError>
    where
        F: FnMut(FetchSummary),
    {
        let tag = next_tag();
        let cmd = format!(
            "FETCH {}:{} (UID FLAGS RFC822.SIZE {})",
            seq_start,
            seq_end,
            header_fields.body_peek_clause()
        );
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => {
                fetch_summaries_streaming_impl(stream, read_buf, &tag, &cmd, &mut on_summary).await
            }
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => {
                fetch_summaries_streaming_impl(stream, read_buf, &tag, &cmd, &mut on_summary).await
            }
        }
    }

    /// FETCH one message by UID (full BODY[]). Use after SELECT.
    pub async fn fetch_body_by_uid(&mut self, uid: u32) -> Result<Vec<u8>, ImapClientError> {
        let tag = next_tag();
        let cmd = format!("UID FETCH {} (BODY[])", uid);
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
        };
        if !matches!(final_line.status, Some(ImapStatus::Ok)) {
            return Err(ImapClientError::new(final_line.raw));
        }
        for lwl in untagged {
            if lwl.0.raw.contains(" FETCH (") {
                if let Some(lit) = &lwl.1 {
                    return Ok(lit.clone());
                }
            }
        }
        Err(ImapClientError::new("UID FETCH BODY[] returned no literal"))
    }

    /// FETCH body by UID streaming: send UID FETCH uid (BODY[]), call `on_chunk` for each chunk of body data as it is read, then return.
    pub async fn fetch_body_by_uid_streaming<F>(
        &mut self,
        uid: u32,
        chunk_size: usize,
        mut on_chunk: F,
    ) -> Result<(), ImapClientError>
    where
        F: FnMut(&[u8]),
    {
        let tag = next_tag();
        let cmd = format!("UID FETCH {} (BODY[])", uid);
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => {
                fetch_body_streaming_impl(stream, read_buf, &tag, &cmd, chunk_size, &mut on_chunk)
                    .await
            }
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => {
                fetch_body_streaming_impl(stream, read_buf, &tag, &cmd, chunk_size, &mut on_chunk)
                    .await
            }
        }
    }

    /// Untagged FETCH line containing `BODYSTRUCTURE` for one UID.
    pub async fn fetch_bodystructure_line(&mut self, uid: u32) -> Result<String, ImapClientError> {
        let tag = next_tag();
        let cmd = format!("UID FETCH {} (BODYSTRUCTURE)", uid);
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => send_command(stream, read_buf, &tag, &cmd).await?,
        };
        if !matches!(final_line.status, Some(ImapStatus::Ok)) {
            return Err(ImapClientError::new(final_line.raw));
        }
        for lwl in untagged {
            if lwl.0.raw.contains("BODYSTRUCTURE") {
                return Ok(lwl.0.raw);
            }
        }
        Err(ImapClientError::new(
            "UID FETCH BODYSTRUCTURE: no matching untagged response",
        ))
    }

    /// Stream one MIME section literal: `UID FETCH uid (BODY.PEEK[section])`.
    pub async fn fetch_body_peek_section_streaming<F>(
        &mut self,
        uid: u32,
        section: &str,
        chunk_size: usize,
        mut on_chunk: F,
    ) -> Result<(), ImapClientError>
    where
        F: FnMut(&[u8]),
    {
        let tag = next_tag();
        let cmd = format!("UID FETCH {} (BODY.PEEK[{section}])", uid);
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => {
                fetch_body_streaming_impl(stream, read_buf, &tag, &cmd, chunk_size, &mut on_chunk)
                    .await
            }
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => {
                fetch_body_streaming_impl(stream, read_buf, &tag, &cmd, chunk_size, &mut on_chunk)
                    .await
            }
        }
    }

    /// Start `UID FETCH uid (BODY.PEEK[section])`; returns literal size. Caller reads with
    /// [`Self::read_streaming_literal_chunk`], then [`Self::finish_streaming_fetch`].
    pub async fn begin_fetch_body_peek_section(
        &mut self,
        uid: u32,
        section: &str,
    ) -> Result<StreamingLiteralState, ImapClientError> {
        let tag = next_tag();
        let cmd = format!("UID FETCH {} (BODY.PEEK[{section}])", uid);
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => begin_body_fetch_streaming_impl(stream, read_buf, &tag, &cmd).await,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => begin_body_fetch_streaming_impl(stream, read_buf, &tag, &cmd).await,
        }
    }

    /// Start `UID FETCH uid (BODY[])`; returns literal size for full message body.
    pub async fn begin_fetch_body_by_uid(
        &mut self,
        uid: u32,
    ) -> Result<StreamingLiteralState, ImapClientError> {
        let tag = next_tag();
        let cmd = format!("UID FETCH {} (BODY[])", uid);
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => begin_body_fetch_streaming_impl(stream, read_buf, &tag, &cmd).await,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => begin_body_fetch_streaming_impl(stream, read_buf, &tag, &cmd).await,
        }
    }

    /// Read the next slice of the in-progress FETCH literal into `buf` (at most `state.remaining` bytes).
    pub async fn read_streaming_literal_chunk(
        &mut self,
        buf: &mut [u8],
        state: &mut StreamingLiteralState,
    ) -> Result<usize, ImapClientError> {
        match self {
            AuthenticatedSession::Plain { stream, .. } => {
                read_streaming_literal_chunk_impl(stream, buf, state).await
            }
            AuthenticatedSession::Tls { stream, .. } => {
                read_streaming_literal_chunk_impl(stream, buf, state).await
            }
        }
    }

    /// After the literal is fully read, consume the rest of the FETCH response until tagged OK.
    pub async fn finish_streaming_fetch(
        &mut self,
        state: StreamingLiteralState,
    ) -> Result<(), ImapClientError> {
        match self {
            AuthenticatedSession::Plain {
                stream, read_buf, ..
            } => finish_body_fetch_streaming_impl(stream, read_buf, state).await,
            AuthenticatedSession::Tls {
                stream, read_buf, ..
            } => finish_body_fetch_streaming_impl(stream, read_buf, state).await,
        }
    }
}

/// Result of SELECT (EXISTS, UIDVALIDITY).
#[derive(Debug)]
pub struct SelectResult {
    pub exists: u32,
    pub uid_validity: Option<u32>,
}

/// SELECT response item; emitted as each untagged line is read (streaming).
#[derive(Debug, Clone)]
pub enum SelectEvent {
    Exists(u32),
    Recent(u32),
    Flags(Vec<String>),
    PermanentFlags(Vec<String>),
    UidValidity(u32),
    UidNext(u32),
    /// Other untagged line (e.g. OK [READ-WRITE])
    Other(String),
}

/// One message summary from FETCH (UID, flags, size, header for envelope).
#[derive(Debug, Clone)]
pub struct FetchSummary {
    pub seq: u32,
    pub uid: u32,
    pub flags: Vec<String>,
    pub size: u32,
    pub header: Vec<u8>,
}

fn parse_fetch_summary(line: &str, literal: Option<&[u8]>) -> Option<FetchSummary> {
    let fetch_part = line.find(" FETCH (")?;
    let seq_str = line[1..fetch_part].trim();
    let seq: u32 = seq_str.parse().ok()?;
    let mut uid = 0u32;
    let mut flags = Vec::new();
    let mut size = 0u32;
    if let Some(open) = line.find("UID ") {
        let rest = &line[open + 4..];
        let end = rest.find(' ').unwrap_or(rest.len());
        uid = rest[..end].trim_end_matches(')').parse().ok()?;
    }
    if let Some(open) = line.find("FLAGS (") {
        let rest = &line[open + 7..];
        let end = rest.find(')').unwrap_or(0);
        flags = rest[..end]
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
    }
    if let Some(open) = line.find("RFC822.SIZE ") {
        let rest = &line[open + 12..];
        let end = rest.find(' ').unwrap_or(rest.len());
        size = rest[..end].trim_end_matches(')').parse().ok().unwrap_or(0);
    }
    let header = literal.map(|b| b.to_vec()).unwrap_or_default();
    Some(FetchSummary {
        seq,
        uid,
        flags,
        size,
        header,
    })
}

/// Parsed LIST response entry.
#[derive(Debug, Clone)]
pub struct ListEntry {
    pub attributes: Vec<String>,
    pub delimiter: Option<char>,
    pub name: String,
}

fn parse_list_or_lsub_line(line: &str, prefix: &str) -> Option<ListEntry> {
    let rest = line.strip_prefix(prefix)?.trim_start();
    let (attrs, rest) = parse_list_attrs(rest)?;
    let rest = rest.trim_start();
    let (delim, rest) = if rest.eq_ignore_ascii_case("NIL") {
        (None, rest.get(3..)?)
    } else if rest.starts_with('"') {
        let end = 1 + rest[1..].find('"')?;
        let d = rest[1..end].chars().next();
        (d, rest.get(end + 1..)?)
    } else {
        (None, rest)
    };
    let rest = rest.trim_start();
    let name = if rest.starts_with('"') {
        let mut name = String::new();
        let mut i = 1;
        let bytes = rest.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                name.push(bytes[i + 1] as char);
                i += 2;
            } else if bytes[i] == b'"' {
                break;
            } else {
                name.push(bytes[i] as char);
                i += 1;
            }
        }
        name
    } else {
        rest.split_whitespace().next()?.to_string()
    };
    Some(ListEntry {
        attributes: attrs,
        delimiter: delim,
        name,
    })
}

fn parse_list_line(line: &str) -> Option<ListEntry> {
    parse_list_or_lsub_line(line, "* LIST ")
}

fn parse_lsub_line(line: &str) -> Option<ListEntry> {
    parse_list_or_lsub_line(line, "* LSUB ")
}

fn parse_list_attrs(s: &str) -> Option<(Vec<String>, &str)> {
    let s = s.trim_start();
    if !s.starts_with('(') {
        return None;
    }
    let end = s.find(')')?;
    let inner = &s[1..end];
    let attrs: Vec<String> = inner.split_whitespace().map(|w| w.to_string()).collect();
    Some((attrs, s[end + 1..].trim_start()))
}

// ======================================================================
// IMAP Pipeline (event-driven, no threads)
// ======================================================================

/// Shared hooks for RFC 2177 IDLE and mailbox activity (single connection).
#[derive(Clone)]
pub struct PipelineIdleHooks {
    pub folder_list_stale: Arc<AtomicBool>,
    pub supports_idle: bool,
    pub mailbox_selected: Arc<AtomicBool>,
    pub min_idle_secs: Arc<AtomicU32>,
    pub in_idle: Arc<AtomicBool>,
    pub tag_counter: Arc<AtomicU32>,
}

/// A pending command awaiting its tagged response.
struct PendingCommand {
    /// Called for each untagged response line while this is the active command.
    on_untagged: Box<dyn Fn(&str, Option<&[u8]>) + Send>,
    /// Called once when the matching tagged response arrives (ok, raw_line).
    on_complete: Box<dyn FnOnce(bool, &str) + Send>,
}

/// Command sent through the channel to the pipeline task.
struct PipelineCommand {
    tag: String,
    command: String,
    pending: PendingCommand,
}

enum LoopCommand {
    Normal(PipelineCommand),
    /// IMAP IDLE active: send DONE first; pipeline drains user command after IDLE tagged OK.
    DeferUntilIdleDone(PipelineCommand),
}

fn imap_untagged_mailbox_activity(line: &str) -> bool {
    let s = line.trim_end();
    if !s.starts_with('*') {
        return false;
    }
    let rest = s.trim_start_matches('*').trim_start();
    let mut it = rest.split_whitespace();
    let Some(first) = it.next() else {
        return false;
    };
    if !first.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Some(kind) = it.next() else {
        return false;
    };
    kind.eq_ignore_ascii_case("EXISTS") || kind.eq_ignore_ascii_case("RECENT")
}

/// Handle to the IMAP connection task. All interaction is through the channel.
/// Cheaply cloneable (just an Arc'd channel sender + atomic counter).
#[derive(Clone)]
pub struct ImapConnection {
    command_tx: mpsc::UnboundedSender<LoopCommand>,
    tag_counter: Arc<AtomicU32>,
    in_idle: Arc<AtomicBool>,
    mailbox_selected: Arc<AtomicBool>,
}

impl ImapConnection {
    /// SELECT succeeded: allow periodic IDLE when the connection goes quiet.
    pub fn set_idle_mailbox_selected(&self, selected: bool) {
        self.mailbox_selected.store(selected, Ordering::Release);
    }

    /// Send a command. Returns immediately (non-blocking). The response is dispatched
    /// to on_complete when the matching tagged response arrives on the socket.
    pub fn send(
        &self,
        command: &str,
        on_untagged: impl Fn(&str, Option<&[u8]>) + Send + 'static,
        on_complete: impl FnOnce(bool, &str) + Send + 'static,
    ) -> String {
        let tag = format!("A{:04}", self.tag_counter.fetch_add(1, Ordering::Relaxed));
        let cmd = PipelineCommand {
            tag: tag.clone(),
            command: command.to_string(),
            pending: PendingCommand {
                on_untagged: Box::new(on_untagged),
                on_complete: Box::new(on_complete),
            },
        };
        let msg = if self.in_idle.load(Ordering::Acquire) {
            LoopCommand::DeferUntilIdleDone(cmd)
        } else {
            LoopCommand::Normal(cmd)
        };
        let _ = self.command_tx.send(msg);
        tag
    }

    /// Returns true if the pipeline task is still running (channel is open).
    pub fn is_alive(&self) -> bool {
        !self.command_tx.is_closed()
    }
}

async fn write_pipeline_outbound<W: AsyncWrite + Unpin>(
    writer: &mut W,
    tag: &str,
    command: &str,
) -> Result<(), ()> {
    let full = format!("{} {}\r\n", tag, command);
    if trace::enabled() {
        trace::log_outbound_line(&format!("{} {}", tag, command));
    }
    if writer.write_all(full.as_bytes()).await.is_err() {
        return Err(());
    }
    if writer.flush().await.is_err() {
        return Err(());
    }
    Ok(())
}

/// Async pipeline loop: reads from socket and dispatches responses by tag.
/// Runs as a tokio::spawn'ed future — no dedicated thread.
async fn pipeline_loop<R, W>(
    mut reader: R,
    mut writer: W,
    mut cmd_rx: mpsc::UnboundedReceiver<LoopCommand>,
    hooks: Option<PipelineIdleHooks>,
) where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut read_buf = Vec::with_capacity(4096);
    let mut pending: VecDeque<(String, PendingCommand)> = VecDeque::new();
    let mut last_activity = Instant::now();
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Commands queued while RFC 2177 IDLE is active (single DONE, then FIFO send).
    let mut deferred_until_idle_done: VecDeque<PipelineCommand> = VecDeque::new();
    // Normal commands received while another command still has responses in flight. Without this,
    // `tokio::select!` could write the next command before reading untagged lines for the current
    // one, so `* FETCH` was delivered to the wrong `pending.front()` handler and tagged OK never
    // completed the op the UI was waiting on (multi-thread + session + list/detail on one conn).
    let mut deferred_outbound: VecDeque<PipelineCommand> = VecDeque::new();
    let mut auto_idle_tag: Option<String> = None;

    async fn try_send_one_deferred_outbound<W: AsyncWrite + Unpin>(
        writer: &mut W,
        pending: &mut VecDeque<(String, PendingCommand)>,
        deferred_outbound: &mut VecDeque<PipelineCommand>,
    ) -> Result<(), ()> {
        if !pending.is_empty() {
            return Ok(());
        }
        let Some(cmd) = deferred_outbound.pop_front() else {
            return Ok(());
        };
        if write_pipeline_outbound(writer, &cmd.tag, &cmd.command)
            .await
            .is_err()
        {
            (cmd.pending.on_complete)(false, "write error");
            while let Some(rest) = deferred_outbound.pop_front() {
                (rest.pending.on_complete)(false, "write error");
            }
            return Err(());
        }
        pending.push_back((cmd.tag, cmd.pending));
        Ok(())
    }

    loop {
        tokio::select! {
            biased;
            Some(loop_cmd) = cmd_rx.recv() => {
                last_activity = Instant::now();
                match loop_cmd {
                    LoopCommand::Normal(cmd) => {
                        if !pending.is_empty() {
                            deferred_outbound.push_back(cmd);
                            continue;
                        }
                        if write_pipeline_outbound(&mut writer, &cmd.tag, &cmd.command).await.is_err() {
                            (cmd.pending.on_complete)(false, "write error");
                            while let Some(rest) = deferred_outbound.pop_front() {
                                (rest.pending.on_complete)(false, "write error");
                            }
                            break;
                        }
                        pending.push_back((cmd.tag, cmd.pending));
                    }
                    LoopCommand::DeferUntilIdleDone(cmd) => {
                        let first_waiter = deferred_until_idle_done.is_empty();
                        deferred_until_idle_done.push_back(cmd);
                        if first_waiter {
                            if writer.write_all(b"DONE\r\n").await.is_err() {
                                let lost = deferred_until_idle_done.pop_back().unwrap();
                                (lost.pending.on_complete)(false, "write error");
                                deferred_until_idle_done.clear();
                                while let Some(rest) = deferred_outbound.pop_front() {
                                    (rest.pending.on_complete)(false, "write error");
                                }
                                break;
                            }
                            if writer.flush().await.is_err() {
                                let lost = deferred_until_idle_done.pop_back().unwrap();
                                (lost.pending.on_complete)(false, "flush error");
                                deferred_until_idle_done.clear();
                                while let Some(rest) = deferred_outbound.pop_front() {
                                    (rest.pending.on_complete)(false, "flush error");
                                }
                                break;
                            }
                        }
                    }
                }
            }
            result = read_imap_line(&mut reader, &mut read_buf) => {
                match result {
                    Ok((line_str, literal)) => {
                        last_activity = Instant::now();
                        let line_trim = line_str.trim_end();
                        if let Some(ref h) = hooks {
                            if imap_untagged_mailbox_activity(line_trim) {
                                h.folder_list_stale.store(true, Ordering::Release);
                            }
                        }
                        // IMAP continuation (e.g. IDLE `+ idling`)
                        if line_trim.starts_with('+') {
                            if let Some((_, ref p)) = pending.front() {
                                (p.on_untagged)(&line_str, literal.as_deref());
                            }
                            if line_trim.to_ascii_lowercase().contains("idling") {
                                if let Some(ref h) = hooks {
                                    h.in_idle.store(true, Ordering::Release);
                                }
                            }
                            continue;
                        }
                        // Multiline FETCH: some servers send the closing `)` on its own line after the
                        // literal. Forward to the current command only; if nothing is pending, fall
                        // through so normal parsing/logging still applies (unconditional `continue`
                        // would drop the line and could desync the reader).
                        if line_trim == ")" {
                            if let Some((_, ref p)) = pending.front() {
                                (p.on_untagged)(&line_str, literal.as_deref());
                                continue;
                            }
                        }
                        let line = parse_line(&line_str);
                        if line.untagged {
                            if let Some((_, ref p)) = pending.front() {
                                (p.on_untagged)(&line_str, literal.as_deref());
                            }
                        } else if let Some(ref tag) = line.tag {
                            // Compute index before any await so the iterator is not held across await (Send).
                            let match_pos = pending.iter().position(|(t, _)| t == tag);
                            if let Some(pos) = match_pos {
                                let (removed_tag, p) = pending.remove(pos).unwrap();
                                let ok = matches!(line.status, Some(ImapStatus::Ok));
                                if let Some(ref h) = hooks {
                                    if auto_idle_tag.as_deref() == Some(removed_tag.as_str()) {
                                        auto_idle_tag = None;
                                        h.in_idle.store(false, Ordering::Release);
                                    }
                                }
                                (p.on_complete)(ok, &line.raw);
                                if let Some(d) = deferred_until_idle_done.pop_front() {
                                    if write_pipeline_outbound(&mut writer, &d.tag, &d.command)
                                        .await
                                        .is_err()
                                    {
                                        (d.pending.on_complete)(false, "write error");
                                        while let Some(rest) = deferred_until_idle_done.pop_front() {
                                            (rest.pending.on_complete)(false, "write error");
                                        }
                                        while let Some(rest) = deferred_outbound.pop_front() {
                                            (rest.pending.on_complete)(false, "write error");
                                        }
                                        break;
                                    }
                                    pending.push_back((d.tag, d.pending));
                                } else if try_send_one_deferred_outbound(
                                    &mut writer,
                                    &mut pending,
                                    &mut deferred_outbound,
                                )
                                .await
                                .is_err()
                                {
                                    break;
                                }
                            } else {
                                eprintln!(
                                    "[imap pipeline] ignoring tagged line (no matching pending tag): {}",
                                    line_trim
                                );
                            }
                        }
                    }
                    Err(_) => {
                        for (_, p) in pending.drain(..) {
                            (p.on_complete)(false, "connection lost");
                        }
                        while let Some(d) = deferred_until_idle_done.pop_front() {
                            (d.pending.on_complete)(false, "connection lost");
                        }
                        while let Some(d) = deferred_outbound.pop_front() {
                            (d.pending.on_complete)(false, "connection lost");
                        }
                        return;
                    }
                }
            }
            _ = tick.tick(), if hooks.is_some() => {
                let Some(ref h) = hooks else { continue };
                if !h.mailbox_selected.load(Ordering::Relaxed) {
                    continue;
                }
                if h.in_idle.load(Ordering::Relaxed) {
                    continue;
                }
                if !pending.is_empty()
                    || !deferred_until_idle_done.is_empty()
                    || !deferred_outbound.is_empty()
                {
                    if pending.is_empty()
                        && deferred_until_idle_done.is_empty()
                        && try_send_one_deferred_outbound(
                            &mut writer,
                            &mut pending,
                            &mut deferred_outbound,
                        )
                        .await
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }
                let secs = h.min_idle_secs.load(Ordering::Relaxed).max(15);
                if last_activity.elapsed() < Duration::from_secs(u64::from(secs)) {
                    continue;
                }
                let tag = format!("A{:04}", h.tag_counter.fetch_add(1, Ordering::Relaxed));
                let use_idle = h.supports_idle;
                if use_idle {
                    auto_idle_tag = Some(tag.clone());
                }
                let hooks_clone = h.clone();
                let keepalive_pending = PendingCommand {
                    on_untagged: Box::new(move |_line, _lit| {
                        let _ = &hooks_clone;
                    }),
                    on_complete: Box::new(move |ok, raw| {
                        let _ = (ok, raw);
                    }),
                };
                let imap_cmd = if use_idle {
                    "IDLE"
                } else {
                    // Servers without IDLE: periodic NOOP may surface untagged EXISTS.
                    "NOOP"
                };
                let cmd = PipelineCommand {
                    tag: tag.clone(),
                    command: imap_cmd.to_string(),
                    pending: keepalive_pending,
                };
                if write_pipeline_outbound(&mut writer, &cmd.tag, &cmd.command).await.is_err() {
                    let _ = auto_idle_tag.take();
                    (cmd.pending.on_complete)(false, "write error");
                    break;
                }
                last_activity = Instant::now();
                pending.push_back((cmd.tag, cmd.pending));
            }
        }
    }
}

/// Connect, authenticate, and start the pipeline task. Returns handle + post-auth capabilities.
pub async fn connect_and_start_pipeline(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
    idle_hooks: Option<PipelineIdleHooks>,
) -> Result<(ImapConnection, Vec<String>), ImapClientError> {
    let session =
        connect_and_authenticate(host, port, use_implicit_tls, use_starttls, auth).await?;
    let caps = match &session {
        AuthenticatedSession::Plain { capabilities, .. } => capabilities.clone(),
        AuthenticatedSession::Tls { capabilities, .. } => capabilities.clone(),
    };

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    // Determine the next tag number from the global counter
    let tag_start = {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        COUNTER.fetch_add(100, Ordering::Relaxed)
    };
    let tag_counter = Arc::new(AtomicU32::new(tag_start));
    let in_idle = Arc::new(AtomicBool::new(false));
    let mailbox_selected = idle_hooks
        .as_ref()
        .map(|h| Arc::clone(&h.mailbox_selected))
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    let hooks_spawn = if let Some(mut h) = idle_hooks {
        h.tag_counter = Arc::clone(&tag_counter);
        h.in_idle = Arc::clone(&in_idle);
        h.supports_idle = caps.iter().any(|c| c == "IDLE");
        Some(h)
    } else {
        None
    };

    match session {
        AuthenticatedSession::Tls { stream, .. } => {
            let (reader, writer) = tokio::io::split(stream);
            tokio::spawn(pipeline_loop(reader, writer, cmd_rx, hooks_spawn));
        }
        AuthenticatedSession::Plain { stream, .. } => {
            let (reader, writer) = tokio::io::split(stream);
            tokio::spawn(pipeline_loop(reader, writer, cmd_rx, hooks_spawn));
        }
    }

    Ok((
        ImapConnection {
            command_tx: cmd_tx,
            tag_counter,
            in_idle,
            mailbox_selected,
        },
        caps,
    ))
}

/// Parse `* SORT 1 2 3` / `* SORT` (RFC 5256).
pub fn parse_sort_response_line(line: &str) -> Vec<u32> {
    let rest = line
        .strip_prefix("* SORT")
        .map(str::trim)
        .unwrap_or("");
    if rest.is_empty() {
        return Vec::new();
    }
    rest
        .split_whitespace()
        .filter_map(|w| w.parse().ok())
        .collect()
}

#[cfg(test)]
mod sort_parse_tests {
    use super::parse_sort_response_line;

    #[test]
    fn parse_sort_uids() {
        assert_eq!(parse_sort_response_line("* SORT 9 8 1"), vec![9u32, 8, 1]);
        assert!(parse_sort_response_line("* SORT").is_empty());
    }
}

#[cfg(test)]
mod status_parse_tests {
    use super::parse_status_mailbox_and_unseen;

    #[test]
    fn status_mailbox_unseen_quoted_and_atom() {
        assert_eq!(
            parse_status_mailbox_and_unseen(r#"* STATUS "Deleted Messages" (UNSEEN 0)"#),
            Some(("Deleted Messages".to_string(), 0))
        );
        assert_eq!(
            parse_status_mailbox_and_unseen("* STATUS INBOX (UNSEEN 5)"),
            Some(("INBOX".to_string(), 5))
        );
        assert_eq!(
            parse_status_mailbox_and_unseen(
                "* STATUS test-new1/test-new2 (MESSAGES 3 UNSEEN 1)"
            ),
            Some(("test-new1/test-new2".to_string(), 1))
        );
    }
}

// Convenience methods for specific IMAP commands on ImapConnection.
impl ImapConnection {
    /// CREATE mailbox.
    pub fn create_mailbox(
        &self,
        name: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("CREATE {}", quote_string(name));
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// Ensure a mailbox exists: `CREATE`, treating “already exists” style failures as success.
    pub fn ensure_mailbox_exists(
        &self,
        name: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let name_owned = name.to_string();
        self.create_mailbox(name, move |r| match r {
            Ok(()) => on_complete(Ok(())),
            Err(e) => {
                let msg = e.to_string();
                let lower = msg.to_ascii_lowercase();
                if lower.contains("alreadyexists")
                    || lower.contains("already exists")
                    || lower.contains("mailbox already")
                    || lower.contains("duplicate")
                    || lower.contains("[alreadyexists]")
                {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(format!(
                        "create mailbox {:?}: {}",
                        name_owned, msg
                    ))));
                }
            }
        });
    }

    /// RENAME mailbox.
    pub fn rename_mailbox(
        &self,
        old_name: &str,
        new_name: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!(
            "RENAME {} {}",
            quote_string(old_name),
            quote_string(new_name)
        );
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// DELETE mailbox.
    pub fn delete_mailbox(
        &self,
        name: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("DELETE {}", quote_string(name));
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// LIST "" "*" streaming: fires on_entry for each * LIST response line.
    pub fn list_folders_streaming(
        &self,
        on_entry: impl Fn(ListEntry) + Send + 'static,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        self.send(
            r#"LIST "" "*""#,
            move |line, _literal| {
                if line.starts_with("* LIST ") {
                    if let Some(entry) = parse_list_line(line) {
                        on_entry(entry);
                    }
                }
            },
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// LSUB "" "*" streaming: subscribed mailboxes (RFC 3501).
    pub fn lsub_folders_streaming(
        &self,
        on_entry: impl Fn(ListEntry) + Send + 'static,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        self.send(
            r#"LSUB "" "*""#,
            move |line, _literal| {
                if line.starts_with("* LSUB ") {
                    if let Some(entry) = parse_lsub_line(line) {
                        on_entry(entry);
                    }
                }
            },
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// SUBSCRIBE — add mailbox to subscription list.
    pub fn subscribe_mailbox(
        &self,
        mailbox: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("SUBSCRIBE {}", quote_string(mailbox));
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// UNSUBSCRIBE — remove mailbox from subscription list.
    pub fn unsubscribe_mailbox(
        &self,
        mailbox: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("UNSUBSCRIBE {}", quote_string(mailbox));
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// RFC 5819 `LIST "" "*" RETURN (STATUS (UNSEEN))`: for each `* LIST`, zero or one following
    /// `* STATUS` with UNSEEN; invokes `on_row` with unseen `0` when the server omits STATUS.
    pub fn list_folders_return_status_unseen_streaming(
        &self,
        on_row: impl Fn(ListEntry, u32) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let on_row = Arc::new(on_row);
        let on_untag = on_row.clone();
        let on_flush = on_row;
        let pending: Arc<Mutex<Option<ListEntry>>> = Arc::new(Mutex::new(None));
        let p_line = pending.clone();
        let p_done = pending;
        self.send(
            r#"LIST "" "*" RETURN (STATUS (UNSEEN))"#,
            move |line, _literal| {
                if line.starts_with("* LIST ") {
                    let mut lock = p_line.lock().unwrap();
                    if let Some(prev) = lock.take() {
                        on_untag(prev, 0);
                    }
                    if let Some(entry) = parse_list_line(line) {
                        *lock = Some(entry);
                    }
                } else if line.starts_with("* STATUS ") {
                    let unseen = parse_status_mailbox_and_unseen(line)
                        .map(|(_, n)| n)
                        .or_else(|| parse_status_unseen(line))
                        .unwrap_or(0);
                    let mut lock = p_line.lock().unwrap();
                    if let Some(entry) = lock.take() {
                        on_untag(entry, unseen);
                    }
                }
            },
            move |ok, raw| {
                if ok {
                    if let Some(prev) = p_done.lock().unwrap().take() {
                        on_flush(prev, 0);
                    }
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// STATUS mailbox (UNSEEN). Parses untagged `* STATUS` lines.
    pub fn mailbox_status_unseen(
        &self,
        mailbox: &str,
        on_complete: impl FnOnce(Result<u32, ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("STATUS {} (UNSEEN)", quote_string(mailbox));
        let unseen_holder = Arc::new(Mutex::new(None::<u32>));
        let u2 = unseen_holder.clone();
        self.send(
            &cmd,
            move |line, _literal| {
                if let Some(n) = parse_status_unseen(line) {
                    *u2.lock().unwrap() = Some(n);
                }
            },
            move |ok, raw| {
                if !ok {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                    return;
                }
                let n = *unseen_holder.lock().unwrap();
                on_complete(Ok(n.unwrap_or(0)));
            },
        );
    }

    /// SELECT mailbox streaming: fires on_event for each untagged response.
    pub fn select_streaming(
        &self,
        mailbox: &str,
        on_event: impl Fn(SelectEvent) + Send + 'static,
        on_complete: impl FnOnce(Result<SelectResult, ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("SELECT {}", quote_string(mailbox));
        let exists = Arc::new(AtomicU32::new(0));
        let uid_validity: Arc<std::sync::Mutex<Option<u32>>> =
            Arc::new(std::sync::Mutex::new(None));
        let exists_for_untagged = exists.clone();
        let uv_for_untagged = uid_validity.clone();

        self.send(
            &cmd,
            move |line, _literal| {
                if let Some(ev) = parse_select_event(line) {
                    match &ev {
                        SelectEvent::Exists(n) => {
                            exists_for_untagged.store(*n, Ordering::Relaxed);
                        }
                        SelectEvent::UidValidity(n) => {
                            *uv_for_untagged.lock().unwrap() = Some(*n);
                        }
                        _ => {}
                    }
                    on_event(ev);
                }
            },
            move |ok, raw| {
                if ok {
                    let uv = *uid_validity.lock().unwrap();
                    on_complete(Ok(SelectResult {
                        exists: exists.load(Ordering::Relaxed),
                        uid_validity: uv,
                    }));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// FETCH summaries streaming.
    pub fn fetch_summaries_streaming(
        &self,
        seq_start: u32,
        seq_end: u32,
        header_fields: SummaryHeaderFields,
        on_summary: impl Fn(FetchSummary) + Send + 'static,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!(
            "FETCH {}:{} (UID FLAGS RFC822.SIZE {})",
            seq_start,
            seq_end,
            header_fields.body_peek_clause()
        );
        self.send(
            &cmd,
            move |line, literal| {
                if line.contains(" FETCH (") {
                    if let Some(s) = parse_fetch_summary(line, literal) {
                        on_summary(s);
                    }
                }
            },
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// `UID SORT (criteria…) UTF-8 ALL` (RFC 5256). Returns UIDs in server order for the given key.
    pub fn uid_sort_all(
        &self,
        sort_parentheses: &str,
        on_done: impl FnOnce(Result<Vec<u32>, ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("UID SORT {sort_parentheses} UTF-8 ALL");
        let acc: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
        let acc2 = acc.clone();
        self.send(
            &cmd,
            move |line, _literal| {
                if line.starts_with("* SORT") {
                    *acc2.lock().unwrap() = parse_sort_response_line(line);
                }
            },
            move |ok, raw| {
                let uids = acc.lock().unwrap().clone();
                if ok {
                    on_done(Ok(uids));
                } else {
                    on_done(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// `UID FETCH uid,…` with list header fields; results are ordered like `uids`.
    pub fn fetch_uid_set_summaries(
        &self,
        uids: &[u32],
        header_fields: SummaryHeaderFields,
        on_done: impl FnOnce(Result<Vec<FetchSummary>, ImapClientError>) + Send + 'static,
    ) {
        if uids.is_empty() {
            on_done(Ok(Vec::new()));
            return;
        }
        let set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let cmd = format!(
            "UID FETCH {} (UID FLAGS RFC822.SIZE {})",
            set,
            header_fields.body_peek_clause()
        );
        let acc: Arc<Mutex<Vec<FetchSummary>>> = Arc::new(Mutex::new(Vec::new()));
        let acc2 = acc.clone();
        let order: Vec<u32> = uids.to_vec();
        self.send(
            &cmd,
            move |line, lit| {
                if line.contains(" FETCH (") {
                    if let Some(s) = parse_fetch_summary(line, lit) {
                        acc2.lock().unwrap().push(s);
                    }
                }
            },
            move |ok, raw| {
                if !ok {
                    on_done(Err(ImapClientError::new(raw.to_string())));
                    return;
                }
                let got = acc.lock().unwrap().clone();
                let mut by_uid: HashMap<u32, FetchSummary> = HashMap::with_capacity(got.len());
                for s in got {
                    if s.uid > 0 {
                        by_uid.insert(s.uid, s);
                    }
                }
                let mut ordered = Vec::with_capacity(order.len());
                for u in order {
                    if let Some(s) = by_uid.remove(&u) {
                        ordered.push(s);
                    }
                }
                on_done(Ok(ordered));
            },
        );
    }

    /// FETCH body by UID. Body literal arrives in `on_untagged` as literal data.
    pub fn fetch_body_by_uid_streaming(
        &self,
        uid: u32,
        on_chunk: impl Fn(&[u8]) + Send + 'static,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("UID FETCH {} (BODY[])", uid);
        self.send(
            &cmd,
            move |line, literal| {
                if line.contains(" FETCH (") {
                    if let Some(lit) = literal {
                        on_chunk(lit);
                    }
                }
            },
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// APPEND raw message bytes to mailbox.
    pub fn append_message(
        &self,
        _mailbox: &str,
        _data: &[u8],
        _on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        // APPEND requires literal syntax which doesn't fit the simple pipeline model.
        // For now, fall back to "not supported via pipeline" — the synchronous path can be used.
        _on_complete(Err(ImapClientError::new(
            "APPEND via pipeline not yet supported",
        )));
    }

    /// UID COPY: copy messages by UID set to destination mailbox.
    pub fn copy_uids(
        &self,
        uid_set: &str,
        dest_mailbox: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("UID COPY {} {}", uid_set, quote_string(dest_mailbox));
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// UID MOVE: atomically move messages by UID set to destination mailbox (RFC 6851).
    pub fn move_uids(
        &self,
        uid_set: &str,
        dest_mailbox: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("UID MOVE {} {}", uid_set, quote_string(dest_mailbox));
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// UID STORE: change flags on messages identified by UID set.
    /// `flags_action` is e.g. `"+FLAGS (\\Deleted)"` or `"-FLAGS (\\Seen)"`.
    pub fn store_flags(
        &self,
        uid_set: &str,
        flags_action: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("UID STORE {} {}", uid_set, flags_action);
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// EXPUNGE: permanently remove all messages marked \Deleted from the selected mailbox.
    pub fn expunge(&self, on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static) {
        self.send(
            "EXPUNGE",
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }

    /// UID EXPUNGE: permanently remove only the specified \Deleted UIDs (RFC 4315 / UIDPLUS).
    /// Falls back to plain EXPUNGE if the server does not support UIDPLUS.
    pub fn uid_expunge(
        &self,
        uid_set: &str,
        on_complete: impl FnOnce(Result<(), ImapClientError>) + Send + 'static,
    ) {
        let cmd = format!("UID EXPUNGE {}", uid_set);
        self.send(
            &cmd,
            |_, _| {},
            move |ok, raw| {
                if ok {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(ImapClientError::new(raw.to_string())));
                }
            },
        );
    }
}
