//! The fetcher's own policy: it will not go to loopback or the user's network.
//! A separate file because the switch that lets the other test reach loopback
//! is process-wide.

use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tagliacarte_app::remote_image::{fetch, Fetched, Refused};

fn get(url: &str) -> Result<Fetched, Refused> {
    let (tx, rx) = channel();
    fetch(url, Box::new(move |r| tx.send(r).unwrap()));
    rx.recv_timeout(Duration::from_secs(30)).expect("answered within 30 s")
}

#[test]
fn addresses_inside_the_users_network_are_refused_without_a_connection() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let touched = Arc::new(AtomicBool::new(false));
    let t = Arc::clone(&touched);
    l.set_nonblocking(false).unwrap();
    std::thread::spawn(move || {
        if l.accept().is_ok() {
            t.store(true, Ordering::SeqCst);
        }
    });

    for url in [
        format!("http://127.0.0.1:{port}/a.png"),
        format!("http://localhost:{port}/a.png"),
        format!("http://[::1]:{port}/a.png"),
        "http://192.168.1.1/a.png".to_owned(),
        "http://10.0.0.5:8080/a.png".to_owned(),
        "http://169.254.169.254/latest/meta-data/".to_owned(),
        "http://[::ffff:127.0.0.1]/a.png".to_owned(),
    ] {
        let e = get(&url).unwrap_err();
        assert_eq!(e.status, 403, "{url}: {}", e.message);
    }
    std::thread::sleep(Duration::from_millis(200));
    assert!(!touched.load(Ordering::SeqCst), "the local server must never have been connected to");
}
