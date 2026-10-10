//! End-to-end: tagliacarte's NNTP store and transport (now on hopf-nntp)
//! against a scripted NNTP server, in process: AUTHINFO, LIST ACTIVE,
//! GROUP, OVER, ARTICLE and POST.

use std::io;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use hopf_core::{Endpoint, ProtocolHandler, TcpListenerConfig};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::nntp::{NntpStore, NntpTransport};
use tagliacarte_core::store::{Address, SendPayload, Store, Transport};

#[derive(Default)]
struct Recorded {
    commands: Vec<String>,
    posted: Vec<Vec<u8>>,
}

struct Server {
    recorded: Arc<Mutex<Recorded>>,
    buf: Vec<u8>,
    authed: bool,
    group: bool,
    posting: Option<Vec<u8>>,
}

fn reply(ep: &mut dyn Endpoint, s: &str) {
    ep.send(s.as_bytes());
    ep.send(b"\r\n");
}

impl Server {
    fn command(&mut self, ep: &mut dyn Endpoint, line: &str) {
        self.recorded.lock().unwrap().commands.push(line.to_string());
        let mut w = line.split_whitespace();
        let verb = w.next().unwrap_or("").to_ascii_uppercase();
        match verb.as_str() {
            "CAPABILITIES" => {
                for l in ["101 Capability list:", "VERSION 2", "READER", "AUTHINFO USER", "LIST ACTIVE", "OVER", "POST", "."] {
                    reply(ep, l);
                }
            }
            "AUTHINFO" => match w.next().map(|s| s.to_ascii_uppercase()).as_deref() {
                Some("USER") => reply(ep, "381 Enter passphrase"),
                Some("PASS") if w.next() == Some("secret") => {
                    self.authed = true;
                    reply(ep, "281 Authentication accepted");
                }
                _ => reply(ep, "481 Authentication failed"),
            },
            "QUIT" => {
                reply(ep, "205 bye");
                ep.close();
            }
            _ if !self.authed => reply(ep, "480 Authentication required"),
            "LIST" => {
                reply(ep, "215 list follows");
                reply(ep, "comp.lang.rust 12 10 y");
                reply(ep, "alt.test 5 5 m");
                reply(ep, ".");
            }
            "GROUP" => {
                if w.next() == Some("comp.lang.rust") {
                    self.group = true;
                    reply(ep, "211 3 10 12 comp.lang.rust");
                } else {
                    reply(ep, "411 No such newsgroup");
                }
            }
            "OVER" if self.group => {
                reply(ep, "224 Overview information follows");
                reply(ep, "10\tFirst post\tann <ann@example.com>\tMon, 6 Oct 2026 10:00:00 +0000\t<a1@example.com>\t\t120\t3");
                reply(ep, "11\tRe: First post\tbob <bob@example.com>\tMon, 6 Oct 2026 11:00:00 +0000\t<b1@example.com>\t<a1@example.com>\t140\t4");
                reply(ep, "12\tDots\tcat <cat@example.com>\tMon, 6 Oct 2026 12:00:00 +0000\t<c1@example.com>\t\t90\t2");
                reply(ep, ".");
            }
            "ARTICLE" | "HEAD" if self.group => {
                let n: u64 = w.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                if !(10..=12).contains(&n) {
                    return reply(ep, "423 No article with that number");
                }
                reply(ep, if verb == "HEAD" { "221 article head follows" } else { "220 article follows" });
                reply(ep, &format!("From: who{n} <who{n}@example.com>"));
                reply(ep, "Newsgroups: comp.lang.rust");
                reply(ep, &format!("Subject: Article {n}"));
                reply(ep, &format!("Message-ID: <m{n}@example.com>"));
                if verb == "ARTICLE" {
                    reply(ep, "");
                    reply(ep, &format!("body of article {n}"));
                    if n == 12 {
                        reply(ep, "..dotted line");
                    }
                }
                reply(ep, ".");
            }
            "POST" => {
                self.posting = Some(Vec::new());
                reply(ep, "340 send article");
            }
            _ => reply(ep, "500 what?"),
        }
    }
}

