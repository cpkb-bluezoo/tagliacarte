//! Saving a message's attachment to a writer, from a Maildir account through
//! the real account/store path.
//!
//! The IMAP half cannot be shown byte for byte here: hopf's in-process server
//! returns the whole message for any numbered section, so which part comes
//! back is for a real server to prove. `reply_source.rs` says the same.

use std::io::{self, Write};
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tagliacarte_app::frb_api::frb_load_config;
use tagliacarte_app::mail_view::{MessageRef, save_attachment};

/// Not a pattern the base64 alphabet or a line length could hide: every byte
/// value, in an order that changes from one block to the next.
fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i.wrapping_mul(31) ^ (i >> 8)) as u8).collect()
}

fn message(attachment: &[u8]) -> Vec<u8> {
    let mut m = String::from(
        "From: a@b.example\r\nTo: c@d.example\r\nSubject: With a file\r\nDate: Tue, 3 Nov 2026 11:00:00 +0000\r\n\
         Message-ID: <f@x>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"m1\"\r\n\r\n\
         --m1\r\nContent-Type: text/plain; charset=utf-8\r\n\r\njust text\r\n\
         --m1\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"x.bin\"\r\n\
         Content-Transfer-Encoding: base64\r\n\r\n",
    );
    let b64 = STANDARD.encode(attachment);
    for line in b64.as_bytes().chunks(76) {
        m.push_str(std::str::from_utf8(line).unwrap());
        m.push_str("\r\n");
    }
    m.push_str("--m1--\r\n");
    m.into_bytes()
}

fn maildir_account(root: &Path, raw: &[u8]) {
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
    let cfg = frb_load_config(config.to_string_lossy().into_owned());
    assert_eq!(cfg.accounts.len(), 1, "the test config must load");
}

#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Full;

impl Write for Full {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("disk full"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn save(index: u32, out: Box<dyn Write + Send>) -> Result<u64, String> {
    let (tx, rx) = channel();
    save_attachment(
        MessageRef { account_id: "s1".into(), folder: "INBOX".into(), message_id: "maildir://INBOX/1733356800.1000.local".into() },
        index,
        out,
        Box::new(move |r| tx.send(r).unwrap()),
    );
    rx.recv_timeout(Duration::from_secs(15)).expect("answered within 15 s")
}

// One test: the configuration `frb_load_config` installs is process-wide, so
// two tests with their own accounts would race.
#[test]
fn an_attachment_is_written_out_exactly_and_failures_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let file = payload(1_500_000);
    maildir_account(dir.path(), &message(&file));

    let sink = Sink::default();
    let n = save(0, Box::new(sink.clone())).expect("saved");
    assert_eq!(n, file.len() as u64);
    assert!(*sink.0.lock().unwrap() == file, "the saved bytes differ from the attachment");

    let e = save(5, Box::new(Sink::default())).unwrap_err();
    assert!(e.contains("attachment"), "{e}");
    let e = save(0, Box::new(Full)).unwrap_err();
    assert!(e.contains("disk full"), "{e}");
}
