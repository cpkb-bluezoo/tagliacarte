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

//! Async SMTP client: connect, EHLO, STARTTLS, AUTH, MAIL FROM, RCPT TO, DATA/BDAT, QUIT.
//! Ported from gumdrop SMTPClientConnection; uses core/net and core/sasl.

use crate::net::{connect_implicit_tls, connect_plain, PlainStream, TlsStreamWrapper};
use crate::protocol::smtp::dot_stuffer::DotStuffer;
use crate::sasl::{
    initial_client_response, login_respond_to_challenge, respond_to_challenge, SaslError,
    SaslFirst, SaslMechanism,
};
use crate::store::{Address, Envelope};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// SMTP client error (network, protocol, auth).
#[derive(Debug)]
pub struct SmtpClientError {
    pub message: String,
}

impl SmtpClientError {
    fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

impl std::fmt::Display for SmtpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SmtpClientError {}

impl From<io::Error> for SmtpClientError {
    fn from(e: io::Error) -> Self {
        Self::new(e.to_string())
    }
}

impl From<SaslError> for SmtpClientError {
    fn from(e: SaslError) -> Self {
        Self::new(e.to_string())
    }
}

/// Parsed SMTP response (code + lines).
struct SmtpResponse {
    code: u16,
    lines: Vec<String>,
}

impl SmtpResponse {
    fn message(&self) -> &str {
        self.lines.last().map(|s| s.as_str()).unwrap_or("")
    }

    fn is_success(&self) -> bool {
        (200..300).contains(&self.code)
    }
}

/// Format envelope address for MAIL FROM / RCPT TO: local_part@domain or local_part.
fn envelope_address(addr: &Address) -> String {
    match &addr.domain {
        Some(d) => format!("{}@{}", addr.local_part, d),
        None => addr.local_part.clone(),
    }
}

/// Read one SMTP response (single line or multi-line) from stream.
async fn read_response<S>(stream: &mut S, buf: &mut Vec<u8>) -> io::Result<SmtpResponse>
where
    S: AsyncRead + Unpin,
{
    buf.clear();
    let mut lines = Vec::new();
    loop {
        while buf.len() < 2 || buf[buf.len().saturating_sub(2)..] != *b"\r\n" {
            let mut b = [0u8; 1];
            let n = stream.read(&mut b).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed",
                ));
            }
            buf.push(b[0]);
        }
        let line_end = buf.len() - 2;
        let line_start = buf
            .iter()
            .position(|&c| c != b'\r' && c != b'\n')
            .unwrap_or(line_end);
        let line = String::from_utf8_lossy(&buf[line_start..line_end]).trim().to_string();
        if line.len() >= 4 {
            let code: u16 = line[..3].parse().unwrap_or(0);
            let continuation = line.as_bytes().get(3) == Some(&b'-');
            let text = if line.len() > 4 {
                line[4..].trim()
            } else {
                ""
            };
            lines.push(text.to_string());
            if !continuation {
                return Ok(SmtpResponse { code, lines });
            }
        }
    }
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

/// Send EHLO, return (starttls, auth_methods, chunking).
async fn ehlo<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    hostname: &str,
) -> Result<(bool, Vec<String>, bool), SmtpClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let cmd = format!("EHLO {}", hostname);
    write_line(stream, cmd.as_bytes()).await?;
    let r = read_response(stream, read_buf).await?;
    if r.code == 502 {
        return Ok((false, Vec::new(), false));
    }
    if !r.is_success() {
        return Err(SmtpClientError::new(format!(
            "EHLO failed: {} {}",
            r.code,
            r.message()
        )));
    }
    let mut starttls = false;
    let mut auth_methods = Vec::new();
    let mut chunking = false;
    for line in &r.lines {
        let upper = line.to_uppercase();
        if upper == "STARTTLS" {
            starttls = true;
        } else if upper.starts_with("AUTH ") {
            for word in line[4..].split_whitespace() {
                auth_methods.push(word.to_uppercase());
            }
        } else if upper == "CHUNKING" {
            chunking = true;
        }
    }
    Ok((starttls, auth_methods, chunking))
}

