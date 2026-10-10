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
//! The loopback listener and the token POST both run on hopf; the flow
//! reports through a callback, with blocking wrappers for FFI entry points.
//! JSON parsing uses the rjsonparser `ContentHandler`.

use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use hopf_core::{BindingId, Endpoint, ProtocolHandler, TcpListenerConfig};
use sha2::{Digest, Sha256};

use crate::json::{parse_bytes_complete, ContentHandler, Number, ParseResult};
use crate::net::hopf_runtime;
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
    /// OpenID Connect JWT from Google/Microsoft (when `openid` scope granted).
    pub id_token: Option<String>,
}

/// Hears the outcome of a flow or a refresh, once, from hopf's reactor.
pub type OnTokens = Box<dyn FnOnce(Result<OAuthTokens, String>) + Send>;

/// What the flow needs from a provider once it continues from hopf's
/// threads, where the caller's `&dyn OAuthProvider` is out of reach.
#[derive(Clone)]
struct ProviderInfo {
    token_url: String,
    client_id: String,
    client_secret: Option<String>,
    scopes: String,
}

impl ProviderInfo {
    fn of(provider: &dyn OAuthProvider) -> Self {
        Self {
            token_url: provider.token_url().to_string(),
            client_id: provider.client_id().to_string(),
            client_secret: provider.client_secret().map(str::to_string),
            scopes: provider.scopes().join(" "),
        }
    }
}

/// Start the OAuth2 Authorization Code flow with PKCE.
///
/// `on_auth_url` is called (before this returns) with the authorization
/// URL that the UI should open in the system browser. `on_done` gets the
/// tokens once the redirect has arrived on the loopback listener and the
/// code has been exchanged, or the reason that did not happen.
pub fn start_oauth_flow(provider: &dyn OAuthProvider, on_auth_url: impl FnOnce(&str), on_done: OnTokens) {
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let state = generate_state();
    let info = ProviderInfo::of(provider);

    // Bind to a random available port on localhost.
    let redirect = Arc::new(Mutex::new(Redirect { expected_state: state.clone(), on_code: None, binding: None }));
    let factory_redirect = Arc::clone(&redirect);
    let listen: SocketAddr = "127.0.0.1:0".parse().expect("loopback address");
    let (addr, binding) = match hopf_runtime().add_tcp_listener(TcpListenerConfig::new(listen, move || {
        Box::new(RedirectHandler { shared: Arc::clone(&factory_redirect), buf: Vec::new() }) as Box<dyn ProtocolHandler>
    })) {
        Ok(bound) => bound,
        Err(e) => {
            on_done(Err(format!("failed to bind localhost listener: {}", e)));
            return;
        }
    };
    let redirect_uri = format!("http://localhost:{}", addr.port());
    {
        let mut g = redirect.lock().unwrap();
        g.binding = Some(binding);
        let redirect_uri = redirect_uri.clone();
        let info = info.clone();
        g.on_code = Some(Box::new(move |code| match code {
            Ok(code) => exchange_code(&info, &code, &redirect_uri, &code_verifier, on_done),
            Err(e) => on_done(Err(e)),
        }));
    }

    // Build authorization URL.
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        provider.auth_url(),
        percent_encode(&info.client_id),
        percent_encode(&redirect_uri),
        percent_encode(&info.scopes),
        percent_encode(&code_challenge),
        percent_encode(&state),
    );

    // Notify caller with the URL to open in the browser.
    on_auth_url(&auth_url);
}

/// [`start_oauth_flow`] waited for on this thread (an FFI entry point,
/// never a hopf reactor thread). There is no time limit: the user is in
/// the browser.
pub fn start_oauth_flow_blocking(provider: &dyn OAuthProvider, on_auth_url: impl FnOnce(&str)) -> Result<OAuthTokens, String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    start_oauth_flow(
        provider,
        on_auth_url,
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    rx.recv().unwrap_or_else(|_| Err("OAuth flow ended without a result".to_string()))
}

/// The loopback listener's state: one redirect is expected; whoever
/// brings it (the browser may open several connections) reports it.
struct Redirect {
    expected_state: String,
    on_code: Option<Box<dyn FnOnce(Result<String, String>) + Send>>,
    binding: Option<BindingId>,
}

/// One browser connection to the loopback listener.
struct RedirectHandler {
    shared: Arc<Mutex<Redirect>>,
    buf: Vec<u8>,
}

const REDIRECT_MAX_REQUEST: usize = 16 * 1024;

fn html_page(status: &str, title: &str, text: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body><h1>{title}</h1><p>{text}</p></body></html>"
    )
}

