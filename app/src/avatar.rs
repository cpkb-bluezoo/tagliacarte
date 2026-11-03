/*
 * avatar.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * An account's picture for the account list, as bytes. The picture is a
 * setting of the account (a web address or a file path, saved in config.xml, or
 * learnt from a Nostr profile); the UI asks for it by account and never says
 * where it is, so it can only be shown what the user put there. A web address
 * goes through the same fetcher as a message's remote images, so the same
 * rules hold: public addresses, images only, a size limit. A file is read here
 * and checked by its first bytes, not its name.
 *
 * A Matrix account's picture is on its homeserver: an `mxc://` address, which
 * is either the one chosen in the account's form or, when none was chosen, the
 * one in the account's own Matrix profile. The homeserver serves it to a
 * signed-in user, so it is asked for with the account's access token, on the
 * current authenticated media paths and then on the earlier ones.
 */

use std::collections::VecDeque;
use std::path::PathBuf;

use tagliacarte_core::config::set_credentials_backend;
use tagliacarte_core::protocol::http::{Method, fetch as http_fetch};
use tagliacarte_core::protocol::matrix::MatrixStore;
use tagliacarte_core::protocol::matrix::connection::MatrixCommand;
use tagliacarte_core::protocol::matrix::types::mxc_picture_urls;

use crate::frb_api::{FrbAccount, resolve_mail_account};
use crate::mail_store::open_cached_store;
use crate::remote_image::{self, Fetched, Refused};

/// The most of a picture read from a file; a profile picture is a few kilobytes.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

type Done = Box<dyn FnOnce(Result<Fetched, Refused>) + Send>;

/// The answer for an account that has no picture (its own, or in its profile): not
/// a failure, so a caller need not report it.
pub const NO_PICTURE: &str = "the account has no picture";

/// The picture of `account_id`, reported once. Returns at once.
pub fn fetch(account_id: &str, done: Done) {
    let (acc, use_keychain) = match resolve_mail_account(account_id) {
        Ok(found) => found,
        Err(e) => return done(Err(Refused { status: 404, message: e })),
    };
    let picture = acc.avatar_url.clone().map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
    let Some(picture) = picture else {
        // None chosen: a Matrix account has one in its profile, others have none.
        if acc.backend_type.trim().eq_ignore_ascii_case("matrix") {
            return matrix_picture(&acc, use_keychain, None, done);
        }
        return done(Err(Refused { status: 404, message: NO_PICTURE.to_owned() }));
    };
    let lower = picture.get(..8).unwrap_or(&picture).to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return remote_image::fetch(&picture, done);
    }
    if lower.starts_with("mxc://") {
        return matrix_picture(&acc, use_keychain, Some(picture), done);
    }
    let path = PathBuf::from(picture.strip_prefix("file://").unwrap_or(&picture));
    // A file is read off the caller's thread, which may be the one drawing the window.
    std::thread::spawn(move || done(read_file(&path)));
}

fn read_file(path: &std::path::Path) -> Result<Fetched, Refused> {
    let meta = std::fs::metadata(path).map_err(|e| Refused { status: 404, message: format!("{}: {e}", path.display()) })?;
    if !meta.is_file() {
        return Err(Refused { status: 404, message: format!("{} is not a file", path.display()) });
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(Refused { status: 413, message: "the picture is larger than 2 MB".to_owned() });
    }
    let body = std::fs::read(path).map_err(|e| Refused { status: 404, message: format!("{}: {e}", path.display()) })?;
    match remote_image::sniff(&body) {
        Some(t) => Ok(Fetched { content_type: t.to_owned(), body }),
        None => Err(Refused { status: 403, message: "not an image".to_owned() }),
    }
}

/// The most of a picture taken from a homeserver; a thumbnail is a few kilobytes.
const MAX_MEDIA_BYTES: usize = 2 * 1024 * 1024;

/// How big a thumbnail to ask for; the rail shows 40 points, which can be 80 pixels.
const THUMBNAIL: u32 = 96;

