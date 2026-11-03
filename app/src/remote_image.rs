/*
 * remote_image.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Fetching an image that a message's HTML points at, for a UI that has
 * decided to show it. The UI never fetches from the internet itself; this is
 * the only place that does, so the rules live here:
 *
 * - only http and https;
 * - only addresses on the public internet, judged on the address the dial
 *   will use (see `HttpClient::connect_public`), at every redirect;
 * - only what answers with an image media type, and no more than `MAX_BYTES` of it;
 * - no cookies, no referrer, nothing that says who the message was sent to.
 *
 * A response is handed over whole, because the custom protocol of the window
 * answers a request with one body; that is what the size cap is for.
 */

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tagliacarte_core::protocol::http::fetch::HttpUrl;
use tagliacarte_core::protocol::http::{HttpClient, HttpConnection, Method, Response, ResponseHandler};

/// The most of one image that is accepted.
pub const MAX_BYTES: usize = 10 * 1024 * 1024;

/// How many redirects are followed before giving up.
const MAX_REDIRECTS: u8 = 4;

static ALLOW_PRIVATE: AtomicBool = AtomicBool::new(false);

/// Lets the fetcher reach loopback and private addresses. For tests and the
/// GUI's self-test only; nothing a message says can turn it on.
pub fn allow_private_addresses(yes: bool) {
    ALLOW_PRIVATE.store(yes, Ordering::Relaxed);
}

/// An image, as the remote server sent it.
#[derive(Debug)]
pub struct Fetched {
    /// `image/png` and the like, without parameters.
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Why nothing is shown: an HTTP status for the response that says so, and
/// words for a log or a tooltip.
#[derive(Debug)]
pub struct Refused {
    pub status: u16,
    pub message: String,
}

impl Refused {
    fn new(status: u16, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }
}

type Done = Box<dyn FnOnce(Result<Fetched, Refused>) + Send>;

/// Fetches `url` and reports once, from a reactor thread. Returns at once.
pub fn fetch(url: &str, done: Done) {
    get(url.trim().split('#').next().unwrap_or("").to_owned(), MAX_REDIRECTS, done);
}

fn get(url: String, redirects_left: u8, done: Done) {
    let parsed = match HttpUrl::parse(&url) {
        Ok(u) => u,
        Err(_) => return done(Err(Refused::new(403, "only web addresses are fetched"))),
    };
    let done = Arc::new(Mutex::new(Some(done)));
    let finish = {
        let done = Arc::clone(&done);
        move |r: Result<Fetched, Refused>| {
            let k = done.lock().unwrap().take();
            if let Some(k) = k {
                k(r);
            }
        }
    };
    let target = parsed.target.clone();
    let base = parsed.clone();
    let on_connect: tagliacarte_core::protocol::http::OnConnect = {
        let finish = finish.clone();
        let done = Arc::clone(&done);
        Box::new(move |conn| {
            let conn = match conn {
                Ok(c) => c,
                Err(e) => return finish(Err(from_io(&e))),
            };
            let mut req = conn.request(Method::Get, target);
            req.header("Accept", "image/*,*/*;q=0.1");
            req.header("User-Agent", "Tagliacarte");
            conn.send(
                req,
                Image {
                    base,
                    redirects_left,
                    status: 0,
                    content_type: String::new(),
                    location: None,
                    declared: None,
                    body: Vec::new(),
                    overflow: false,
                    conn: Some(conn.clone()),
                    done,
                },
            );
        })
    };
    HttpClient::connect_public(&parsed.host, parsed.port, parsed.tls, ALLOW_PRIVATE.load(Ordering::Relaxed), on_connect);
}

fn from_io(e: &io::Error) -> Refused {
    if e.kind() == io::ErrorKind::PermissionDenied {
        Refused::new(403, e.to_string())
    } else {
        Refused::new(502, e.to_string())
    }
}

/// One response, collected up to the cap.
struct Image {
    base: HttpUrl,
    redirects_left: u8,
    status: u16,
    content_type: String,
    location: Option<String>,
    declared: Option<usize>,
    body: Vec<u8>,
    overflow: bool,
    /// Held so that it can be dropped, which closes the connection, as soon as
    /// the rest of the body is not wanted.
    conn: Option<HttpConnection>,
    done: Arc<Mutex<Option<Done>>>,
}

impl Image {
    fn finish(&mut self, r: Result<Fetched, Refused>) {
        self.conn = None;
        let k = self.done.lock().unwrap().take();
        if let Some(k) = k {
            k(r);
        }
    }

    fn too_large(&mut self) {
        self.body = Vec::new();
        self.finish(Err(Refused::new(413, format!("the image is larger than {} MB", MAX_BYTES / (1024 * 1024)))));
    }
}

impl ResponseHandler for Image {
    fn ok(&mut self, r: Response) {
        self.status = r.code;
    }

    fn error(&mut self, r: Response) {
        self.status = r.code;
    }

    fn header(&mut self, name: &str, value: &str) {
        if name.eq_ignore_ascii_case("content-type") {
            self.content_type = media_type(value);
        } else if name.eq_ignore_ascii_case("location") {
            self.location = Some(value.trim().to_owned());
        } else if name.eq_ignore_ascii_case("content-length") {
            self.declared = value.trim().parse().ok();
        }
    }

    fn start_body(&mut self) {
        if self.declared.is_some_and(|n| n > MAX_BYTES) && (200..300).contains(&self.status) {
            self.overflow = true;
            self.too_large();
        }
    }