impl ProtocolHandler for Server {
    fn connected(&mut self, ep: &mut dyn Endpoint) {
        reply(ep, "200 scripted server ready (posting ok)");
    }
    fn receive(&mut self, ep: &mut dyn Endpoint, data: &mut &[u8]) {
        self.buf.extend_from_slice(data);
        *data = &[];
        while let Some(i) = self.buf.windows(2).position(|w| w == b"\r\n") {
            let line: Vec<u8> = self.buf.drain(..i + 2).collect();
            let line = &line[..line.len() - 2];
            if let Some(body) = self.posting.as_mut() {
                if line == b"." {
                    let article = self.posting.take().unwrap();
                    self.recorded.lock().unwrap().posted.push(article);
                    reply(ep, "240 Article received");
                } else {
                    let l: &[u8] = if line.starts_with(b"..") { &line[1..] } else { line };
                    body.extend_from_slice(l);
                    body.extend_from_slice(b"\r\n");
                }
                continue;
            }
            let text = String::from_utf8_lossy(line).into_owned();
            self.command(ep, &text);
        }
    }
    fn disconnected(&mut self, _ep: &mut dyn Endpoint) {}
    fn error(&mut self, _ep: &mut dyn Endpoint, _err: &io::Error) {}
}

fn start_server() -> (SocketAddr, Arc<Mutex<Recorded>>) {
    let recorded = Arc::new(Mutex::new(Recorded::default()));
    let rec = Arc::clone(&recorded);
    let (addr, _) = hopf_runtime()
        .add_tcp_listener(TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
            Box::new(Server { recorded: Arc::clone(&rec), buf: Vec::new(), authed: false, group: false, posting: None })
                as Box<dyn ProtocolHandler>
        }))
        .unwrap();
    (addr, recorded)
}

fn wait<T>(rx: mpsc::Receiver<T>) -> T {
    rx.recv_timeout(Duration::from_secs(10)).expect("callback within 10s")
}

