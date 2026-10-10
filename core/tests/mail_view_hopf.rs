//! End-to-end: the mail-view HTTPS server (now on hopf-http) with mutual
//! TLS, driven by hopf's own HTTP client: a small response, a large chunked
//! one that exercises the drain backpressure, and a client without a
//! certificate that is refused.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hopf_core::{connector_from_pem, connector_from_pem_with_client_cert, SharedTlsConnector};
use hopf_http::{HttpClient, HttpClientSessionHandle, HttpConnectionHandler, HttpResponseHandler};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::http::mail_view_server::{
    start_mail_view_server, MtlsMaterial, ParsedRequest, RequestHandler, ResponseSink,
};

const BIG: usize = 3 * 1024 * 1024;

fn big_byte(i: usize) -> u8 {
    (i % 251) as u8
}

/// Answers on the reactor: a small body at once, the big one in pieces
/// with the drain waits the sink asks for.
fn answer(mut sink: ResponseSink, req: ParsedRequest) {
    let path = req.target.split('?').next().unwrap_or("/");
    match path {
        "/small" => {
            let _ = sink.bytes(200, &[("Content-Type", "text/plain")], b"hello");
        }
        "/big" => {
            if sink.head(200, &[("Content-Type", "application/octet-stream"), ("Transfer-Encoding", "chunked")]).is_err() {
                return;
            }
            let body: Vec<u8> = (0..BIG).map(big_byte).collect();
            sink.write_all(
                body,
                Box::new(|r| {
                    if let Ok(mut s) = r {
                        let _ = s.end();
                    }
                }),
            );
        }
        _ => {
            let _ = sink.bytes(404, &[], b"Not Found");
        }
    }
}

#[derive(Default)]
struct Outcome {
    status: u16,
    body: Vec<u8>,
    done: bool,
    failed: Option<String>,
    refused: Option<String>,
}

struct Collect(Arc<Mutex<Outcome>>);

impl HttpResponseHandler for Collect {
    fn ok(&mut self, status: u16) {
        self.0.lock().unwrap().status = status;
    }
    fn error(&mut self, status: u16) {
        self.0.lock().unwrap().status = status;
    }
    fn header(&mut self, _n: &str, _v: &str) {}
    fn response_body_content(&mut self, data: &[u8]) {
        self.0.lock().unwrap().body.extend_from_slice(data);
    }
    fn close(&mut self) {
        self.0.lock().unwrap().done = true;
    }
    fn failed(&mut self, err: io::Error) {
        let mut g = self.0.lock().unwrap();
        g.failed = Some(err.to_string());
        g.done = true;
    }
}

struct GetOnce {
    path: &'static str,
    out: Arc<Mutex<Outcome>>,
}

impl HttpConnectionHandler for GetOnce {
    fn on_connected(&mut self, session: &mut HttpClientSessionHandle) {
        let _ = session.get(self.path).send(Box::new(Collect(Arc::clone(&self.out))));
    }
    fn on_error(&mut self, err: &io::Error) {
        let mut g = self.out.lock().unwrap();
        g.refused = Some(err.to_string());
        g.done = true;
    }
    fn on_disconnected(&mut self) {
        let mut g = self.out.lock().unwrap();
        if g.refused.is_none() && !g.done {
            g.refused = Some("disconnected".into());
            g.done = true;
        }
    }
}

fn fetch(port: u16, connector: SharedTlsConnector, path: &'static str) -> Arc<Mutex<Outcome>> {
    let out = Arc::new(Mutex::new(Outcome::default()));
    HttpClient::new("127.0.0.1", port)
        .tls(connector, "localhost")
        .disable_content_encoding()
        .connect(&hopf_runtime(), Box::new(GetOnce { path, out: Arc::clone(&out) }))
        .unwrap();
    out
}

fn wait_done(out: &Arc<Mutex<Outcome>>, max: Duration) {
    let deadline = Instant::now() + max;
    while !out.lock().unwrap().done {
        assert!(Instant::now() < deadline, "request did not finish in time");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn serves_small_and_large_responses_to_a_certified_client_only() {
    let mtls = MtlsMaterial::generate_with_mtls().unwrap();
    let handler: RequestHandler = Arc::new(move |req, sink| answer(sink, req));
    let addr = start_mail_view_server(mtls.tls_acceptor.clone(), handler).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    let cert = dir.path().join("client.pem");
    let key = dir.path().join("client-key.pem");
    std::fs::write(&ca, &mtls.ca_cert_pem).unwrap();
    std::fs::write(&cert, &mtls.client_cert_pem).unwrap();
    std::fs::write(&key, &mtls.client_key_pem).unwrap();
    let certified = connector_from_pem_with_client_cert(&ca, &cert, &key, &[b"http/1.1"]).unwrap();
    let anonymous = connector_from_pem(&ca, &[b"http/1.1"]).unwrap();

    let small = fetch(addr.port(), Arc::clone(&certified), "/small");
    wait_done(&small, Duration::from_secs(5));
    {
        let g = small.lock().unwrap();
        assert_eq!(g.failed, None);
        assert_eq!(g.status, 200);
        assert_eq!(g.body, b"hello");
    }

    let big = fetch(addr.port(), Arc::clone(&certified), "/big");
    wait_done(&big, Duration::from_secs(30));
    {
        let g = big.lock().unwrap();
        assert_eq!(g.failed, None);
        assert_eq!(g.status, 200);
        assert_eq!(g.body.len(), BIG);
        assert!(g.body.iter().enumerate().all(|(i, b)| *b == big_byte(i)), "streamed body differs");
    }

    let missing = fetch(addr.port(), Arc::clone(&certified), "/nope");
    wait_done(&missing, Duration::from_secs(5));
    assert_eq!(missing.lock().unwrap().status, 404);

    let refused = fetch(addr.port(), anonymous, "/small");
    wait_done(&refused, Duration::from_secs(5));
    let g = refused.lock().unwrap();
    assert_ne!(g.status, 200, "a client without the certificate must not be served");
    assert!(g.refused.is_some() || g.failed.is_some(), "the refusal must be reported");
}
