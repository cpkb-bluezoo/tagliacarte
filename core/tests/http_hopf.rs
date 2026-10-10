//! End-to-end: tagliacarte's HTTP client (now on hopf-http) against hopf's
//! own HTTP server, in process: HTTP/1.1 and cleartext HTTP/2.

use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use hopf_core::{acceptor_from_pem, insecure_connector, ProtocolHandler, TcpListenerConfig};
use hopf_http::{AlpnHttpEndpoint, CleartextHttpEndpoint, Headers, HttpLimits, ServerHandler, ServerHandlerFactory, ServerWriter};
use tagliacarte_core::net::{hopf_runtime, set_hopf_http_tls_connector};
use tagliacarte_core::protocol::http::{HttpClient, HttpConnection, HttpVersion, Method, RequestBuilder, Response, ResponseHandler};

/// Answers every request with what it received: the method and path in
/// headers, the request body (or a description of the request) as the
/// body, and the status the `x-echo-status` request header asks for.
struct Echo {
    method: String,
    path: String,
    status: String,
    body: Vec<u8>,
}

impl ServerHandler for Echo {
    fn headers(&mut self, _response: &mut dyn ServerWriter, headers: &Headers) {
        self.method = headers.get(":method").unwrap_or_default().to_string();
        self.path = headers.get(":path").unwrap_or_default().to_string();
        self.status = headers.get("x-echo-status").unwrap_or("200").to_string();
    }
    fn request_body_content(&mut self, _response: &mut dyn ServerWriter, data: &[u8]) {
        self.body.extend_from_slice(data);
    }
    fn request_complete(&mut self, response: &mut dyn ServerWriter) {
        let mut h = Headers::new();
        h.set(":status", &self.status);
        h.set("x-echo-method", &self.method);
        h.set("x-echo-path", &self.path);
        h.set("content-type", "text/plain");
        response.headers(h);
        if self.body.is_empty() {
            response.response_body_content(format!("echo:{} {}", self.method, self.path).as_bytes());
        } else {
            response.response_body_content(&self.body);
        }
        response.end_response_body();
        response.complete();
    }
}

struct EchoFactory;
impl ServerHandlerFactory for EchoFactory {
    fn create_handler(&self) -> Box<dyn ServerHandler> {
        Box::new(Echo { method: String::new(), path: String::new(), status: "200".into(), body: Vec::new() })
    }
}

fn start_server() -> SocketAddr {
    let factory: Arc<dyn ServerHandlerFactory> = Arc::new(EchoFactory);
    let (addr, _) = hopf_runtime()
        .add_tcp_listener(TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
            Box::new(CleartextHttpEndpoint::new(Arc::clone(&factory), HttpLimits::default())) as Box<dyn ProtocolHandler>
        }))
        .unwrap();
    addr
}

#[derive(Default)]
struct Seen {
    ok: Option<u16>,
    error: Option<u16>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    started: bool,
    ended: bool,
    complete: bool,
    failed: Option<String>,
}

struct Recorder(Arc<Mutex<Seen>>, Option<mpsc::Sender<()>>);

impl Recorder {
    fn finished(&mut self) {
        if let Some(tx) = self.1.take() {
            let _ = tx.send(());
        }
    }
}

impl ResponseHandler for Recorder {
    fn ok(&mut self, r: Response) {
        self.0.lock().unwrap().ok = Some(r.code);
    }
    fn error(&mut self, r: Response) {
        self.0.lock().unwrap().error = Some(r.code);
    }
    fn header(&mut self, n: &str, v: &str) {
        self.0.lock().unwrap().headers.push((n.to_string(), v.to_string()));
    }
    fn start_body(&mut self) {
        self.0.lock().unwrap().started = true;
    }
    fn body_chunk(&mut self, d: &[u8]) {
        self.0.lock().unwrap().body.extend_from_slice(d);
    }
    fn end_body(&mut self) {
        self.0.lock().unwrap().ended = true;
    }
    fn complete(&mut self) {
        self.0.lock().unwrap().complete = true;
        self.finished();
    }
    fn failed(&mut self, e: &std::io::Error) {
        self.0.lock().unwrap().failed = Some(e.to_string());
        self.finished();
    }
}

fn header<'a>(seen: &'a Seen, name: &str) -> Option<&'a str> {
    seen.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

const WAIT: Duration = Duration::from_secs(10);

