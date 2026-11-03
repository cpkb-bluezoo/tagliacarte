//! An account's picture, from the saved configuration: a file path or a web
//! address. Only what is an image is served, and the page never says where it
//! comes from, only which account.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use tagliacarte_app::avatar;
use tagliacarte_app::frb_api::frb_load_config;
use tagliacarte_app::remote_image::{Fetched, Refused, allow_private_addresses};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-pretend avatar-";

fn serve_png() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", PNG.len());
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(PNG);
        }
    });
    port
}

fn config(root: &Path, accounts: &[(&str, Option<String>)]) {
    let mut stores = String::new();
    for (id, avatar) in accounts {
        let attr = avatar.as_ref().map(|a| format!(r#" avatar-url="{a}""#)).unwrap_or_default();
        stores.push_str(&format!(
            r#"<store id="{id}" type="maildir" display-name="{id}" path="{}"{attr}/>"#,
            root.join("mail").display()
        ));
    }
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let path = data.join("config.xml");
    std::fs::write(&path, format!(r#"<tagliacarte><security use-keychain="false"/><stores>{stores}</stores></tagliacarte>"#)).unwrap();
    assert_eq!(frb_load_config(path.to_string_lossy().into_owned()).accounts.len(), accounts.len());
}

fn get(id: &str) -> Result<Fetched, Refused> {
    let (tx, rx) = channel();
    avatar::fetch(id, Box::new(move |r| tx.send(r).unwrap()));
    rx.recv_timeout(Duration::from_secs(30)).expect("answered within 30 s")
}

// One test: `frb_load_config` installs process-wide state.
#[test]
fn an_accounts_picture_comes_from_its_saved_file_or_address() {
    allow_private_addresses(true);
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("me.png");
    std::fs::write(&png, PNG).unwrap();
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, "not a picture").unwrap();
    let big = dir.path().join("big.png");
    let mut b = PNG.to_vec();
    b.resize(3 * 1024 * 1024, 0);
    std::fs::write(&big, b).unwrap();
    let port = serve_png();

    config(
        dir.path(),
        &[
            ("file", Some(png.to_string_lossy().into_owned())),
            ("url", Some(format!("file://{}", png.display()))),
            ("web", Some(format!("http://127.0.0.1:{port}/me.png"))),
            ("text", Some(text.to_string_lossy().into_owned())),
            ("big", Some(big.to_string_lossy().into_owned())),
            ("gone", Some(dir.path().join("missing.png").to_string_lossy().into_owned())),
            ("none", None),
        ],
    );

    for id in ["file", "url", "web"] {
        let f = get(id).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        assert_eq!((f.content_type.as_str(), f.body.as_slice()), ("image/png", PNG), "{id}");
    }
    assert_eq!(get("text").unwrap_err().status, 403, "a file that is not an image");
    assert_eq!(get("big").unwrap_err().status, 413, "too large for a picture");
    assert_eq!(get("gone").unwrap_err().status, 404, "a file that is not there");
    assert_eq!(get("none").unwrap_err().status, 404, "no picture set");
    assert_eq!(get("nobody").unwrap_err().status, 404, "no such account");
}