impl RedirectHandler {
    /// Handle one complete request head; returns the page to send.
    fn answer(&mut self, request: &str) -> String {
        let code = extract_query_param(request, "code");
        let error = extract_query_param(request, "error");
        if code.is_none() && error.is_none() {
            // Not the redirect (a favicon, say).
            return html_page("404 Not Found", "Not found", "");
        }
        let (on_code, expected_state, binding) = {
            let mut g = self.shared.lock().unwrap();
            (g.on_code.take(), g.expected_state.clone(), g.binding.take())
        };
        let Some(on_code) = on_code else {
            return html_page("200 OK", "Authorization already handled", "You can close this window.");
        };
        if let Some(id) = binding {
            hopf_runtime().remove_binding(id);
        }
        let received_state = extract_query_param(request, "state");
        if received_state.as_deref() != Some(expected_state.as_str()) {
            on_code(Err("OAuth2 state mismatch - possible CSRF attack".to_string()));
            return html_page("400 Bad Request", "Authorization failed", "State mismatch. Please try again.");
        }
        if let Some(error) = error {
            let desc = extract_query_param(request, "error_description").unwrap_or_else(|| error.clone());
            on_code(Err(format!("OAuth2 error: {}", desc)));
            return html_page("200 OK", "Authorization failed", &html_escape(&desc));
        }
        match code {
            Some(code) => {
                on_code(Ok(code));
                html_page("200 OK", "Authorization successful", "You can close this window and return to Tagliacarte.")
            }
            None => {
                on_code(Err("no 'code' parameter in redirect".to_string()));
                html_page("400 Bad Request", "Authorization failed", "No code in the redirect.")
            }
        }
    }
}

impl ProtocolHandler for RedirectHandler {
    fn connected(&mut self, _ep: &mut dyn Endpoint) {}

    fn receive(&mut self, ep: &mut dyn Endpoint, data: &mut &[u8]) {
        self.buf.extend_from_slice(data);
        *data = &[];
        let Some(end) = self.buf.windows(4).position(|w| w == b"\r\n\r\n") else {
            if self.buf.len() > REDIRECT_MAX_REQUEST {
                ep.send(html_page("431 Request Header Fields Too Large", "Request too large", "").as_bytes());
                ep.close();
            }
            return;
        };
        let request = String::from_utf8_lossy(&self.buf[..end]).into_owned();
        self.buf.clear();
        let page = self.answer(&request);
        ep.send(page.as_bytes());
        ep.close();
    }

    fn disconnected(&mut self, _ep: &mut dyn Endpoint) {}

    fn error(&mut self, _ep: &mut dyn Endpoint, _err: &io::Error) {}
}

/// Refresh an access token using a refresh token; `on_done` hears the
/// outcome from hopf's reactor.
pub fn refresh_access_token(provider: &dyn OAuthProvider, refresh_token_str: &str, on_done: OnTokens) {
    let info = ProviderInfo::of(provider);
    let mut body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        percent_encode(refresh_token_str),
        percent_encode(&info.client_id),
    );
    if let Some(secret) = info.client_secret.as_deref() {
        body.push_str("&client_secret=");
        body.push_str(&percent_encode(secret));
    }
    body.push_str("&scope=");
    body.push_str(&percent_encode(&info.scopes));
    post_token_request(&info.token_url, body, "token refresh", on_done)
}

/// [`refresh_access_token`] waited for on this thread; never call it from
/// a hopf reactor thread.
pub fn refresh_access_token_blocking(provider: &dyn OAuthProvider, refresh_token_str: &str) -> Result<OAuthTokens, String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    refresh_access_token(
        provider,
        refresh_token_str,
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    rx.recv().unwrap_or_else(|_| Err("token refresh ended without a result".to_string()))
}

/// Exchange the authorization code for tokens.
fn exchange_code(info: &ProviderInfo, code: &str, redirect_uri: &str, code_verifier: &str, on_done: OnTokens) {
    let mut body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}",
        percent_encode(code),
        percent_encode(redirect_uri),
        percent_encode(&info.client_id),
    );
    if let Some(secret) = info.client_secret.as_deref() {
        body.push_str("&client_secret=");
        body.push_str(&percent_encode(secret));
    }
    body.push_str("&code_verifier=");
    body.push_str(&percent_encode(code_verifier));
    post_token_request(&info.token_url, body, "token exchange", on_done)
}

// ── HTTP token POST on the hopf HTTP client ───────────────────────────

/// Parse a full URL into (host, port, path). Only handles https:// URLs.
fn parse_https_url(url: &str) -> Result<(&str, u16, &str), String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("expected https:// URL: {}", url))?;
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.find(':') {
        Some(i) => (
            &host_port[..i],
            host_port[i + 1..]
                .parse::<u16>()
                .map_err(|_| format!("invalid port in URL: {}", url))?,
        ),
        None => (host_port, 443),
    };
    Ok((host, port, path))
}