/// Pick mechanism if server advertises it.
fn server_supports(auth_methods: &[String], mechanism: SaslMechanism) -> bool {
    auth_methods.iter().any(|m| m == mechanism.name())
}

/// Perform AUTH (PLAIN, LOGIN, or SCRAM-SHA-256).
async fn do_auth<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    mechanism: SaslMechanism,
    authcid: &str,
    password: &str,
    auth_methods: &[String],
) -> Result<(), SmtpClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if !server_supports(auth_methods, mechanism) {
        return Err(SmtpClientError::new(format!(
            "server does not support {}",
            mechanism.name()
        )));
    }

    let first = initial_client_response(mechanism, "", authcid, password)?;
    let (initial_b64, mut scram_state) = match &first {
        SaslFirst::Done(b) => (base64_encode(b), None),
        SaslFirst::ScramContinue(b, state) => (base64_encode(b), Some(state.clone())),
    };

    let mut cmd = format!("AUTH {} ", mechanism.name());
    if !initial_b64.is_empty() {
        cmd.push_str(&String::from_utf8_lossy(&initial_b64));
    }
    write_line(stream, cmd.trim_end().as_bytes()).await?;

    loop {
        let r = read_response(stream, read_buf).await?;
        if r.code == 235 {
            return Ok(());
        }
        if r.code == 535 || r.code >= 500 {
            return Err(SmtpClientError::new(format!(
                "auth failed: {} {}",
                r.code,
                r.message()
            )));
        }
        if r.code == 334 {
            let challenge = r.message().trim();
            let response = if mechanism == SaslMechanism::Login {
                login_respond_to_challenge(challenge, authcid, password)?
            } else {
                respond_to_challenge(
                    mechanism,
                    challenge,
                    authcid,
                    password,
                    scram_state.as_ref(),
                )?
            };
            scram_state = None;
            let resp_b64 = base64_encode(&response);
            write_line(stream, &resp_b64).await?;
            continue;
        }
        return Err(SmtpClientError::new(format!(
            "unexpected AUTH response: {} {}",
            r.code,
            r.message()
        )));
    }
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

/// MAIL FROM, RCPT TO (to + cc), then DATA or BDAT.
/// We default to BDAT when the server advertises CHUNKING (no dot-stuffing, efficient); otherwise use DATA with dot stuffing.
async fn send_transaction<S>(
    stream: &mut S,
    read_buf: &mut Vec<u8>,
    envelope: &Envelope,
    message: &[u8],
    use_bdat: bool,
) -> Result<(), SmtpClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let sender = envelope
        .from
        .first()
        .map(envelope_address)
        .unwrap_or_else(|| "".to_string());
    let mail_cmd = format!("MAIL FROM:<{}>", sender);
    write_line(stream, mail_cmd.as_bytes()).await?;
    let r = read_response(stream, read_buf).await?;
    if !r.is_success() {
        return Err(SmtpClientError::new(format!(
            "MAIL FROM failed: {} {}",
            r.code,
            r.message()
        )));
    }

    let mut recipients: Vec<String> = envelope.to.iter().map(envelope_address).collect();
    recipients.extend(envelope.cc.iter().map(envelope_address));
    if recipients.is_empty() {
        return Err(SmtpClientError::new("no recipients"));
    }

    for rcpt in &recipients {
        let cmd = format!("RCPT TO:<{}>", rcpt);
        write_line(stream, cmd.as_bytes()).await?;
        let r = read_response(stream, read_buf).await?;
        if !r.is_success() && r.code != 251 && r.code != 252 {
            return Err(SmtpClientError::new(format!(
                "RCPT TO failed: {} {}",
                r.code,
                r.message()
            )));
        }
    }

    if use_bdat {
        write_line(stream, format!("BDAT {} LAST", message.len()).as_bytes()).await?;
        stream.write_all(message).await?;
        stream.flush().await?;
    } else {
        write_line(stream, b"DATA").await?;
        let r = read_response(stream, read_buf).await?;
        if r.code != 354 {
            return Err(SmtpClientError::new(format!(
                "DATA not accepted: {} {}",
                r.code,
                r.message()
            )));
        }
        let mut data_buf = Vec::with_capacity(message.len() + 128);
        let mut stuffer = DotStuffer::new();
        stuffer.process_chunk(message, |s| data_buf.extend_from_slice(s));
        stuffer.end_message(|s| data_buf.extend_from_slice(s));
        stream.write_all(&data_buf).await?;
        stream.flush().await?;
    }

    let r = read_response(stream, read_buf).await?;
    if !r.is_success() {
        return Err(SmtpClientError::new(format!(
            "message rejected: {} {}",
            r.code,
            r.message()
        )));
    }
    Ok(())
}