    fn body_chunk(&mut self, data: &[u8]) {
        if self.overflow || !(200..300).contains(&self.status) {
            return;
        }
        if self.body.len() + data.len() > MAX_BYTES {
            self.overflow = true;
            return self.too_large();
        }
        self.body.extend_from_slice(data);
    }

    fn end_body(&mut self) {}

    fn complete(&mut self) {
        if self.overflow {
            return;
        }
        let status = self.status;
        if (300..400).contains(&status) {
            let Some(location) = self.location.take() else {
                return self.finish(Err(Refused::new(502, format!("the server answered {status} with no address"))));
            };
            if self.redirects_left == 0 {
                return self.finish(Err(Refused::new(502, "too many redirects")));
            }
            let next = resolve(&self.base, &location);
            let (left, done) = (self.redirects_left - 1, Arc::clone(&self.done));
            self.conn = None;
            // The next hop answers to the same callback as the first.
            let k = done.lock().unwrap().take();
            if let Some(k) = k {
                get(next, left, k);
            }
            return;
        }
        if !(200..300).contains(&status) {
            return self.finish(Err(Refused::new(502, format!("the server answered {status}"))));
        }
        let declared = std::mem::take(&mut self.content_type);
        let content_type = if declared.starts_with("image/") {
            declared
        } else if is_generic(&declared) {
            // A browser would look at the bytes, and so do we, for the few
            // formats it knows; what they say is what is served.
            match sniff(&self.body) {
                Some(t) => t.to_owned(),
                None => return self.finish(Err(Refused::new(403, "not an image"))),
            }
        } else {
            return self.finish(Err(Refused::new(403, "not an image")));
        };
        let fetched = Fetched { content_type, body: std::mem::take(&mut self.body) };
        self.finish(Ok(fetched));
    }

    fn failed(&mut self, e: &io::Error) {
        if !self.overflow {
            self.finish(Err(from_io(e)));
        }
    }
}

/// A type that says nothing about what the bytes are: servers that call every
/// file a download, or send no type at all.
fn is_generic(media_type: &str) -> bool {
    matches!(media_type, "" | "application/octet-stream" | "binary/octet-stream" | "application/binary")
}

/// The image type the first bytes announce, for the formats mail uses.
pub(crate) fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if b.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.len() >= 12 && &b[4..8] == b"ftyp" && (&b[8..12] == b"avif" || &b[8..12] == b"avis") {
        Some("image/avif")
    } else if b.starts_with(b"\x00\x00\x01\x00") {
        Some("image/x-icon")
    } else if b.starts_with(b"BM") && b.len() > 14 {
        Some("image/bmp")
    } else {
        None
    }
}

/// `Image/PNG; charset=binary` as `image/png`.
fn media_type(value: &str) -> String {
    value.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

/// An absolute URL for the `Location` of a redirect from `base`.
fn resolve(base: &HttpUrl, location: &str) -> String {
    let scheme = if base.tls { "https" } else { "http" };
    let default_port = if base.tls { 443 } else { 80 };
    let host = if base.host.contains(':') { format!("[{}]", base.host) } else { base.host.clone() };
    let origin = if base.port == default_port { format!("{scheme}://{host}") } else { format!("{scheme}://{host}:{}", base.port) };
    let lower = location.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        location.to_owned()
    } else if let Some(rest) = location.strip_prefix("//") {
        format!("{scheme}://{rest}")
    } else if location.starts_with('/') {
        format!("{origin}{location}")
    } else {
        let path = base.target.split(['?', '#']).next().unwrap_or("/");
        let dir = &path[..path.rfind('/').map_or(0, |i| i + 1)];
        format!("{origin}{dir}{location}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(url: &str) -> HttpUrl {
        HttpUrl::parse(url).unwrap()
    }

    #[test]
    fn a_redirect_target_is_made_absolute_from_where_it_came() {
        let base = at("https://img.example.com/a/b/c.png?x=1");
        assert_eq!(resolve(&base, "https://other.example/z.png"), "https://other.example/z.png");
        assert_eq!(resolve(&base, "//cdn.example/z.png"), "https://cdn.example/z.png");
        assert_eq!(resolve(&base, "/z.png"), "https://img.example.com/z.png");
        assert_eq!(resolve(&base, "z.png"), "https://img.example.com/a/b/z.png");
        let odd = at("http://127.0.0.1:8080/c.png");
        assert_eq!(resolve(&odd, "/z.png"), "http://127.0.0.1:8080/z.png");
        assert_eq!(resolve(&odd, "y.png"), "http://127.0.0.1:8080/y.png");
        assert_eq!(resolve(&at("http://[2001:db8::1]:81/q"), "/z"), "http://[2001:db8::1]:81/z");
    }

    #[test]
    fn only_generic_types_are_settled_by_the_bytes() {
        for t in ["", "application/octet-stream", "binary/octet-stream"] {
            assert!(is_generic(t), "{t:?}");
        }
        for t in ["text/html", "image/png", "application/pdf", "text/plain"] {
            assert!(!is_generic(t), "{t:?}");
        }
        assert_eq!(sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"\x00\x00\x00\x1cftypavif"), Some("image/avif"));
        assert_eq!(sniff(b"\x00\x00\x01\x00\x01\x00"), Some("image/x-icon"));
        assert_eq!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"), None);
        assert_eq!(sniff(b"<html>"), None);
        assert_eq!(sniff(b""), None);
        assert_eq!(sniff(b"BM"), None, "too short to be a bitmap");
    }

    #[test]
    fn a_content_type_is_read_without_its_parameters_or_case() {
        assert_eq!(media_type("Image/PNG; charset=binary"), "image/png");
        assert_eq!(media_type(" text/html "), "text/html");
        assert_eq!(media_type(""), "");
    }
}
