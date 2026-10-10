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

//! NNTP connection on hopf-nntp: the session handle keeps the callback API
//! the store and transport were written against, and `connect_and_start_pipeline`
//! bridges hopf's connection lifecycle to an async result.

use std::io;
use std::sync::{Arc, Mutex};

use hopf_nntp::{NntpClient, NntpClientHandler, NntpClientTimeouts, NntpGreeting, NntpSession};

pub use hopf_nntp::{GroupResult, NewsgroupEntry, NntpClientError, OverviewEntry};

use crate::net::{hopf_runtime, hopf_tls_connector};

/// Receives the outcome of a dial, once.
pub type OnConnect = Box<dyn FnOnce(Result<NntpConnection, NntpClientError>) + Send>;

/// A live, authenticated NNTP session. Cheap to clone; commands go out in
/// order, one at a time.
#[derive(Clone)]
pub struct NntpConnection {
    session: NntpSession,
}

impl NntpConnection {
    /// Any command. `expects_multiline` says a 2xx reply announces a
    /// multi-line block, delivered line by line to `on_line` (lossily
    /// decoded); `on_complete` gets the status code and text, or `0` with
    /// the reason when the session ended first.
    pub fn send(
        &self,
        command: &str,
        expects_multiline: bool,
        on_line: impl Fn(&str) + Send + Sync + 'static,
        on_complete: impl FnOnce(u16, &str) + Send + 'static,
    ) {
        self.session.command(
            command,
            expects_multiline,
            move |l| on_line(&String::from_utf8_lossy(l)),
            move |r| match r {
                Ok(s) => on_complete(s.code, &s.text),
                Err(e) => on_complete(e.code, &e.message),
            },
        );
    }

    pub fn is_alive(&self) -> bool {
        self.session.is_alive()
    }

    pub fn posting_allowed(&self) -> bool {
        self.session.posting_allowed()
    }

    pub fn list_newsgroups_streaming(
        &self,
        on_entry: impl Fn(NewsgroupEntry) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        self.session.list_active("", on_entry, on_complete);
    }

    pub fn list_newsgroups_active_wildmat_streaming(
        &self,
        wildmat: &str,
        on_entry: impl Fn(NewsgroupEntry) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        self.session.list_active(wildmat, on_entry, on_complete);
    }

    pub fn group(&self, name: &str, on_complete: impl FnOnce(Result<GroupResult, NntpClientError>) + Send + 'static) {
        self.session.group(name, on_complete);
    }

    pub fn over_streaming(
        &self,
        first: u64,
        last: u64,
        on_entry: impl Fn(OverviewEntry) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        self.session.over(first, last, on_entry, on_complete);
    }

    pub fn article_streaming(
        &self,
        number: u64,
        on_line: impl Fn(&str) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        self.session.article(number, move |l| on_line(&String::from_utf8_lossy(l)), on_complete);
    }

    pub fn head_streaming(
        &self,
        number: u64,
        on_line: impl Fn(&str) + Send + Sync + 'static,
        on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static,
    ) {
        self.session.head(number, move |l| on_line(&String::from_utf8_lossy(l)), on_complete);
    }

    /// `POST` an article (headers, blank line, body). Dot-stuffing and the
    /// terminator are added by hopf-nntp.
    pub fn post(&self, article: &str, on_complete: impl FnOnce(Result<(), NntpClientError>) + Send + 'static) {
        self.session.post(article.as_bytes(), on_complete);
    }

    /// `QUIT` and close.
    pub fn quit(&self) {
        self.session.quit();
    }
}

/// Reports the connection outcome once; later lifecycle events are logged.
struct Dial {
    reply: Arc<Mutex<Option<OnConnect>>>,
}

impl NntpClientHandler for Dial {
    fn on_connected(&mut self, session: &NntpSession, _greeting: &NntpGreeting) {
        let reply = self.reply.lock().unwrap().take();
        if let Some(k) = reply {
            k(Ok(NntpConnection { session: session.clone() }));
        }
    }

    fn on_error(&mut self, error: &io::Error) {
        let reply = self.reply.lock().unwrap().take();
        match reply {
            Some(k) => k(Err(NntpClientError::transport(error.to_string()))),
            None => crate::trace_log!("nntp", "session error: {}", error),
        }
    }

    fn on_disconnected(&mut self) {
        crate::trace_log!("nntp", "disconnected");
    }
}

/// Dial, greet, negotiate TLS as asked, authenticate, and hand the session
/// to `on_connect` from hopf's reactor (or from this thread when the dial is
/// refused outright). `use_starttls` requires STARTTLS on a cleartext
/// connection; the server must offer it. DNS, connect and greeting waits
/// are bounded by hopf-nntp's own timers.
pub fn connect(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str)>,
    on_connect: OnConnect,
) {
    let mut client = NntpClient::new(host, port)
        .timeouts(NntpClientTimeouts::default())
        .resolver(crate::net::hopf_dns_resolver());
    if use_implicit_tls {
        client = client.implicit_tls(hopf_tls_connector(), host);
    } else if use_starttls {
        client = client.starttls(hopf_tls_connector(), host);
    }
    if let Some((user, pass)) = auth {
        client = client.credentials(user, pass);
    }
    let reply = Arc::new(Mutex::new(Some(on_connect)));
    let dial = Dial { reply: Arc::clone(&reply) };
    if let Err(e) = client.connect_with(&hopf_runtime(), Box::new(dial)) {
        let k = reply.lock().unwrap().take();
        if let Some(k) = k {
            k(Err(NntpClientError::transport(e.to_string())));
        }
    }
}
