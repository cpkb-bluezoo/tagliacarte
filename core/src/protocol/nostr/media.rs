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
//! discovery and automatic fallback. Ported from Plume's media module.

use std::sync::{Arc, Mutex, RwLock};

use crate::protocol::http::{HttpClient, Method, Response, ResponseHandler};

use super::crypto::{create_blossom_auth_event, create_nip98_auth_event, nostr_auth_header, sha256_hex};

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
    Ok(UrlParts { host, port, use_tls: scheme, path_prefix })
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

// ── Response handlers ────────────────────────────────────────────────

/// Shared state for collecting response data, accessible after send() consumes the handler.
struct CollectorState {
    success: bool,
    body: Vec<u8>,
    error_msg: Option<String>,
}

/// Collects full body bytes from an HTTP response. State is shared via Arc<Mutex>.
struct BodyCollector {
    state: Arc<Mutex<CollectorState>>,
}

impl BodyCollector {
    fn new() -> (Self, Arc<Mutex<CollectorState>>) {
        let state = Arc::new(Mutex::new(CollectorState {
            success: false, body: Vec::new(), error_msg: None,
        }));
        (Self { state: state.clone() }, state)
    }
}

impl ResponseHandler for BodyCollector {
    fn ok(&mut self, _response: Response) { self.state.lock().unwrap().success = true; }
    fn error(&mut self, response: Response) {
        self.state.lock().unwrap().error_msg = Some(format!("HTTP {}{}", response.code,
            response.reason.as_deref().map(|r| format!(" {}", r)).unwrap_or_default()));
    }
    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}
    fn body_chunk(&mut self, data: &[u8]) { self.state.lock().unwrap().body.extend_from_slice(data); }
    fn end_body(&mut self) {}
    fn complete(&mut self) {}
    fn failed(&mut self, error: &std::io::Error) {
        self.state.lock().unwrap().error_msg = Some(format!("connection error: {}", error));
    }
}

/// Fire-and-forget handler for DELETE responses.
struct NoOpHandler;

impl ResponseHandler for NoOpHandler {
    fn ok(&mut self, _response: Response) {}
    fn error(&mut self, _response: Response) {}
    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}
    fn body_chunk(&mut self, _data: &[u8]) {}
    fn end_body(&mut self) {}
    fn complete(&mut self) {}
    fn failed(&mut self, _error: &std::io::Error) {}
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

// ── NIP-96 Discovery ─────────────────────────────────────────────────

async fn discover_protocol(server_url: &str) -> MediaProtocol {
    let parts = match parse_server_url(server_url) {
        Ok(p) => p,
        Err(_) => return MediaProtocol::Blossom,
    };
    let path = format!("{}/.well-known/nostr/nip96.json", parts.path_prefix);
    let mut conn = match HttpClient::connect(&parts.host, parts.port, parts.use_tls).await {
        Ok(c) => c,
        Err(_) => return MediaProtocol::Blossom,
    };
    let req = conn.request(Method::Get, &path);
    let (handler, state) = BodyCollector::new();
    if conn.send(req, handler).await.is_err() {
        return MediaProtocol::Blossom;
    }
    let guard = state.lock().unwrap();
    if !guard.success {
        return MediaProtocol::Blossom;
    }
    match extract_json_string_value(&guard.body, "api_url") {
        Some(api_url) => {
            let resolved = if api_url.starts_with("http://") || api_url.starts_with("https://") {
                api_url
            } else {
                let base = server_url.trim_end_matches('/');
                if api_url.starts_with('/') {
                    format!("{}{}", base, api_url)
                } else {
                    format!("{}/{}", base, api_url)
                }
            };
            MediaProtocol::Nip96 { api_url: resolved }
        }
        None => MediaProtocol::Blossom,
    }
}

// ── Upload ───────────────────────────────────────────────────────────

/// Upload a file to a Nostr media server. Returns `(url, file_hash)`.
pub async fn upload(
    server_url: &str,
    file_path: &str,
    secret_key_hex: &str,
) -> Result<(String, String), String> {
    let file_data = std::fs::read(file_path)
        .map_err(|e| format!("cannot read file: {}", e))?;
    let file_hash = sha256_hex(&file_data);
    let file_name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let content_type = mime_from_extension(file_name);

    let protocol = cached_protocol(server_url)
        .unwrap_or_else(|| {
            // Discovery requires async; run it in a blocking context
            // Since we're already in an async context, just use Blossom as initial guess
            MediaProtocol::Blossom
        });

    // Try the cached/default protocol; on failure, discover and try the other
    match do_upload(server_url, &protocol, &file_data, &file_hash, file_name, content_type, secret_key_hex).await {
        Ok(url) => {
            cache_protocol(server_url, protocol);
            Ok((url, file_hash))
        }
        Err(first_err) => {
            eprintln!("[media] first upload attempt failed: {}, trying discovery", first_err);
            let discovered = discover_protocol(server_url).await;
            match do_upload(server_url, &discovered, &file_data, &file_hash, file_name, content_type, secret_key_hex).await {
                Ok(url) => {
                    cache_protocol(server_url, discovered);
                    Ok((url, file_hash))
                }
                Err(second_err) => Err(format!("upload failed: {} (also tried: {})", second_err, first_err)),
            }
        }
    }
}

async fn do_upload(
    server_url: &str,
    protocol: &MediaProtocol,
    file_data: &[u8],
    file_hash: &str,
    file_name: &str,
    content_type: &str,
    secret_key_hex: &str,
) -> Result<String, String> {
    match protocol {
        MediaProtocol::Blossom => blossom_upload(server_url, file_data, file_hash, content_type, secret_key_hex).await,
        MediaProtocol::Nip96 { api_url } => nip96_upload(api_url, file_data, file_hash, file_name, content_type, secret_key_hex).await,
    }
}

