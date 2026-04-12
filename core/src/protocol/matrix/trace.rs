/*
 * trace.rs — Matrix Client-Server API tracing (stderr).
 *
 * Enable with **`TAGLIACARTE_TRACE=matrix`** (or `all`). Outbound: logs after the request head is
 * on the wire; with **`TAGLIACARTE_TRACE_FULL=matrix`**, logs each outbound body segment as sent.
 * Inbound: one line with HTTP status (and `Content-Type` when present) when the response head is
 * complete; with full mode, logs each body chunk as received. A short summary is still printed when the body finishes (non-full:
 * capped JSON preview; full: byte total only). Passwords, access tokens, and refresh tokens are
 * redacted in aggregated JSON previews; per-chunk logs use UTF-8 lossy text (secrets may appear).
 */

/// Max bytes printed per wire chunk in `TAGLIACARTE_TRACE_FULL` mode.
const CHUNK_TRACE_CAP: usize = 4096;

/// One-time hint when Matrix trace is off (same idea as Nostr).
pub fn hint_once_if_tracing_off() {
    use std::sync::OnceLock;
    static HINT: OnceLock<()> = OnceLock::new();
    if crate::trace::enabled("matrix") {
        return;
    }
    HINT.get_or_init(|| {
        let raw = std::env::var("TAGLIACARTE_TRACE").unwrap_or_default();
        let shown = if raw.is_empty() {
            "(unset)".to_string()
        } else {
            raw
        };
        eprintln!(
            "[matrix] hint: set TAGLIACARTE_TRACE=matrix (or all) for request/response summaries; \
             TAGLIACARTE_TRACE_FULL=matrix for per-chunk wire logs and JSON previews (secrets redacted in previews). \
             TAGLIACARTE_TRACE={shown:?}. IDE / .app bundles often ignore shell env — set launch.json or scheme."
        );
    });
}

fn format_chunk_lossy(chunk: &[u8]) -> String {
    let slice = if chunk.len() > CHUNK_TRACE_CAP {
        &chunk[..CHUNK_TRACE_CAP]
    } else {
        chunk
    };
    let mut s = String::from_utf8_lossy(slice).into_owned();
    if chunk.len() > CHUNK_TRACE_CAP {
        s.push_str(&format!(
            "\n[matrix trace] … {} more bytes in chunk (not shown)",
            chunk.len().saturating_sub(CHUNK_TRACE_CAP)
        ));
    }
    s
}

fn auth_note(auth_bearer: bool) -> &'static str {
    if auth_bearer {
        " (Authorization: Bearer <redacted>)"
    } else {
        ""
    }
}

/// Log immediately after the outbound request line and headers have been written (before body).
pub fn log_outbound_headers_sent(
    method: &str,
    path: &str,
    auth_bearer: bool,
    body_len: Option<usize>,
) {
    if !crate::trace::enabled("matrix") {
        return;
    }
    let a = auth_note(auth_bearer);
    match body_len {
        None | Some(0) => {
            eprintln!("[matrix trace] >> {method} {path}{a} — headers sent");
        }
        Some(n) => {
            eprintln!("[matrix trace] >> {method} {path}{a} — headers sent; sending body ({n} bytes)");
        }
    }
}

/// Log one outbound body segment as sent (`TAGLIACARTE_TRACE` + `TAGLIACARTE_TRACE_FULL=matrix` only).
pub fn log_outbound_body_chunk(method: &str, path: &str, seq: usize, chunk: &[u8]) {
    if !crate::trace::enabled("matrix") || !crate::trace::full_enabled("matrix") {
        return;
    }
    let preview = format_chunk_lossy(chunk);
    eprintln!(
        "[matrix trace] >> {method} {path} outbound body chunk #{seq} ({} bytes):\n{}",
        chunk.len(),
        preview
    );
}

/// Log status (and `Content-Type` when present) as soon as the response head is complete.
pub fn log_inbound_response_head(
    method: &str,
    path: &str,
    status: u16,
    headers: &[(String, String)],
) {
    if !crate::trace::enabled("matrix") {
        return;
    }
    let mut content_type: Option<&str> = None;
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("content-type") {
            content_type = Some(v.as_str());
            break;
        }
    }
    match content_type {
        Some(ct) => eprintln!(
            "[matrix trace] << {method} {path} HTTP {status} — content-type: {ct}"
        ),
        None => eprintln!("[matrix trace] << {method} {path} HTTP {status}"),
    }
}

/// Log one inbound body chunk (`TAGLIACARTE_TRACE` + `TAGLIACARTE_TRACE_FULL=matrix` only).
pub fn log_inbound_body_chunk(method: &str, path: &str, seq: usize, chunk: &[u8]) {
    if !crate::trace::enabled("matrix") || !crate::trace::full_enabled("matrix") {
        return;
    }
    let preview = format_chunk_lossy(chunk);
    eprintln!(
        "[matrix trace] << {method} {path} inbound body chunk #{seq} ({} bytes):\n{}",
        chunk.len(),
        preview
    );
}

/// After a fully streamed body in full mode: one line with total size (no duplicate JSON dump).
pub fn log_inbound_body_complete_full(
    method: &str,
    path: &str,
    status: u16,
    total_body_bytes: usize,
) {
    if !crate::trace::enabled("matrix") {
        return;
    }
    eprintln!(
        "[matrix trace] << {method} {path} HTTP {status} — body complete ({total_body_bytes} bytes total; chunks logged above)"
    );
}

/// Log a completed Matrix HTTP response when **not** using `TAGLIACARTE_TRACE_FULL` per-chunk logs.
///
/// With full mode, prefer [`log_inbound_body_chunk`] + [`log_inbound_body_complete_full`] instead;
/// this function is a no-op when `TAGLIACARTE_TRACE_FULL=matrix` is set.
///
/// `total_body_bytes` is the full response length. The `body` argument is unused (kept for call sites).
pub fn log_response(method: &str, path: &str, status: u16, _body: &[u8], total_body_bytes: usize) {
    if !crate::trace::enabled("matrix") {
        return;
    }
    if crate::trace::full_enabled("matrix") {
        return;
    }
    eprintln!(
        "[matrix trace] << {method} {path} HTTP {status} body {} bytes \
         (set TAGLIACARTE_TRACE_FULL=matrix for per-chunk and JSON)",
        total_body_bytes
    );
}
