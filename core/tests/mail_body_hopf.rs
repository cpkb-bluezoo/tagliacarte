//! The dedicated mail-body IMAP session against hopf's in-process IMAP server:
//! fetching a message's header block (what the reader shows above the body)
//! and turning it into an envelope.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_auth::PasswordStore;
use hopf_imap::{ImapConfig, ImapService};
use hopf_mailbox::{MailboxFactory, MaildirFactory};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::imap::{
    ImapClientError, ImapSessionConfig, LiteralSink, MailBodySession, Reads,
};
use tagliacarte_core::sasl::SaslMechanism;
use tagliacarte_core::store::message_for_display_from_raw;

const T: Duration = Duration::from_secs(10);

const MSG1: &[u8] = b"From: Ann Example <ann@example.org>\r\nTo: bob@example.org\r\nSubject: Question\r\nMessage-Id: <1@x>\r\nDate: Thu, 01 Jan 2026 10:00:00 +0000\r\n\r\nfirst\r\n";

/// What most real mail looks like: a long, folded header block, then a
/// multipart body that a header-only fetch never reaches.
const MSG2: &[u8] = b"Received: by 2002:a05:7022:1 with SMTP id x;\r\n        Tue, 3 Nov 2026 03:00:00 -0800 (PST)\r\nFrom: \"Example Sender\" <news@example.com>\r\nTo: L'escoces <lescoces@gmail.com>\r\nSubject: Weekly digest\r\nDate: Tue, 3 Nov 2026 11:00:00 +0000\r\nMessage-ID: <abc@example.com>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: text/plain\r\n\r\nhello\r\n--b1\r\nContent-Type: text/html\r\n\r\n<p>hello</p>\r\n--b1--\r\n";

fn start_server(dir: &std::path::Path) -> SocketAddr {
    let factory = Arc::new(MaildirFactory::new(dir));
    {
        let mut store = factory.create_store();
        store.open("alice").unwrap();
        let mut mb = store.open_mailbox("INBOX", false).unwrap();
        for m in [MSG1, MSG2] {
            let mut guard = hopf_mailbox::AppendGuard::start(mb.as_mut(), &BTreeSet::new(), None).unwrap();
            guard.append_content(m).unwrap();
            guard.commit().unwrap();
        }
        mb.close(false).unwrap();
        store.close().unwrap();
    }
    let store = Arc::new(PasswordStore::new().with_user("alice", "secret"));
    let config = ImapConfig::new("127.0.0.1:0".parse().unwrap(), "localhost", store, factory);
    ImapService::new(config, hopf_runtime()).start().unwrap()
}

fn session_config(bound: SocketAddr) -> ImapSessionConfig {
    ImapSessionConfig {
        host: bound.ip().to_string(),
        port: bound.port(),
        implicit_tls: false,
        starttls: false,
        auth: Some(("alice".into(), "secret".into(), SaslMechanism::ScramSha256)),
        min_idle_secs: Arc::new(AtomicU32::new(120)),
        folder_list_stale: Arc::new(AtomicBool::new(false)),
        auto_idle: false,
    }
}

/// Collects a whole literal, as the reader's header fetch does.
struct Collect {
    bytes: Arc<Mutex<Vec<u8>>>,
    done: Sender<Result<(), String>>,
}

impl LiteralSink for Collect {
    fn chunk(&mut self, data: &[u8], _reads: &mut Reads<'_>) {
        self.bytes.lock().unwrap().extend_from_slice(data);
    }
    fn done(&mut self, r: Result<(), ImapClientError>) {
        let _ = self.done.send(r.map_err(|e| e.message));
    }
}

/// SELECT INBOX, then fetch `section` of `uid` and return what arrived.
fn fetch_section(session: &MailBodySession, uid: u32, section: &'static str) -> Vec<u8> {
    let (selected_tx, selected_rx) = channel();
    let (done_tx, done_rx) = channel();
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let (s, b) = (session.clone(), Arc::clone(&bytes));
    session.acquire(Box::new(move |lease| {
        let s2 = s.clone();
        s.select(
            "INBOX",
            Box::new(move |r| {
                selected_tx.send(r.map_err(|e| e.message)).unwrap();
                s2.fetch_body_peek_section(uid, section, Box::new(Collect { bytes: b, done: done_tx }));
                drop(lease);
            }),
        );
    }));
    selected_rx.recv_timeout(T).expect("select answered").expect("select ok");
    done_rx.recv_timeout(T).expect("fetch answered").expect("fetch ok");
    let out = bytes.lock().unwrap().clone();
    out
}

#[test]
fn the_header_block_of_a_message_becomes_its_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(dir.path());
    let session = MailBodySession::connect(session_config(bound));

    let header = fetch_section(&session, 1, "HEADER");
    let text = String::from_utf8_lossy(&header).into_owned();
    assert!(text.starts_with("From: Ann Example"), "the literal should be the header block, got {text:?}");

    let env = message_for_display_from_raw(&header).envelope;
    assert_eq!(env.from.len(), 1, "From was lost from {text:?}");
    assert_eq!(env.from[0].local_part, "ann");
    assert_eq!(env.from[0].domain.as_deref(), Some("example.org"));
    assert_eq!(env.from[0].display_name.as_deref(), Some("Ann Example"));
    assert_eq!(env.to.len(), 1);
    assert_eq!(env.subject.as_deref(), Some("Question"));
    assert!(env.date.is_some(), "Date was lost from {text:?}");
}

#[test]
fn a_multipart_messages_header_block_becomes_its_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(dir.path());
    let session = MailBodySession::connect(session_config(bound));

    let header = fetch_section(&session, 2, "HEADER");
    let text = String::from_utf8_lossy(&header).into_owned();
    assert!(!text.contains("--b1"), "a header fetch must not return the parts: {text:?}");
    assert!(text.contains("multipart/alternative"), "{text:?}");

    let env = message_for_display_from_raw(&header).envelope;
    assert_eq!(env.from.len(), 1, "From was lost from {text:?}");
    assert_eq!(env.from[0].display_name.as_deref(), Some("Example Sender"));
    assert_eq!(env.to.len(), 1, "To was lost");
    assert_eq!(env.subject.as_deref(), Some("Weekly digest"));
    assert!(env.date.is_some(), "Date was lost");
}
