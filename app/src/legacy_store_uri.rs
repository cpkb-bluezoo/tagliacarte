/*
 * legacy_store_uri.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * One-time migration: parse legacy connection URIs into attrs + backend type.
 */

use percent_encoding::percent_decode_str;
use url::Url;

use crate::frb_api::FrbAccount;
use crate::mail_kind::normalize_store_type;

/// Merge a legacy `storeUri` (JSON) or similar into [FrbAccount] attrs.
pub fn merge_legacy_store_uri_into_account(acc: &mut FrbAccount, uri: &str) -> Result<(), String> {
    let u = uri.trim();
    if u.is_empty() {
        return Ok(());
    }
    let parsed = Url::parse(u).map_err(|e| format!("legacy store URI: {e}"))?;
    let scheme = parsed.scheme();
    match scheme {
        "maildir" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "maildir".to_owned();
            }
            let path = parsed.path();
            if !path.is_empty() {
                acc.attrs.insert("path".to_owned(), path.to_owned());
            }
        }
        "mbox" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "mbox".to_owned();
            }
            let path = parsed.path();
            if !path.is_empty() {
                acc.attrs.insert("path".to_owned(), path.to_owned());
            }
        }
        "imap" | "imaps" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "imap".to_owned();
            }
            let host = parsed
                .host_str()
                .ok_or_else(|| "legacy IMAP URI: missing host".to_owned())?
                .to_owned();
            let port = parsed
                .port()
                .unwrap_or(if scheme == "imaps" { 993 } else { 143 });
            let security = if scheme == "imaps" || port == 993 {
                "tls"
            } else {
                "starttls"
            };
            let user = percent_decode_str(parsed.username())
                .decode_utf8_lossy()
                .into_owned();
            if !user.is_empty() {
                acc.attrs.insert("username".to_owned(), user);
            }
            acc.attrs.insert("host".to_owned(), host);
            acc.attrs.insert("port".to_owned(), port.to_string());
            acc.attrs.insert("security".to_owned(), security.to_owned());
        }
        "pop3" | "pop3s" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "pop3".to_owned();
            }
            let host = parsed
                .host_str()
                .ok_or_else(|| "legacy POP3 URI: missing host".to_owned())?
                .to_owned();
            let port = parsed.port().unwrap_or(995);
            let user = percent_decode_str(parsed.username())
                .decode_utf8_lossy()
                .into_owned();
            if !user.is_empty() {
                acc.attrs.insert("username".to_owned(), user);
            }
            acc.attrs.insert("host".to_owned(), host);
            acc.attrs.insert("port".to_owned(), port.to_string());
            acc.attrs.insert("security".to_owned(), "tls".to_owned());
        }
        "nntp" | "nntps" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "nntp".to_owned();
            }
            let host = parsed
                .host_str()
                .ok_or_else(|| "legacy NNTP URI: missing host".to_owned())?
                .to_owned();
            let port = parsed.port().unwrap_or(563);
            let user = percent_decode_str(parsed.username())
                .decode_utf8_lossy()
                .into_owned();
            if !user.is_empty() {
                acc.attrs.insert("username".to_owned(), user);
            }
            acc.attrs.insert("host".to_owned(), host);
            acc.attrs.insert("port".to_owned(), port.to_string());
            acc.attrs.insert("security".to_owned(), "tls".to_owned());
        }
        "gmail" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "gmail".to_owned();
            }
            let user = percent_decode_str(parsed.username())
                .decode_utf8_lossy()
                .into_owned();
            if !user.is_empty() {
                acc.attrs.insert("email".to_owned(), user);
            }
        }
        "graph" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "graph".to_owned();
            }
            let user = percent_decode_str(parsed.username())
                .decode_utf8_lossy()
                .into_owned();
            if !user.is_empty() {
                acc.attrs.insert("email".to_owned(), user);
            }
        }
        "nostr" => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "nostr".to_owned();
            }
            merge_nostr_uri_attrs(acc, u)?;
        }
        _ if u.starts_with("matrix:store:") => {
            if acc.backend_type.trim().is_empty() {
                acc.backend_type = "matrix".to_owned();
            }
            let rest = u.trim().strip_prefix("matrix:store:").unwrap_or("");
            let colon = rest.find(':').ok_or_else(|| {
                "legacy matrix URI: expected matrix:store:<homeserver>:<user>".to_owned()
            })?;
            let hs = rest[..colon].trim();
            let mx = rest[colon + 1..].trim();
            if !hs.is_empty() {
                acc.attrs.insert("host".to_owned(), hs.to_owned());
                acc.attrs.insert("homeserver".to_owned(), hs.to_owned());
            }
            if !mx.is_empty() {
                acc.attrs.insert("username".to_owned(), mx.to_owned());
            }
        }
        _ => {
            return Err(format!(
                "unsupported legacy store URI scheme {:?}",
                scheme
            ));
        }
    }
    Ok(())
}

fn merge_nostr_uri_attrs(acc: &mut FrbAccount, u: &str) -> Result<(), String> {
    if let Some(rest) = u.strip_prefix("nostr:store:") {
        let id_part = rest
            .split(|c| c == '?' || c == '#')
            .next()
            .unwrap_or(rest)
            .trim();
        if !id_part.is_empty() {
            acc.attrs.insert("npub".to_owned(), id_part.to_owned());
        }
        return Ok(());
    }
    let rest = u
        .strip_prefix("nostr:")
        .ok_or_else(|| format!("not a nostr URI: {u}"))?;
    if rest.starts_with("transport:") {
        return Err(format!("not a nostr store URI: {u}"));
    }
    let id_part = rest
        .split(|c| c == '?' || c == '#')
        .next()
        .unwrap_or(rest)
        .trim();
    if id_part.is_empty() {
        return Err(format!("empty nostr identity in URI: {u}"));
    }
    acc.attrs.insert("npub".to_owned(), id_part.to_owned());
    Ok(())
}

/// Expand XML legacy fields (`connection-uri`, legacy id-as-URI) into attrs; clear those fields.
pub fn migrate_store_xml_legacy_uri(
    s: &mut tagliacarte_core::tagliacarte_config_xml::StoreXml,
) -> Result<(), String> {
    let uri = s
        .connection_uri_attr
        .clone()
        .or_else(|| s.legacy_connection_uri.clone());
    let Some(uri) = uri else {
        return Ok(());
    };
    let mut tmp = FrbAccount {
        id: s.id.clone(),
        label: s.display_name.clone(),
        backend_type: s.store_type.clone(),
        avatar_url: None,
        last_folder: None,
        last_message_id: None,
        attrs: s.attrs.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        lists: std::collections::HashMap::new(),
    };
    merge_legacy_store_uri_into_account(&mut tmp, &uri)?;
    s.store_type = normalize_store_type(&tmp.backend_type);
    s.attrs = tmp.attrs.into_iter().collect();
    s.connection_uri_attr = None;
    s.legacy_connection_uri = None;
    Ok(())
}
