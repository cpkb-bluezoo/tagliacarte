//! Sending through the real send path (`send_smtp_streaming`) to an in-process
//! SMTP server: a large attachment arrives intact, the message is built from
//! files rather than from a base64 string, and the staged message is cleaned up.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use hopf_smtp::{AcceptAllSmtpHandler, AcceptAllSmtpHandlerFactory, SmtpConfig, SmtpService};
use tagliacarte_app::compose_send::{OutgoingFile, send_smtp_streaming};
use tagliacarte_app::frb_api::{FrbComposeMessage, frb_load_config};
use tagliacarte_core::net::hopf_runtime;

fn start_server() -> (SocketAddr, Arc<Mutex<Vec<u8>>>) {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let listen: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = SmtpConfig::new(listen, "test.example.com").auth_required(false);
    let handler = AcceptAllSmtpHandler::new("test.example.com").with_capture(Arc::clone(&capture));
    let factory = Arc::new(AcceptAllSmtpHandlerFactory::new(handler));
    let bound = SmtpService::with_handler_factory(config, factory).start(hopf_runtime()).unwrap();
    (bound, capture)
}

fn compose(subject: &str) -> FrbComposeMessage {
    FrbComposeMessage {
        from: "Alice <alice@example.com>".into(),
        to: vec!["Bob <bob@example.org>".into()],
        cc: vec![],
        bcc: vec!["hidden@example.org".into()],
        subject: subject.into(),
        body_plain: "see the attachment\n.a line that starts with a dot".into(),
        body_html: None,
        attachments: vec![],
        dsn_notify: None,
        store_account_id: None,
        in_reply_to: None,
        references: None,
        message_id: None,
        crypto_mode: None,
    }
}

#[test]
fn a_large_attachment_is_streamed_through_the_real_send_path() {
    let dir = tempfile::tempdir().unwrap();
    let (bound, capture) = start_server();

    let config = dir.path().join("config.xml");
    std::fs::write(
        &config,
        format!(
            r#"<tagliacarte><transports><transport id="t1" type="smtp" display-name="Test" host="{}" port="{}" security="plain" default-from="alice@example.com"/></transports><stores><store id="s1" type="maildir" display-name="L" path="{}"/></stores></tagliacarte>"#,
            bound.ip(),
            bound.port(),
            dir.path().display()
        ),
    )
    .unwrap();
    // Loading registers this as the active configuration.
    let cfg = frb_load_config(config.to_string_lossy().into_owned());
    assert_eq!(cfg.transports.len(), 1, "the test config must load");

    let attachment: Vec<u8> = (0..5_000_000u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();
    let file = dir.path().join("report.bin");
    std::fs::write(&file, &attachment).unwrap();

    let before = staged_messages();
    send_smtp_streaming(
        "t1",
        &compose("streamed"),
        &[OutgoingFile { filename: Some("report.bin".into()), mime_type: "application/octet-stream".into(), path: file }],
    )
    .expect("sent");
    std::thread::sleep(Duration::from_millis(300));

    let got = capture.lock().unwrap().clone();
    let text = String::from_utf8_lossy(&got).into_owned();
    assert!(text.contains("Subject: streamed"), "headers arrived");
    assert!(!text.contains("hidden@example.org"), "Bcc must never appear in the message");
    let marker = "Content-Transfer-Encoding: base64\r\n\r\n";
    let start = text.find(marker).expect("attachment part") + marker.len();
    let end = text[start..].find("\r\n--").map(|i| start + i).expect("closing boundary");
    let joined: String = text[start..end].split("\r\n").collect();
    let decoded = base64::engine::general_purpose::STANDARD.decode(joined).unwrap();
    assert!(decoded == attachment, "the attachment must arrive unchanged ({} bytes)", decoded.len());

    assert_eq!(staged_messages(), before, "the staged message file is removed after sending");
}

/// How many staged outgoing messages of this process are in the temp dir.
fn staged_messages() -> usize {
    let prefix = format!("tagliacarte-out-{}-", std::process::id());
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .count()
}
