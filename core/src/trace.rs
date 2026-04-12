/*
 * trace.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Unified protocol tracing via **`TAGLIACARTE_TRACE`**: a comma- or whitespace-separated list of
//! provider names (`imap`, `nostr`, `smtp`, `matrix`, `pop3`, `nntp`, `mail_body`, `gmail`, `graph`,
//! `http`, …). Matching is case-insensitive.
//!
//! **Examples**
//! ```text
//! export TAGLIACARTE_TRACE="imap,nostr"
//! export TAGLIACARTE_TRACE="imap smtp nostr"
//! export TAGLIACARTE_TRACE="all"
//! ```
//!
//! The token **`all`** enables every built-in provider name ([`ALL_TRACE_PROVIDERS`]).
//!
//! **Where logs go:** protocol code uses `eprintln!` (stderr). With Flutter, native Rust logs
//! appear in the **same terminal as `flutter run`** if you start the app with the variable set on
//! that command (e.g. `TAGLIACARTE_TRACE=imap,smtp flutter run`). IDE Run configs, macOS `.app`
//! bundles, and Xcode schemes often **do not** inherit your shell profile — set the variable in
//! the launch configuration or scheme. They are **not** shown in the Dart-only debug console unless
//! stderr is forwarded.
//!
//! Effective flags are cached and **invalidated when `TAGLIACARTE_TRACE`, `TAGLIACARTE_TRACE_FULL`,
//! or legacy `TAGLIACARTE_IMAP_TRACE*` / `TAGLIACARTE_MAIL_BODY_TRACE` values change**, so tests
//! and runtime env updates are not stuck behind a single parse at first use.
//!
//! **Legacy** (still honored): `TAGLIACARTE_IMAP_TRACE=1` implies `imap`; `TAGLIACARTE_MAIL_BODY_TRACE=1`
//! implies `mail_body`.
//!
//! **Unsafe / full wire logging** per provider: set **`TAGLIACARTE_TRACE_FULL`** with the same list
//! shape (e.g. `imap`) to disable redaction where implemented. Legacy: `TAGLIACARTE_IMAP_TRACE_FULL=1`
//! still enables full IMAP outbound lines.
//!
//! For **`smtp`**, logging the full `DATA` / `BDAT` message body requires **both**
//! `TAGLIACARTE_TRACE=smtp` and `TAGLIACARTE_TRACE_FULL=smtp` (full mode alone does not turn on SMTP).

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

/// Provider names recognized for `TAGLIACARTE_TRACE=all` and documentation.
pub const ALL_TRACE_PROVIDERS: &[&str] = &[
    "imap",
    "nostr",
    "smtp",
    "matrix",
    "pop3",
    "nntp",
    "mail_body",
    "gmail",
    "graph",
    // Low-level HTTP client: HTTP/2 GOAWAY, RST_STREAM, and similar connection issues.
    "http",
];

fn parse_provider_list(var: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(raw) = std::env::var(var) {
        for part in raw.split(|c: char| c == ',' || c.is_whitespace()) {
            let t = part.trim().to_ascii_lowercase();
            if !t.is_empty() {
                set.insert(t);
            }
        }
    }
    if set.contains("all") {
        for p in ALL_TRACE_PROVIDERS {
            set.insert((*p).to_string());
        }
    }
    set
}

/// Snapshot key for trace env: must change when any input to the computed sets changes.
fn trace_env_snapshot_key() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        std::env::var("TAGLIACARTE_TRACE").unwrap_or_default(),
        std::env::var("TAGLIACARTE_TRACE_FULL").unwrap_or_default(),
        env_truthy("TAGLIACARTE_IMAP_TRACE"),
        env_truthy("TAGLIACARTE_MAIL_BODY_TRACE"),
        env_truthy("TAGLIACARTE_IMAP_TRACE_FULL"),
    )
}

fn build_providers_set() -> HashSet<String> {
    let mut s = parse_provider_list("TAGLIACARTE_TRACE");
    if env_truthy("TAGLIACARTE_IMAP_TRACE") {
        s.insert("imap".to_string());
    }
    if env_truthy("TAGLIACARTE_MAIL_BODY_TRACE") {
        s.insert("mail_body".to_string());
    }
    s
}

fn build_full_providers_set() -> HashSet<String> {
    let mut s = parse_provider_list("TAGLIACARTE_TRACE_FULL");
    if env_truthy("TAGLIACARTE_IMAP_TRACE_FULL") {
        s.insert("imap".to_string());
    }
    if s.contains("all") {
        for p in ALL_TRACE_PROVIDERS {
            s.insert((*p).to_string());
        }
    }
    s
}

static TRACE_SNAPSHOT: RwLock<Option<(String, Arc<HashSet<String>>, Arc<HashSet<String>>)>> =
    RwLock::new(None);

fn providers_arc() -> Arc<HashSet<String>> {
    let key = trace_env_snapshot_key();
    {
        let r = TRACE_SNAPSHOT.read().unwrap();
        if let Some((ref k, ref p, _)) = *r {
            if k == &key {
                return Arc::clone(p);
            }
        }
    }
    let mut w = TRACE_SNAPSHOT.write().unwrap();
    if let Some((ref k, ref p, _)) = *w {
        if k == &key {
            return Arc::clone(p);
        }
    }
    let p = Arc::new(build_providers_set());
    let f = Arc::new(build_full_providers_set());
    *w = Some((key, Arc::clone(&p), f));
    p
}

fn full_providers_arc() -> Arc<HashSet<String>> {
    let key = trace_env_snapshot_key();
    {
        let r = TRACE_SNAPSHOT.read().unwrap();
        if let Some((ref k, _, ref f)) = *r {
            if k == &key {
                return Arc::clone(f);
            }
        }
    }
    let _ = providers_arc();
    TRACE_SNAPSHOT
        .read()
        .unwrap()
        .as_ref()
        .map(|(_, _, f)| Arc::clone(f))
        .expect("full_providers_arc after providers_arc")
}

/// True if `TAGLIACARTE_TRACE` (or a legacy flag) includes this provider.
///
/// Used in-tree for wire logging: `imap`, `nostr`, `smtp`, `mail_body`, `gmail`, `graph`, `http`,
/// and TLS handshake lines in [`crate::net`] when `smtp` / `imap` / `pop3` / `nntp` is enabled.
/// Other names: `matrix` logs Matrix Client-Server HTTP (see `protocol::matrix::trace`).
pub fn enabled(provider: &str) -> bool {
    let key = provider.trim().to_ascii_lowercase();
    providers_arc().contains(key.as_str())
}

/// Full / non-redacted wire logging for this provider (`TAGLIACARTE_TRACE_FULL` or legacy IMAP full).
pub fn full_enabled(provider: &str) -> bool {
    let key = provider.trim().to_ascii_lowercase();
    full_providers_arc().contains(key.as_str())
}

/// MIME/body extraction diagnostics in the app layer.
pub fn mail_body_enabled() -> bool {
    enabled("mail_body")
}

/// Log to stderr when [`enabled`](enabled) is true for `provider`.
#[macro_export]
macro_rules! trace_log {
    ($provider:expr, $($arg:tt)*) => {
        if $crate::trace::enabled($provider) {
            eprintln!("[{} trace] {}", $provider, format!($($arg)*));
        }
    };
}
