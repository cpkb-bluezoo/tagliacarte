/*
 * mail_kind.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Normalized store/transport type strings (lowercase). Shared by app session and config.
 */

/// Lowercase store `type` / account `backendType`.
pub fn normalize_store_type(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

/// Lowercase transport `type`.
pub fn normalize_transport_type(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

pub fn is_nostr_store(t: &str) -> bool {
    normalize_store_type(t) == "nostr"
}

pub fn is_matrix_store(t: &str) -> bool {
    normalize_store_type(t) == "matrix"
}

pub fn is_maildir_store(t: &str) -> bool {
    matches!(normalize_store_type(t).as_str(), "maildir")
}

pub fn is_mbox_store(t: &str) -> bool {
    normalize_store_type(t) == "mbox"
}

/// IMAP over TCP (custom host) or Gmail (Google IMAP + XOAUTH2).
pub fn is_imap_like_store(t: &str) -> bool {
    matches!(
        normalize_store_type(t).as_str(),
        "imap" | "imaps" | "gmail"
    )
}

/// Microsoft Graph mailbox (`graph` in legacy URIs, `exchange` from UI / config).
pub fn is_graph_mailbox_store(t: &str) -> bool {
    matches!(
        normalize_store_type(t).as_str(),
        "graph" | "exchange"
    )
}

pub fn uses_long_imap_fetch_timeout(t: &str) -> bool {
    is_imap_like_store(t) || is_graph_mailbox_store(t)
}
