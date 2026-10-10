/*
 * media.rs
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

//! Nostr media uploads via Blossom (BUD-02/04) and NIP-96, with protocol
//! discovery and automatic fallback. Ported from Plume's media module;
//! every request is a one-shot hopf fetch reported through a callback.

use std::sync::RwLock;

use hopf_http::reason_phrase;

use crate::protocol::http::{fetch, Collected, Method};

use super::crypto::{
    create_blossom_auth_event, create_nip98_auth_event, nostr_auth_header, sha256_hex,
};

// ── Protocol cache ───────────────────────────────────────────────────

#[derive(Clone)]
enum MediaProtocol {
    Blossom,
    Nip96 { api_url: String },
}

static PROTOCOL_CACHE: RwLock<Option<(String, MediaProtocol)>> = RwLock::new(None);

fn cached_protocol(server_url: &str) -> Option<MediaProtocol> {
    PROTOCOL_CACHE
        .read()
        .ok()?
        .as_ref()
        .filter(|(url, _)| url == server_url)
        .map(|(_, p)| p.clone())
}

fn cache_protocol(server_url: &str, protocol: MediaProtocol) {
    if let Ok(mut guard) = PROTOCOL_CACHE.write() {
        *guard = Some((server_url.to_string(), protocol));
    }
}

// ── URL helpers ──────────────────────────────────────────────────────

struct UrlParts {
    host: String,
    port: u16,
    use_tls: bool,
    path_prefix: String,
}

fn parse_server_url(server_url: &str) -> Result<UrlParts, String> {
    let (scheme, rest) = if let Some(r) = server_url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = server_url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(format!("invalid server URL: {}", server_url));
    };
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let path_prefix = path.trim_end_matches('/').to_string();
    let (host, port) = if let Some(colon) = host_port.rfind(':') {
        let port_str = &host_port[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (host_port[..colon].to_string(), p)
        } else {
            (host_port.to_string(), if scheme { 443 } else { 80 })
        }
    } else {
        (host_port.to_string(), if scheme { 443 } else { 80 })
    };
    Ok(UrlParts {
        host,
        port,
        use_tls: scheme,
        path_prefix,
    })
}

fn mime_from_extension(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

// ── JSON parsing (minimal, no serde) ─────────────────────────────────

fn extract_json_string_value(json: &[u8], key: &str) -> Option<String> {
    let json_str = std::str::from_utf8(json).ok()?;
    let needle = format!("\"{}\"", key);
    let pos = json_str.find(&needle)?;
    let after_key = &json_str[pos + needle.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_ws = after_colon.trim_start();
    if !after_ws.starts_with('"') {
        return None;
    }
    let value_start = &after_ws[1..];
    let mut result = String::new();
    let mut chars = value_start.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(result),
            '\\' => {
                if let Some(esc) = chars.next() {
                    result.push(esc);
                }
            }
            _ => result.push(c),
        }
    }
    None
}

/// Extract URL from upload response. Tries top-level "url", then NIP-96 nip94_event.tags [["url","..."]].
fn extract_upload_url(body: &[u8]) -> Option<String> {
    if let Some(url) = extract_json_string_value(body, "url") {
        return Some(url);
    }
    let text = std::str::from_utf8(body).ok()?;
    // NIP-96: look for "nip94_event" then tags containing ["url", "..."]
    if let Some(nip94_pos) = text.find("\"nip94_event\"") {
        let rest = &text[nip94_pos..];
        if let Some(tags_pos) = rest.find("\"tags\"") {
            let tags_rest = &rest[tags_pos..];
            // Simple scan: find ["url","<value>"]
            let pattern = "[\"url\"";
            if let Some(url_tag_pos) = tags_rest.find(pattern) {
                let after = &tags_rest[url_tag_pos + pattern.len()..];
                let after_comma = after.trim_start().strip_prefix(',')?;
                let after_ws = after_comma.trim_start();
                if after_ws.starts_with('"') {
                    let val_start = &after_ws[1..];
                    if let Some(end) = val_start.find('"') {
                        return Some(val_start[..end].to_string());
                    }
                }
            }
        }
    }
    None
}

// ── Requests ─────────────────────────────────────────────────────────

/// Hears one outcome, from hopf's reactor.
type Done<T> = Box<dyn FnOnce(Result<T, String>) + Send>;

/// Collected reply or the error text the old handlers produced.
fn checked(r: std::io::Result<Collected>) -> Result<Collected, String> {
    match r {
        Ok(c) if c.is_success() => Ok(c),
        Ok(c) => Err(format!("HTTP {} {}", c.status, reason_phrase(c.status))),
        Err(e) => Err(format!("connection error: {}", e)),
    }
}

fn url_from_parts(parts: &UrlParts, path: &str) -> String {
    format!(
        "{}://{}:{}{}",
        if parts.use_tls { "https" } else { "http" },
        parts.host,
        parts.port,
        path
    )
}

// ── NIP-96 Discovery ─────────────────────────────────────────────────

fn discover_protocol(server_url: &str, done: Box<dyn FnOnce(MediaProtocol) + Send>) {
    let parts = match parse_server_url(server_url) {
        Ok(p) => p,
        Err(_) => return done(MediaProtocol::Blossom),
    };
    let url = url_from_parts(&parts, &format!("{}/.well-known/nostr/nip96.json", parts.path_prefix));
    let base = server_url.trim_end_matches('/').to_string();
    fetch(
        Method::Get,
        &url,
        &[],
        None,
        Box::new(move |r| {
            let body = match checked(r) {
                Ok(c) => c.body,
                Err(_) => return done(MediaProtocol::Blossom),
            };
            done(match extract_json_string_value(&body, "api_url") {
                Some(api_url) => {
                    let resolved = if api_url.starts_with("http://") || api_url.starts_with("https://") {
                        api_url
                    } else if api_url.starts_with('/') {
                        format!("{}{}", base, api_url)
                    } else {
                        format!("{}/{}", base, api_url)
                    };
                    MediaProtocol::Nip96 { api_url: resolved }
                }
                None => MediaProtocol::Blossom,
            })
        }),
    );
}

// ── Upload ───────────────────────────────────────────────────────────

/// What an upload needs, shared by both attempts.
#[derive(Clone)]
struct UploadJob {
    server_url: String,
    file_data: Vec<u8>,
    file_hash: String,
    file_name: String,
    content_type: &'static str,
    secret_key_hex: String,
}

/// Upload a file to a Nostr media server; `done` gets `(url, file_hash)`.
/// The cached (or default, Blossom) protocol is tried first; on failure
/// the server's NIP-96 descriptor is consulted and the other tried.
pub fn upload(server_url: &str, file_path: &str, secret_key_hex: &str, done: Done<(String, String)>) {
    let file_data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(e) => return done(Err(format!("cannot read file: {}", e))),
    };
    let file_hash = sha256_hex(&file_data);
    let file_name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let content_type = mime_from_extension(&file_name);
    let job = UploadJob {
        server_url: server_url.to_string(),
        file_data,
        file_hash,
        file_name,
        content_type,
        secret_key_hex: secret_key_hex.to_string(),
    };
    let protocol = cached_protocol(server_url).unwrap_or(MediaProtocol::Blossom);
    let retry = job.clone();
    let tried = protocol.clone();
    do_upload(
        &job,
        &protocol,
        Box::new(move |first| match first {
            Ok(url) => {
                cache_protocol(&retry.server_url, tried);
                done(Ok((url, retry.file_hash)))
            }
            Err(first_err) => {
                crate::trace_log!(
                    "nostr",
                    "media: first upload attempt failed: {}, trying discovery",
                    first_err
                );
                let server_url = retry.server_url.clone();
                discover_protocol(
                    &server_url,
                    Box::new(move |discovered| {
                        let again = retry.clone();
                        let found = discovered.clone();
                        do_upload(
                            &retry,
                            &discovered,
                            Box::new(move |second| match second {
                                Ok(url) => {
                                    cache_protocol(&again.server_url, found);
                                    done(Ok((url, again.file_hash)))
                                }
                                Err(second_err) => done(Err(format!(
                                    "upload failed: {} (also tried: {})",
                                    second_err, first_err
                                ))),
                            }),
                        )
                    }),
                )
            }
        }),
    );
}

/// [`upload`] waited for on this thread; never call it from a hopf
/// reactor thread.
pub fn upload_blocking(server_url: &str, file_path: &str, secret_key_hex: &str) -> Result<(String, String), String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    upload(
        server_url,
        file_path,
        secret_key_hex,
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    rx.recv().unwrap_or_else(|_| Err("upload ended without a result".to_string()))
}

fn do_upload(job: &UploadJob, protocol: &MediaProtocol, done: Done<String>) {
    match protocol {
        MediaProtocol::Blossom => blossom_upload(job, done),
        MediaProtocol::Nip96 { api_url } => nip96_upload(api_url, job, done),
    }
}

fn upload_url_from(r: std::io::Result<Collected>) -> Result<String, String> {
    let c = checked(r)?;
    extract_upload_url(&c.body).ok_or_else(|| {
        let body_str = String::from_utf8_lossy(&c.body);
        format!(
            "no url in response: {}",
            &body_str[..body_str.len().min(200)]
        )
    })
}

fn blossom_upload(job: &UploadJob, done: Done<String>) {
    let parts = match parse_server_url(&job.server_url) {
        Ok(p) => p,
        Err(e) => return done(Err(e)),
    };
    let url = url_from_parts(&parts, &format!("{}/upload", parts.path_prefix));
    let auth_event = match create_blossom_auth_event("upload", &job.file_hash, &job.secret_key_hex) {
        Ok(e) => e,
        Err(e) => return done(Err(e)),
    };
    let auth_header = nostr_auth_header(&auth_event);
    fetch(
        Method::Put,
        &url,
        &[("Authorization", &auth_header), ("Content-Type", job.content_type)],
        Some(job.file_data.clone()),
        Box::new(move |r| done(upload_url_from(r))),
    );
}

fn nip96_upload(api_url: &str, job: &UploadJob, done: Done<String>) {
    let upload_url = format!("{}/upload", api_url.trim_end_matches('/'));
    let auth_event = match create_nip98_auth_event(&upload_url, "POST", None, &job.secret_key_hex) {
        Ok(e) => e,
        Err(e) => return done(Err(e)),
    };
    let auth_header = nostr_auth_header(&auth_event);
    let boundary = format!(
        "----tagliacarte{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let mut body = Vec::new();
    // file part
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
            job.file_name
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", job.content_type).as_bytes());
    body.extend_from_slice(&job.file_data);
    body.extend_from_slice(b"\r\n");
    // content_type part
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"content_type\"\r\n\r\n");
    body.extend_from_slice(job.content_type.as_bytes());
    body.extend_from_slice(b"\r\n");
    // closing boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    let multipart_ct = format!("multipart/form-data; boundary={}", boundary);
    fetch(
        Method::Post,
        &upload_url,
        &[("Authorization", &auth_header), ("Content-Type", &multipart_ct)],
        Some(body),
        Box::new(move |r| done(upload_url_from(r))),
    );
}

// ── Delete ───────────────────────────────────────────────────────────

/// Delete a previously uploaded file from the media server.
pub fn delete(server_url: &str, file_hash: &str, secret_key_hex: &str, done: Done<()>) {
    let protocol = cached_protocol(server_url).unwrap_or(MediaProtocol::Blossom);
    match &protocol {
        MediaProtocol::Blossom => blossom_delete(server_url, file_hash, secret_key_hex, done),
        MediaProtocol::Nip96 { api_url } => nip96_delete(api_url, file_hash, secret_key_hex, done),
    }
}

/// [`delete`] waited for on this thread; never call it from a hopf
/// reactor thread.
pub fn delete_blocking(server_url: &str, file_hash: &str, secret_key_hex: &str) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    delete(
        server_url,
        file_hash,
        secret_key_hex,
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    rx.recv().unwrap_or_else(|_| Err("delete ended without a result".to_string()))
}

fn blossom_delete(server_url: &str, file_hash: &str, secret_key_hex: &str, done: Done<()>) {
    let parts = match parse_server_url(server_url) {
        Ok(p) => p,
        Err(e) => return done(Err(e)),
    };
    let url = url_from_parts(&parts, &format!("{}/{}", parts.path_prefix, file_hash));
    let auth_event = match create_blossom_auth_event("delete", file_hash, secret_key_hex) {
        Ok(e) => e,
        Err(e) => return done(Err(e)),
    };
    let auth_header = nostr_auth_header(&auth_event);
    fetch(
        Method::Delete,
        &url,
        &[("Authorization", &auth_header)],
        None,
        Box::new(move |r| done(r.map(|_| ()).map_err(|e| format!("delete failed: {}", e)))),
    );
}

fn nip96_delete(api_url: &str, file_hash: &str, secret_key_hex: &str, done: Done<()>) {
    let delete_url = format!("{}/{}", api_url.trim_end_matches('/'), file_hash);
    let auth_event = match create_nip98_auth_event(&delete_url, "DELETE", None, secret_key_hex) {
        Ok(e) => e,
        Err(e) => return done(Err(e)),
    };
    let auth_header = nostr_auth_header(&auth_event);
    fetch(
        Method::Delete,
        &delete_url,
        &[("Authorization", &auth_header)],
        None,
        Box::new(move |r| done(r.map(|_| ()).map_err(|e| format!("delete failed: {}", e)))),
    );
}
