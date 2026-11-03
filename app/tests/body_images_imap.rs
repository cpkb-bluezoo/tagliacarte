//! The IMAP half of `body_images.rs`: the body is decoded as it arrives, so the
//! rewrite has to cope with a document that comes in pieces. hopf's in-process
//! server returns the whole message for any numbered section, so the document
//! here also carries the headers; what is checked is the rewriting, not which
//! part was chosen (a real server shows that).

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;

use hopf_auth::PasswordStore;
use hopf_imap::{ImapConfig, ImapService};
use hopf_mailbox::{MailboxFactory, MaildirFactory};
use tagliacarte_app::frb_api::{frb_load_config, frb_save_store_credential};
use tagliacarte_app::mail_view::{MessageRef, serve_body};
use tagliacarte_app::remote_rewrite::proxy_url;
use tagliacarte_core::net::hopf_runtime;

const HTML: &[u8] = b"From: a@b.example\r\nTo: c@d.example\r\nSubject: News\r\nDate: Tue, 3 Nov 2026 11:00:00 +0000\r\nMessage-ID: <n@x>\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hello <a href=\"https://example.com/read?id=7\">read online</a></p>\r\n<img src=\"https://img.example/logo.png\" width=1 height=1>\r\n<style>td{background:url(https://img.example/bg.png)}</style>\r\n";

fn start_server(dir: &std::path::Path) -> SocketAddr {
    let factory = Arc::new(MaildirFactory::new(dir));
    {
        let mut store = factory.create_store();
        store.open("alice").unwrap();
        let mut mb = store.open_mailbox("INBOX", false).unwrap();
        let mut g = hopf_mailbox::AppendGuard::start(mb.as_mut(), &BTreeSet::new(), None).unwrap();
        g.append_content(HTML).unwrap();
        g.commit().unwrap();
        mb.close(false).unwrap();
        store.close().unwrap();
    }
    let users = Arc::new(PasswordStore::new().with_user("alice", "secret"));
    let config = ImapConfig::new("127.0.0.1:0".parse().unwrap(), "localhost", users, factory);
    ImapService::new(config, hopf_runtime()).start().unwrap()
}

fn body(images: Option<&str>) -> String {
    body_and_count(images).0
}

fn body_and_count(images: Option<&str>) -> (String, usize) {
    let (tx, rx) = channel();
    serve_body(
        MessageRef { account_id: "s1".into(), folder: "INBOX".into(), message_id: "1".into() },
        images.map(str::to_owned),
        Box::new(move |r| tx.send(r).unwrap()),
    );
    let r = rx.recv_timeout(Duration::from_secs(15)).expect("answered within 15 s");
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    (String::from_utf8(r.body).unwrap(), r.remote_images)
}

#[test]
fn an_imap_body_is_rewritten_as_it_streams_in() {
    let dir = tempfile::tempdir().unwrap();
    let bound = start_server(&dir.path().join("mail"));
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
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
    assert!(frb_load_config(config.to_string_lossy().into_owned()).accounts.len() == 1);
    frb_save_store_credential("s1".into(), "alice".into(), "secret".into()).expect("credential saved");

    // Counted while blocked too: the image, and the one in the style element.
    assert_eq!(body_and_count(None).1, 2);
    let blocked = body(None);
    assert!(blocked.contains(r#"src="https://img.example/logo.png""#), "{blocked}");
    assert!(!blocked.contains("/remote/"), "{blocked}");

    let allowed = body(Some("/remote/KEY/"));
    assert!(allowed.contains(&format!(r#"src="{}""#, proxy_url("/remote/KEY/", "https://img.example/logo.png"))), "{allowed}");
    assert!(allowed.contains(&format!("url({})", proxy_url("/remote/KEY/", "https://img.example/bg.png"))), "{allowed}");
    assert!(!allowed.contains("img.example"), "{allowed}");
    assert!(allowed.contains(r#"href="https://example.com/read?id=7""#), "{allowed}");
}
