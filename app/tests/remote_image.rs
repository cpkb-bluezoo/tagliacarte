//! Fetching a remote image for a message, against an in-process HTTP server.
//! The server is on loopback, which the fetcher refuses unless told otherwise;
//! `remote_image_policy.rs` shows that it does refuse.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::time::Duration;

use tagliacarte_app::remote_image::{allow_private_addresses, fetch, Fetched, Refused};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-pretend image bytes-";

fn respond(mut s: TcpStream, port: u16) {
    let mut req = Vec::new();
    let mut buf = [0u8; 1024];
    while !req.windows(4).any(|w| w == b"\r\n\r\n") {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => req.extend_from_slice(&buf[..n]),
        }
    }
    let text = String::from_utf8_lossy(&req).into_owned();
    let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
    // What a tracker would see: these must never be sent.
    let leaked = text.to_ascii_lowercase().contains("cookie:") || text.to_ascii_lowercase().contains("referer:");
    let head = |status: &str, extra: &str, len: usize| {
        format!("HTTP/1.1 {status}\r\n{extra}Content-Length: {len}\r\nConnection: close\r\n\r\n")
    };
    let (status_line, extra, body): (&str, String, Vec<u8>) = match path.as_str() {
        _ if leaked => ("400 Bad Request", "Content-Type: text/plain\r\n".into(), b"cookie or referer sent".to_vec()),
        // Says who it meant: the name it was given, not the address it dialled.
        "/host.png" if !text.to_ascii_lowercase().contains(&format!("host: localhost:{port}\r\n")) => {
            ("400 Bad Request", "Content-Type: text/plain\r\n".into(), b"wrong host".to_vec())
        }
        "/host.png" => ("200 OK", "Content-Type: image/png\r\n".into(), PNG.to_vec()),
        // Servers that call every file octet-stream, or say nothing: browsers look at the bytes.
        "/generic.png" => ("200 OK", "Content-Type: application/octet-stream\r\n".into(), PNG.to_vec()),
        "/binary.jpg" => ("200 OK", "Content-Type: binary/octet-stream\r\n".into(), b"\xff\xd8\xff\xe0--jpeg--".to_vec()),
        "/none.gif" => ("200 OK", String::new(), b"GIF89a--gif--".to_vec()),
        "/generic.html" => ("200 OK", "Content-Type: application/octet-stream\r\n".into(), b"<html><script>alert(1)</script>".to_vec()),
        "/liar.html" => ("200 OK", "Content-Type: text/html\r\n".into(), PNG.to_vec()),
        "/ok.png" => ("200 OK", "Content-Type: image/png\r\n".into(), PNG.to_vec()),
        "/params.png" => ("200 OK", "Content-Type: Image/PNG; charset=binary\r\n".into(), PNG.to_vec()),
        "/redirect" => ("302 Found", "Location: /ok.png\r\n".into(), Vec::new()),
        "/redirect-abs" => ("301 Moved", format!("Location: http://127.0.0.1:{port}/ok.png\r\n"), Vec::new()),
        "/loop" => ("302 Found", "Location: /loop\r\n".into(), Vec::new()),
        "/page.html" => ("200 OK", "Content-Type: text/html\r\n".into(), b"<script>alert(1)</script>".to_vec()),
        "/big.png" => ("200 OK", "Content-Type: image/png\r\n".into(), vec![0u8; 11 * 1024 * 1024]),
        _ => ("404 Not Found", "Content-Type: text/plain\r\n".into(), b"nope".to_vec()),
    };
    let _ = s.write_all(head(status_line, &extra, body.len()).as_bytes());
    let _ = s.write_all(&body);
}

fn serve() -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let port = addr.port();
            std::thread::spawn(move || respond(s, port));
        }
    });
    addr
}

fn get(url: &str) -> Result<Fetched, Refused> {
    let (tx, rx) = channel();
    fetch(url, Box::new(move |r| tx.send(r).unwrap()));
    rx.recv_timeout(Duration::from_secs(30)).expect("answered within 30 s")
}

#[test]
fn images_are_fetched_followed_and_checked() {
    allow_private_addresses(true);
    let addr = serve();
    let at = |p: &str| format!("http://{addr}{p}");

    let ok = get(&at("/ok.png")).expect("a plain image");
    assert_eq!(ok.content_type, "image/png");
    assert_eq!(ok.body, PNG);

    // The type is judged without its parameters or its case.
    assert_eq!(get(&at("/params.png")).expect("type with parameters").body, PNG);

    // The server is told the name that was asked for, and when the name's first
    // address (`::1`, say) is not where it listens, the next one is tried.
    let by_name = format!("http://localhost:{}/host.png", addr.port());
    assert_eq!(get(&by_name).expect("by name").body, PNG);

    // Redirects are followed, relative or absolute.
    assert_eq!(get(&at("/redirect")).expect("relative redirect").body, PNG);
    assert_eq!(get(&at("/redirect-abs")).expect("absolute redirect").body, PNG);
    let e = get(&at("/loop")).unwrap_err();
    assert!(e.message.contains("redirect"), "{}", e.message);

    // Only images, however the URL is dressed.
    let e = get(&at("/page.html")).unwrap_err();
    assert_eq!(e.status, 403, "{}", e.message);
    assert!(e.message.contains("not an image"), "{}", e.message);

    // A generic or missing type is settled by the bytes, never by the name; what
    // the bytes say is what is served. A page that says it is a page stays one.
    let g = get(&at("/generic.png")).expect("octet-stream PNG");
    assert_eq!((g.content_type.as_str(), g.body.as_slice()), ("image/png", PNG));
    assert_eq!(get(&at("/binary.jpg")).expect("binary/octet-stream JPEG").content_type, "image/jpeg");
    assert_eq!(get(&at("/none.gif")).expect("no type, GIF bytes").content_type, "image/gif");
    assert_eq!(get(&at("/generic.html")).unwrap_err().status, 403, "octet-stream that is not an image");
    assert_eq!(get(&at("/liar.html")).unwrap_err().status, 403, "text/html is never second-guessed");

    // Not too much of it.
    let e = get(&at("/big.png")).unwrap_err();
    assert_eq!(e.status, 413, "{}", e.message);

    // A missing image is the server's answer, passed on as a bad gateway.
    let e = get(&at("/missing.png")).unwrap_err();
    assert_eq!(e.status, 502);
    assert!(e.message.contains("404"), "{}", e.message);

    // Nothing but web addresses.
    for bad in ["file:///etc/passwd", "ftp://example.com/a.png", "data:image/png;base64,AAAA", "javascript:alert(1)"] {
        assert_eq!(get(bad).unwrap_err().status, 403, "{bad}");
    }
}