/// Persistent SMTP connection (after greeting, EHLO, optional STARTTLS, AUTH). Used for connection reuse.
pub enum SmtpConnection {
    Tls(TlsStreamWrapper, Vec<u8>, bool),
    Plain(PlainStream, Vec<u8>, bool),
}

impl SmtpConnection {
    /// Send one message over this connection (no QUIT; connection stays open).
    pub async fn send_one(
        &mut self,
        envelope: &Envelope,
        message: &[u8],
    ) -> Result<(), SmtpClientError> {
        match self {
            SmtpConnection::Tls(stream, read_buf, use_bdat) => {
                send_transaction(stream, read_buf, envelope, message, *use_bdat).await
            }
            SmtpConnection::Plain(stream, read_buf, use_bdat) => {
                send_transaction(stream, read_buf, envelope, message, *use_bdat).await
            }
        }
    }
}

/// Setup only (greeting, EHLO, auth) over an already-TLS stream. No send, no QUIT.
async fn run_setup_tls(
    stream: &mut TlsStreamWrapper,
    read_buf: &mut Vec<u8>,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname: &str,
) -> Result<bool, SmtpClientError> {
    let r = read_response(stream, read_buf).await?;
    if r.code != 220 {
        return Err(SmtpClientError::new(format!(
            "expected 220 greeting, got {} {}",
            r.code,
            r.message()
        )));
    }
    let (_starttls, auth_methods, chunking) = ehlo(stream, read_buf, ehlo_hostname).await?;
    if let Some((authcid, password, mechanism)) = auth {
        do_auth(stream, read_buf, mechanism, authcid, password, &auth_methods).await?;
    }
    Ok(chunking)
}

/// Run session over an already-TLS stream (implicit TLS path).
async fn run_session_tls(
    stream: &mut TlsStreamWrapper,
    read_buf: &mut Vec<u8>,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname: &str,
    message: &[u8],
    envelope: &Envelope,
) -> Result<(), SmtpClientError> {
    let chunking = run_setup_tls(stream, read_buf, auth, ehlo_hostname).await?;
    send_transaction(stream, read_buf, envelope, message, chunking).await?;
    write_line(stream, b"QUIT").await?;
    let _ = read_response(stream, read_buf).await?;
    Ok(())
}

