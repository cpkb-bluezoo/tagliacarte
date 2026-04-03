/*
 * h1.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Minimal HTTP/1.1 request parsing and chunked response writing (Gumdrop-style framing).
 */

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Maximum bytes per header or request line (RFC 9112 recommends 8 KiB).
pub const MAX_LINE: usize = 8192;

#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub method: String,
    /// Path including query string (request-target from the request line).
    pub target: String,
    pub headers: Vec<(String, String)>,
}

/// Read one HTTP/1.1 request from `r`. Returns `None` on clean EOF before any bytes.
pub async fn read_http_request<R: AsyncRead + Unpin>(
    mut r: R,
) -> io::Result<Option<ParsedRequest>> {
    let mut line_buf = Vec::<u8>::new();
    let request_line = match read_line_crlf(&mut r, &mut line_buf, MAX_LINE).await? {
        None => return Ok(None),
        Some(l) if l.is_empty() => return Ok(None),
        Some(l) => l,
    };

    let line_str = std::str::from_utf8(&request_line)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "request line: invalid UTF-8"))?;
    let parts: Vec<&str> = line_str.splitn(3, ' ').collect();
    if parts.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad request line",
        ));
    }
    let method = parts[0].to_string();
    let target = parts[1].to_string();
    let version = parts[2];
    if !version.eq_ignore_ascii_case("HTTP/1.1") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP version not supported (need HTTP/1.1)",
        ));
    }

    let mut headers = Vec::new();
    loop {
        let line = read_line_crlf(&mut r, &mut line_buf, MAX_LINE)
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "EOF in headers"))?;
        if line.is_empty() {
            break;
        }
        let hs = std::str::from_utf8(&line)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "header: invalid UTF-8"))?;
        let Some(colon) = hs.find(':') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad header line",
            ));
        };
        let name = hs[..colon].trim().to_string();
        let value = hs[colon + 1..].trim().to_string();
        headers.push((name, value));
    }

    // Drain request body if indicated (WebView should only send GET).
    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    for (k, v) in &headers {
        if k.eq_ignore_ascii_case("content-length") {
            content_length = v.parse().ok();
        }
        if k.eq_ignore_ascii_case("transfer-encoding") && v.to_ascii_lowercase().contains("chunked")
        {
            chunked = true;
        }
    }
    if let Some(n) = content_length {
        if n > 0 {
            let mut discard = vec![0u8; n.min(1024 * 1024)];
            let mut left = n;
            while left > 0 {
                let take = discard.len().min(left);
                r.read_exact(&mut discard[..take]).await?;
                left -= take;
            }
        }
    } else if chunked {
        // Minimal chunked discard (not expected for GET)
        loop {
            let sz_line = read_line_crlf(&mut r, &mut line_buf, MAX_LINE)
                .await?
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "chunked EOF"))?;
            let sz_str = std::str::from_utf8(&sz_line)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "chunk size"))?;
            let hex_part = sz_str.split(';').next().unwrap_or("").trim();
            let chunk_len = usize::from_str_radix(hex_part, 16).unwrap_or(0);
            if chunk_len == 0 {
                // trailers
                loop {
                    let t = read_line_crlf(&mut r, &mut line_buf, MAX_LINE)
                        .await?
                        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "trailers"))?;
                    if t.is_empty() {
                        break;
                    }
                }
                break;
            }
            let mut buf = vec![0u8; chunk_len];
            r.read_exact(&mut buf).await?;
            r.read_exact(&mut [0u8; 2]).await?; // CRLF
        }
    }

    Ok(Some(ParsedRequest {
        method,
        target,
        headers,
    }))
}

async fn read_line_crlf<R: AsyncRead + Unpin>(
    r: &mut R,
    reuse: &mut Vec<u8>,
    max: usize,
) -> io::Result<Option<Vec<u8>>> {
    reuse.clear();
    let mut one = [0u8; 1];
    loop {
        let n = r.read(&mut one).await?;
        if n == 0 {
            if reuse.is_empty() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF inside request line",
            ));
        }
        if reuse.len() >= max {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
        }
        reuse.push(one[0]);
        if reuse.len() >= 2 && reuse[reuse.len() - 2] == b'\r' && reuse[reuse.len() - 1] == b'\n' {
            reuse.truncate(reuse.len() - 2);
            return Ok(Some(reuse.clone()));
        }
    }
}

/// Write status line + headers + blank line. Caller then writes body (chunked or Content-Length).
pub async fn write_response_head<W: AsyncWrite + Unpin>(
    w: &mut W,
    status: u16,
    headers: &[(&str, &str)],
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let mut block = format!("HTTP/1.1 {} {}\r\n", status, reason);
    for (k, v) in headers {
        block.push_str(k);
        block.push_str(": ");
        block.push_str(v);
        block.push_str("\r\n");
    }
    block.push_str("\r\n");
    w.write_all(block.as_bytes()).await?;
    w.flush().await?;
    Ok(())
}

/// One HTTP/1.1 chunk (hex size, CRLF, data, CRLF).
pub async fn write_chunk<W: AsyncWrite + Unpin>(w: &mut W, data: &[u8]) -> io::Result<()> {
    let hex = format!("{:x}\r\n", data.len());
    w.write_all(hex.as_bytes()).await?;
    w.write_all(data).await?;
    w.write_all(b"\r\n").await?;
    Ok(())
}

/// End chunked body: `0\r\n\r\n`.
pub async fn write_chunk_end<W: AsyncWrite + Unpin>(w: &mut W) -> io::Result<()> {
    w.write_all(b"0\r\n\r\n").await?;
    w.flush().await?;
    Ok(())
}

/// Write a full response with `Content-Length` body (no chunking).
pub async fn write_response_bytes<W: AsyncWrite + Unpin>(
    w: &mut W,
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
) -> io::Result<()> {
    let len = body.len().to_string();
    let mut extra = vec![("Content-Length", len.as_str())];
    let mut all: Vec<(&str, &str)> = headers.to_vec();
    all.append(&mut extra);
    write_response_head(w, status, &all).await?;
    w.write_all(body).await?;
    w.flush().await?;
    Ok(())
}
