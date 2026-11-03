//! Remote images in a message body: left alone while blocked, pointed at the
//! proxy when allowed, and never touching links. A Maildir account (here) and
//! an IMAP one (`body_images_imap.rs`) take different paths to the same result.

use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use tagliacarte_app::frb_api::frb_load_config;
use std::sync::{Arc, Mutex};

use tagliacarte_app::mail_view::{MessageRef, OpenEvent, ViewResponse, open_message, serve_body};
use tagliacarte_app::remote_rewrite::proxy_url;

const HTML: &str = "From: a@b.example\r\nTo: c@d.example\r\nSubject: News\r\nDate: Tue, 3 Nov 2026 11:00:00 +0000\r\n\
Message-ID: <n@x>\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
<p>Hello <a href=\"https://example.com/read?id=7\">read online</a></p>\r\n\
<img src=\"https://img.example/logo.png\" width=1 height=1>\r\n\
<img src=\"cid:inline@example\">\r\n";

fn maildir_account(root: &Path, raw: &str) {
    let mail = root.join("mail");
    for d in ["cur", "new", "tmp"] {
        std::fs::create_dir_all(mail.join(d)).unwrap();
    }
    std::fs::write(mail.join("cur/1733356800.1000.local:2,S"), raw).unwrap();
    std::fs::write(
        mail.join(".uidlist"),
        "# gumdrop-uidlist v1\nuidvalidity 1770639819\nuidnext 2\n1 1733356800.1000.local\n",
    )
    .unwrap();
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let config = data.join("config.xml");
    std::fs::write(
        &config,
        format!(
            r#"<tagliacarte><security use-keychain="false"/><stores><store id="s1" type="maildir" display-name="T" path="{}"/></stores></tagliacarte>"#,
            mail.display()
        ),
    )
    .unwrap();
    assert_eq!(frb_load_config(config.to_string_lossy().into_owned()).accounts.len(), 1);
}

fn body(images: Option<&str>) -> ViewResponse {
    let (tx, rx) = channel();
    serve_body(
        MessageRef { account_id: "s1".into(), folder: "INBOX".into(), message_id: "maildir://INBOX/1733356800.1000.local".into() },
        images.map(str::to_owned),
        Box::new(move |r| tx.send(r).unwrap()),
    );
    rx.recv_timeout(Duration::from_secs(15)).expect("answered within 15 s")
}

// One test: `frb_load_config` installs process-wide state.
#[test]
fn remote_images_are_blocked_until_allowed_and_then_proxied() {
    let dir = tempfile::tempdir().unwrap();
    maildir_account(dir.path(), HTML);

    let blocked = body(None);
    assert_eq!(blocked.status, 200);
    // The reader is told how much there is to load, though nothing is loaded yet:
    // the one remote image, not the inline one nor the link.
    assert_eq!(blocked.remote_images, 1);
    let text = String::from_utf8(blocked.body).unwrap();
    assert!(text.contains(r#"src="https://img.example/logo.png""#), "left as the sender wrote it: {text}");
    assert!(!text.contains("/remote/"), "nothing is proxied while blocked: {text}");

    let allowed_response = body(Some("/remote/KEY/"));
    assert_eq!(allowed_response.remote_images, 1);
    let allowed = String::from_utf8(allowed_response.body).unwrap();
    let proxied = proxy_url("/remote/KEY/", "https://img.example/logo.png");
    assert!(allowed.contains(&format!(r#"src="{proxied}""#)), "{allowed}");
    assert!(!allowed.contains("img.example"), "the remote address is gone from the page: {allowed}");
    assert!(allowed.contains(r#"href="https://example.com/read?id=7""#), "links are untouched: {allowed}");
    // An inline image still comes from the message itself.
    assert!(allowed.contains("/cid/inline%40example") || allowed.contains("/cid/inline@example"), "{allowed}");
    assert!(allowed.contains("width=1 height=1"), "the rest of the tag is kept: {allowed}");

    // The reader is told there is HTML, so that it can offer to load images.
    let (tx, rx) = channel();
    let seen: Arc<Mutex<Vec<bool>>> = Arc::default();
    let sink = Arc::clone(&seen);
    open_message(
        MessageRef { account_id: "s1".into(), folder: "INBOX".into(), message_id: "maildir://INBOX/1733356800.1000.local".into() },
        Arc::new(move |ev| {
            if let OpenEvent::Body { html } = ev {
                sink.lock().unwrap().push(html);
            }
        }),
        Box::new(move |r| tx.send(r).unwrap()),
    );
    rx.recv_timeout(Duration::from_secs(15)).expect("opened").expect("no error");
    assert_eq!(*seen.lock().unwrap(), [true]);
}
