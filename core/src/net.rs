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
            // FIPS mode needs the FIPS build of AWS-LC, which this client
            // does not link.
            fips: false,
        };
        let rt = Arc::new(Runtime::start(config).expect("start hopf runtime"));
        // First use of the runtime cannot be on one of its own reactors, so
        // this wait for a reactor to open the socket always gets answered.
        let dns = DnsResolver::for_runtime(&rt).expect("open hopf DNS resolver");
        let _ = HOPF_DNS.set(Arc::new(dns));
        let foreign = DnsResolver::for_runtime(&rt).expect("open hopf DNS resolver for foreign hosts");
        let _ = HOPF_DNS_FOREIGN.set(Arc::new(foreign));
        rt
    }))
}

static HOPF_DNS: OnceLock<Arc<DnsResolver>> = OnceLock::new();
static HOPF_DNS_FOREIGN: OnceLock<Arc<DnsResolver>> = OnceLock::new();

/// A second resolver, with a cache of its own, for dialling hosts that someone
/// else named (a link or image in a message). Its cache is where the answer
/// that was judged is pinned, so the dial cannot be given another one; keeping
/// it apart means those entries never touch the mail and API connections.
pub fn hopf_dns_resolver_for_foreign_hosts() -> Arc<DnsResolver> {
    let _ = hopf_runtime();
    Arc::clone(HOPF_DNS_FOREIGN.get().expect("resolver built with the runtime"))
}

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

// ── addresses a remote resource may not live at ─────────────────────────────

/// Whether `ip` is a place on the public internet. Loopback, private,
/// link-local, shared (CGNAT), unspecified, multicast and broadcast addresses
/// are not: a message that names one is trying to make this machine knock on
/// something inside the user's own network. IPv4 addresses written as IPv6
/// (`::ffff:10.0.0.1`) are judged as the IPv4 address they are.
pub fn is_public_ip(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !(a.is_loopback()
                || a.is_private()
                || a.is_link_local()
                || a.is_unspecified()
                || a.is_broadcast()
                || a.is_multicast()
                || o[0] == 0
                // 100.64.0.0/10, carrier-grade NAT
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
                // 192.0.0.0/24, IETF protocol assignments
                || (o[0] == 192 && o[1] == 0 && o[2] == 0))
        }
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return is_public_ip(IpAddr::V4(v4));
            }
            let seg = a.segments();
            !(a.is_loopback()
                || a.is_unspecified()
                || a.is_multicast()
                // fc00::/7 unique local
                || (seg[0] & 0xfe00) == 0xfc00
                // fe80::/10 link-local
                || (seg[0] & 0xffc0) == 0xfe80)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_public_ip;
    use std::net::IpAddr;

    fn public(s: &str) -> bool {
        is_public_ip(s.parse::<IpAddr>().unwrap())
    }

    #[test]
    fn addresses_inside_the_users_network_are_not_public() {
        for a in [
            "127.0.0.1", "127.8.9.10", "10.0.0.1", "172.16.0.1", "172.31.255.255", "192.168.1.1",
            "169.254.169.254", "100.64.0.1", "100.127.255.255", "0.0.0.0", "0.1.2.3", "255.255.255.255",
            "224.0.0.1", "192.0.0.8", "::1", "::", "fe80::1", "fc00::1", "fd12:3456::1", "ff02::1",
            "::ffff:127.0.0.1", "::ffff:10.0.0.1", "::ffff:192.168.0.9",
        ] {
            assert!(!public(a), "{a} must not be treated as public");
        }
    }

    #[test]
    fn ordinary_internet_addresses_are_public() {
        for a in [
            "8.8.8.8", "93.184.216.34", "172.15.0.1", "172.32.0.1", "100.63.255.255", "100.128.0.1",
            "2606:4700:4700::1111", "2001:4860:4860::8888", "::ffff:8.8.8.8",
        ] {
            assert!(public(a), "{a} must be public");
        }
    }
}
