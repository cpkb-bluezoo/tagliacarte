//! End-to-end: tagliacarte's WebSocket client (now on hopf) against
//! hopf-websocket's echo server, in process.

use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use hopf_core::{ProtocolHandler, TcpListenerConfig};
use hopf_http::{CleartextHttpEndpoint, HttpLimits, ServerHandlerFactory};
use hopf_websocket::{EchoFactory, WebSocketConfig, WebSocketFactory};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::websocket::{WebSocketClient, WebSocketHandler, WebSocketSession};

fn start_echo_server() -> SocketAddr {
    let factory = Arc::new(WebSocketFactory::new(EchoFactory, WebSocketConfig::default()));
    let (addr, _) = hopf_runtime()
        .add_tcp_listener(TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
            Box::new(CleartextHttpEndpoint::new(
                Arc::clone(&factory) as Arc<dyn ServerHandlerFactory>,
                HttpLimits::default(),
            )) as Box<dyn ProtocolHandler>
        }))
        .unwrap();
    addr
}

#[derive(Default)]
struct Seen {
    texts: Vec<String>,
    binaries: Vec<Vec<u8>>,
    pongs: Vec<Vec<u8>>,
    close: Option<(Option<u16>, String)>,
    failed: Option<String>,
}

/// On connect sends the scripted frames; records what comes back; after
/// `stop_after` text frames sends `follow_up` (once), and once that many
/// plus the follow-up have echoed, closes. `done` fires when the handler
/// hears close or failed.
struct Recorder {
    seen: Arc<Mutex<Seen>>,
    on_connect: Vec<Frame>,
    stop_after: usize,
    follow_up: Option<String>,
    done: Option<mpsc::Sender<()>>,
}

