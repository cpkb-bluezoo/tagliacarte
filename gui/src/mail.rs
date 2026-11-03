//! Message content over the `app` protocol:
//!
//! - `/mail/<account>/<folder>/<message>/body`: the message as an HTML
//!   document, to be shown in a sandboxed frame;
//! - `/mail/<account>/<folder>/<message>/cid/<content-id>`: an inline part
//!   the body refers to as `cid:`;
//! - `/mail/<account>/<folder>/<message>/body?images=1`: the same document
//!   with its remote images pointed at the proxy below, once the user has
//!   asked for them;
//! - `/remote/<key>/<address>`: a remote image, fetched by this process. The
//!   address is base64url, and `key` is a secret chosen at launch that only the
//!   rewritten body knows, so a message cannot make the app fetch for it by
//!   naming a proxy URL itself.
//!
//! Each path segment is percent-encoded (an IMAP folder name may hold `/`),
//! so the path is split before it is decoded.

use std::sync::{Arc, OnceLock};

use percent_encoding::percent_decode_str;
use tagliacarte_app::mail_view::{self, MessageRef, ViewResponse};
use tagliacarte_app::avatar;
use tagliacarte_app::remote_image;
use tagliacarte_app::remote_rewrite::proxied_url;
use wry::RequestAsyncResponder;
use wry::http::{Response, header};

use crate::BASE;
use crate::events;
use crate::hub::Hub;

/// A message body runs no script and reaches nothing but images the back
/// end serves. `sandbox` makes its origin opaque even if it were ever
/// loaded outside the sandboxed frame; `allow-popups` lets `target=_blank`
/// links reach the shell, which hands them to the system browser.
fn body_csp() -> String {
    format!(
        "default-src 'none'; img-src {BASE}; style-src 'unsafe-inline'; base-uri 'none'; \
         form-action 'none'; sandbox allow-popups"
    )
}

/// An inline part is never a document.
const PART_CSP: &str = "default-src 'none'; sandbox";

/// `cacheable`: a part (an inline image) never changes for a given message, so a short
/// private lifetime is claimed for it. A body is not kept: loading it is what tells the
/// page how many remote images it holds, so it must reach this process each time.
fn finish(responder: RequestAsyncResponder, r: ViewResponse, csp: String, cacheable: bool) {
    let cache = if r.status == 200 && cacheable { "private, max-age=300" } else { "no-store" };
    let resp = Response::builder()
        .status(r.status)
        .header(header::CONTENT_TYPE, r.content_type)
        .header(header::CONTENT_SECURITY_POLICY, csp)
        .header(header::CACHE_CONTROL, cache)
        .header("X-Content-Type-Options", "nosniff")
        .body(r.body)
        .expect("response is valid");
    responder.respond(resp);
}

fn not_found(responder: RequestAsyncResponder) {
    responder.respond(Response::builder().status(404).body(Vec::new()).expect("static response"));
}

/// The secret in the proxy URLs of this launch.
fn remote_key() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(crate::make_token)
}

/// Where the proxy lives, for the body to point at.
fn proxy_prefix() -> String {
    format!("/remote/{}/", remote_key())
}

fn wants_images(query: Option<&str>) -> bool {
    query.is_some_and(|q| q.split('&').any(|p| p == "images=1"))
}