/// Setup only on plain stream: greeting, EHLO, optional STARTTLS+re-EHLO+auth. Returns connection (Plain or Tls) and use_bdat. No send, no QUIT.
async fn run_setup_plain(
    plain: PlainStream,
    read_buf: &mut Vec<u8>,
    host: &str,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname: &str,
) -> Result<SmtpConnection, SmtpClientError> {
    let mut plain = plain;
    let r = read_response(&mut plain, read_buf).await?;
    if r.code != 220 {
        return Err(SmtpClientError::new(format!(
            "expected 220 greeting, got {} {}",
            r.code,
            r.message()
        )));
    }
    let (starttls_capability, auth_methods, chunking) = ehlo(&mut plain, read_buf, ehlo_hostname).await?;
    let do_starttls = starttls_capability && use_starttls;

    if do_starttls {
        write_line(&mut plain, b"STARTTLS").await?;
        let r = read_response(&mut plain, read_buf).await?;
        if r.code != 220 {
            return Err(SmtpClientError::new(format!(
                "STARTTLS failed: {} {}",
                r.code,
                r.message()
            )));
        }
        let mut tls = plain.upgrade_to_tls(host).await?;
        let (_, auth_methods, chunking) = ehlo(&mut tls, read_buf, ehlo_hostname).await?;
        if let Some((authcid, password, mechanism)) = auth {
            do_auth(&mut tls, read_buf, mechanism, authcid, password, &auth_methods).await?;
        }
        let buf = std::mem::take(read_buf);
        return Ok(SmtpConnection::Tls(tls, buf, chunking));
    }

    if let Some((authcid, password, mechanism)) = auth {
        do_auth(&mut plain, read_buf, mechanism, authcid, password, &auth_methods).await?;
    }
    let buf = std::mem::take(read_buf);
    Ok(SmtpConnection::Plain(plain, buf, chunking))
}

/// Run session starting on plain stream: greeting, EHLO, optional STARTTLS (consumes plain, continues on TLS), re-EHLO, auth, send, QUIT.
async fn run_session_plain(
    plain: PlainStream,
    read_buf: &mut Vec<u8>,
    host: &str,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname: &str,
    message: &[u8],
    envelope: &Envelope,
) -> Result<(), SmtpClientError> {
    let mut conn = run_setup_plain(plain, read_buf, host, use_starttls, auth, ehlo_hostname).await?;
    conn.send_one(envelope, message).await?;
    // QUIT for one-shot session (caller typically drops connection after)
    match &mut conn {
        SmtpConnection::Tls(stream, read_buf, _) => {
            write_line(stream, b"QUIT").await?;
            let _ = read_response(stream, read_buf).await?;
        }
        SmtpConnection::Plain(stream, read_buf, _) => {
            write_line(stream, b"QUIT").await?;
            let _ = read_response(stream, read_buf).await?;
        }
    }
    Ok(())
}

/// Connect and setup (greeting, EHLO, optional STARTTLS, AUTH). No send, no QUIT. For connection reuse.
pub async fn connect_smtp_async(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname: &str,
) -> Result<SmtpConnection, SmtpClientError> {
    if use_implicit_tls {
        let mut stream = connect_implicit_tls(host, port).await?;
        let mut read_buf = Vec::with_capacity(4096);
        let chunking = run_setup_tls(&mut stream, &mut read_buf, auth, ehlo_hostname).await?;
        Ok(SmtpConnection::Tls(stream, read_buf, chunking))
    } else {
        let plain = connect_plain(host, port).await?;
        let mut read_buf = Vec::with_capacity(4096);
        run_setup_plain(plain, &mut read_buf, host, use_starttls, auth, ehlo_hostname).await
    }
}

/// Run SMTP session: connect (plain or implicit TLS), EHLO, optional STARTTLS+AUTH, send message, QUIT.
/// BDAT is used when the server advertises CHUNKING; otherwise DATA. STARTTLS is used on plain connections when advertised unless use_starttls is false (debug).
pub async fn send_message_async(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname: &str,
    message: &[u8],
    envelope: &Envelope,
) -> Result<(), SmtpClientError> {
    let mut read_buf = Vec::with_capacity(4096);

    if use_implicit_tls {
        let mut stream = connect_implicit_tls(host, port).await?;
        run_session_tls(
            &mut stream,
            &mut read_buf,
            auth,
            ehlo_hostname,
            message,
            envelope,
        )
        .await
    } else {
        let plain = connect_plain(host, port).await?;
        run_session_plain(
            plain,
            &mut read_buf,
            host,
            use_starttls,
            auth,
            ehlo_hostname,
            message,
            envelope,
        )
        .await
    }
}