enum Frame {
    Text(&'static str),
    Binary(Vec<u8>),
    Ping(&'static [u8]),
    Close(Option<u16>, &'static str),
}

impl Recorder {
    fn finished(&mut self) {
        if let Some(d) = self.done.take() {
            let _ = d.send(());
        }
    }
}

impl WebSocketHandler for Recorder {
    fn connected(&mut self, ws: &WebSocketSession) {
        for f in std::mem::take(&mut self.on_connect) {
            match f {
                Frame::Text(t) => ws.send_text(t).unwrap(),
                Frame::Binary(b) => ws.send_binary(&b).unwrap(),
                Frame::Ping(p) => ws.send_ping(p).unwrap(),
                Frame::Close(c, r) => ws.send_close(c, r).unwrap(),
            }
        }
    }
    fn text_frame(&mut self, ws: &WebSocketSession, data: &[u8]) {
        let n = {
            let mut s = self.seen.lock().unwrap();
            s.texts.push(String::from_utf8_lossy(data).into_owned());
            s.texts.len()
        };
        if n == 1 {
            if let Some(f) = self.follow_up.take() {
                ws.send_text(&f).unwrap();
            }
        }
        if n >= self.stop_after {
            ws.send_close(Some(1000), "done").unwrap();
        }
    }
    fn binary_frame(&mut self, _ws: &WebSocketSession, data: &[u8]) {
        self.seen.lock().unwrap().binaries.push(data.to_vec());
    }
    fn close(&mut self, code: Option<u16>, reason: &str) {
        self.seen.lock().unwrap().close = Some((code, reason.to_string()));
        self.finished();
    }
    fn pong(&mut self, _ws: &WebSocketSession, data: &[u8]) {
        self.seen.lock().unwrap().pongs.push(data.to_vec());
    }
    fn failed(&mut self, error: &std::io::Error) {
        self.seen.lock().unwrap().failed = Some(error.to_string());
        self.finished();
    }
}

fn run(url: &str, on_connect: Vec<Frame>, stop_after: usize, follow_up: Option<&str>) -> Arc<Mutex<Seen>> {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let (tx, rx) = mpsc::channel();
    WebSocketClient::connect(
        url,
        Box::new(Recorder { seen: Arc::clone(&seen), on_connect, stop_after, follow_up: follow_up.map(str::to_string), done: Some(tx) }),
    );
    rx.recv_timeout(Duration::from_secs(10)).expect("handler finished within 10 s");
    seen
}

#[test]
fn text_binary_ping_and_follow_up_round_trip() {
    let addr = start_echo_server();
    let url = format!("ws://{}:{}/echo", addr.ip(), addr.port());
    let seen = run(
        &url,
        vec![Frame::Text(r#"["REQ","sub",{}]"#), Frame::Binary(vec![1, 2, 3]), Frame::Ping(b"hb")],
        2,
        Some(r#"["CLOSE","sub"]"#),
    );
    let s = seen.lock().unwrap();
    assert_eq!(s.texts, vec![r#"["REQ","sub",{}]"#.to_string(), r#"["CLOSE","sub"]"#.to_string()]);
    assert_eq!(s.binaries, vec![vec![1, 2, 3]]);
    assert_eq!(s.pongs, vec![b"hb".to_vec()]);
    assert_eq!(s.failed, None, "closing after the echo is not a failure");
    assert_eq!(s.close.as_ref().map(|(c, _)| *c), Some(Some(1000)), "the echo server answers our Close");
}

#[test]
fn close_frame_reaches_the_handler() {
    let addr = start_echo_server();
    let url = format!("ws://{}:{}/echo", addr.ip(), addr.port());
    let seen = run(&url, vec![Frame::Close(Some(1000), "bye")], usize::MAX, None);
    let s = seen.lock().unwrap();
    assert_eq!(s.close.as_ref().map(|(c, _)| *c), Some(Some(1000)));
}

#[test]
fn non_websocket_server_is_a_handshake_error() {
    // A plain HTTP server answers the upgrade with a normal response.
    let addr = {
        use hopf_http::{Headers, ServerHandler, ServerWriter};
        struct Plain;
        impl ServerHandler for Plain {
            fn headers(&mut self, _r: &mut dyn ServerWriter, _h: &Headers) {}
            fn request_complete(&mut self, r: &mut dyn ServerWriter) {
                let mut h = Headers::new();
                h.set(":status", "404");
                r.headers(h);
                r.end_response_body();
                r.complete();
            }
        }
        struct PlainFactory;
        impl ServerHandlerFactory for PlainFactory {
            fn create_handler(&self) -> Box<dyn ServerHandler> {
                Box::new(Plain)
            }
        }
        let factory: Arc<dyn ServerHandlerFactory> = Arc::new(PlainFactory);
        let (addr, _) = hopf_runtime()
            .add_tcp_listener(TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
                Box::new(CleartextHttpEndpoint::new(Arc::clone(&factory), HttpLimits::default())) as Box<dyn ProtocolHandler>
            }))
            .unwrap();
        addr
    };
    let url = format!("ws://{}:{}/", addr.ip(), addr.port());
    let seen = run(&url, vec![], usize::MAX, None);
    let s = seen.lock().unwrap();
    let err = s.failed.as_deref().expect("not a WebSocket server");
    assert!(err.contains("404"), "{err}");
    assert!(s.close.is_none());
}

/// Nothing listens: hopf does not report a refused dial to the handler, so
/// the client's own handshake timer must.
#[test]
fn refused_dial_times_out_through_the_handler() {
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = free.local_addr().unwrap();
    drop(free);
    let url = format!("ws://{}:{}/", addr.ip(), addr.port());
    let seen = Arc::new(Mutex::new(Seen::default()));
    let (tx, rx) = mpsc::channel();
    let started = std::time::Instant::now();
    WebSocketClient::connect_with_timeout(
        &url,
        Duration::from_secs(1),
        Box::new(Recorder { seen: Arc::clone(&seen), on_connect: vec![], stop_after: usize::MAX, follow_up: None, done: Some(tx) }),
    );
    rx.recv_timeout(Duration::from_secs(10)).expect("handler finished");
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
    let s = seen.lock().unwrap();
    assert!(s.failed.as_deref().unwrap_or("").contains("timed out"), "{:?}", s.failed);
}
