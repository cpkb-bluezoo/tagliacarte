/*
 * api_trace.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

//! Optional stderr logging for REST mail backends (Gmail, Microsoft Graph) when
//! `TAGLIACARTE_TRACE` includes `gmail` or `graph`. Bodies are size-only unless
//! `TAGLIACARTE_TRACE_FULL` includes the same token. Authorization is never logged.

const BODY_PREVIEW_CAP: usize = 256 * 1024;

/// Log an outbound HTTP request (method, path, optional body). Bearer tokens are never printed.
pub(crate) fn log_http_request(
    provider: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
) {
    if !crate::trace::enabled(provider) {
        return;
    }
    let b = match body {
        Some(s) if !s.is_empty() => s,
        _ => {
            eprintln!(
                "[{provider} trace] >> {method} {path} (Authorization: Bearer <redacted>)"
            );
            return;
        }
    };
    if crate::trace::full_enabled(provider) {
        let preview = if b.len() <= BODY_PREVIEW_CAP {
            String::from_utf8_lossy(b).into_owned()
        } else {
            format!(
                "{}… [{} more bytes not logged]",
                String::from_utf8_lossy(&b[..BODY_PREVIEW_CAP]),
                b.len() - BODY_PREVIEW_CAP
            )
        };
        eprintln!(
            "[{provider} trace] >> {method} {path} (Authorization: Bearer <redacted>) body:\n{preview}"
        );
    } else {
        eprintln!(
            "[{provider} trace] >> {method} {path} body {} bytes (Authorization: Bearer <redacted>; set TAGLIACARTE_TRACE_FULL={provider} for body text)",
            b.len()
        );
    }
}

/// Log an HTTP response status and body (or body size only).
pub(crate) fn log_http_response(provider: &str, status: u16, body: &[u8]) {
    if !crate::trace::enabled(provider) {
        return;
    }
    if crate::trace::full_enabled(provider) {
        if body.is_empty() {
            eprintln!("[{provider} trace] << HTTP {status} (empty body)");
            return;
        }
        let preview = if body.len() <= BODY_PREVIEW_CAP {
            String::from_utf8_lossy(body).into_owned()
        } else {
            format!(
                "{}… [{} more bytes not logged]",
                String::from_utf8_lossy(&body[..BODY_PREVIEW_CAP]),
                body.len() - BODY_PREVIEW_CAP
            )
        };
        eprintln!("[{provider} trace] << HTTP {status} body:\n{preview}");
    } else {
        eprintln!(
            "[{provider} trace] << HTTP {status} body {} bytes (set TAGLIACARTE_TRACE_FULL={provider} for body text)",
            body.len()
        );
    }
}

/// Log a successful raw stream response where the body is not buffered (e.g. MIME `$value`).
pub(crate) fn log_http_stream_summary(provider: &str, status: u16, total_streamed: usize) {
    if !crate::trace::enabled(provider) {
        return;
    }
    eprintln!(
        "[{provider} trace] << HTTP {status} streamed body {total_streamed} bytes (raw stream not captured for logging)"
    );
}
