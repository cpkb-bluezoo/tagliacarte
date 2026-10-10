/*
 * net.rs
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

//! The process-wide hopf runtime and the TLS connectors every hopf-based
//! protocol client in this crate dials with.

use std::sync::{Arc, OnceLock};

use hopf_dns::DnsResolver;
use hopf_core::{public_trust_connector, Runtime, RuntimeConfig, SharedTlsConnector, StorageConfig};

// ── hopf runtime ────────────────────────────────────────────────────────────
//
// The hopf reactor that every hopf-based protocol client in this crate dials
// from. One per process, started on first use and never shut down: a mail
// client's connections live as long as the application does. Callbacks from
// hopf run on the reactor threads; the Store/Transport traits already deliver
// results through `Send` callbacks and channels, so nothing above this layer
// needs to know.

static HOPF_RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();
static HOPF_TLS_CONNECTOR: OnceLock<SharedTlsConnector> = OnceLock::new();

/// The process-wide hopf [`Runtime`], started on first use.
pub fn hopf_runtime() -> Arc<Runtime> {
    Arc::clone(HOPF_RUNTIME.get_or_init(|| {
        let config = RuntimeConfig {
            // A client holds a handful of connections; two reactors keep one
            // slow TLS handshake from stalling everything else.
            worker_threads: 2,
            storage: StorageConfig { threads: 2, queue_capacity: 256 },
        };
        let rt = Arc::new(Runtime::start(config).expect("start hopf runtime"));
        // First use of the runtime cannot be on one of its own reactors, so
        // this wait for a reactor to open the socket always gets answered.
        let dns = DnsResolver::for_runtime(&rt).expect("open hopf DNS resolver");
        let _ = HOPF_DNS.set(Arc::new(dns));
        rt
    }))
}

static HOPF_DNS: OnceLock<Arc<DnsResolver>> = OnceLock::new();

/// The one DNS resolver every dial uses. hopf builds a fresh resolver per
/// dial when none is given, and doing that registers a socket with a
/// reactor and waits for the answer: from a reactor thread that waits for
/// itself. Continuations run on reactor threads and routinely dial the
/// next connection (a relay fan-out moving to its next stage, a retry), so
/// the resolver is built once with the runtime, off the reactors, and shared.
pub fn hopf_dns_resolver() -> Arc<DnsResolver> {
    // Starting the runtime builds the resolver.
    let _ = hopf_runtime();
    Arc::clone(HOPF_DNS.get().expect("resolver built with the runtime"))
}

/// The TLS connector hopf-based clients use for mail protocols: public
/// WebPKI trust (the platform store, with vendored roots as fallback), no
/// ALPN, TLS 1.3 with 1.2 fallback. Built once; loading the native store
/// is blocking work that must not happen per connection.
pub fn hopf_tls_connector() -> SharedTlsConnector {
    Arc::clone(HOPF_TLS_CONNECTOR.get_or_init(|| public_trust_connector(&[])))
}

/// Install a different connector before the first use — a private CA for
/// tests, or a pinned trust model. Returns `false` if one was already in
/// use.
pub fn set_hopf_tls_connector(connector: SharedTlsConnector) -> bool {
    HOPF_TLS_CONNECTOR.set(connector).is_ok()
}

static HOPF_HTTP_TLS_CONNECTOR: OnceLock<SharedTlsConnector> = OnceLock::new();

/// The TLS connector for HTTPS: the same trust as [`hopf_tls_connector`]
/// but offering `h2` and `http/1.1` by ALPN, so Gmail and Graph negotiate
/// HTTP/2. WebSocket dials use the plain one, since their `Upgrade` needs
/// HTTP/1.1.
pub fn hopf_http_tls_connector() -> SharedTlsConnector {
    Arc::clone(HOPF_HTTP_TLS_CONNECTOR.get_or_init(|| public_trust_connector(&[b"h2", b"http/1.1"])))
}

/// Install a different HTTPS connector before the first use (tests).
/// Returns `false` if one was already in use.
pub fn set_hopf_http_tls_connector(connector: SharedTlsConnector) -> bool {
    HOPF_HTTP_TLS_CONNECTOR.set(connector).is_ok()
}
