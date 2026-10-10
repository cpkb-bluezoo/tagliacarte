//! End-to-end: the OAuth2 PKCE flow on hopf, in process. A scripted HTTPS
//! token endpoint stands in for the provider, and a plain TCP client plays
//! the browser being redirected to the loopback listener.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use hopf_core::{acceptor_from_pem, insecure_connector, ProtocolHandler, TcpListenerConfig};
use hopf_http::{AlpnHttpEndpoint, Headers, HttpLimits, ServerHandler, ServerHandlerFactory, ServerWriter};
use tagliacarte_core::net::{hopf_runtime, set_hopf_http_tls_connector};
use tagliacarte_core::oauth::{refresh_access_token_blocking, start_oauth_flow, OAuthProvider, OAuthTokens};

/// Records every token request's form body and answers with tokens.
struct TokenEndpoint {
    bodies: Arc<Mutex<Vec<String>>>,
    body: Vec<u8>,
}

impl ServerHandler for TokenEndpoint {
    fn headers(&mut self, _response: &mut dyn ServerWriter, _headers: &Headers) {}
    fn request_body_content(&mut self, _response: &mut dyn ServerWriter, data: &[u8]) {
        self.body.extend_from_slice(data);
    }
    fn request_complete(&mut self, response: &mut dyn ServerWriter) {
        let form = String::from_utf8_lossy(&self.body).into_owned();
        let kind = if form.contains("grant_type=refresh_token") { "refreshed" } else { "exchanged" };
        self.bodies.lock().unwrap().push(form);
        let mut h = Headers::new();
        h.set(":status", "200");
        h.set("content-type", "application/json");
        response.headers(h);
        response.response_body_content(
            format!("{{\"access_token\":\"at-{kind}\",\"refresh_token\":\"rt-1\",\"expires_in\":3600,\"id_token\":\"jwt\"}}").as_bytes(),
        );
        response.end_response_body();
        response.complete();
    }
}

struct Factory(Arc<Mutex<Vec<String>>>);
impl ServerHandlerFactory for Factory {
    fn create_handler(&self) -> Box<dyn ServerHandler> {
        Box::new(TokenEndpoint { bodies: Arc::clone(&self.0), body: Vec::new() })
    }
}

fn start_token_server() -> (SocketAddr, Arc<Mutex<Vec<String>>>) {
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    std::fs::write(&cert_path, ck.cert.pem()).unwrap();
    std::fs::write(&key_path, ck.key_pair.serialize_pem()).unwrap();
    let acceptor = acceptor_from_pem(&cert_path, &key_path, &[b"h2", b"http/1.1"]).unwrap();
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let factory: Arc<dyn ServerHandlerFactory> = Arc::new(Factory(Arc::clone(&bodies)));
    let (addr, _) = hopf_runtime()
        .add_tcp_listener(
            TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
                Box::new(AlpnHttpEndpoint::new(Arc::clone(&factory), HttpLimits::default())) as Box<dyn ProtocolHandler>
            })
            .with_tls(acceptor),
        )
        .unwrap();
    // Self-signed test certificate: skip verification for this process.
    let _ = set_hopf_http_tls_connector(insecure_connector(&[b"h2", b"http/1.1"]));
    (addr, bodies)
}

struct TestProvider {
    token_url: String,
}

impl OAuthProvider for TestProvider {
    fn provider_id(&self) -> &str {
        "test"
    }
    fn auth_url(&self) -> &str {
        "https://auth.example/authorize"
    }
    fn token_url(&self) -> &str {
        &self.token_url
    }
    fn scopes(&self) -> &[&str] {
        &["mail", "openid"]
    }
    fn client_id(&self) -> &str {
        "client-1"
    }
    fn client_secret(&self) -> Option<&str> {
        Some("s3cret")
    }
}

fn query_param(url: &str, name: &str) -> String {
    let q = url.split('?').nth(1).unwrap_or("");
    q.split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{name}=")))
        .map(|v| v.replace("%3A", ":").replace("%2F", "/"))
        .unwrap_or_else(|| panic!("{name} in {url}"))
}

