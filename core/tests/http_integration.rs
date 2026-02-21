/*
 * http_integration.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Integration test for the HTTP client. Performs a real HTTPS GET to a known
 * h2-capable server and verifies the full request/response cycle including
 * ALPN negotiation, HPACK, and Huffman decoding.
 *
 * Run with:
 *   cargo test -p tagliacarte_core --test http_integration -- --nocapture
 * Or via the Makefile:
 *   make test-integration
 */

use std::sync::{Arc, Mutex};

use tagliacarte_core::protocol::http::client::HttpClient;
use tagliacarte_core::protocol::http::connection::HttpVersion;
use tagliacarte_core::protocol::http::Method;
use tagliacarte_core::protocol::http::{Response, ResponseHandler};

/// ResponseHandler that records all events for inspection and logging.
struct RecordingResponseHandler {
    status_code: Option<u16>,
    is_success: bool,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    completed: bool,
    failed: Option<String>,
}

impl RecordingResponseHandler {
    fn new() -> Self {
        Self {
            status_code: None,
            is_success: false,
            headers: Vec::new(),
            body: Vec::new(),
            completed: false,
            failed: None,
        }
    }
}

impl ResponseHandler for RecordingResponseHandler {
    fn ok(&mut self, response: Response) {
        self.status_code = Some(response.code);
        self.is_success = true;
    }
    fn error(&mut self, response: Response) {
        self.status_code = Some(response.code);
        self.is_success = false;
    }
    fn header(&mut self, name: &str, value: &str) {
        self.headers.push((name.to_string(), value.to_string()));
    }
    fn start_body(&mut self) {}
    fn body_chunk(&mut self, data: &[u8]) {
        self.body.extend_from_slice(data);
    }
    fn end_body(&mut self) {}
    fn complete(&mut self) {
        self.completed = true;
    }
    fn failed(&mut self, error: &std::io::Error) {
        self.failed = Some(error.to_string());
    }
}

/// Shared wrapper so we can pass the handler to send() which requires 'static.
struct SharedHandler(Arc<Mutex<RecordingResponseHandler>>);

impl ResponseHandler for SharedHandler {
    fn ok(&mut self, response: Response) {
        self.0.lock().unwrap().ok(response);
    }
    fn error(&mut self, response: Response) {
        self.0.lock().unwrap().error(response);
    }
    fn header(&mut self, name: &str, value: &str) {
        self.0.lock().unwrap().header(name, value);
    }
    fn start_body(&mut self) {
        self.0.lock().unwrap().start_body();
    }
    fn body_chunk(&mut self, data: &[u8]) {
        self.0.lock().unwrap().body_chunk(data);
    }
    fn end_body(&mut self) {
        self.0.lock().unwrap().end_body();
    }
    fn complete(&mut self) {
        self.0.lock().unwrap().complete();
    }
    fn failed(&mut self, error: &std::io::Error) {
        self.0.lock().unwrap().failed(error);
    }
}

#[tokio::test]
#[ignore] // requires network; run with: cargo test --test http_integration -- --ignored --nocapture
async fn get_svg_over_h2() {
    let host = "r2a.primal.net";
    let port = 443u16;
    let path = "/uploads2/0/50/fb/050fbcda05e13c2051cf857683645e19c6b180861ef4d0cc28a6f09ba1ea1666.svg";

    println!("=== HTTP/2 Integration Test ===");
    println!("Connecting to {}:{}...", host, port);

    let mut conn = HttpClient::connect(host, port, true)
        .await
        .expect("TLS connect failed");

    let version = conn.version();
    println!("Negotiated protocol: {:?}", version);
    assert_eq!(
        version,
        HttpVersion::Http2,
        "Expected h2 ALPN negotiation with {}",
        host
    );

    let mut req = conn.request(Method::Get, path);
    req.header("Accept", "*/*");
    req.header("User-Agent", "Tagliacarte/0.1 (integration-test)");

    println!("\n--- Request ---");
    println!("{} {} HTTP/2", req.method.as_str(), req.path);
    println!(":authority: {}", host);
    println!(":scheme: https");
    for (k, v) in &req.headers {
        println!("{}: {}", k, v);
    }

    let inner = Arc::new(Mutex::new(RecordingResponseHandler::new()));
    let handler = SharedHandler(inner.clone());

    conn.send(req, handler).await.expect("request failed");

    let h = inner.lock().unwrap();

    println!("\n--- Response ---");
    println!(
        "Status: {} ({})",
        h.status_code.unwrap_or(0),
        if h.is_success { "success" } else { "error" }
    );
    for (name, value) in &h.headers {
        println!("{}: {}", name, value);
    }
    println!("\nBody length: {} bytes", h.body.len());
    if h.body.len() < 4096 {
        println!("Body:\n{}", String::from_utf8_lossy(&h.body));
    }

    assert!(h.completed, "response should be complete");
    assert!(h.failed.is_none(), "request should not fail: {:?}", h.failed);
    assert_eq!(h.status_code, Some(200));
    assert!(h.is_success);
    assert!(!h.body.is_empty(), "body should not be empty");

    let body_str = String::from_utf8_lossy(&h.body);
    assert!(body_str.contains("<svg"), "body should be an SVG");

    let content_type = h
        .headers
        .iter()
        .find(|(k, _)| k == "content-type")
        .map(|(_, v)| v.as_str());
    println!("\nContent-Type: {:?}", content_type);
    assert!(
        content_type.is_some(),
        "response should have content-type header"
    );

    println!("\n=== PASS ===");
}
