/*
 * flow.rs
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

//! OAuth2 Authorization Code flow with PKCE for native desktop apps.
//!
//! 1. Generate PKCE code_verifier + code_challenge (S256).
//! 2. Open system browser to the authorization URL.
//! 3. Spin up an ephemeral `http://localhost:{port}` server to receive the redirect.
//! 4. Exchange the authorization code for tokens via HTTP POST.
//!
//! Token refresh is also provided here.
//!
//! HTTP calls use the in-tree `HttpClient` (no reqwest). JSON parsing uses the
//! in-tree `JsonParser` + `JsonContentHandler` (no serde_json).

use std::sync::{Arc, Mutex};

use bytes::BytesMut;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::json::{JsonContentHandler, JsonNumber, JsonParser};
use crate::protocol::http::client::HttpClient;
use crate::protocol::http::{Method, Response, ResponseHandler};

use super::provider::OAuthProvider;

/// Tokens returned from the OAuth2 authorization code exchange or refresh.
#[derive(Debug, Clone)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Seconds until the access token expires (from the provider's `expires_in` field).
    pub expires_in: Option<u64>,
}

/// Start the OAuth2 Authorization Code flow with PKCE.
///
/// `on_auth_url` is called with the authorization URL that the UI should open in the system browser.
/// This function blocks (async) until the redirect is received and the code is exchanged for tokens.
pub async fn start_oauth_flow(
    provider: &dyn OAuthProvider,
    on_auth_url: impl FnOnce(&str) + Send,
) -> Result<OAuthTokens, String> {
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let state = generate_state();

    // Bind to a random available port on localhost.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("failed to bind localhost listener: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to get local address: {}", e))?
        .port();
    let redirect_uri = format!("http://localhost:{}", port);

    // Build authorization URL.
    let scopes = provider.scopes().join(" ");
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        provider.auth_url(),
        percent_encode(provider.client_id()),
        percent_encode(&redirect_uri),
        percent_encode(&scopes),
        percent_encode(&code_challenge),
        percent_encode(&state),
    );

    // Notify caller with the URL to open in the browser.
    on_auth_url(&auth_url);

    // Wait for the redirect callback.
    let (mut socket, _addr) = listener
        .accept()
        .await
        .map_err(|e| format!("failed to accept redirect connection: {}", e))?;

    let mut buf = vec![0u8; 8192];
    let n = socket
        .read(&mut buf)
        .await
        .map_err(|e| format!("failed to read redirect request: {}", e))?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the authorization code and state from the GET request.
    let code = extract_query_param(&request, "code")
        .ok_or_else(|| "no 'code' parameter in redirect".to_string())?;
    let received_state = extract_query_param(&request, "state")
        .ok_or_else(|| "no 'state' parameter in redirect".to_string())?;

    if received_state != state {
        let error_response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body><h1>Authorization failed</h1><p>State mismatch. Please try again.</p></body></html>";
        let _ = socket.write_all(error_response.as_bytes()).await;
        return Err("OAuth2 state mismatch — possible CSRF attack".to_string());
    }

    // Check for error parameter.
    if let Some(error) = extract_query_param(&request, "error") {
        let desc = extract_query_param(&request, "error_description")
            .unwrap_or_else(|| error.clone());
        let error_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body><h1>Authorization failed</h1><p>{}</p></body></html>",
            html_escape(&desc)
        );
        let _ = socket.write_all(error_response.as_bytes()).await;
        return Err(format!("OAuth2 error: {}", desc));
    }

    // Send success page to the browser.
    let success_response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body><h1>Authorization successful</h1><p>You can close this window and return to Tagliacarte.</p></body></html>";
    let _ = socket.write_all(success_response.as_bytes()).await;
    drop(socket);

    // Exchange the authorization code for tokens.
    exchange_code(provider, &code, &redirect_uri, &code_verifier).await
}

/// Refresh an access token using a refresh token.
pub async fn refresh_access_token(
    provider: &dyn OAuthProvider,
    refresh_token_str: &str,
) -> Result<OAuthTokens, String> {
    let scopes = provider.scopes().join(" ");
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}&scope={}",
        percent_encode(refresh_token_str),
        percent_encode(provider.client_id()),
        percent_encode(&scopes),
    );

    post_token_request(provider.token_url(), &body, "token refresh").await
}

/// Exchange the authorization code for tokens.
async fn exchange_code(
    provider: &dyn OAuthProvider,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<OAuthTokens, String> {
    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
        percent_encode(code),
        percent_encode(redirect_uri),
        percent_encode(provider.client_id()),
        percent_encode(code_verifier),
    );

    post_token_request(provider.token_url(), &body, "token exchange").await
}

// ── HTTP token POST using in-tree HttpClient ──────────────────────────

/// Parse a full URL into (host, port, path). Only handles https:// URLs.
fn parse_https_url(url: &str) -> Result<(&str, u16, &str), String> {
    let rest = url.strip_prefix("https://")
        .ok_or_else(|| format!("expected https:// URL: {}", url))?;
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.find(':') {
        Some(i) => (&host_port[..i], host_port[i + 1..].parse::<u16>()
            .map_err(|_| format!("invalid port in URL: {}", url))?),
        None => (host_port, 443),
    };
    Ok((host, port, path))
}

/// Send a POST to a token endpoint and parse the JSON response into OAuthTokens.
async fn post_token_request(
    token_url: &str,
    form_body: &str,
    context: &str,
) -> Result<OAuthTokens, String> {
    let (host, port, path) = parse_https_url(token_url)?;

    let mut conn = HttpClient::connect(host, port, true)
        .await
        .map_err(|e| format!("{} connect failed: {}", context, e))?;

    let body_bytes = form_body.as_bytes().to_vec();
    let content_length = body_bytes.len().to_string();
    let mut req = conn.request(Method::Post, path);
    req.header("Content-Type", "application/x-www-form-urlencoded")
       .header("Content-Length", &content_length)
       .body(body_bytes);

    let state = Arc::new(Mutex::new(TokenResponseState::new()));
    let handler = TokenResponseHandler { state: state.clone() };
    conn.send(req, handler)
        .await
        .map_err(|e| format!("{} request failed: {}", context, e))?;

    let state = state.lock().unwrap();
    if !state.success {
        return Err(format!(
            "{} failed ({}): {}",
            context,
            state.status_code,
            String::from_utf8_lossy(&state.body_buf),
        ));
    }

    // Parse the collected body JSON.
    parse_token_response_bytes(&state.body_buf)
}

/// Shared state for the token response handler.
/// Token responses are tiny (~200 bytes), so buffering here is fine.
struct TokenResponseState {
    success: bool,
    status_code: u16,
    body_buf: Vec<u8>,
}

impl TokenResponseState {
    fn new() -> Self {
        Self {
            success: false,
            status_code: 0,
            body_buf: Vec::with_capacity(1024),
        }
    }
}

/// ResponseHandler that collects the body and records the status into shared state.
struct TokenResponseHandler {
    state: Arc<Mutex<TokenResponseState>>,
}

impl ResponseHandler for TokenResponseHandler {
    fn ok(&mut self, response: Response) {
        if let Ok(mut s) = self.state.lock() {
            s.success = true;
            s.status_code = response.code;
        }
    }

    fn error(&mut self, response: Response) {
        if let Ok(mut s) = self.state.lock() {
            s.success = false;
            s.status_code = response.code;
        }
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        if let Ok(mut s) = self.state.lock() {
            s.body_buf.extend_from_slice(data);
        }
    }

    fn end_body(&mut self) {}
    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        if let Ok(mut s) = self.state.lock() {
            s.success = false;
            s.body_buf.clear();
            s.body_buf.extend_from_slice(error.to_string().as_bytes());
        }
    }
}

/// Parse a JSON token response into OAuthTokens using the push parser.
fn parse_token_response_bytes(data: &[u8]) -> Result<OAuthTokens, String> {
    let mut handler = TokenJsonHandler::default();
    let mut parser = JsonParser::new();
    let mut buf = BytesMut::from(data);
    parser
        .receive(&mut buf, &mut handler)
        .map_err(|e| format!("invalid token JSON: {}", e))?;
    parser
        .close(&mut handler)
        .map_err(|e| format!("incomplete token JSON: {}", e))?;

    let access_token = handler
        .access_token
        .ok_or("missing access_token in response")?;

    Ok(OAuthTokens {
        access_token,
        refresh_token: handler.refresh_token,
        expires_in: handler.expires_in,
    })
}

/// Push-parser handler for OAuth token JSON responses.
/// Extracts `access_token`, `refresh_token`, `expires_in`.
#[derive(Default)]
struct TokenJsonHandler {
    current_key: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

impl JsonContentHandler for TokenJsonHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        match self.current_key.as_deref() {
            Some("access_token") => self.access_token = Some(value.to_string()),
            Some("refresh_token") => self.refresh_token = Some(value.to_string()),
            _ => {}
        }
        self.current_key = None;
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.current_key.as_deref() == Some("expires_in") {
            self.expires_in = number.as_i64().map(|n| n as u64);
        }
        self.current_key = None;
    }

    fn boolean_value(&mut self, _value: bool) {
        self.current_key = None;
    }

    fn null_value(&mut self) {
        self.current_key = None;
    }
}

// ── PKCE helpers ──────────────────────────────────────────────────────

/// Generate a random PKCE code verifier (43–128 characters, base64url-encoded).
fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");
    base64url_encode(&bytes)
}

/// Compute the S256 code challenge: `BASE64URL(SHA256(code_verifier))`.
fn generate_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64url_encode(&digest)
}

/// Generate a random state parameter (16 bytes, base64url-encoded).
fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");
    base64url_encode(&bytes)
}

/// Base64url encode without padding (RFC 4648 §5).
fn base64url_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let n = (chunk[0] as usize) << 16
            | (chunk.get(1).copied().unwrap_or(0) as usize) << 8
            | chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(ALPHABET[n >> 18] as char);
        out.push(ALPHABET[(n >> 12) & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n & 63] as char);
        }
    }
    out
}

// ── URL / query helpers ───────────────────────────────────────────────

/// Percent-encode a string for use in URL query parameters.
fn percent_encode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(
        s,
        &percent_encoding::NON_ALPHANUMERIC,
    )
    .to_string()
}

/// Extract a query parameter from an HTTP GET request line.
/// Expects the first line to be "GET /path?key=val&... HTTP/1.1".
fn extract_query_param(request: &str, param: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next()?;
        let value = kv.next().unwrap_or("");
        if key == param {
            return Some(percent_decode(value));
        }
    }
    None
}

/// Simple percent-decode for query parameter values.
fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

/// Minimal HTML escaping for error messages.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64url_encode() {
        // Empty input
        assert_eq!(base64url_encode(b""), "");
        // Known test vector: "f" → "Zg"
        assert_eq!(base64url_encode(b"f"), "Zg");
        // "fo" → "Zm8"
        assert_eq!(base64url_encode(b"fo"), "Zm8");
        // "foo" → "Zm9v"
        assert_eq!(base64url_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn test_pkce_code_challenge() {
        // RFC 7636 Appendix B test vector:
        // code_verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        // S256 code_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = generate_code_challenge(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn test_extract_query_param() {
        let request = "GET /?code=abc123&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(extract_query_param(request, "code"), Some("abc123".to_string()));
        assert_eq!(extract_query_param(request, "state"), Some("xyz".to_string()));
        assert_eq!(extract_query_param(request, "missing"), None);
    }

    #[test]
    fn test_parse_https_url() {
        let (host, port, path) = parse_https_url("https://oauth2.googleapis.com/token").unwrap();
        assert_eq!(host, "oauth2.googleapis.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/token");

        let (host, port, path) = parse_https_url("https://login.microsoftonline.com/common/oauth2/v2.0/token").unwrap();
        assert_eq!(host, "login.microsoftonline.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/common/oauth2/v2.0/token");
    }

    #[test]
    fn test_parse_token_response() {
        let json = r#"{"access_token":"ya29.xxx","token_type":"Bearer","expires_in":3600,"refresh_token":"1//0abc"}"#;
        let tokens = parse_token_response_bytes(json.as_bytes()).unwrap();
        assert_eq!(tokens.access_token, "ya29.xxx");
        assert_eq!(tokens.refresh_token.as_deref(), Some("1//0abc"));
        assert_eq!(tokens.expires_in, Some(3600));
    }
}