/// Play the browser: hit the loopback redirect URI with the given query.
fn browser_redirect(redirect_uri: &str, query: &str) -> String {
    let authority = redirect_uri.strip_prefix("http://").unwrap();
    let port: u16 = authority.rsplit(':').next().unwrap().parse().unwrap();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(s, "GET /?{query} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n").unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

fn run_flow(provider: &TestProvider) -> (String, mpsc::Receiver<Result<OAuthTokens, String>>) {
    let (tx, rx) = mpsc::channel();
    let auth_url = Arc::new(Mutex::new(String::new()));
    let a2 = Arc::clone(&auth_url);
    start_oauth_flow(provider, move |u| *a2.lock().unwrap() = u.to_string(), Box::new(move |r| tx.send(r).unwrap()));
    let url = auth_url.lock().unwrap().clone();
    assert!(url.starts_with("https://auth.example/authorize?"), "{url}");
    (url, rx)
}

#[test]
fn redirect_to_the_loopback_listener_exchanges_the_code() {
    let (addr, bodies) = start_token_server();
    let provider = TestProvider { token_url: format!("https://{}:{}/token", addr.ip(), addr.port()) };
    let (url, rx) = run_flow(&provider);
    let redirect_uri = query_param(&url, "redirect_uri");
    let state = query_param(&url, "state");
    assert!(url.contains("code_challenge_method=S256"));

    // A stray request first (the browser asking for a favicon) must not
    // consume the flow.
    let stray = browser_redirect(&redirect_uri, "x=1");
    assert!(stray.starts_with("HTTP/1.1 404"), "{stray}");

    let page = browser_redirect(&redirect_uri, &format!("code=abc123&state={state}"));
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("Authorization successful"), "{page}");

    let tokens = rx.recv_timeout(Duration::from_secs(10)).expect("flow reports").expect("tokens");
    assert_eq!(tokens.access_token, "at-exchanged");
    assert_eq!(tokens.refresh_token.as_deref(), Some("rt-1"));
    assert_eq!(tokens.expires_in, Some(3600));
    assert_eq!(tokens.id_token.as_deref(), Some("jwt"));
    let form = bodies.lock().unwrap().last().cloned().unwrap();
    assert!(form.contains("grant_type=authorization_code"), "{form}");
    assert!(form.contains("code=abc123"), "{form}");
    assert!(form.contains("client_secret=s3cret"), "{form}");
    assert!(form.contains("code_verifier="), "{form}");

    // The listener is gone once the redirect has been handled.
    let port: u16 = redirect_uri.rsplit(':').next().unwrap().parse().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "listener removed after the redirect");
}

#[test]
fn state_mismatch_and_provider_error_are_reported() {
    let (addr, _) = start_token_server();
    let provider = TestProvider { token_url: format!("https://{}:{}/token", addr.ip(), addr.port()) };

    let (url, rx) = run_flow(&provider);
    let page = browser_redirect(&query_param(&url, "redirect_uri"), "code=abc&state=wrong");
    assert!(page.starts_with("HTTP/1.1 400"), "{page}");
    let err = rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap_err();
    assert!(err.contains("state mismatch"), "{err}");

    let (url, rx) = run_flow(&provider);
    let state = query_param(&url, "state");
    let page = browser_redirect(&query_param(&url, "redirect_uri"), &format!("error=access_denied&error_description=nope&state={state}"));
    assert!(page.contains("Authorization failed"), "{page}");
    let err = rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap_err();
    assert!(err.contains("nope"), "{err}");
}

#[test]
fn refresh_posts_the_refresh_grant_and_waits() {
    let (addr, bodies) = start_token_server();
    let provider = TestProvider { token_url: format!("https://{}:{}/token", addr.ip(), addr.port()) };
    let tokens = refresh_access_token_blocking(&provider, "rt-old").unwrap();
    assert_eq!(tokens.access_token, "at-refreshed");
    let form = bodies.lock().unwrap().last().cloned().unwrap();
    assert!(form.contains("grant_type=refresh_token"), "{form}");
    assert!(form.contains("refresh_token=rt%2Dold"), "{form}");
    assert!(form.contains("scope=mail%20openid") || form.contains("scope=mail+openid"), "{form}");
}
