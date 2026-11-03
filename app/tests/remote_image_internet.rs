//! Needs the internet, so it is ignored by default:
//! `cargo test -p tagliacarte_app --test remote_image_internet -- --ignored`
//!
//! The in-process tests speak plain HTTP. This one shows that HTTPS works
//! through the checked dial (the address is resolved and judged first, then
//! dialled with the host name kept for TLS verification), and that the TLS
//! name is really checked.

use std::sync::mpsc::channel;
use std::time::Duration;

use tagliacarte_app::remote_image::{fetch, Fetched, Refused};

fn get(url: &str) -> Result<Fetched, Refused> {
    let (tx, rx) = channel();
    fetch(url, Box::new(move |r| tx.send(r).unwrap()));
    rx.recv_timeout(Duration::from_secs(40)).expect("answered within 40 s")
}

#[test]
#[ignore = "needs the internet"]
fn an_https_image_is_fetched_and_a_wrong_certificate_name_is_refused() {
    let img = get("https://www.wikipedia.org/static/favicon/wikipedia.ico").expect("an https image");
    assert!(img.content_type.starts_with("image/"), "{}", img.content_type);
    assert!(img.body.len() > 100);

    // The certificate of this host is for another name: TLS must refuse it.
    let e = get("https://wrong.host.badssl.com/favicon.ico").unwrap_err();
    assert_eq!(e.status, 502, "{}", e.message);
}
