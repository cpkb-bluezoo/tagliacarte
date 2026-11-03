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
    submit, submit_body, verify_smtp_blocking, SmtpBody, SmtpTarget,
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

/// A message with a multi-megabyte attachment, written to disk by the
/// streaming MIME writer and sent from there: it arrives intact, and the
/// client never held it whole.
#[test]
fn a_large_message_is_streamed_from_a_file() {
    use base64::Engine;
    use tagliacarte_core::protocol::smtp::{write_rfc822, AttachmentBody, AttachmentSpec};
    use tagliacarte_core::store::SendPayload;

    let dir = tempfile::tempdir().unwrap();
    let attachment: Vec<u8> = (0..6_000_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    let att_path = dir.path().join("big.bin");
    std::fs::write(&att_path, &attachment).unwrap();

    let a = |l: &str, d: &str| addr(l, d);
    let payload = SendPayload {
        from: vec![a("alice", "example.com")],
        to: vec![a("bob", "example.org")],
        cc: vec![],
        bcc: vec![],
        subject: Some("big".into()),
        body_plain: Some(".leading dot line\nsecond line".into()),
        body_html: None,
        attachments: vec![],
        newsgroups: vec![],
        nntp_in_reply_to: None,
        nntp_references: None,
        smtp_notify: None,
        smtp_in_reply_to: None,
        smtp_references: None,
        smtp_message_id: None,
    };
    let spool = dir.path().join("message.eml");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&spool).unwrap());
    write_rfc822(
        &payload,
        &[AttachmentSpec { filename: Some("big.bin"), mime_type: "application/octet-stream", body: AttachmentBody::File(&att_path) }],
        &mut f,
    )
    .unwrap();
    std::io::Write::flush(&mut f).unwrap();
    drop(f);

    let (bound, capture) = start_server(false);
    let target = SmtpTarget::new(&bound.ip().to_string(), bound.port(), false, false);
    let (tx, rx) = std::sync::mpsc::channel();
    submit_body(&target, SmtpBody::File(spool.clone()), &envelope(), None, Box::new(move |r| tx.send(r).unwrap()));
    rx.recv_timeout(Duration::from_secs(60)).expect("result within 60 s").expect("delivered");
    std::thread::sleep(Duration::from_millis(200));

    let got = capture.lock().unwrap().clone();
    let text = String::from_utf8_lossy(&got);
    let marker = "Content-Transfer-Encoding: base64\r\n\r\n";
    let start = text.find(marker).expect("attachment part arrived") + marker.len();
    let end = text[start..].find("\r\n--").map(|i| start + i).expect("closing boundary arrived");
    let joined: String = text[start..end].split("\r\n").collect();
    let decoded = base64::engine::general_purpose::STANDARD.decode(joined).expect("valid base64");
    assert_eq!(decoded.len(), attachment.len());
    assert!(decoded == attachment, "the attachment was altered in transit");
    // The text body's leading dot must have been protected on the wire and restored by the server.
    assert!(text.contains("\r\n.leading dot line") || text.contains("\r\n..leading dot line"), "body line missing");
}
