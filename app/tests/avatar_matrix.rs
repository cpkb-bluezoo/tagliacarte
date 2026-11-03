//! A Matrix account's picture: its profile names an `mxc://` address, which the
//! account's own homeserver serves for a signed-in user. Against a stand-in
//! homeserver that records what it is asked, as a current one (authenticated
//! media), an older one (only the earlier media paths) and one with no picture.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tagliacarte_app::avatar;
use tagliacarte_app::frb_api::{frb_load_config, frb_save_store_credential};
use tagliacarte_app::remote_image::{Fetched, Refused};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-matrix picture-";

#[derive(Clone, Copy)]
struct Server {
    /// Answers the authenticated client media paths.
    modern: bool,
    /// Answers the earlier media paths.
    legacy: bool,
    /// The profile names a picture.
    has_picture: bool,
}

/// A homeserver on loopback; the requests it gets are kept as `"METHOD path auth"`.
fn serve(cfg: Server) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let log: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = Arc::clone(&log);
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let seen = Arc::clone(&seen);
            std::thread::spawn(move || {
                let mut pending = Vec::new();
                let mut buf = [0u8; 2048];
                loop {
                    // One request at a time, on a connection that stays open.
                    while !pending.windows(4).any(|w| w == b"\r\n\r\n") {
                        match s.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => pending.extend_from_slice(&buf[..n]),
                        }
                    }
                    let end = pending.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                    let head = String::from_utf8_lossy(&pending[..end]).into_owned();
                    pending.drain(..end);
                    // A POST carries a body; read it so the next request starts cleanly.
                    let wanted: usize = head
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse().ok()))
                        .unwrap_or(0);
                    while pending.len() < wanted {
                        match s.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => pending.extend_from_slice(&buf[..n]),
                        }
                    }
                    pending.drain(..wanted);
                    let mut lines = head.lines();
                    let mut first = lines.next().unwrap_or("").split_whitespace();
                    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or("").to_owned());
                    let auth = head
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("authorization:").map(|v| v.trim().to_owned()))
                        .unwrap_or_default();
                    seen.lock().unwrap().push(format!("{method} {path} {auth}"));

                    let json = |status: &str, body: &str| (status.to_owned(), "application/json".to_owned(), body.as_bytes().to_vec());
                    let (status, ctype, body) = if path == "/_matrix/client/v3/login" {
                        // Signing in with the saved password gives the access token the media wants.
                        json("200 OK", r#"{"user_id":"@u:test","access_token":"tok","device_id":"DEV1"}"#)
                    } else if path.starts_with("/_matrix/client/v3/profile/") {
                        if cfg.has_picture {
                            json("200 OK", r#"{"displayname":"U","avatar_url":"mxc://test/abc123"}"#)
                        } else {
                            json("200 OK", r#"{"displayname":"U"}"#)
                        }
                    } else if path.starts_with("/_matrix/client/v1/media/") {
                        if !cfg.modern {
                            json("404 Not Found", r#"{"errcode":"M_UNRECOGNIZED"}"#)
                        } else if auth != "bearer tok" {
                            json("401 Unauthorized", r#"{"errcode":"M_MISSING_TOKEN"}"#)
                        } else {
                            ("200 OK".to_owned(), "image/png".to_owned(), PNG.to_vec())
                        }
                    } else if path.starts_with("/_matrix/media/v3/") {
                        if cfg.legacy {
                            ("200 OK".to_owned(), "image/png".to_owned(), PNG.to_vec())
                        } else {
                            json("404 Not Found", r#"{"errcode":"M_UNRECOGNIZED"}"#)
                        }
                    } else {
                        json("404 Not Found", "{}")
                    };
                    let reply = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\r\n", body.len());
                    if s.write_all(reply.as_bytes()).and_then(|()| s.write_all(&body)).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, log)
}

fn config(root: &Path, accounts: &[(&str, u16, Option<&str>)]) {
    let mut stores = String::new();
    for (id, port, avatar) in accounts {
        let attr = avatar.map(|a| format!(r#" avatar-url="{a}""#)).unwrap_or_default();
        stores.push_str(&format!(
            r#"<store id="{id}" type="matrix" display-name="{id}" host="http://127.0.0.1:{port}" username="@u:test"{attr}/>"#
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

/// What the homeserver was asked, apart from signing in.
fn requests(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    log.lock().unwrap().iter().filter(|l| !l.contains("/_matrix/client/v3/login")).cloned().collect()
}

// One test: `frb_load_config` installs process-wide state.
#[test]
fn a_matrix_accounts_picture_comes_from_its_profile_through_its_homeserver() {
    let (modern, modern_log) = serve(Server { modern: true, legacy: false, has_picture: true });
    let (older, older_log) = serve(Server { modern: false, legacy: true, has_picture: true });
    let (bare, bare_log) = serve(Server { modern: true, legacy: true, has_picture: false });
    let (named, named_log) = serve(Server { modern: true, legacy: true, has_picture: true });
    let (hostile, hostile_log) = serve(Server { modern: true, legacy: true, has_picture: true });
    let (anon, anon_log) = serve(Server { modern: true, legacy: true, has_picture: true });
    let dir = tempfile::tempdir().unwrap();
    config(
        dir.path(),
        &[
            ("current", modern, None),
            ("older", older, None),
            ("bare", bare, None),
            // A picture chosen in the account's form wins over the profile.
            ("named", named, Some("mxc://test/chosen")),
            ("hostile", hostile, Some("mxc://test/../../_synapse/admin")),
            ("anon", anon, None),
        ],
    );
    for id in ["current", "older", "bare", "named", "hostile"] {
        frb_save_store_credential(id.into(), "@u:test".into(), "tok".into()).expect("credential saved");
    }

    // A current homeserver: the profile, then the authenticated thumbnail with the token.
    let f = get("current").expect("current homeserver");
    assert_eq!((f.content_type.as_str(), f.body.as_slice()), ("image/png", PNG));
    let r = requests(&modern_log);
    assert!(r[0].starts_with("GET /_matrix/client/v3/profile/") && r[0].ends_with("bearer tok"), "{r:?}");
    assert_eq!(r[1], "GET /_matrix/client/v1/media/thumbnail/test/abc123?width=96&height=96&method=crop bearer tok", "{r:?}");
    assert_eq!(r.len(), 2, "nothing more was asked for: {r:?}");

    // An older one does not know the new paths: the old ones are tried next.
    let f = get("older").expect("older homeserver");
    assert_eq!(f.body, PNG);
    let r = requests(&older_log);
    assert!(r[1].contains("/_matrix/client/v1/media/thumbnail/") && r[2].contains("/_matrix/media/v3/thumbnail/"), "{r:?}");

    // A profile without a picture is no picture, and no media is asked for.
    assert_eq!(get("bare").unwrap_err().status, 404);
    assert_eq!(requests(&bare_log).len(), 1, "{:?}", requests(&bare_log));

    // The address chosen in the form is used as it is: the profile is not asked.
    let f = get("named").expect("a chosen mxc address");
    assert_eq!(f.body, PNG);
    let r = requests(&named_log);
    assert!(r.iter().all(|l| !l.contains("/profile/")) && r[0].contains("test/chosen"), "{r:?}");

    // An address that would climb out of the media path is refused and goes nowhere.
    assert_eq!(get("hostile").unwrap_err().status, 403);
    assert!(requests(&hostile_log).is_empty(), "{:?}", requests(&hostile_log));

    // Not signed in: there is no token to ask with, so nothing is asked.
    assert!(get("anon").is_err());
    assert!(requests(&anon_log).is_empty(), "{:?}", requests(&anon_log));
}
