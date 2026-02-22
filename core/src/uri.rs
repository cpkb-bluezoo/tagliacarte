/*
 * uri.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Store, folder, and transport URL/URI schemes. URLs for maildir, mbox, imap, imaps, smtp, smtps
//! (authority + path); URIs for protocols without authority (e.g. nostr:). Paths use three slashes
//! (e.g. maildir:///absolute/path). Folder names are percent-encoded in path segments.

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

/// Path segment safe set: encode everything except unreserved and sub-delims used in paths.
/// So we encode / ? # [ ] @ and space, %, etc.
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b'/')
    .add(b'?')
    .add(b'#')
    .add(b'[')
    .add(b']')
    .add(b'@')
    .add(b'%')
    .add(b' ');

/// Userinfo in authority: encode @ and other reserved so one @ separates userinfo from host.
const USERINFO: &AsciiSet = &CONTROLS.add(b'@').add(b':').add(b'%').add(b'/').add(b'?').add(b'#').add(b'[').add(b']');

/// Normalize path for URL: ensure single leading slash (so scheme:///path).
fn path_with_leading_slash(path: &str) -> String {
    let path = path.trim_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path)
    }
}

/// Maildir store URL: maildir:///absolute/path (three slashes).
pub fn maildir_store_uri(path: &str) -> String {
    format!("maildir://{}", path_with_leading_slash(path))
}

/// Mbox store URL: mbox:///path (three slashes).
pub fn mbox_store_uri(path: &str) -> String {
    format!("mbox://{}", path_with_leading_slash(path))
}

/// IMAP store URL: imap://user@host:port or imaps://user@host:port (imaps for implicit TLS, e.g. port 993).
pub fn imap_store_uri(user_at_host: &str, host: &str, port: u16) -> String {
    let userinfo = utf8_percent_encode(user_at_host, USERINFO).to_string();
    let scheme = if port == 993 {
        "imaps"
    } else {
        "imap"
    };
    format!("{}://{}@{}:{}", scheme, userinfo, host, port)
}

/// POP3 store URL: pop3://user@host:port or pop3s://user@host:port (pop3s for implicit TLS, e.g. port 995).
pub fn pop3_store_uri(user_at_host: &str, host: &str, port: u16) -> String {
    let userinfo = utf8_percent_encode(user_at_host, USERINFO).to_string();
    let scheme = if port == 995 {
        "pop3s"
    } else {
        "pop3"
    };
    format!("{}://{}@{}:{}", scheme, userinfo, host, port)
}

/// SMTP transport URL: smtp://host:port or smtps://host:port (smtps for implicit TLS, e.g. port 465).
pub fn smtp_transport_uri(host: &str, port: u16) -> String {
    let scheme = if port == 465 {
        "smtps"
    } else {
        "smtp"
    };
    format!("{}://{}:{}", scheme, host, port)
}

/// SMTP transport URL with user identity: smtp://user@host:port or smtps://user@host:port.
pub fn smtp_transport_uri_with_user(user_at_host: &str, host: &str, port: u16) -> String {
    let userinfo = utf8_percent_encode(user_at_host, USERINFO).to_string();
    let scheme = if port == 465 {
        "smtps"
    } else {
        "smtp"
    };
    format!("{}://{}@{}:{}", scheme, userinfo, host, port)
}

/// Nostr store URI (identity-based; no host in URI per ARCHITECTURE). Format: nostr:store:<id>.
pub fn nostr_store_uri(id: &str) -> String {
    format!("nostr:store:{}", id)
}

/// Nostr transport URI (same identity as store). Format: nostr:transport:<id>.
pub fn nostr_transport_uri(id: &str) -> String {
    format!("nostr:transport:{}", id)
}

/// Matrix store URI (homeserver + user id). Format: matrix:store:<homeserver>:<user_id_or_localpart>.
pub fn matrix_store_uri(homeserver: &str, user_id_or_localpart: &str) -> String {
    format!("matrix:store:{}:{}", homeserver, user_id_or_localpart)
}

/// Matrix transport URI (same account as store). Format: matrix:transport:<homeserver>:<user_id_or_localpart>.
pub fn matrix_transport_uri(homeserver: &str, user_id_or_localpart: &str) -> String {
    format!("matrix:transport:{}:{}", homeserver, user_id_or_localpart)
}

/// NNTP store URL: nntp://user@host:port or nntps://user@host:port (nntps for implicit TLS, e.g. port 563).
pub fn nntp_store_uri(user_at_host: &str, host: &str, port: u16) -> String {
    let userinfo = utf8_percent_encode(user_at_host, USERINFO).to_string();
    let scheme = if port == 563 {
        "nntps"
    } else {
        "nntp"
    };
    format!("{}://{}@{}:{}", scheme, userinfo, host, port)
}

/// NNTP transport URL (POST via same server): nntp+post://user@host:port.
pub fn nntp_transport_uri(user_at_host: &str, host: &str, port: u16) -> String {
    let userinfo = utf8_percent_encode(user_at_host, USERINFO).to_string();
    format!("nntp+post://{}@{}:{}", userinfo, host, port)
}

/// Percent-encode a folder name for use as a path segment (encodes /, non-ASCII, etc.).
pub fn encode_folder_name(folder_name: &str) -> String {
    utf8_percent_encode(folder_name, PATH_SEGMENT).to_string()
}

/// Folder URL: store_uri + "/" + encoded folder name. Store URI must not end with a slash.
pub fn folder_uri(store_uri: &str, folder_name: &str) -> String {
    let encoded = encode_folder_name(folder_name);
    format!("{}/{}", store_uri.trim_end_matches('/'), encoded)
}

/// Gmail IMAP store URI (uses XOAUTH2). Format: gmail://email@host
pub fn gmail_store_uri(email: &str) -> String {
    let userinfo = utf8_percent_encode(email, USERINFO).to_string();
    format!("gmail://{}", userinfo)
}

/// Gmail SMTP transport URI (uses XOAUTH2). Format: gmail+smtp://email@host
pub fn gmail_smtp_transport_uri(email: &str) -> String {
    let userinfo = utf8_percent_encode(email, USERINFO).to_string();
    format!("gmail+smtp://{}", userinfo)
}

/// Microsoft Graph store URI. Format: graph://email@host
pub fn graph_store_uri(email: &str) -> String {
    let userinfo = utf8_percent_encode(email, USERINFO).to_string();
    format!("graph://{}", userinfo)
}

/// Microsoft Graph transport URI. Format: graph+send://email@host
pub fn graph_transport_uri(email: &str) -> String {
    let userinfo = utf8_percent_encode(email, USERINFO).to_string();
    format!("graph+send://{}", userinfo)
}

/// Decode a percent-encoded path segment back to folder name.
pub fn decode_folder_name(encoded: &str) -> String {
    percent_encoding::percent_decode_str(encoded).decode_utf8_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maildir_three_slashes() {
        let u = maildir_store_uri("/var/mail/me");
        assert_eq!(u, "maildir:///var/mail/me");
    }

    #[test]
    fn imap_imaps_scheme() {
        let u = imap_store_uri("user", "host", 993);
        assert_eq!(u, "imaps://user@host:993");
        let u2 = imap_store_uri("user", "host", 143);
        assert_eq!(u2, "imap://user@host:143");
    }

    #[test]
    fn folder_uri_encodes_slash() {
        let store = "maildir:///var/mail";
        let u = folder_uri(store, "INBOX/Work");
        assert!(u.contains("%2F") || u.contains("%2f")); // encoded /
        assert!(u.starts_with("maildir:///var/mail/"));
    }

    #[test]
    fn decode_folder_name_roundtrip() {
        let name = "INBOX/Work";
        let enc = encode_folder_name(name);
        let dec = decode_folder_name(&enc);
        assert_eq!(dec, name);
    }
}
