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

use crate::net::{connect_implicit_tls, connect_plain, PlainStream, TlsStreamWrapper};
use crate::sasl::{
    initial_client_response, login_respond_to_challenge, respond_to_challenge, SaslError,
    SaslFirst, SaslMechanism,
};
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// IMAP client error (network, protocol, auth).
#[derive(Debug)]
pub struct ImapClientError {
    pub message: String,
}

impl ImapClientError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
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
async fn read_imap_line<S>(stream: &mut S, buf: &mut Vec<u8>) -> io::Result<(String, Option<Vec<u8>>)>
where
    S: AsyncRead + Unpin,
{
    let (line, literal_size) = read_imap_line_literal_size(stream, buf).await?;
    if let Some(n) = literal_size {
        let mut lit = vec![0u8; n as usize];
        stream.read_exact(&mut lit).await?;
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
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
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
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
        }
        on_chunk(&buf[..n]);
        remaining -= n;
        if buf.len() > remaining {
            buf.truncate(remaining);
        }
    }
    Ok(())
}

/// Write a line (no CRLF) then CRLF.
async fn write_line<S>(stream: &mut S, line: &[u8]) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
{
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
            let n: u32 = after.split_whitespace().next()?.trim_end_matches(']').parse().ok()?;
            return Some(SelectEvent::UidValidity(n));
        }
        if let Some(bracket) = rest.find("[UIDNEXT ") {
            let after = &rest[bracket + 9..];
            let n: u32 = after.split_whitespace().next()?.trim_end_matches(']').parse().ok()?;
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

/// Check if capability string contains STARTTLS.
fn has_starttls(capabilities: &[String]) -> bool {
    capabilities.iter().any(|c| c.eq_ignore_ascii_case("STARTTLS"))
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
            lwl.1.as_ref()
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
) -> Result<(Vec<String>, String), ImapClientError> {
    let caps = ensure_capabilities(stream, read_buf, Some(&greeting_line)).await?;
    if let Some((user, pass, mechanism)) = auth {
        if server_supports_auth(&caps, mechanism) {
            auth_sasl(stream, read_buf, mechanism, user, pass).await?;
        } else {
            login_plain(stream, read_buf, user, pass).await?;
        }
    }
    let tag = next_tag();
    let (_u, final_line) = send_command(stream, read_buf, &tag, "CAPABILITY").await?;
    let _ = (_u, final_line);
    Ok((caps, greeting_line.to_string()))
}

fn server_supports_auth(caps: &[String], mechanism: SaslMechanism) -> bool {
    caps.iter().any(|c| c == &format!("AUTH={}", mechanism.name()))
}

/// Read greeting line (* OK ...).
async fn read_greeting<S>(stream: &mut S, read_buf: &mut Vec<u8>) -> Result<String, ImapClientError>
where
    S: AsyncRead + Unpin,
{
    let (line, _lit) = read_imap_line(stream, read_buf).await?;
    if !line.starts_with("* OK") && !line.starts_with("* PREECH") {
        return Err(ImapClientError::new(format!("expected * OK greeting, got: {}", line)));
    }
    Ok(line)
}

/// Connect and authenticate. Returns session for LIST, SELECT, FETCH.
/// use_starttls: if true and server advertises STARTTLS, we upgrade (default); set false for debug.
pub async fn connect_and_authenticate(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
) -> Result<AuthenticatedSession, ImapClientError> {

    if use_implicit_tls {
        let mut stream = connect_implicit_tls(host, port).await?;
        let mut read_buf = Vec::with_capacity(4096);
        let greeting = read_greeting(&mut stream, &mut read_buf).await?;
        let (caps, _) = run_authenticated_tls(&mut stream, &mut read_buf, &greeting, auth).await?;
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
        return Ok(AuthenticatedSession::Tls {
            stream: tls,
            read_buf,
            host: host.to_string(),
            capabilities: caps2,
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
    Ok(AuthenticatedSession::Plain {
        stream: plain,
        read_buf,
        host: host.to_string(),
        capabilities: caps,
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
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, r#"LIST "" "*""#).await?
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, r#"LIST "" "*""#).await?
            }
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
    pub async fn list_folders_streaming<F>(&mut self, mut on_entry: F) -> Result<(), ImapClientError>
    where
        F: FnMut(ListEntry),
    {
        let tag = next_tag();
        let cmd = r#"LIST "" "*""#;
        match self {
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                list_folders_streaming_impl(stream, read_buf, &tag, cmd, &mut on_entry).await
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                list_folders_streaming_impl(stream, read_buf, &tag, cmd, &mut on_entry).await
            }
        }
    }

    /// SELECT mailbox; returns exists (message count) and optional UIDVALIDITY.
    pub async fn select(&mut self, mailbox: &str) -> Result<SelectResult, ImapClientError> {
        let tag = next_tag();
        let cmd = format!("SELECT {}", quote_string(mailbox));
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, &cmd).await?
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, &cmd).await?
            }
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
                        let num = after.split_whitespace().next().and_then(|s| s.trim_end_matches(']').parse().ok());
                        if let Some(n) = num {
                            uid_validity = Some(n);
                        }
                    }
                }
            }
        }
        Ok(SelectResult { exists, uid_validity })
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
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                select_streaming_impl(stream, read_buf, &tag, &cmd, &mut on_event, &mut exists, &mut uid_validity).await?;
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                select_streaming_impl(stream, read_buf, &tag, &cmd, &mut on_event, &mut exists, &mut uid_validity).await?;
            }
        }
        Ok(SelectResult { exists, uid_validity })
    }

    /// FETCH sequence range for envelope summaries (UID, FLAGS, RFC822.SIZE, header fields).
    pub async fn fetch_summaries(
        &mut self,
        seq_start: u32,
        seq_end: u32,
    ) -> Result<Vec<FetchSummary>, ImapClientError> {
        let tag = next_tag();
        let cmd = format!(
            "FETCH {}:{} (UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (FROM TO CC SUBJECT DATE MESSAGE-ID REFERENCES IN-REPLY-TO)])",
            seq_start, seq_end
        );
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, &cmd).await?
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, &cmd).await?
            }
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
        mut on_summary: F,
    ) -> Result<(), ImapClientError>
    where
        F: FnMut(FetchSummary),
    {
        let tag = next_tag();
        let cmd = format!(
            "FETCH {}:{} (UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (FROM TO CC SUBJECT DATE MESSAGE-ID REFERENCES IN-REPLY-TO)])",
            seq_start, seq_end
        );
        match self {
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                fetch_summaries_streaming_impl(stream, read_buf, &tag, &cmd, &mut on_summary).await
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                fetch_summaries_streaming_impl(stream, read_buf, &tag, &cmd, &mut on_summary).await
            }
        }
    }

    /// FETCH one message by UID (full BODY[]). Use after SELECT.
    pub async fn fetch_body_by_uid(&mut self, uid: u32) -> Result<Vec<u8>, ImapClientError> {
        let tag = next_tag();
        let cmd = format!("UID FETCH {} (BODY[])", uid);
        let (untagged, final_line) = match self {
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, &cmd).await?
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                send_command(stream, read_buf, &tag, &cmd).await?
            }
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
            AuthenticatedSession::Plain { stream, read_buf, .. } => {
                fetch_body_streaming_impl(stream, read_buf, &tag, &cmd, chunk_size, &mut on_chunk).await
            }
            AuthenticatedSession::Tls { stream, read_buf, .. } => {
                fetch_body_streaming_impl(stream, read_buf, &tag, &cmd, chunk_size, &mut on_chunk).await
            }
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
#[derive(Debug)]
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
        flags = rest[..end].split_whitespace().map(|s| s.to_string()).collect();
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

fn parse_list_line(line: &str) -> Option<ListEntry> {
    let rest = line.strip_prefix("* LIST ")?.trim_start();
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
