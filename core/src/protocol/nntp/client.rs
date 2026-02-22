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

//! Async NNTP client: connect, CAPABILITIES, STARTTLS, AUTHINFO USER/PASS,
//! LIST ACTIVE, GROUP, OVER, ARTICLE, HEAD, POST.
//! Pipeline is simpler than IMAP: NNTP is strictly sequential (no tags).

use crate::net::{connect_implicit_tls, connect_plain, PlainStream, TlsStreamWrapper};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct NntpClientError {
    pub message: String,
}

impl NntpClientError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

impl std::fmt::Display for NntpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NntpClientError {}

impl From<io::Error> for NntpClientError {
    fn from(e: io::Error) -> Self {
        Self::new(e.to_string())
    }
}

/// Parsed NNTP status line: 3-digit code + rest of line.
#[derive(Debug, Clone)]
pub struct NntpStatus {
    pub code: u16,
    pub text: String,
}

impl NntpStatus {
    #[allow(dead_code)]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.code)
    }
}

fn parse_status(line: &str) -> Option<NntpStatus> {
    if line.len() < 3 {
        return None;
    }
    let code: u16 = line[..3].parse().ok()?;
    let text = if line.len() > 4 { line[4..].to_string() } else { String::new() };
    Some(NntpStatus { code, text })
}

/// Whether a status code indicates a multi-line response follows.
fn is_multiline_response(code: u16) -> bool {
    matches!(code, 100 | 101 | 215 | 220 | 221 | 222 | 224 | 225 | 230 | 231)
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
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
        }
        buf.push(b[0]);
        if buf.len() >= 2 && buf[buf.len() - 2..] == *b"\r\n" {
            break;
        }
    }
    let line_end = buf.len() - 2;
    Ok(String::from_utf8_lossy(&buf[..line_end]).to_string())
}

async fn write_line<S>(stream: &mut S, line: &[u8]) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
{
    stream.write_all(line).await?;
    stream.write_all(b"\r\n").await?;
    stream.flush().await?;
    Ok(())
}

/// Read a multi-line response (terminated by lone "."), calling on_line for each data line.
/// Performs dot-unstuffing (leading ".." -> ".").
async fn read_multiline<S, F>(stream: &mut S, buf: &mut Vec<u8>, mut on_line: F) -> io::Result<()>
where
    S: AsyncRead + Unpin,
    F: FnMut(&str),
{
    loop {
        let line = read_line(stream, buf).await?;
        if line == "." {
            return Ok(());
        }
        let data = if line.starts_with("..") { &line[1..] } else { &line };
        on_line(data);
    }
}

/// Parse CAPABILITIES response lines into a list of capability tokens.
fn parse_capabilities(lines: &[String]) -> Vec<String> {
    lines.iter().map(|l| l.trim().to_uppercase()).collect()
}

fn has_starttls(caps: &[String]) -> bool {
    caps.iter().any(|c| c == "STARTTLS")
}

fn has_authinfo(caps: &[String]) -> bool {
    caps.iter().any(|c| c.starts_with("AUTHINFO"))
}

async fn read_greeting<S>(stream: &mut S, buf: &mut Vec<u8>) -> Result<NntpStatus, NntpClientError>
where
    S: AsyncRead + Unpin,
{
    let line = read_line(stream, buf).await?;
    let status = parse_status(&line)
        .ok_or_else(|| NntpClientError::new(format!("invalid NNTP greeting: {}", line)))?;
    if status.code != 200 && status.code != 201 {
        return Err(NntpClientError::new(format!("NNTP greeting error: {}", line)));
    }
    Ok(status)
}

/// Send a command and read the status line response.
async fn send_command<S>(
    stream: &mut S,
    buf: &mut Vec<u8>,
    command: &str,
) -> Result<NntpStatus, NntpClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    write_line(stream, command.as_bytes()).await?;
    let line = read_line(stream, buf).await?;
    parse_status(&line).ok_or_else(|| NntpClientError::new(format!("bad status: {}", line)))
}

/// Send CAPABILITIES, read multi-line response.
async fn fetch_capabilities<S>(
    stream: &mut S,
    buf: &mut Vec<u8>,
) -> Result<Vec<String>, NntpClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    write_line(stream, b"CAPABILITIES").await?;
    let line = read_line(stream, buf).await?;
    let status = parse_status(&line)
        .ok_or_else(|| NntpClientError::new(format!("bad CAPABILITIES response: {}", line)))?;
    if status.code != 101 {
        return Ok(Vec::new());
    }
    let mut lines = Vec::new();
    read_multiline(stream, buf, |l| lines.push(l.to_string())).await?;
    Ok(parse_capabilities(&lines))
}

