/*
 * client.rs
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
//! HTTP client: dial a host on hopf and hand the connection to a
//! continuation; requests then go out with a callback handler.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hopf_http::{HttpClientSessionHandle, HttpClientTimeouts, HttpConnectionHandler, HttpLimits};

use crate::net::{hopf_http_tls_connector, hopf_runtime};
use crate::protocol::http::connection::{HttpConnection, HttpVersion};

const DNS_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Quiet-time budget for one request once it is on the wire; hopf re-arms
/// it on every byte in either direction, so a slow but live download is
/// not cut off.
const STAGE_TIMEOUT: Duration = Duration::from_secs(120);

/// Hears the outcome of a dial, once: from hopf's reactor when the
/// connection (and TLS handshake) is up or has failed, or from the calling
/// thread when the dial is refused outright.
pub type OnConnect = Box<dyn FnOnce(io::Result<HttpConnection>) + Send>;

/// HTTP client. Dial with [`HttpClient::connect`]; the continuation gets an
/// [`HttpConnection`] to build requests on and send with a handler.
pub struct HttpClient;

struct DialHandler {
    reply: Arc<Mutex<Option<OnConnect>>>,
    host: String,
    port: u16,
    secure: bool,
    alive: Arc<AtomicBool>,
}

impl DialHandler {
    fn reply(&self, r: io::Result<HttpConnection>) {
        let k = self.reply.lock().unwrap().take();
        if let Some(k) = k {
            k(r);
        }
    }
}

impl HttpConnectionHandler for DialHandler {
    fn on_connected(&mut self, session: &mut HttpClientSessionHandle) {
        let version = match session.version() {
            hopf_http::HttpVersion::Http2 => HttpVersion::Http2,
            _ => HttpVersion::Http1_1,
        };
        self.reply(Ok(HttpConnection::new(
            session.clone(),
            self.host.clone(),
            self.port,
            self.secure,
            version,
            Arc::clone(&self.alive),
        )));
    }

    fn on_disconnected(&mut self) {
        self.alive.store(false, Ordering::Release);
        self.reply(Err(io::Error::new(io::ErrorKind::ConnectionReset, "connection closed")));
    }

    fn on_error(&mut self, err: &io::Error) {
        self.alive.store(false, Ordering::Release);
        self.reply(Err(io::Error::new(err.kind(), err.to_string())));
    }
}

impl HttpClient {
    /// Connect to the given host and port. If `use_tls` is true, performs a
    /// TLS handshake with ALPN (h2, http/1.1). `on_connect` gets an
    /// `HttpConnection` to issue requests on. The negotiated protocol
    /// (HTTP/1.1 or HTTP/2) is set from ALPN when using TLS; plain TCP uses
    /// HTTP/1.1.
    pub fn connect(host: &str, port: u16, use_tls: bool, on_connect: OnConnect) {
        Self::dial(host, port, use_tls, false, on_connect)
    }

    /// Cleartext HTTP/2 with prior knowledge (RFC 9113 §3.3), for servers
    /// known to speak it without TLS, such as a local one in tests.
    pub fn connect_h2c(host: &str, port: u16, on_connect: OnConnect) {
        Self::dial(host, port, false, true, on_connect)
    }

    /// Dial a host named by someone else (a link in a message). The name is
    /// resolved first and every address that is not on the public internet is
    /// refused (unless `allow_private`, for tests). What was judged is then
    /// pinned in the resolver's cache, and the dial goes by name through that
    /// resolver, so it reaches only the addresses that were judged: looking the
    /// name up again would let a hostile DNS server answer a public address to
    /// the check and a private one to the dial. The name itself is kept for
    /// TLS and the `Host` header.
    pub fn connect_public(host: &str, port: u16, use_tls: bool, allow_private: bool, on_connect: OnConnect) {
        let name = host.to_owned();
        let resolver = crate::net::hopf_dns_resolver_for_foreign_hosts();
        let on_connect = Arc::new(Mutex::new(Some(on_connect)));
        let fail = {
            let on_connect = Arc::clone(&on_connect);
            move |e: io::Error| {
                let k = on_connect.lock().unwrap().take();
                if let Some(k) = k {
                    k(Err(e));
                }
            }
        };
        let pin_to = Arc::clone(&resolver);
        resolver.resolve(
            host,
            port,
            Box::new(move |r| {
                let addrs = match r {
                    Ok(a) => a,
                    Err(e) => return fail(io::Error::new(e.kind(), format!("could not resolve {name}: {e}"))),
                };
                let allowed: Vec<_> =
                    addrs.into_iter().filter(|a| allow_private || crate::net::is_public_ip(a.ip())).collect();
                if allowed.is_empty() {
                    return fail(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("{name} is not on the public internet"),
                    ));
                }
                pin(&pin_to, &name, &allowed);
                let k = on_connect.lock().unwrap().take();
                if let Some(k) = k {
                    Self::dial_to(&name, port, use_tls, false, Some(pin_to), k);
                }
            }),
        );
    }

    fn dial(host: &str, port: u16, use_tls: bool, h2c: bool, on_connect: OnConnect) {
        Self::dial_to(host, port, use_tls, h2c, None, on_connect)
    }

    /// `resolver` is the one to look `host` up with; the shared one when `None`.
    fn dial_to(host: &str, port: u16, use_tls: bool, h2c: bool, resolver: Option<Arc<hopf_dns::DnsResolver>>, on_connect: OnConnect) {
        let limits = HttpLimits {
            // Attachments come through here whole; no cap on what a decoded
            // response may grow to (the consumer streams it anyway).
            max_decoded_body: usize::MAX,
            ..HttpLimits::default()
        };
        let mut client = hopf_http::HttpClient::new(host, port)
            .timeouts(HttpClientTimeouts { dns: DNS_TIMEOUT, connect: CONNECT_TIMEOUT, stage: STAGE_TIMEOUT, ..HttpClientTimeouts::default() })
            .limits(limits)
            .resolver(resolver.unwrap_or_else(crate::net::hopf_dns_resolver))
            .h2_prior_knowledge(h2c);
        if use_tls {
            client = client.tls(hopf_http_tls_connector(), host);
        }
        let reply = Arc::new(Mutex::new(Some(on_connect)));
        let handler = DialHandler {
            reply: Arc::clone(&reply),
            host: host.to_string(),
            port,
            secure: use_tls,
            alive: Arc::new(AtomicBool::new(true)),
        };
        if let Err(e) = client.connect(&hopf_runtime(), Box::new(handler)) {
            let k = reply.lock().unwrap().take();
            if let Some(k) = k {
                k(Err(e));
            }
        }
    }
}

/// Makes `resolver` answer `name` with `addrs` and nothing else, for a minute.
fn pin(resolver: &hopf_dns::DnsResolver, name: &str, addrs: &[std::net::SocketAddr]) {
    use hopf_dns::{DnsQuestion, DnsResourceRecord, DnsType};
    use std::net::IpAddr;
    const PIN_SECONDS: u32 = 60;
    let cache = resolver.cache();
    let (mut v4, mut v6) = (Vec::new(), Vec::new());
    for a in addrs {
        match a.ip() {
            IpAddr::V4(ip) => v4.push(DnsResourceRecord::a(name, PIN_SECONDS, ip)),
            IpAddr::V6(ip) => v6.push(DnsResourceRecord::aaaa(name, PIN_SECONDS, ip)),
        }
    }
    cache.put(&DnsQuestion::in_class(name, DnsType::A), v4, PIN_SECONDS);
    cache.put(&DnsQuestion::in_class(name, DnsType::Aaaa), v6, PIN_SECONDS);
}

#[cfg(test)]
mod tests {
    use super::pin;
    use std::net::SocketAddr;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    #[test]
    fn a_pinned_name_resolves_to_what_was_pinned_and_nothing_else() {
        let resolver = crate::net::hopf_dns_resolver_for_foreign_hosts();
        // Not a name that exists: only the pin can answer it.
        let name = "pinned.rebinding-test.invalid";
        let want: Vec<SocketAddr> = vec!["203.0.113.7:443".parse().unwrap(), "[2001:db8::7]:443".parse().unwrap()];
        pin(&resolver, name, &want);
        let (tx, rx) = channel();
        resolver.resolve(name, 443, Box::new(move |r| tx.send(r).unwrap()));
        let mut got = rx.recv_timeout(Duration::from_secs(10)).expect("answered").expect("resolved from the pin");
        got.sort();
        let mut want = want;
        want.sort();
        assert_eq!(got, want);
    }
}