#[test]
fn store_lists_groups_reads_articles_and_transport_posts() {
    let (addr, recorded) = start_server();
    let mut store = NntpStore::new(addr.ip().to_string(), addr.port());
    store.set_implicit_tls(false);
    store.set_use_starttls(false);
    store.set_username("alice");
    store.set_credential(Some("alice"), "secret");

    // LIST ACTIVE through the Store trait.
    let names = Arc::new(Mutex::new(Vec::new()));
    let n2 = Arc::clone(&names);
    let (tx, rx) = mpsc::channel();
    store.list_folders(Box::new(move |f| n2.lock().unwrap().push(f.name)), Box::new(move |r| tx.send(r).unwrap()));
    wait(rx).unwrap();
    assert_eq!(*names.lock().unwrap(), vec!["comp.lang.rust", "alt.test"]);

    // GROUP + OVER through the Folder trait.
    let (tx, rx) = mpsc::channel();
    store.open_folder("comp.lang.rust", Box::new(|_| {}), Box::new(move |r| tx.send(r).unwrap()));
    let folder = wait(rx).unwrap();
    let (tx, rx) = mpsc::channel();
    folder.message_count(Box::new(move |r| tx.send(r).unwrap()));
    assert_eq!(wait(rx).unwrap(), 3);
    let summaries = Arc::new(Mutex::new(Vec::new()));
    let s2 = Arc::clone(&summaries);
    let (tx, rx) = mpsc::channel();
    folder.list_conversations(0..3, Box::new(move |s| s2.lock().unwrap().push(s)), Box::new(move |r| tx.send(r).unwrap()));
    wait(rx).unwrap();
    let summaries = summaries.lock().unwrap().clone();
    assert_eq!(summaries.len(), 3, "{summaries:?}");
    let subjects: Vec<String> = summaries.iter().map(|s| s.envelope.subject.clone().unwrap_or_default()).collect();
    assert!(subjects.contains(&"Dots".to_string()), "{subjects:?}");

    // ARTICLE through the Folder trait, with the stuffed dot removed.
    let dots = summaries.iter().find(|s| s.envelope.subject.as_deref() == Some("Dots")).unwrap();
    let raw = Arc::new(Mutex::new(Vec::<u8>::new()));
    let r2 = Arc::clone(&raw);
    let (tx, rx) = mpsc::channel();
    folder.get_message(&dots.id, Box::new(|_| {}), Box::new(move |c| r2.lock().unwrap().extend_from_slice(c)), Box::new(move |r| tx.send(r).unwrap()));
    wait(rx).unwrap();
    let text = String::from_utf8_lossy(&raw.lock().unwrap()).into_owned();
    assert!(text.contains("Subject: Article 12"), "{text}");
    assert!(text.contains("body of article 12"), "{text}");
    assert!(text.contains("\n.dotted line"), "stuffed dot removed: {text}");
    assert!(!text.contains("..dotted"), "{text}");

    // POST through the Transport, sharing the store's session.
    let transport = NntpTransport::from_store_state(store.shared_state());
    let payload = SendPayload {
        from: vec![Address { display_name: Some("Alice".into()), local_part: "alice".into(), domain: Some("example.com".into()) }],
        to: vec![],
        cc: vec![],
        bcc: vec![],
        subject: Some("hello".into()),
        body_plain: Some(".leading dot\nsecond line".into()),
        body_html: None,
        attachments: vec![],
        newsgroups: vec!["comp.lang.rust".into()],
        nntp_in_reply_to: Some("<m12@example.com>".into()),
        nntp_references: None,
        smtp_notify: None,
        smtp_in_reply_to: None,
        smtp_references: None,
        smtp_message_id: None,
    };
    let (tx, rx) = mpsc::channel();
    transport.send(&payload, Box::new(move |r| tx.send(r).unwrap()));
    wait(rx).unwrap();
    let rec = recorded.lock().unwrap();
    assert_eq!(rec.posted.len(), 1);
    let posted = String::from_utf8_lossy(&rec.posted[0]).into_owned();
    assert!(posted.contains("Newsgroups: comp.lang.rust\r\n"), "{posted}");
    assert!(posted.contains("In-Reply-To: <m12@example.com>\r\n"), "{posted}");
    assert!(posted.contains("\r\n\r\n.leading dot\r\nsecond line\r\n"), "server saw the body unstuffed: {posted}");
    assert_eq!(rec.commands.iter().filter(|c| c.starts_with("AUTHINFO USER")).count(), 1, "one session for store and transport: {:?}", rec.commands);
}

/// Operations issued before the session is up queue behind one dial: the
/// second must not open a second connection (the server would see a second
/// AUTHINFO), and both must complete once the greeting and AUTHINFO are done.
#[test]
fn operations_before_the_greeting_share_one_dial() {
    let (addr, recorded) = start_server();
    let mut store = NntpStore::new(addr.ip().to_string(), addr.port());
    store.set_implicit_tls(false);
    store.set_use_starttls(false);
    store.set_credential(Some("alice"), "secret");

    let (tx1, rx1) = mpsc::channel();
    store.list_folders(Box::new(|_| {}), Box::new(move |r| tx1.send(r).unwrap()));
    let (tx2, rx2) = mpsc::channel();
    store.open_folder("comp.lang.rust", Box::new(|_| {}), Box::new(move |r| tx2.send(r).unwrap()));
    wait(rx1).unwrap();
    let folder = wait(rx2).unwrap();
    let (tx, rx) = mpsc::channel();
    folder.message_count(Box::new(move |r| tx.send(r).unwrap()));
    assert_eq!(wait(rx).unwrap(), 3);

    let commands = recorded.lock().unwrap().commands.clone();
    let dials = commands.iter().filter(|c| c.to_ascii_uppercase().starts_with("AUTHINFO USER")).count();
    assert_eq!(dials, 1, "one AUTHINFO per connection: {commands:?}");
    let list_at = commands.iter().position(|c| c.starts_with("LIST")).unwrap();
    let group_at = commands.iter().position(|c| c.starts_with("GROUP")).unwrap();
    assert!(list_at < group_at, "queued operations keep their order: {commands:?}");
}