/// Authenticate with AUTHINFO USER/PASS (RFC 4643).
async fn authinfo<S>(
    stream: &mut S,
    buf: &mut Vec<u8>,
    user: &str,
    pass: &str,
) -> Result<(), NntpClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let user_cmd = format!("AUTHINFO USER {}", user);
    let status = send_command(stream, buf, &user_cmd).await?;
    match status.code {
        281 => return Ok(()), // authenticated (some servers accept user-only)
        381 => {} // password required
        _ => return Err(NntpClientError::new(format!("AUTHINFO USER failed: {} {}", status.code, status.text))),
    }
    let pass_cmd = format!("AUTHINFO PASS {}", pass);
    let status = send_command(stream, buf, &pass_cmd).await?;
    if status.code == 281 {
        Ok(())
    } else {
        Err(NntpClientError::new(format!("AUTHINFO PASS failed: {} {}", status.code, status.text)))
    }
}

/// Authenticated NNTP session (plain or TLS).
pub enum AuthenticatedSession {
    Plain {
        stream: PlainStream,
        read_buf: Vec<u8>,
        capabilities: Vec<String>,
        posting_allowed: bool,
    },
    Tls {
        stream: TlsStreamWrapper,
        read_buf: Vec<u8>,
        capabilities: Vec<String>,
        posting_allowed: bool,
    },
}

/// Connect and authenticate. Returns session for LIST, GROUP, OVER, ARTICLE.
pub async fn connect_and_authenticate(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str)>,
) -> Result<AuthenticatedSession, NntpClientError> {
    if use_implicit_tls {
        let mut stream = connect_implicit_tls(host, port).await?;
        let mut buf = Vec::with_capacity(4096);
        let greeting = read_greeting(&mut stream, &mut buf).await?;
        let posting_allowed = greeting.code == 200;
        let caps = fetch_capabilities(&mut stream, &mut buf).await?;
        if let Some((user, pass)) = auth {
            authinfo(&mut stream, &mut buf, user, pass).await?;
        }
        return Ok(AuthenticatedSession::Tls { stream, read_buf: buf, capabilities: caps, posting_allowed });
    }

    let mut plain = connect_plain(host, port).await?;
    let mut buf = Vec::with_capacity(4096);
    let greeting = read_greeting(&mut plain, &mut buf).await?;
    let posting_allowed = greeting.code == 200;
    let caps = fetch_capabilities(&mut plain, &mut buf).await?;

    if has_starttls(&caps) && use_starttls {
        let status = send_command(&mut plain, &mut buf, "STARTTLS").await?;
        if status.code != 382 {
            return Err(NntpClientError::new(format!("STARTTLS failed: {} {}", status.code, status.text)));
        }
        let mut tls = plain.upgrade_to_tls(host).await?;
        let caps2 = fetch_capabilities(&mut tls, &mut buf).await?;
        if let Some((user, pass)) = auth {
            authinfo(&mut tls, &mut buf, user, pass).await?;
        }
        return Ok(AuthenticatedSession::Tls { stream: tls, read_buf: buf, capabilities: caps2, posting_allowed });
    }

    if let Some((user, pass)) = auth {
        if has_authinfo(&caps) {
            authinfo(&mut plain, &mut buf, user, pass).await?;
        }
    }
    Ok(AuthenticatedSession::Plain { stream: plain, read_buf: buf, capabilities: caps, posting_allowed })
}

// ======================================================================
// NNTP Pipeline (event-driven, sequential)
// ======================================================================

struct PendingCommand {
    expects_multiline: bool,
    on_line: Box<dyn Fn(&str) + Send + Sync>,
    on_complete: Box<dyn FnOnce(u16, &str) + Send>,
}

struct PipelineCommand {
    command: String,
    pending: PendingCommand,
}

/// Handle to the NNTP connection task. All interaction is through the channel.
/// Cheaply cloneable (just an Arc'd channel sender).
#[derive(Clone)]
pub struct NntpConnection {
    command_tx: mpsc::UnboundedSender<PipelineCommand>,
    posting_allowed: bool,
}

impl NntpConnection {
    pub fn send(
        &self,
        command: &str,
        expects_multiline: bool,
        on_line: impl Fn(&str) + Send + Sync + 'static,
        on_complete: impl FnOnce(u16, &str) + Send + 'static,
    ) {
        let _ = self.command_tx.send(PipelineCommand {
            command: command.to_string(),
            pending: PendingCommand {
                expects_multiline,
                on_line: Box::new(on_line),
                on_complete: Box::new(on_complete),
            },
        });
    }