fn connect(addr: SocketAddr, tls: bool, h2c: bool) -> std::io::Result<HttpConnection> {
    let (tx, rx) = mpsc::channel();
    let on_connect: tagliacarte_core::protocol::http::OnConnect = Box::new(move |r| tx.send(r).unwrap());
    if h2c {
        HttpClient::connect_h2c(&addr.ip().to_string(), addr.port(), on_connect);
    } else {
        HttpClient::connect(&addr.ip().to_string(), addr.port(), tls, on_connect);
    }
    rx.recv_timeout(WAIT).expect("dial reports within 10 s")
}

/// Send and wait for the handler's `complete`/`failed`; the recorded
/// events are left in `seen`.
fn send(conn: &HttpConnection, req: RequestBuilder) -> Arc<Mutex<Seen>> {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let (tx, rx) = mpsc::channel();
    conn.send(req, Recorder(Arc::clone(&seen), Some(tx)));
    rx.recv_timeout(WAIT).expect("request completes within 10 s");
    seen
}

#[test]
fn http1_get_post_and_error_status_on_one_connection() {
    let addr = start_server();
    let conn = connect(addr, false, false).unwrap();
    assert_eq!(conn.version(), HttpVersion::Http1_1);

    // GET
    let mut req = conn.request(Method::Get, "/hello?x=1");
    req.header("Accept", "text/plain");
    let seen = send(&conn, req);
    {
        let s = seen.lock().unwrap();
        assert_eq!(s.ok, Some(200));
        assert_eq!(header(&s, "x-echo-method"), Some("GET"));
        assert_eq!(header(&s, "x-echo-path"), Some("/hello?x=1"));
        assert_eq!(s.body, b"echo:GET /hello?x=1");
        assert!(s.started && s.ended && s.complete, "body events then completion");
        assert_eq!(s.failed, None);
    }

    // POST with a body large enough to need several rounds through hopf's
    // outbound buffers (short writes + writable callbacks).
    let body: Vec<u8> = (0..600 * 1024).map(|i| (i % 251) as u8).collect();
    let mut req = conn.request(Method::Post, "/upload");
    req.header("Content-Type", "application/octet-stream");
    req.header("Content-Length", body.len().to_string());
    req.body(body.clone());
    let seen = send(&conn, req);
    {
        let s = seen.lock().unwrap();
        assert_eq!(s.ok, Some(200));
        assert_eq!(header(&s, "x-echo-method"), Some("POST"));
        assert_eq!(s.body.len(), body.len());
        assert!(s.body == body, "echoed body differs");
    }

    // 404 reaches `error`, and the request still completes normally.
    let mut req = conn.request(Method::Delete, "/missing");
    req.header("X-Echo-Status", "404");
    let seen = send(&conn, req);
    {
        let s = seen.lock().unwrap();
        assert_eq!(s.error, Some(404));
        assert_eq!(s.ok, None);
        assert!(s.complete);
    }
}

/// Requests chained from inside a handler (as the store code does) run on
/// hopf's reactor thread; the next request must go out from there.
#[test]
fn requests_chain_from_the_completion_callback() {
    let addr = start_server();
    let conn = connect(addr, false, false).unwrap();
    struct Chain {
        conn: HttpConnection,
        left: u32,
        bodies: Arc<Mutex<Vec<Vec<u8>>>>,
        body: Vec<u8>,
        done: Option<mpsc::Sender<()>>,
    }
    impl ResponseHandler for Chain {
        fn ok(&mut self, _r: Response) {}
        fn error(&mut self, _r: Response) {}
        fn header(&mut self, _n: &str, _v: &str) {}
        fn start_body(&mut self) {}
        fn body_chunk(&mut self, d: &[u8]) {
            self.body.extend_from_slice(d);
        }
        fn end_body(&mut self) {}
        fn complete(&mut self) {
            self.bodies.lock().unwrap().push(std::mem::take(&mut self.body));
            if self.left == 0 {
                let _ = self.done.take().unwrap().send(());
                return;
            }
            let req = self.conn.request(Method::Get, format!("/step/{}", self.left));
            let next = Chain {
                conn: self.conn.clone(),
                left: self.left - 1,
                bodies: Arc::clone(&self.bodies),
                body: Vec::new(),
                done: self.done.take(),
            };
            self.conn.send(req, next);
        }
        fn failed(&mut self, e: &std::io::Error) {
            panic!("chain failed: {e}");
        }
    }
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    let req = conn.request(Method::Get, "/step/3");
    conn.send(req, Chain { conn: conn.clone(), left: 2, bodies: Arc::clone(&bodies), body: Vec::new(), done: Some(tx) });
    rx.recv_timeout(WAIT).expect("chain completes");
    let got = bodies.lock().unwrap().clone();
    assert_eq!(got, vec![b"echo:GET /step/3".to_vec(), b"echo:GET /step/2".to_vec(), b"echo:GET /step/1".to_vec()]);
}

