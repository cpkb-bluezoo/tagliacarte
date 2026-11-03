//! A signed message says so when it is opened: `valid`, `invalid`, or nothing when
//! there is no key for the sender to check it with. From a Maildir account (the
//! message is at hand) and from an IMAP one (the message is fetched whole, but only
//! because its structure says it is signed).

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_auth::PasswordStore;
use hopf_imap::{ImapConfig, ImapService};
use hopf_mailbox::{MailboxFactory, MaildirFactory};
use tagliacarte_app::frb_api::frb_contacts::{FrbContactEmailInput, FrbContactUpsert, frb_contacts_upsert};
use tagliacarte_app::frb_api::{frb_load_config, frb_save_store_credential};
use tagliacarte_app::mail_view::{MessageRef, OpenEvent, open_message};
use tagliacarte_core::net::hopf_runtime;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
}

const PLAIN: &[u8] = b"From: Test Signer <signer@example.org>\r\nTo: reader@example.org\r\nSubject: Not signed\r\nDate: Tue, 3 Nov 2026 11:00:00 +0000\r\nMessage-ID: <plain@example.org>\r\n\r\nNo signature here.\r\n";

/// The three messages, in the order they get UIDs 1, 2 and 3.
fn messages() -> [Vec<u8>; 3] {
    let good = fixture("pgp_signed.eml");
    let mut tampered = good.clone();
    let at = tampered.windows(5).position(|w| w == b"Hello").unwrap();
    tampered[at] = b'J';
    [good, tampered, PLAIN.to_vec()]
}

/// What opening the message reports about its signature, and whether it finished.
fn signature_of(account: &str, message_id: &str) -> Option<String> {
    let (tx, rx) = channel();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = Arc::clone(&seen);
    open_message(
        MessageRef { account_id: account.into(), folder: "INBOX".into(), message_id: message_id.into() },
        Arc::new(move |ev| {
            if let OpenEvent::Signature(s) = ev {
                sink.lock().unwrap().push(s);
            }
        }),
        Box::new(move |r| tx.send(r).unwrap()),
    );
    rx.recv_timeout(Duration::from_secs(30)).expect("opened within 30 s").expect("no error");
    let got = seen.lock().unwrap().clone();
    assert!(got.len() <= 1, "one verdict at most: {got:?}");
    got.into_iter().next()
}

fn config(root: &Path, store: &str) {
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let path = data.join("config.xml");
    std::fs::write(&path, format!(r#"<tagliacarte><security use-keychain="false"/><stores>{store}</stores></tagliacarte>"#)).unwrap();
    assert_eq!(frb_load_config(path.to_string_lossy().into_owned()).accounts.len(), 1);
}

fn start_imap(dir: &Path) -> SocketAddr {
    let factory = Arc::new(MaildirFactory::new(dir));
    {
        let mut store = factory.create_store();
        store.open("alice").unwrap();
        let mut mb = store.open_mailbox("INBOX", false).unwrap();
        for m in messages() {
            let mut g = hopf_mailbox::AppendGuard::start(mb.as_mut(), &BTreeSet::new(), None).unwrap();
            g.append_content(&m).unwrap();
            g.commit().unwrap();
        }
        mb.close(false).unwrap();
        store.close().unwrap();
    }
    let users = Arc::new(PasswordStore::new().with_user("alice", "secret"));
    ImapService::new(ImapConfig::new("127.0.0.1:0".parse().unwrap(), "localhost", users, factory), hopf_runtime())
        .start()
        .unwrap()
}

// One test: the configuration and the data directory are process-wide.
#[test]
fn a_signed_message_reports_whether_its_signature_checks_out() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    // SAFETY: nothing else in this process reads the environment while it runs.
    unsafe { std::env::set_var("TAGLIACARTE_DATA_DIR", &data) };
    // The sender's key is in the address book.
    frb_contacts_upsert(FrbContactUpsert {
        id: None,
        display_name: Some("Test Signer".into()),
        nickname: None,
        organization: None,
        title: None,
        birthday: None,
        notes: None,
        import_origin: None,
        externally_share_ok: None,
        emails: Some(vec![FrbContactEmailInput { email: "signer@example.org".into(), label: None }]),
        phones: None,
        urls: None,
        postal_addresses: None,
        photos: None,
        pgp_fingerprint: None,
        pgp_key_path: Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/signer_public.asc").to_string_lossy().into_owned()),
        smime_cert_path: None,
        smime_notes: None,
    })
    .expect("contact saved");

    // A Maildir account.
    let mail = dir.path().join("mail");
    for d in ["cur", "new", "tmp"] {
        std::fs::create_dir_all(mail.join(d)).unwrap();
    }
    let names = ["1733356800.1000.local", "1733356801.1001.local", "1733356802.1002.local"];
    let mut uidlist = String::from("# gumdrop-uidlist v1\nuidvalidity 1770639819\nuidnext 4\n");
    for (i, (name, m)) in names.iter().zip(messages()).enumerate() {
        std::fs::write(mail.join("cur").join(format!("{name}:2,S")), m).unwrap();
        uidlist.push_str(&format!("{} {name}\n", i + 1));
    }
    std::fs::write(mail.join(".uidlist"), uidlist).unwrap();
    config(dir.path(), &format!(r#"<store id="local" type="maildir" display-name="L" path="{}"/>"#, mail.display()));
    let id = |n: &str| format!("maildir://INBOX/{n}");
    assert_eq!(signature_of("local", &id(names[0])).as_deref(), Some("valid"), "signed, and as it was signed");
    assert_eq!(signature_of("local", &id(names[1])).as_deref(), Some("invalid"), "changed since");
    assert_eq!(signature_of("local", &id(names[2])), None, "not signed");

    // An IMAP account: the same, by way of the structure.
    let bound = start_imap(&dir.path().join("imap"));
    config(
        dir.path(),
        &format!(r#"<store id="remote" type="imap" display-name="R" host="{}" port="{}" security="plain"/>"#, bound.ip(), bound.port()),
    );
    frb_save_store_credential("remote".into(), "alice".into(), "secret".into()).expect("credential saved");
    assert_eq!(signature_of("remote", "1").as_deref(), Some("valid"), "IMAP, signed");
    assert_eq!(signature_of("remote", "2").as_deref(), Some("invalid"), "IMAP, changed");
    assert_eq!(signature_of("remote", "3"), None, "IMAP, not signed");
}
