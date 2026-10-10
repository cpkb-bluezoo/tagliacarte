//! End-to-end: tagliacarte's `Pop3Store` (now on hopf) against hopf's own
//! POP3 server over a seeded Maildir, in process.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_auth::PasswordStore;
use hopf_mailbox::{MailboxFactory, MaildirFactory};
use hopf_pop3::{Pop3Config, Pop3Service};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::pop3::Pop3Store;
use tagliacarte_core::store::{ConversationSummary, Folder, Store, StoreError};

fn start_server(dir: &std::path::Path) -> SocketAddr {
    let factory = Arc::new(MaildirFactory::new(dir));
    {
        let mut store = factory.create_store();
        store.open("alice").unwrap();
        let mut mb = store.open_mailbox("INBOX", false).unwrap();
        for body in [
            &b"From: a@b.example\r\nTo: alice@x\r\nSubject: one\r\nDate: Thu, 1 Jan 2026 10:00:00 +0000\r\n\r\nfirst body\r\n"[..],
            &b"From: c@d.example\r\nTo: alice@x\r\nSubject: two\r\nDate: Thu, 1 Jan 2026 11:00:00 +0000\r\n\r\nsecond body\r\n.dotted line\r\n"[..],
        ] {
            let mut guard = hopf_mailbox::AppendGuard::start(mb.as_mut(), &BTreeSet::new(), None).unwrap();
            guard.append_content(body).unwrap();
            guard.commit().unwrap();
        }
        mb.close(false).unwrap();
        store.close().unwrap();
    }
    let store = Arc::new(PasswordStore::new().with_user("alice", "secret"));
    let config = Pop3Config::new("127.0.0.1:0".parse().unwrap(), "localhost", store, factory);
    Pop3Service::new(config, hopf_runtime()).start().unwrap()
}

fn open_inbox(store: &Pop3Store) -> Result<Box<dyn Folder>, StoreError> {
    let (tx, rx) = mpsc::channel();
    store.open_folder("INBOX", Box::new(|_| {}), Box::new(move |r| tx.send(r).unwrap()));
    rx.recv_timeout(Duration::from_secs(5)).expect("open_folder completes")
}

#[test]
fn pop3_store_lists_and_fetches_over_hopf() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(dir.path());
    let mut store = Pop3Store::new(bound.ip().to_string(), bound.port());
    store.set_use_stls(false); // the test server is plaintext
    store.set_auth("alice", "secret");

    let folder = open_inbox(&store).expect("INBOX opens");
    let (tx, rx) = mpsc::channel();
    folder.message_count(Box::new(move |r| tx.send(r).unwrap()));
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), 2);

    // Summaries for the whole window, one TOP each, one session.
    let summaries: Arc<Mutex<Vec<ConversationSummary>>> = Arc::new(Mutex::new(Vec::new()));
    let s2 = Arc::clone(&summaries);
    let (tx, rx) = mpsc::channel();
    folder.list_conversations(
        0..10,
        Box::new(move |s| s2.lock().unwrap().push(s)),
        Box::new(move |r| tx.send(r).unwrap()),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    let summaries = summaries.lock().unwrap().clone();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].envelope.subject.as_deref(), Some("one"));
    assert_eq!(summaries[1].envelope.subject.as_deref(), Some("two"));
    assert_eq!(summaries[1].envelope.from[0].local_part, "c");
    assert!(summaries.iter().all(|s| s.id.as_str().starts_with("pop3://alice@")), "{:?}", summaries[0].id);

    // The second message, whole, dot-unstuffed.
    let body: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let b2 = Arc::clone(&body);
    let envelope = Arc::new(Mutex::new(None));
    let e2 = Arc::clone(&envelope);
    let (tx, rx) = mpsc::channel();
    folder.get_message(
        &summaries[1].id,
        Box::new(move |e| *e2.lock().unwrap() = Some(e)),
        Box::new(move |c| b2.lock().unwrap().extend_from_slice(c)),
        Box::new(move |r| tx.send(r).unwrap()),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    let body = body.lock().unwrap().clone();
    assert!(body.windows(11).any(|w| w == b"second body"));
    assert!(body.windows(12).any(|w| w == b".dotted line"), "{:?}", String::from_utf8_lossy(&body));
    assert_eq!(envelope.lock().unwrap().as_ref().unwrap().subject.as_deref(), Some("two"));
}

#[test]
fn pop3_store_reports_missing_and_wrong_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(dir.path());
    let mut store = Pop3Store::new(bound.ip().to_string(), bound.port());
    store.set_use_stls(false);
    store.set_username("alice");
    match open_inbox(&store) {
        Err(StoreError::NeedsCredential { username, is_plaintext, .. }) => {
            assert_eq!(username, "alice");
            assert!(is_plaintext);
        }
        other => panic!("expected NeedsCredential, got {:?}", other.map(|_| ())),
    }
    store.set_credential(Some("alice"), "wrong");
    let err = open_inbox(&store).err().expect("wrong password fails");
    assert!(err.to_string().contains("authentication failed"), "{err}");

    // STLS required against a server that has none: refused before login.
    store.set_use_stls(true);
    store.set_credential(Some("alice"), "secret");
    let err = open_inbox(&store).err().expect("STLS requirement enforced");
    assert!(err.to_string().contains("STLS"), "{err}");
}