    /// Send raw data (for POST body). No status line is read; the pending command
    /// from the initial POST will handle the final response.
    pub fn send_raw(
        &self,
        data: &str,
        on_complete: impl FnOnce(u16, &str) + Send + 'static,
    ) {
        let _ = self.command_tx.send(PipelineCommand {
            command: data.to_string(),
            pending: PendingCommand {
                expects_multiline: false,
                on_line: Box::new(|_| {}),
                on_complete: Box::new(on_complete),
            },
        });
    }

    pub fn is_alive(&self) -> bool {
        !self.command_tx.is_closed()
    }

    pub fn posting_allowed(&self) -> bool {
        self.posting_allowed
    }
}

/// Async pipeline loop for NNTP. NNTP is sequential: one command at a time.
async fn pipeline_loop<R, W>(
    mut reader: R,
    mut writer: W,
    mut cmd_rx: mpsc::UnboundedReceiver<PipelineCommand>,
)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut read_buf = Vec::with_capacity(4096);

    while let Some(cmd) = cmd_rx.recv().await {
        // Write command
        let full = format!("{}\r\n", cmd.command);
        if writer.write_all(full.as_bytes()).await.is_err() {
            (cmd.pending.on_complete)(0, "write error");
            break;
        }
        if writer.flush().await.is_err() {
            (cmd.pending.on_complete)(0, "flush error");
            break;
        }

        // Read status line
        let line = match read_line(&mut reader, &mut read_buf).await {
            Ok(l) => l,
            Err(_) => {
                (cmd.pending.on_complete)(0, "connection lost");
                break;
            }
        };
        let status = match parse_status(&line) {
            Some(s) => s,
            None => {
                (cmd.pending.on_complete)(0, &line);
                continue;
            }
        };

        // If multi-line response expected and status indicates it, read data lines
        if cmd.pending.expects_multiline && is_multiline_response(status.code) {
            if read_multiline(&mut reader, &mut read_buf, |l| (cmd.pending.on_line)(l)).await.is_err() {
                (cmd.pending.on_complete)(0, "connection lost during multiline");
                break;
            }
        }

        (cmd.pending.on_complete)(status.code, &status.text);
    }
}

/// Connect, authenticate, and start the pipeline task. Returns an NntpConnection handle.
pub async fn connect_and_start_pipeline(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str)>,
) -> Result<NntpConnection, NntpClientError> {
    let session = connect_and_authenticate(host, port, use_implicit_tls, use_starttls, auth).await?;

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    let posting_allowed = match &session {
        AuthenticatedSession::Plain { posting_allowed, .. } => *posting_allowed,
        AuthenticatedSession::Tls { posting_allowed, .. } => *posting_allowed,
    };

    match session {
        AuthenticatedSession::Tls { stream, .. } => {
            let (reader, writer) = tokio::io::split(stream);
            tokio::spawn(pipeline_loop(reader, writer, cmd_rx));
        }
        AuthenticatedSession::Plain { stream, .. } => {
            let (reader, writer) = tokio::io::split(stream);
            tokio::spawn(pipeline_loop(reader, writer, cmd_rx));
        }
    }

    Ok(NntpConnection { command_tx: cmd_tx, posting_allowed })
}

// ======================================================================
// Parsed protocol data types
// ======================================================================

/// Newsgroup entry from LIST ACTIVE.
#[derive(Debug, Clone)]
pub struct NewsgroupEntry {
    pub name: String,
    pub high: u64,
    pub low: u64,
    pub status: char,
}

/// Result of GROUP command.
#[derive(Debug, Clone)]
pub struct GroupResult {
    pub count: u64,
    pub first: u64,
    pub last: u64,
    pub name: String,
}

/// Article overview from OVER response (tab-separated fields).
#[derive(Debug, Clone)]
pub struct OverviewEntry {
    pub article_number: u64,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub message_id: String,
    pub references: String,
    pub bytes: u64,
    pub lines: u64,
}

fn parse_overview_line(line: &str) -> Option<OverviewEntry> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() < 8 {
        return None;
    }
    Some(OverviewEntry {
        article_number: fields[0].parse().ok()?,
        subject: fields[1].to_string(),
        from: fields[2].to_string(),
        date: fields[3].to_string(),
        message_id: fields[4].to_string(),
        references: fields[5].to_string(),
        bytes: fields[6].parse().unwrap_or(0),
        lines: fields[7].parse().unwrap_or(0),
    })
}