#[test]
fn http2_parallel_gets_return_in_request_order() {
    let addr = start_server();
    let conn = connect(addr, false, true).unwrap();
    assert_eq!(conn.version(), HttpVersion::Http2);

    let mut batch = Vec::new();
    for i in 0..8u8 {
        let mut req = conn.request(Method::Get, format!("/item/{i}"));
        req.header("Accept", "text/plain");
        // Later items more urgent, so the wire order differs from the
        // result order the caller relies on.
        batch.push((req, 20 + i * 25));
    }
    let (tx, rx) = mpsc::channel();
    conn.send_http2_parallel_gets(batch, Box::new(move |r| tx.send(r).unwrap())).unwrap();
    let results = rx.recv_timeout(WAIT).expect("batch completes");
    assert_eq!(results.len(), 8);
    for (i, r) in results.into_iter().enumerate() {
        let (status, body) = r.expect("each GET completes");
        assert_eq!(status, 200);
        assert_eq!(body, format!("echo:GET /item/{i}").into_bytes());
    }

    // The same connection still serves an ordinary request afterwards.
    let seen = send(&conn, conn.request(Method::Get, "/after"));
    assert_eq!(seen.lock().unwrap().body, b"echo:GET /after");
}

#[test]
fn http2_post_body_streams_through_flow_control() {
    let addr = start_server();
    let conn = connect(addr, false, true).unwrap();
    let body: Vec<u8> = (0..900 * 1024).map(|i| (i % 253) as u8).collect();
    let mut req = conn.request(Method::Put, "/big");
    req.header("Content-Length", body.len().to_string());
    req.body(body.clone());
    let seen = send(&conn, req);
    let s = seen.lock().unwrap();
    assert_eq!(s.ok, Some(200));
    assert_eq!(s.body.len(), body.len());
    assert!(s.body == body);
}

#[test]
fn refused_dial_is_an_error_not_a_hang() {
    // Bind then drop a listener so the port is free and closed.
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = free.local_addr().unwrap();
    drop(free);
    let err = connect(addr, false, false).err().expect("refused");
    assert!(!err.to_string().is_empty());
}

/// A request on a connection the server has closed fails through the
/// handler rather than being forgotten.
#[test]
fn request_on_a_dead_connection_fails_through_the_handler() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    // Accept and drop: the client sees a clean close right after connecting.
    std::thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        drop(s);
    });
    let conn = connect(addr, false, false).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let seen = send(&conn, conn.request(Method::Get, "/gone"));
    let s = seen.lock().unwrap();
    assert!(s.failed.is_some(), "failed must be reported: {:?}", s.failed);
    assert!(!s.complete);
}

/// Over TLS the client offers `h2` and `http/1.1` by ALPN and takes what
/// the server picks: HTTP/2 here, which is what Gmail and Graph speak.
#[test]
fn https_negotiates_http2_by_alpn() {
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    std::fs::write(&cert_path, ck.cert.pem()).unwrap();
    std::fs::write(&key_path, ck.key_pair.serialize_pem()).unwrap();
    let acceptor = acceptor_from_pem(&cert_path, &key_path, &[b"h2", b"http/1.1"]).unwrap();
    let factory: Arc<dyn ServerHandlerFactory> = Arc::new(EchoFactory);
    let (addr, _) = hopf_runtime()
        .add_tcp_listener(
            TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
                Box::new(AlpnHttpEndpoint::new(Arc::clone(&factory), HttpLimits::default())) as Box<dyn ProtocolHandler>
            })
            .with_tls(acceptor),
        )
        .unwrap();
    // The self-signed test certificate is not trusted by anything, so the
    // test connector skips verification; ALPN is what this test is about.
    let _ = set_hopf_http_tls_connector(insecure_connector(&[b"h2", b"http/1.1"]));

    let conn = connect(addr, true, false).unwrap();
    assert_eq!(conn.version(), HttpVersion::Http2);
    let seen = send(&conn, conn.request(Method::Get, "/secure"));
    assert_eq!(seen.lock().unwrap().body, b"echo:GET /secure");
}
