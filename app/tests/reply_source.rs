//! Reading the original of a reply from an IMAP account through the real
//! account/store path, against hopf's in-process IMAP server.
//!
//! What this can and cannot show: the server here returns the whole message for
//! any numbered section (`BODY[1]`, `BODY[2]`); only `HEADER` and `TEXT` are
//! implemented. So these checks cover the envelope, the chained header and body
//! fetches on one session, and which kind of part was asked for. They cannot show
//! that the right part is selected; a real server does that.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;

use hopf_auth::PasswordStore;
use hopf_imap::{ImapConfig, ImapService};
use hopf_mailbox::{MailboxFactory, MaildirFactory};
use tagliacarte_app::frb_api::{frb_load_config, frb_save_store_credential};
use tagliacarte_app::mail_view::{MessageRef, load_source};
use tagliacarte_core::net::hopf_runtime;

const MULTIPART: &[u8] = b"Received: by x with SMTP id y;\r\n        Tue, 3 Nov 2026 03:00:00 -0800 (PST)\r\nFrom: \"Example Sender\" <news@example.com>\r\nTo: Alice <alice@example.org>, bob@example.org\r\nCc: carol@example.org\r\nSubject: Weekly digest\r\nDate: Tue, 3 Nov 2026 11:00:00 +0000\r\nMessage-ID: <abc@example.com>\r\nReferences: <r1@example.com> <r2@example.com>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nhello plain \xc3\xa9\r\nsecond line\r\n--b1\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>hello <b>html</b></p>\r\n--b1--\r\n";

/// Text plus an attachment: the text is section 1, as it is in most real mail.
const WITH_ATTACHMENT: &[u8] = b"From: a@b.example\r\nTo: c@d.example\r\nSubject: Short\r\nDate: Tue, 3 Nov 2026 11:00:00 +0000\r\nMessage-ID: <p@x>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"m1\"\r\n\r\n--m1\r\nContent-Type: text/plain; charset=utf-8\r\n\r\njust text\r\n--m1\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"x.bin\"\r\nContent-Transfer-Encoding: base64\r\n\r\nAAAA\r\n--m1--\r\n";

fn start_server(dir: &std::path::Path) -> SocketAddr {
    let factory = Arc::new(MaildirFactory::new(dir));
    {
        let mut store = factory.create_store();
        store.open("alice").unwrap();
        let mut mb = store.open_mailbox("INBOX", false).unwrap();
        for m in [MULTIPART, WITH_ATTACHMENT] {
            let mut g = hopf_mailbox::AppendGuard::start(mb.as_mut(), &BTreeSet::new(), None).unwrap();
            g.append_content(m).unwrap();
            g.commit().unwrap();
        }
        mb.close(false).unwrap();
        store.close().unwrap();
    }
    let users = Arc::new(PasswordStore::new().with_user("alice", "secret"));
    let config = ImapConfig::new("127.0.0.1:0".parse().unwrap(), "localhost", users, factory);
    ImapService::new(config, hopf_runtime()).start().unwrap()
}

fn source(account: &str, uid: &str) -> Result<tagliacarte_app::mail_view::SourceMessage, String> {
    let (tx, rx) = channel();
    load_source(
        MessageRef { account_id: account.into(), folder: "INBOX".into(), message_id: uid.into() },
        Box::new(move |r| tx.send(r).unwrap()),
    );
    rx.recv_timeout(Duration::from_secs(15)).expect("answered within 15 s")
}

#[test]
fn the_original_of_a_reply_is_read_from_an_imap_account() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(&dir.path().join("mail"));
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    // The credentials file sits beside the config; the file backend, not the keychain.
    let config = data.join("config.xml");
    std::fs::write(
        &config,
        format!(
            r#"<tagliacarte><security use-keychain="false"/><stores><store id="s1" type="imap" display-name="T" host="{}" port="{}" security="plain"/></stores></tagliacarte>"#,
            bound.ip(),
            bound.port()
        ),
    )
    .unwrap();
    let cfg = frb_load_config(config.to_string_lossy().into_owned());
    assert!(!cfg.use_keychain && cfg.accounts.len() == 1, "the test config must load");
    frb_save_store_credential("s1".into(), "alice".into(), "secret".into()).expect("credential saved");

    // Multipart: the envelope is whole, and the HTML part (the one the reader shows) comes back.
    let m = source("s1", "1").expect("multipart message");
    let h = &m.headers;
    assert_eq!(h.from[0].email, "news@example.com");
    assert_eq!(h.from[0].name.as_deref(), Some("Example Sender"));
    assert_eq!(h.to.len(), 2);
    assert_eq!(h.cc.len(), 1);
    assert_eq!(h.subject.as_deref(), Some("Weekly digest"));
    assert!(h.date_ms.is_some());
    assert_eq!(h.message_id.as_deref(), Some("<abc@example.com>"));
    assert_eq!(h.references.as_deref(), Some("<r1@example.com> <r2@example.com>"));
    // The HTML part was the one asked for (the plan's choice), so it is the HTML body.
    let html = m.body_html.as_deref().expect("an html body");
    assert!(html.contains("<b>html</b>"), "{html:?}");
    assert!(m.body_plain.is_none());

    // Text and an attachment: the text, and not the attachment.
    let p = source("s1", "2").expect("message with an attachment");
    assert_eq!(p.headers.subject.as_deref(), Some("Short"));
    assert!(p.body_plain.as_deref().is_some_and(|t| t.contains("just text")));
    assert!(p.body_html.is_none());

    // A message that is not there is an error, not an empty source.
    assert!(source("s1", "99").is_err());
}