async fn blossom_upload(
    server_url: &str,
    file_data: &[u8],
    file_hash: &str,
    content_type: &str,
    secret_key_hex: &str,
) -> Result<String, String> {
    let parts = parse_server_url(server_url)?;
    let path = format!("{}/upload", parts.path_prefix);
    let auth_event = create_blossom_auth_event("upload", file_hash, secret_key_hex)?;
    let auth_header = nostr_auth_header(&auth_event);

    let mut conn = HttpClient::connect(&parts.host, parts.port, parts.use_tls)
        .await
        .map_err(|e| format!("connect failed: {}", e))?;

    let mut req = conn.request(Method::Put, &path);
    req.header("Authorization", &auth_header)
       .header("Content-Type", content_type)
       .header("Content-Length", &file_data.len().to_string())
       .body(file_data.to_vec());

    let (handler, state) = BodyCollector::new();
    conn.send(req, handler)
        .await
        .map_err(|e| format!("send failed: {}", e))?;

    let guard = state.lock().unwrap();
    if let Some(ref err) = guard.error_msg {
        return Err(err.clone());
    }
    extract_upload_url(&guard.body)
        .ok_or_else(|| {
            let body_str = String::from_utf8_lossy(&guard.body);
            format!("no url in response: {}", &body_str[..body_str.len().min(200)])
        })
}

async fn nip96_upload(
    api_url: &str,
    file_data: &[u8],
    _file_hash: &str,
    file_name: &str,
    content_type: &str,
    secret_key_hex: &str,
) -> Result<String, String> {
    let upload_url = format!("{}/upload", api_url.trim_end_matches('/'));
    let parts = parse_server_url(&upload_url)?;
    let auth_event = create_nip98_auth_event(&upload_url, "POST", None, secret_key_hex)?;
    let auth_header = nostr_auth_header(&auth_event);

    let boundary = format!("----tagliacarte{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());

    let mut body = Vec::new();
    // file part
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n", file_name).as_bytes());
    body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", content_type).as_bytes());
    body.extend_from_slice(file_data);
    body.extend_from_slice(b"\r\n");
    // content_type part
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"content_type\"\r\n\r\n");
    body.extend_from_slice(content_type.as_bytes());
    body.extend_from_slice(b"\r\n");
    // closing boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let multipart_ct = format!("multipart/form-data; boundary={}", boundary);

    let mut conn = HttpClient::connect(&parts.host, parts.port, parts.use_tls)
        .await
        .map_err(|e| format!("connect failed: {}", e))?;

    let mut req = conn.request(Method::Post, &parts.path_prefix);
    req.header("Authorization", &auth_header)
       .header("Content-Type", &multipart_ct)
       .header("Content-Length", &body.len().to_string())
       .body(body);

    let (handler, state) = BodyCollector::new();
    conn.send(req, handler)
        .await
        .map_err(|e| format!("send failed: {}", e))?;

    let guard = state.lock().unwrap();
    if let Some(ref err) = guard.error_msg {
        return Err(err.clone());
    }
    extract_upload_url(&guard.body)
        .ok_or_else(|| {
            let body_str = String::from_utf8_lossy(&guard.body);
            format!("no url in response: {}", &body_str[..body_str.len().min(200)])
        })
}

// ── Delete ───────────────────────────────────────────────────────────

/// Delete a previously uploaded file from the media server.
pub async fn delete(
    server_url: &str,
    file_hash: &str,
    secret_key_hex: &str,
) -> Result<(), String> {
    let protocol = cached_protocol(server_url)
        .unwrap_or(MediaProtocol::Blossom);

    match &protocol {
        MediaProtocol::Blossom => blossom_delete(server_url, file_hash, secret_key_hex).await,
        MediaProtocol::Nip96 { api_url } => nip96_delete(api_url, file_hash, secret_key_hex).await,
    }
}

async fn blossom_delete(
    server_url: &str,
    file_hash: &str,
    secret_key_hex: &str,
) -> Result<(), String> {
    let parts = parse_server_url(server_url)?;
    let path = format!("{}/{}", parts.path_prefix, file_hash);
    let auth_event = create_blossom_auth_event("delete", file_hash, secret_key_hex)?;
    let auth_header = nostr_auth_header(&auth_event);

    let mut conn = HttpClient::connect(&parts.host, parts.port, parts.use_tls)
        .await
        .map_err(|e| format!("connect failed: {}", e))?;

    let mut req = conn.request(Method::Delete, &path);
    req.header("Authorization", &auth_header);

    conn.send(req, NoOpHandler)
        .await
        .map_err(|e| format!("delete failed: {}", e))?;
    Ok(())
}

async fn nip96_delete(
    api_url: &str,
    file_hash: &str,
    secret_key_hex: &str,
) -> Result<(), String> {
    let delete_url = format!("{}/{}", api_url.trim_end_matches('/'), file_hash);
    let parts = parse_server_url(&delete_url)?;
    let auth_event = create_nip98_auth_event(&delete_url, "DELETE", None, secret_key_hex)?;
    let auth_header = nostr_auth_header(&auth_event);

    let mut conn = HttpClient::connect(&parts.host, parts.port, parts.use_tls)
        .await
        .map_err(|e| format!("connect failed: {}", e))?;

    let mut req = conn.request(Method::Delete, &parts.path_prefix);
    req.header("Authorization", &auth_header);

    conn.send(req, NoOpHandler)
        .await
        .map_err(|e| format!("delete failed: {}", e))?;
    Ok(())
}
