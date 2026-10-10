//! End-to-end: tagliacarte's hopf-based IMAP session against hopf's own IMAP
//! server over a seeded Maildir, in process.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_auth::PasswordStore;
use hopf_imap::{ImapConfig, ImapService};
use hopf_mailbox::{MailboxFactory, MaildirFactory};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::imap::session::{
    probe_capabilities_blocking, FetchItem, ImapSession, ImapSessionConfig, Job,
};
use tagliacarte_core::sasl::SaslMechanism;

const T: Duration = Duration::from_secs(10);

fn start_server(dir: &std::path::Path) -> SocketAddr {
    let factory = Arc::new(MaildirFactory::new(dir));
    {
        let mut store = factory.create_store();
        store.open("alice").unwrap();
        let mut mb = store.open_mailbox("INBOX", false).unwrap();
        let msgs: [&[u8]; 3] = [
            b"From: a@b\r\nSubject: Question\r\nMessage-Id: <1@x>\r\nDate: Thu, 01 Jan 2026 10:00:00 +0000\r\n\r\nfirst\r\n",
            b"From: c@d\r\nSubject: Re: Question\r\nMessage-Id: <2@x>\r\nIn-Reply-To: <1@x>\r\nReferences: <1@x>\r\nDate: Thu, 01 Jan 2026 11:00:00 +0000\r\n\r\nreply\r\n",
            b"From: e@f\r\nSubject: Other\r\nMessage-Id: <3@x>\r\nDate: Thu, 01 Jan 2026 09:00:00 +0000\r\n\r\nunrelated\r\n",
        ];
        for m in msgs {
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

fn config(bound: SocketAddr, password: &str, stale: &Arc<AtomicBool>, idle_secs: u32) -> ImapSessionConfig {
    ImapSessionConfig {
        host: bound.ip().to_string(),
        port: bound.port(),
        implicit_tls: false,
        starttls: false,
        auth: Some(("alice".into(), password.into(), SaslMechanism::ScramSha256)),
        min_idle_secs: Arc::new(AtomicU32::new(idle_secs)),
        folder_list_stale: Arc::clone(stale),
        auto_idle: true,
    }
}

fn wait_until(pred: impl Fn() -> bool, max: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < max {
        if pred() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    pred()
}

#[test]
fn session_runs_jobs_selects_mailboxes_and_idles() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(dir.path());
    let stale = Arc::new(AtomicBool::new(false));
    let session = ImapSession::connect(config(bound, "secret", &stale, 1));

    let caps = session.wait_ready(T).expect("login");
    for want in ["IDLE", "SORT", "LIST-STATUS", "UIDPLUS"] {
        assert!(caps.has(want), "server lacks {want}: {:?}", caps.tokens);
    }

    // LIST with RETURN (STATUS (UNSEEN)) in one round trip.
    let rows: Arc<Mutex<Vec<(String, Option<u32>)>>> = Arc::new(Mutex::new(Vec::new()));
    let r2 = Arc::clone(&rows);
    session
        .run_blocking(
            |done| Job::List {
                pattern: "*".into(),
                status_unseen: true,
                on_row: Box::new(move |e, unseen| r2.lock().unwrap().push((e.name.clone(), unseen))),
                done,
            },
            T,
        )
        .unwrap();
    let rows = rows.lock().unwrap().clone();
    assert!(rows.iter().any(|(n, u)| n == "INBOX" && *u == Some(3)), "{rows:?}");

    // SELECT INBOX.
    let info = session.run_blocking(|done| Job::Select { mailbox: "INBOX".into(), done }, T).unwrap();
    assert_eq!(info.exists, 3);

    // FETCH summaries: parsed attributes plus the header literal.
    let items: Arc<Mutex<Vec<FetchItem>>> = Arc::new(Mutex::new(Vec::new()));
    let i2 = Arc::clone(&items);
    session
        .run_blocking(
            |done| Job::Fetch {
                mailbox: "INBOX".into(),
                uid: false,
                set: "1:3".into(),
                items: "(UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (FROM SUBJECT DATE)])".into(),
                on_item: Box::new(move |it| i2.lock().unwrap().push(it)),
                stream: None,
                done,
            },
            T,
        )
        .unwrap();
    let items = items.lock().unwrap().clone();
    assert_eq!(items.len(), 3, "{items:?}");
    for it in &items {
        assert!(it.uid.is_some() && it.size.is_some(), "{it:?}");
        assert!(it.literal.windows(8).any(|w| w == b"Subject:"), "{:?}", String::from_utf8_lossy(&it.literal));
    }

    // UID SORT by date: message 3 (09:00) comes first.
    let sorted = session.run_blocking(|done| Job::UidSort { mailbox: "INBOX".into(), criteria: "(DATE)".into(), done }, T).unwrap();
    assert_eq!(sorted.len(), 3);
    let uid_of = |seq: u32| items.iter().find(|i| i.seq == seq).unwrap().uid.unwrap();
    assert_eq!(sorted[0], uid_of(3), "{sorted:?}");

    // UID SEARCH by header.
    let found = session
        .run_blocking(|done| Job::UidSearch { mailbox: "INBOX".into(), criteria: "HEADER Message-Id <2@x>".into(), done }, T)
        .unwrap();
    assert_eq!(found, vec![uid_of(2)]);

    // APPEND with UIDPLUS, already seen: unseen count unchanged.
    let appended = session
        .run_blocking(
            |done| Job::Append {
                mailbox: "INBOX".into(),
                flags: Some("(\\Seen)".into()),
                data: b"From: me@x\r\nSubject: appended\r\n\r\nbody\r\n".to_vec(),
                done,
            },
            T,
        )
        .unwrap()
        .expect("APPENDUID");
    let status = session.run_blocking(|done| Job::Status { mailbox: "INBOX".into(), done }, T).unwrap();
    assert_eq!(status.unseen, Some(3), "{status:?}");

    // Flag it deleted and UID EXPUNGE it: back to 3.
    session
        .run_blocking(
            |done| Job::UidStore { mailbox: "INBOX".into(), set: appended.uid.to_string(), action: "+FLAGS".into(), flags: "(\\Deleted)".into(), done },
            T,
        )
        .unwrap();
    session.run_blocking(|done| Job::UidExpunge { mailbox: "INBOX".into(), set: appended.uid.to_string(), done }, T).unwrap();
    let info = session.run_blocking(|done| Job::Select { mailbox: "INBOX".into(), done }, T).unwrap();
    assert_eq!(info.exists, 3);

    // CREATE + UID COPY, then a FETCH on the other mailbox selects it by itself.
    session.run_blocking(|done| Job::Create { mailbox: "Archive".into(), done }, T).unwrap();
    let copied = session
        .run_blocking(|done| Job::UidCopy { mailbox: "INBOX".into(), set: uid_of(1).to_string(), dest: "Archive".into(), done }, T)
        .unwrap();
    assert!(copied.is_some(), "COPYUID expected");
    let archived: Arc<Mutex<Vec<FetchItem>>> = Arc::new(Mutex::new(Vec::new()));
    let a2 = Arc::clone(&archived);
    session
        .run_blocking(
            |done| Job::Fetch {
                mailbox: "Archive".into(),
                uid: false,
                set: "1:*".into(),
                items: "(UID BODY.PEEK[HEADER.FIELDS (SUBJECT)])".into(),
                on_item: Box::new(move |it| a2.lock().unwrap().push(it)),
                stream: None,
                done,
            },
            T,
        )
        .unwrap();
    let archived = archived.lock().unwrap().clone();
    assert_eq!(archived.len(), 1);
    assert!(archived[0].literal.windows(8).any(|w| w == b"Question"));

    // Idle: after the 1 s quiet period the session is in IDLE on Archive;
    // a message appended there by another session shows up as staleness.
    stale.store(false, Ordering::Release);
    std::thread::sleep(Duration::from_millis(1500));
    let other = ImapSession::connect(ImapSessionConfig { auto_idle: false, ..config(bound, "secret", &Arc::new(AtomicBool::new(false)), 60) });
    other
        .run_blocking(
            |done| Job::Append { mailbox: "Archive".into(), flags: None, data: b"From: x@y\r\nSubject: new\r\n\r\nhi\r\n".to_vec(), done },
            T,
        )
        .unwrap();
    assert!(wait_until(|| stale.load(Ordering::Acquire), Duration::from_secs(5)), "EXISTS during IDLE must set the stale flag");
    // A job submitted while idling ends IDLE and runs.
    let status = session.run_blocking(|done| Job::Status { mailbox: "Archive".into(), done }, T).unwrap();
    assert!(status.unseen.unwrap_or(0) >= 1, "{status:?}");

    session.close();
    other.close();
    assert!(wait_until(|| !session.is_alive(), Duration::from_secs(5)));
}

#[test]
fn session_reports_bad_credentials_and_probe_lists_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(dir.path());
    let stale = Arc::new(AtomicBool::new(false));
    let session = ImapSession::connect(config(bound, "wrong", &stale, 60));
    let err = session.wait_ready(T).unwrap_err();
    assert!(err.message.contains("authentication failed"), "{err}");
    let err = session.run_blocking(|done| Job::Status { mailbox: "INBOX".into(), done }, T).unwrap_err();
    assert!(!err.message.is_empty());

    let caps = probe_capabilities_blocking(&ImapSessionConfig { auth: None, ..config(bound, "", &stale, 60) }, T).unwrap();
    assert!(caps.iter().any(|c| c.starts_with("IMAP4")), "{caps:?}");
}

#[test]
fn reselecting_the_selected_mailbox_does_not_wedge_later_commands() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(dir.path());
    let stale = Arc::new(AtomicBool::new(false));
    let session = Arc::new(ImapSession::connect(config(bound, "secret", &stale, 3600)));
    session.wait_ready(T).expect("login");

    // The app opens a folder from several places at once: each one
    // SELECTs, sorts and fetches, so SELECTs of the mailbox that is
    // already selected are interleaved with FETCHes.
    let workers: Vec<_> = (0..3)
        .map(|_| {
            let session = Arc::clone(&session);
            std::thread::spawn(move || {
                for _ in 0..5 {
                    let info = session.run_blocking(|done| Job::Select { mailbox: "INBOX".into(), done }, T).unwrap();
                    assert_eq!(info.exists, 3);
                    let uids = session
                        .run_blocking(|done| Job::UidSort { mailbox: "INBOX".into(), criteria: "(DATE)".into(), done }, T)
                        .unwrap();
                    assert_eq!(uids.len(), 3);
                    let set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
                    let got = Arc::new(Mutex::new(0usize));
                    let g2 = Arc::clone(&got);
                    session
                        .run_blocking(
                            |done| Job::Fetch {
                                mailbox: "INBOX".into(),
                                uid: true,
                                set,
                                items: "(UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (FROM SUBJECT DATE)])".into(),
                                on_item: Box::new(move |_| *g2.lock().unwrap() += 1),
                                stream: None,
                                done,
                            },
                            T,
                        )
                        .unwrap();
                    assert_eq!(*got.lock().unwrap(), 3);
                }
            })
        })
        .collect();
    for w in workers {
        w.join().expect("worker finished without a timeout");
    }
}
