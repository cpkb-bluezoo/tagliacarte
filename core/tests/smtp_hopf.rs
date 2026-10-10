//! End-to-end: tagliacarte's SMTP submission functions (now on hopf) against
//! hopf's own SMTP server, in process. No network, no credentials beyond a
//! test store.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_auth::PasswordStore;
use hopf_smtp::{AcceptAllSmtpHandler, AcceptAllSmtpHandlerFactory, SmtpConfig, SmtpService};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::smtp::{
    probe_smtp_ehlo_auth_methods_blocking, probe_smtp_tcp_connect_blocking, send_message_blocking,
    submit, verify_smtp_blocking, SmtpTarget,
};
use tagliacarte_core::sasl::SaslMechanism;
use tagliacarte_core::store::{Address, Envelope};

fn start_server(with_auth: bool) -> (SocketAddr, Arc<Mutex<Vec<u8>>>) {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let listen: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut config = SmtpConfig::new(listen, "test.example.com");
    if with_auth {
        config = config.with_store(Arc::new(PasswordStore::new().with_user("alice", "secret")));
    } else {
        config = config.auth_required(false);
    }
    let handler = AcceptAllSmtpHandler::new("test.example.com").with_capture(Arc::clone(&capture));
    let factory = Arc::new(AcceptAllSmtpHandlerFactory::new(handler));
    let service = SmtpService::with_handler_factory(config, factory);
    let bound = service.start(hopf_runtime()).unwrap();
    (bound, capture)
}

fn addr(local: &str, domain: &str) -> Address {
    Address { display_name: None, local_part: local.into(), domain: Some(domain.into()) }
}

fn envelope() -> Envelope {
    Envelope {
        from: vec![addr("alice", "example.com")],
        to: vec![addr("bob", "example.org")],
        cc: vec![addr("carol", "example.org")],
        date: None,
        subject: Some("hi".into()),
        message_id: None,
        in_reply_to: None,
        references: None,
    }
}

const MESSAGE: &[u8] = b"From: alice@example.com\r\nTo: bob@example.org\r\nSubject: hi\r\n\r\nhello from tagliacarte on hopf\r\n";

#[test]
fn send_verify_and_probe_against_hopf_server() {
    let (bound, capture) = start_server(true);
    let host = bound.ip().to_string();
    let port = bound.port();

    // Capabilities first, as the settings UI does before asking for a password.
    let methods = probe_smtp_ehlo_auth_methods_blocking(&host, port, false, false, None).unwrap();
    assert!(methods.iter().any(|m| m == "SCRAM-SHA-256"), "{methods:?}");
    probe_smtp_tcp_connect_blocking(&host, port, false).unwrap();

    // Verify credentials: good, then bad.
    verify_smtp_blocking(&host, port, false, false, Some(("alice", "secret", SaslMechanism::Plain)), None).unwrap();
    let err = verify_smtp_blocking(&host, port, false, false, Some(("alice", "wrong", SaslMechanism::Plain)), None)
        .unwrap_err();
    assert!(err.message.contains("535"), "{err}");

    // Deliver, authenticated, with DSN NOTIFY requested (the server does not
    // advertise DSN, so hopf must not send it).
    send_message_blocking(
        &host,
        port,
        false,
        false,
        Some(("alice", "secret", SaslMechanism::Plain)),
        None,
        MESSAGE,
        &envelope(),
        Some("FAILURE,SUCCESS"),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let got = capture.lock().unwrap().clone();
    assert!(
        got.windows(b"hello from tagliacarte on hopf".len()).any(|w| w == b"hello from tagliacarte on hopf"),
        "capture={:?}",
        String::from_utf8_lossy(&got)
    );

    // STARTTLS required but the server does not offer it: refused before
    // credentials go anywhere.
    let err = send_message_blocking(
        &host, port, false, true, Some(("alice", "secret", SaslMechanism::Plain)), None, MESSAGE, &envelope(), None,
    )
    .unwrap_err();
    assert!(err.message.contains("STARTTLS"), "{err}");
}

/// The callback-first entry point, as the Transport trait uses it.
#[test]
fn submit_reports_through_the_callback() {
    let (bound, capture) = start_server(false);
    let target = SmtpTarget::new(&bound.ip().to_string(), bound.port(), false, false);
    let (tx, rx) = std::sync::mpsc::channel();
    submit(&target, MESSAGE.to_vec(), &envelope(), None, Box::new(move |r| tx.send(r).unwrap()));
    let result = rx.recv_timeout(Duration::from_secs(5)).expect("result within 5 s");
    result.expect("delivered");
    std::thread::sleep(Duration::from_millis(50));
    assert!(!capture.lock().unwrap().is_empty());
}

/// A server that is not there: the failure arrives as a result, promptly.
#[test]
fn unreachable_server_is_reported_not_hung() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // nothing listens here now
    let started = std::time::Instant::now();
    let err = verify_smtp_blocking("127.0.0.1", port, false, false, None, None).unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    assert!(!err.message.is_empty());
}