/// POST to a token endpoint; the JSON response is parsed into
/// [`OAuthTokens`] for `on_done`.
fn post_token_request(token_url: &str, form_body: String, context: &'static str, on_done: OnTokens) {
    let (host, port, path) = match parse_https_url(token_url) {
        Ok(p) => p,
        Err(e) => {
            on_done(Err(e));
            return;
        }
    };
    let path = path.to_string();
    HttpClient::connect(
        host,
        port,
        true,
        Box::new(move |conn| {
            let conn = match conn {
                Ok(c) => c,
                Err(e) => {
                    on_done(Err(format!("{} connect failed: {}", context, e)));
                    return;
                }
            };
            let body_bytes = form_body.into_bytes();
            let content_length = body_bytes.len().to_string();
            let mut req = conn.request(Method::Post, path);
            req.header("Content-Type", "application/x-www-form-urlencoded")
                .header("Content-Length", &content_length)
                .body(body_bytes);
            let handler = TokenResponseHandler {
                success: false,
                status_code: 0,
                body_buf: Vec::with_capacity(1024),
                context,
                done: Some(on_done),
                _keep: conn.clone(),
            };
            conn.send(req, handler);
        }),
    );
}

/// Collects the (tiny) token response and reports it once.
struct TokenResponseHandler {
    success: bool,
    status_code: u16,
    body_buf: Vec<u8>,
    context: &'static str,
    done: Option<OnTokens>,
    /// The connection lives until the response is in.
    _keep: crate::protocol::http::HttpConnection,
}

impl ResponseHandler for TokenResponseHandler {
    fn ok(&mut self, response: Response) {
        self.success = true;
        self.status_code = response.code;
    }

    fn error(&mut self, response: Response) {
        self.success = false;
        self.status_code = response.code;
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        self.body_buf.extend_from_slice(data);
    }

    fn end_body(&mut self) {}

    fn complete(&mut self) {
        let Some(done) = self.done.take() else { return };
        if !self.success {
            done(Err(format!(
                "{} failed ({}): {}",
                self.context,
                self.status_code,
                String::from_utf8_lossy(&self.body_buf),
            )));
            return;
        }
        done(parse_token_response_bytes(&self.body_buf));
    }

    fn failed(&mut self, error: &std::io::Error) {
        if let Some(done) = self.done.take() {
            done(Err(format!("{} request failed: {}", self.context, error)));
        }
    }
}

/// Parse a JSON token response into OAuthTokens using the push parser.
fn parse_token_response_bytes(data: &[u8]) -> Result<OAuthTokens, String> {
    let mut handler = TokenJsonHandler::default();
    parse_bytes_complete(data, &mut handler).map_err(|e| format!("invalid token JSON: {}", e))?;

    let access_token = handler
        .access_token
        .ok_or("missing access_token in response")?;

    Ok(OAuthTokens {
        access_token,
        refresh_token: handler.refresh_token,
        expires_in: handler.expires_in,
        id_token: handler.id_token,
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
    id_token: Option<String>,
}

impl ContentHandler for TokenJsonHandler {
    fn start_object(&mut self) -> ParseResult<()> { Ok(()) }
    fn end_object(&mut self) -> ParseResult<()> { Ok(()) }
    fn start_array(&mut self) -> ParseResult<()> { Ok(()) }
    fn end_array(&mut self) -> ParseResult<()> { Ok(()) }

    fn key(&mut self, key: &str) -> ParseResult<()> {
        self.current_key = Some(key.to_string());
        Ok(())
    }

    fn string_value(&mut self, value: &str) -> ParseResult<()> {
        match self.current_key.as_deref() {
            Some("access_token") => self.access_token = Some(value.to_string()),
            Some("refresh_token") => self.refresh_token = Some(value.to_string()),
            Some("id_token") => self.id_token = Some(value.to_string()),
            _ => {}
        }
        self.current_key = None;
        Ok(())
    }

    fn number_value(&mut self, number: &Number) -> ParseResult<()> {
        if self.current_key.as_deref() == Some("expires_in") {
            self.expires_in = number.as_i64().map(|n| n as u64);
        }
        self.current_key = None;
        Ok(())
    }

    fn boolean_value(&mut self, _value: bool) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
    }

    fn null_value(&mut self) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
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
    percent_encoding::utf8_percent_encode(s, &percent_encoding::NON_ALPHANUMERIC).to_string()
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
        assert_eq!(
            extract_query_param(request, "code"),
            Some("abc123".to_string())
        );
        assert_eq!(
            extract_query_param(request, "state"),
            Some("xyz".to_string())
        );
        assert_eq!(extract_query_param(request, "missing"), None);
    }

    #[test]
    fn test_parse_https_url() {
        let (host, port, path) = parse_https_url("https://oauth2.googleapis.com/token").unwrap();
        assert_eq!(host, "oauth2.googleapis.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/token");

        let (host, port, path) =
            parse_https_url("https://login.microsoftonline.com/common/oauth2/v2.0/token").unwrap();
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