fn parse_newsgroup_line(line: &str) -> Option<NewsgroupEntry> {
    let mut parts = line.split_whitespace();
    let name = parts.next()?.to_string();
    let high: u64 = parts.next()?.parse().ok()?;
    let low: u64 = parts.next()?.parse().ok()?;
    let status = parts.next()?.chars().next().unwrap_or('y');
    Some(NewsgroupEntry { name, high, low, status })
}

fn parse_group_response(text: &str) -> Option<GroupResult> {
    let mut parts = text.split_whitespace();
    let count: u64 = parts.next()?.parse().ok()?;
    let first: u64 = parts.next()?.parse().ok()?;
    let last: u64 = parts.next()?.parse().ok()?;
    let name = parts.next()?.to_string();
    Some(GroupResult { count, first, last, name })
}

// ======================================================================
// Convenience methods on NntpConnection
// ======================================================================

impl NntpConnection {
    /// LIST ACTIVE: list all newsgroups.
    pub fn list_newsgroups_streaming(
        &self,
        on_entry: impl Fn(NewsgroupEntry) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        self.send(
            "LIST ACTIVE",
            true,
            move |line| {
                if let Some(entry) = parse_newsgroup_line(line) {
                    on_entry(entry);
                }
            },
            move |code, text| {
                if code == 215 {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(NntpClientError::new(format!("LIST ACTIVE failed: {} {}", code, text))));
                }
            },
        );
    }

    /// GROUP: select a newsgroup.
    pub fn group(
        &self,
        name: &str,
        on_complete: impl FnOnce(Result<GroupResult, NntpClientError>) + Send + 'static,
    ) {
        let cmd = format!("GROUP {}", name);
        self.send(
            &cmd,
            false,
            |_| {},
            move |code, text| {
                if code == 211 {
                    match parse_group_response(text) {
                        Some(result) => on_complete(Ok(result)),
                        None => on_complete(Err(NntpClientError::new(format!("bad GROUP response: {}", text)))),
                    }
                } else {
                    on_complete(Err(NntpClientError::new(format!("GROUP failed: {} {}", code, text))));
                }
            },
        );
    }

    /// OVER: fetch article overviews for a range.
    pub fn over_streaming(
        &self,
        first: u64,
        last: u64,
        on_entry: impl Fn(OverviewEntry) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        let cmd = format!("OVER {}-{}", first, last);
        self.send(
            &cmd,
            true,
            move |line| {
                if let Some(entry) = parse_overview_line(line) {
                    on_entry(entry);
                }
            },
            move |code, text| {
                if code == 224 {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(NntpClientError::new(format!("OVER failed: {} {}", code, text))));
                }
            },
        );
    }

    /// ARTICLE: fetch full article by number.
    pub fn article_streaming(
        &self,
        number: u64,
        on_line: impl Fn(&str) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        let cmd = format!("ARTICLE {}", number);
        self.send(
            &cmd,
            true,
            on_line,
            move |code, text| {
                if code == 220 {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(NntpClientError::new(format!("ARTICLE failed: {} {}", code, text))));
                }
            },
        );
    }

    /// HEAD: fetch article headers only.
    pub fn head_streaming(
        &self,
        number: u64,
        on_line: impl Fn(&str) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        let cmd = format!("HEAD {}", number);
        self.send(
            &cmd,
            true,
            on_line,
            move |code, text| {
                if code == 221 {
                    on_complete(Ok(()));
                } else {
                    on_complete(Err(NntpClientError::new(format!("HEAD failed: {} {}", code, text))));
                }
            },
        );
    }

    /// POST: post an article. `article_data` is the full article including headers,
    /// terminated by a lone "." line. The caller must dot-stuff lines starting with ".".
    pub fn post(
        &self,
        article_data: &str,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        let data = article_data.to_string();
        let conn = self.clone();
        self.send(
            "POST",
            false,
            |_| {},
            move |code, text| {
                if code == 340 {
                    // Server ready for article data
                    conn.send_raw(
                        &data,
                        move |code2, text2| {
                            if code2 == 240 {
                                on_complete(Ok(()));
                            } else {
                                on_complete(Err(NntpClientError::new(
                                    format!("POST rejected: {} {}", code2, text2),
                                )));
                            }
                        },
                    );
                } else {
                    on_complete(Err(NntpClientError::new(
                        format!("POST not allowed: {} {}", code, text),
                    )));
                }
            },
        );
    }
}
