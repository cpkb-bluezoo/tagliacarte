/*
 * api_trace.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

//! Optional stderr logging for REST mail backends (Gmail, Microsoft Graph) when
//! `TAGLIACARTE_TRACE` includes `gmail` or `graph`. Outbound: logs after request headers are on the
//! wire; with `TAGLIACARTE_TRACE_FULL`, logs each outbound body segment. Inbound: logs status and
//! response headers when the head completes; with full mode, logs each body chunk. Authorization is
//! never logged. Per-chunk logs use UTF-8 lossy text.

use crate::protocol::http::request::ApiOutboundTrace;

const CHUNK_TRACE_CAP: usize = 4096;

fn chunk_preview(provider: &str, chunk: &[u8]) -> String {
    let slice = if chunk.len() > CHUNK_TRACE_CAP {
        &chunk[..CHUNK_TRACE_CAP]
    } else {
        chunk
    };
    let mut s = String::from_utf8_lossy(slice).into_owned();
    if chunk.len() > CHUNK_TRACE_CAP {
        s.push_str(&format!(
            "\n[{provider} trace] … {} more bytes in chunk (not shown)",
            chunk.len().saturating_sub(CHUNK_TRACE_CAP)
        ));
    }
    s
}

/// Log immediately after the outbound request line and headers have been written (before body).
pub(crate) fn log_outbound_headers_sent(t: &ApiOutboundTrace, body_len: Option<usize>) {
    if !crate::trace::enabled(t.provider) {
        return;
    }
    let p = t.provider;
    match body_len {
        None | Some(0) => {
            eprintln!(
                "[{p} trace] >> {} {} (Authorization: Bearer <redacted>) — headers sent",
                t.method, t.path
            );
        }
        Some(n) => {
            eprintln!(
                "[{p} trace] >> {} {} (Authorization: Bearer <redacted>) — headers sent; sending body ({n} bytes)",
                t.method, t.path
            );
        }
    }
}

/// Log one outbound body segment (`TAGLIACARTE_TRACE` + `TAGLIACARTE_TRACE_FULL` for this provider).
pub(crate) fn log_outbound_body_chunk(t: &ApiOutboundTrace, seq: usize, chunk: &[u8]) {
    if !crate::trace::enabled(t.provider) || !crate::trace::full_enabled(t.provider) {
        return;
    }
    let p = t.provider;
    let preview = chunk_preview(p, chunk);
    eprintln!(
        "[{p} trace] >> {} {} outbound body chunk #{seq} ({} bytes):\n{}",
        t.method,
        t.path,
        chunk.len(),
        preview
    );
}

/// Log status and response headers as soon as the response head is complete (before body chunks).
pub(crate) fn log_inbound_response_head(
    provider: &str,
    method: &str,
    path: &str,
    status: u16,
    headers: &[(String, String)],
) {
    if !crate::trace::enabled(provider) {
        return;
    }
    eprintln!(
        "[{provider} trace] << {method} {path} HTTP {status} — response headers received"
    );
    for (k, v) in headers {
        let show = if k.eq_ignore_ascii_case("set-cookie") {
            "<redacted>"
        } else {
            v.as_str()
        };
        eprintln!("[{provider} trace] <<    {k}: {show}");
    }
}

/// Log one inbound body chunk (`TAGLIACARTE_TRACE` + `TAGLIACARTE_TRACE_FULL`).
pub(crate) fn log_inbound_body_chunk(
    provider: &str,
    method: &str,
    path: &str,
    seq: usize,
    chunk: &[u8],
) {
    if !crate::trace::enabled(provider) || !crate::trace::full_enabled(provider) {
        return;
    }
    let preview = chunk_preview(provider, chunk);
    eprintln!(
        "[{provider} trace] << {method} {path} inbound body chunk #{seq} ({} bytes):\n{}",
        chunk.len(),
        preview
    );
}

/// After a fully streamed body in full mode: one line with total size.
pub(crate) fn log_inbound_body_complete_full(
    provider: &str,
    method: &str,
    path: &str,
    status: u16,
    total_body_bytes: usize,
) {
    if !crate::trace::enabled(provider) {
        return;
    }
    eprintln!(
        "[{provider} trace] << {method} {path} HTTP {status} — body complete ({total_body_bytes} bytes total; chunks logged above)"
    );
}

/// Log a completed HTTP response when **`TAGLIACARTE_TRACE_FULL`** is off (body size only).
pub(crate) fn log_http_response_size_only(provider: &str, status: u16, total_body_bytes: usize) {
    if !crate::trace::enabled(provider) {
        return;
    }
    if crate::trace::full_enabled(provider) {
        return;
    }
    eprintln!(
        "[{provider} trace] << HTTP {status} body {total_body_bytes} bytes (set TAGLIACARTE_TRACE_FULL={provider} for per-chunk text)"
    );
}

/// Raw stream success summary (MIME `$value` etc.).
pub(crate) fn log_http_stream_summary(provider: &str, status: u16, total_streamed: usize) {
    if !crate::trace::enabled(provider) {
        return;
    }
    if crate::trace::full_enabled(provider) {
        eprintln!(
            "[{provider} trace] << HTTP {status} streamed body {total_streamed} bytes total (per-chunk lines logged above)"
        );
    } else {
        eprintln!(
            "[{provider} trace] << HTTP {status} streamed body {total_streamed} bytes (raw stream not captured for logging)"
        );
    }
}

const ERR_BODY_PREVIEW_CAP: usize = 256 * 1024;

/// Log an error response body (aggregated), e.g. Graph JSON error on non-2xx.
pub(crate) fn log_http_error_response_body(provider: &str, status: u16, body: &[u8]) {
    if !crate::trace::enabled(provider) {
        return;
    }
    if crate::trace::full_enabled(provider) {
        if body.is_empty() {
            eprintln!("[{provider} trace] << HTTP {status} (empty error body)");
            return;
        }
        let preview = if body.len() <= ERR_BODY_PREVIEW_CAP {
            String::from_utf8_lossy(body).into_owned()
        } else {
            format!(
                "{}… [{} more bytes not logged]",
                String::from_utf8_lossy(&body[..ERR_BODY_PREVIEW_CAP]),
                body.len() - ERR_BODY_PREVIEW_CAP
            )
        };
        eprintln!("[{provider} trace] << HTTP {status} error body:\n{preview}");
    } else {
        eprintln!(
            "[{provider} trace] << HTTP {status} error body {} bytes (set TAGLIACARTE_TRACE_FULL={provider} for text)",
            body.len()
        );
    }
}
