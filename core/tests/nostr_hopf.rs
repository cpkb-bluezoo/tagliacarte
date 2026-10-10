//! End-to-end: tagliacarte's Nostr relay layer (now on hopf) against a
//! scripted relay served by hopf-websocket, in process: REQ/EVENT/EOSE,
//! EVENT/OK, the multi-relay fan-out, and a relay that is not there.

use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use hopf_core::{ConnHandle, ProtocolHandler, TcpListenerConfig};
use hopf_http::{CleartextHttpEndpoint, Headers, HttpLimits, ServerHandlerFactory};
use hopf_websocket::{WebSocketConfig, WebSocketFactory, WsEventHandler, WsEventHandlerFactory, WsSession};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::nostr::{
    fetch_notes_from_relay, fetch_profile_from_relays_blocking, filter_profile_by_author, publish_event_blocking,
};

const PUBKEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn event_json(id: &str, kind: u32, created_at: u64, content: &str) -> String {
    let escaped = content.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\"id\":\"{id}\",\"pubkey\":\"{}\",\"created_at\":{created_at},\"kind\":{kind},\"tags\":[],\"content\":\"{escaped}\",\"sig\":\"00\"}}",
        PUBKEY
    )
}

/// A relay that answers every REQ with two profile events (the newer one
/// second) and EOSE, every EVENT with OK, and records what it saw.
struct Relay {
    seen: Arc<Mutex<Vec<String>>>,
}

impl WsEventHandler for Relay {
    fn opened(&mut self, _session: &mut WsSession<'_>, _conn: &ConnHandle) {}
    fn text_message(&mut self, session: &mut WsSession<'_>, text: &str) {
        self.seen.lock().unwrap().push(text.to_string());
        if text.starts_with("[\"REQ\"") {
            let sub = text.split('"').nth(3).unwrap_or("sub").to_string();
            session.send_text(&format!("[\"EVENT\",\"{sub}\",{}]", event_json("e1", 0, 10, "{\"name\":\"Old\"}")));
            session.send_text(&format!("[\"EVENT\",\"{sub}\",{}]", event_json("e2", 0, 20, "{\"name\":\"Alice\"}")));
            session.send_text(&format!("[\"EOSE\",\"{sub}\"]"));
        } else if text.starts_with("[\"EVENT\"") {
            session.send_text("[\"OK\",\"e9\",true,\"\"]");
        }
    }
    fn closed(&mut self, _session: &mut WsSession<'_>, _code: u16, _reason: &str) {}
    fn error(&mut self, _err: hopf_websocket::WsFrameError) {}
}

struct RelayFactory(Arc<Mutex<Vec<String>>>);

impl WsEventHandlerFactory for RelayFactory {
    fn create(&self, _path: &str, _headers: &Headers, _conn: ConnHandle) -> Box<dyn WsEventHandler> {
        Box::new(Relay { seen: Arc::clone(&self.0) })
    }
}

fn start_relay() -> (SocketAddr, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let factory = Arc::new(WebSocketFactory::new(RelayFactory(Arc::clone(&seen)), WebSocketConfig::default()));
    let (addr, _) = hopf_runtime()
        .add_tcp_listener(TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
            Box::new(CleartextHttpEndpoint::new(
                Arc::clone(&factory) as Arc<dyn ServerHandlerFactory>,
                HttpLimits::default(),
            )) as Box<dyn ProtocolHandler>
        }))
        .unwrap();
    (addr, seen)
}

fn url(addr: SocketAddr) -> String {
    format!("ws://{}:{}/", addr.ip(), addr.port())
}

#[test]
fn feed_stream_collects_events_until_eose() {
    let (addr, seen) = start_relay();
    let (tx, rx) = mpsc::channel();
    fetch_notes_from_relay(&url(addr), &filter_profile_by_author(PUBKEY), 10, None, Box::new(move |r| tx.send(r).unwrap()));
    let events = rx.recv_timeout(Duration::from_secs(10)).expect("stream finishes").expect("events");
    assert_eq!(events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["e1", "e2"]);
    assert_eq!(events[1].content, "{\"name\":\"Alice\"}");
    let req = seen.lock().unwrap().iter().find(|m| m.starts_with("[\"REQ\"")).cloned().expect("REQ reached the relay");
    assert!(req.contains(PUBKEY), "{req}");
}

#[test]
fn publish_waits_for_the_ok() {
    let (addr, seen) = start_relay();
    publish_event_blocking(&url(addr), &event_json("e9", 1, 30, "hello")).expect("accepted");
    assert!(seen.lock().unwrap().iter().any(|m| m.starts_with("[\"EVENT\"") && m.contains("hello")));
}

#[test]
fn fan_out_returns_the_newest_profile_from_the_first_relay_to_answer() {
    let (a, _) = start_relay();
    let (b, _) = start_relay();
    let (res, dead) = fetch_profile_from_relays_blocking(&[url(a), url(b)], PUBKEY, 10, None);
    let profile = res.expect("ok").expect("a profile");
    assert_eq!(profile.name.as_deref(), Some("Alice"));
    assert_eq!(profile.created_at, Some(20));
    assert!(dead.is_empty());
}

#[test]
fn unreachable_relay_finishes_the_stream_with_a_notice() {
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = free.local_addr().unwrap();
    drop(free);
    let started = std::time::Instant::now();
    let (tx, rx) = mpsc::channel();
    fetch_notes_from_relay(&url(addr), &filter_profile_by_author(PUBKEY), 10, None, Box::new(move |r| tx.send(r).unwrap()));
    let events = rx.recv_timeout(Duration::from_secs(20)).expect("stream finishes").expect("no events, no error");
    assert!(events.is_empty());
    assert!(started.elapsed() < Duration::from_secs(10), "connect timeout bounds the wait: {:?}", started.elapsed());
}