/// The picture of a Matrix account: `mxc` if one was chosen, else the one in the
/// account's profile, then the media itself through the account's homeserver.
///
/// Opening a Matrix account's store signs in (once; the store is cached and shared
/// with the session) and waits for the homeserver, so this does not run on the
/// caller's thread, which may be the one drawing the window.
fn matrix_picture(acc: &FrbAccount, use_keychain: bool, mxc: Option<String>, done: Done) {
    let acc = acc.clone();
    std::thread::spawn(move || matrix_picture_blocking(&acc, use_keychain, mxc, done));
}

fn matrix_picture_blocking(acc: &FrbAccount, use_keychain: bool, mxc: Option<String>, done: Done) {
    let refuse = |status: u16, message: &str| Refused { status, message: message.to_owned() };
    set_credentials_backend(use_keychain);
    let store = match open_cached_store(acc, use_keychain) {
        Ok(s) => s,
        Err(e) => return done(Err(refuse(404, &format!("the account cannot be opened: {e}")))),
    };
    let Some(mx) = store.as_any().downcast_ref::<MatrixStore>() else {
        return done(Err(refuse(404, "not a Matrix account")));
    };
    let homeserver = mx.homeserver().to_owned();
    // An address that cannot be asked for safely is refused before anything is sent.
    if let Some(m) = &mxc {
        if mxc_picture_urls(&homeserver, m, THUMBNAIL).is_none() {
            return done(Err(refuse(403, "not a usable mxc address")));
        }
    }
    let Some(token) = mx.access_token() else {
        return done(Err(refuse(401, "the account is not signed in")));
    };
    match mxc {
        Some(m) => download(&homeserver, &m, token, done),
        None => {
            let conn = match mx.ensure_connection_pub() {
                Ok(c) => c,
                Err(e) => return done(Err(refuse(502, &e.to_string()))),
            };
            let bearer = token.clone();
            conn.send(MatrixCommand::GetProfile {
                user_id: mx.user_id().to_owned(),
                token,
                on_complete: Box::new(move |r| match r {
                    Err(e) => done(Err(Refused { status: 502, message: e.to_string() })),
                    Ok(profile) => match profile.avatar_url.filter(|u| !u.trim().is_empty()) {
                        None => done(Err(Refused { status: 404, message: NO_PICTURE.to_owned() })),
                        Some(m) => download(&homeserver, m.trim(), bearer, done),
                    },
                }),
            });
        }
    }
}

/// Tries the places the picture may be, in order, until one gives an image.
fn download(homeserver: &str, mxc: &str, token: String, done: Done) {
    match mxc_picture_urls(homeserver, mxc, THUMBNAIL) {
        Some(urls) => try_urls(urls.into(), token, "no answer".to_owned(), done),
        None => done(Err(Refused { status: 403, message: "not a usable mxc address".to_owned() })),
    }
}

fn try_urls(mut urls: VecDeque<String>, token: String, last: String, done: Done) {
    let Some(url) = urls.pop_front() else {
        return done(Err(Refused { status: 404, message: format!("the homeserver has no such picture ({last})") }));
    };
    let bearer = format!("Bearer {token}");
    http_fetch(
        Method::Get,
        &url,
        &[("Authorization", bearer.as_str()), ("Accept", "image/*")],
        None,
        Box::new(move |r| match r {
            Ok(got) if got.is_success() && got.body.len() <= MAX_MEDIA_BYTES => {
                let declared = got.header("content-type").unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
                let kind = if declared.starts_with("image/") { Some(declared) } else { remote_image::sniff(&got.body).map(str::to_owned) };
                match kind {
                    Some(content_type) => done(Ok(Fetched { content_type, body: got.body })),
                    None => try_urls(urls, token, "not an image".to_owned(), done),
                }
            }
            Ok(got) => {
                let why = if got.is_success() { "too large".to_owned() } else { format!("HTTP {}", got.status) };
                try_urls(urls, token, why, done)
            }
            Err(e) => try_urls(urls, token, e.to_string(), done),
        }),
    );
}