/// Compared without stopping at the first difference.
fn same_key(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The address a proxy request names, or the status to refuse it with.
fn remote_target(path: &str, key: &str) -> Result<String, u16> {
    let rest = path.strip_prefix("/remote/").ok_or(404u16)?;
    let (given, encoded) = rest.split_once('/').ok_or(404u16)?;
    if !same_key(given, key) {
        return Err(403);
    }
    proxied_url(encoded).ok_or(400)
}

fn decode(seg: &str) -> String {
    percent_decode_str(seg).decode_utf8_lossy().into_owned()
}

/// Serves a `/mail/...` path. Returns at once; the response is sent when the
/// store or IMAP session has produced it.
pub fn serve(path: &str, query: Option<&str>, hub: &Arc<Hub>, responder: RequestAsyncResponder) {
    let segs: Vec<&str> = path.trim_start_matches("/mail/").split('/').collect();
    match segs.as_slice() {
        [account, folder, message, "body"] => {
            let msg = MessageRef {
                account_id: decode(account),
                folder: decode(folder),
                message_id: decode(message),
            };
            let images = wants_images(query).then(proxy_prefix);
            let hub = Arc::clone(hub);
            let (account, folder, message) = (msg.account_id.clone(), msg.folder.clone(), msg.message_id.clone());
            mail_view::serve_body(
                msg,
                images,
                Box::new(move |r| {
                    if r.status == 200 {
                        hub.push(events::remote_content_json(&account, &folder, &message, r.remote_images));
                    }
                    finish(responder, r, body_csp(), false);
                }),
            );
        }
        [account, folder, message, "cid", cid] => {
            let msg = MessageRef {
                account_id: decode(account),
                folder: decode(folder),
                message_id: decode(message),
            };
            mail_view::serve_cid(
                msg,
                decode(cid),
                Box::new(move |r| finish(responder, r, PART_CSP.to_owned(), true)),
            );
        }
        _ => not_found(responder),
    }
}

/// Serves `/avatar/<account>`: the account's picture, from its saved setting. The
/// page says which account and nothing about where the picture is, and the
/// address of the picture is never the page's to choose.
pub fn serve_avatar(path: &str, responder: RequestAsyncResponder) {
    let id = decode(path.trim_start_matches("/avatar/"));
    let named = id.clone();
    avatar::fetch(
        &id,
        Box::new(move |r| {
            // Having no picture is ordinary (the page falls back to the disc); one
            // that is set and could not be had is worth a line.
            if let Some(e) = r.as_ref().err().filter(|e| e.message != avatar::NO_PICTURE) {
                eprintln!("tagliacarte: no picture for account '{named}' ({}): {}", e.status, e.message);
            }
            responder.respond(image_response(r));
        }),
    );
}

/// Serves a `/remote/...` path: fetches the image it names, if the key is right.
pub fn serve_remote(path: &str, responder: RequestAsyncResponder) {
    let url = match remote_target(path, remote_key()) {
        Ok(u) => u,
        Err(code) => return responder.respond(Response::builder().status(code).body(Vec::new()).expect("static response")),
    };
    let shown: String = url.chars().take(200).collect();
    remote_image::fetch(
        &url,
        Box::new(move |r| {
            if let Err(e) = &r {
                // The page only sees a broken image; this says why.
                eprintln!("tagliacarte: remote image refused ({}): {} [{shown}]", e.status, e.message);
            }
            responder.respond(image_response(r));
        }),
    );
}

/// The answer to a proxy request, from what the fetch came to.
fn image_response(r: Result<remote_image::Fetched, remote_image::Refused>) -> Response<Vec<u8>> {
    let resp = match r {
        // Nothing in the page changes what an address answers with, so it may
        // be kept for a while; it is the page's own copy, not shared.
        Ok(img) => Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, img.content_type)
            .header(header::CONTENT_SECURITY_POLICY, PART_CSP)
            .header(header::CACHE_CONTROL, "private, max-age=3600")
            .header("X-Content-Type-Options", "nosniff")
            .body(img.body),
        Err(e) => Response::builder()
            .status(e.status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-store")
            .body(e.message.into_bytes()),
    };
    resp.expect("response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tagliacarte_app::remote_rewrite::proxy_url;

    #[test]
    fn only_the_images_option_turns_images_on() {
        assert!(wants_images(Some("images=1")));
        assert!(wants_images(Some("a=b&images=1")));
        assert!(!wants_images(None));
        assert!(!wants_images(Some("")));
        assert!(!wants_images(Some("images=0")));
        assert!(!wants_images(Some("images=11")));
        assert!(!wants_images(Some("xmages=1")));
    }

    #[test]
    fn the_proxy_wants_the_launch_key_and_a_well_formed_address() {
        let ok = proxy_url("/remote/secret/", "https://img.example/a.png?x=1&y=2");
        assert_eq!(remote_target(&ok, "secret").as_deref(), Ok("https://img.example/a.png?x=1&y=2"));
        assert_eq!(remote_target(&ok, "other"), Err(403));
        assert_eq!(remote_target("/remote/secret/!!!", "secret"), Err(400));
        assert_eq!(remote_target("/remote/secret/", "secret"), Err(400));
        assert_eq!(remote_target("/remote/secret", "secret"), Err(404));
        assert_eq!(remote_target("/remote/", "secret"), Err(404));
        assert_eq!(remote_target("/mail/x", "secret"), Err(404));
        // A longer or shorter key is no key.
        assert_eq!(remote_target(&proxy_url("/remote/secret2/", "https://a/"), "secret"), Err(403));
        assert_eq!(remote_target(&proxy_url("/remote/secre/", "https://a/"), "secret"), Err(403));
    }

    #[test]
    fn the_launch_key_is_long_and_stays_the_same() {
        assert_eq!(remote_key().len(), 64);
        assert_eq!(remote_key(), remote_key());
        assert!(proxy_prefix().starts_with("/remote/") && proxy_prefix().ends_with('/'));
    }

    #[test]
    fn a_fetched_image_is_served_locked_down_and_a_refusal_says_why() {
        let ok = image_response(Ok(remote_image::Fetched { content_type: "image/png".into(), body: vec![1, 2, 3] }));
        assert_eq!(ok.status(), 200);
        assert_eq!(ok.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(ok.headers()["X-Content-Type-Options"], "nosniff");
        assert_eq!(ok.headers()[header::CONTENT_SECURITY_POLICY], PART_CSP);
        assert!(ok.headers()[header::CACHE_CONTROL].to_str().unwrap().starts_with("private"));
        assert_eq!(ok.body(), &vec![1, 2, 3]);

        let no = image_response(Err(remote_image::Refused { status: 403, message: "not an image".into() }));
        assert_eq!(no.status(), 403);
        assert_eq!(no.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(no.body(), b"not an image");
    }
}
