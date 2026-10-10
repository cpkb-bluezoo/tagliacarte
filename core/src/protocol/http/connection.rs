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
//! HTTP connection on hopf-http: one `HttpClientSessionHandle` (HTTP/1.1
//! or HTTP/2, as negotiated). Requests are handed to hopf and the reactor
//! is poked to put them on the wire; responses come back as hopf callbacks,
//! translated to [`ResponseHandler`] events. A request body larger than
//! hopf's outbound buffers goes out in rounds, each resumed by hopf's
//! writable callback on the reactor thread: nothing here blocks or waits.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use hopf_core::ConnHandle;
use hopf_http::{reason_phrase, Headers, HttpClientSessionHandle, HttpRequest, HttpResponseHandler, PriorityParams, PRIORITY_HEADER};

use crate::protocol::http::api_trace;
use crate::protocol::http::request::RequestBuilder;
use crate::protocol::http::response::Response;
use crate::protocol::http::{Method, ResponseHandler};
use crate::protocol::matrix::trace as matrix_trace;

/// Negotiated protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion {
    Http1_1,
    Http2,
}

/// One parallel GET's outcome: status and body.
pub type GetResult = io::Result<(u16, Vec<u8>)>;
/// Hears a whole batch of parallel GETs, in the order they were given.
pub type OnBatch = Box<dyn FnOnce(Vec<GetResult>) + Send>;

/// HTTP connection: a live hopf session plus what was dialled. Cheap to
/// clone; the connection closes when the last clone is dropped, so a
/// continuation that must outlive the caller keeps a clone.
#[derive(Clone)]
pub struct HttpConnection {
    inner: Arc<Inner>,
}

struct Inner {
    session: HttpClientSessionHandle,
    host: String,
    port: u16,
    #[allow(dead_code)]
    secure: bool,
    version: HttpVersion,
    alive: Arc<AtomicBool>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(h) = self.session.conn_handle() {
            h.close();
        }
    }
}

/// The consumer's handler, shared between the hopf-facing relay and the
/// sender (which reports a refused request through it), delivered exactly
/// one of `complete`/`failed`: hopf may drop a relay without a word when
/// the connection goes away, and that counts as a failure.
struct Guarded<H> {
    inner: H,
    finished: bool,
}

impl<H: ResponseHandler> Guarded<H> {
    fn complete(&mut self) {
        if !self.finished {
            self.finished = true;
            self.inner.complete();
        }
    }

    fn failed(&mut self, err: &io::Error) {
        if !self.finished {
            self.finished = true;
            self.inner.failed(err);
        }
    }
}

type Shared<H> = Arc<Mutex<Guarded<H>>>;

/// Translates hopf's response events to [`ResponseHandler`].
struct Relay<H: ResponseHandler + Send> {
    handler: Shared<H>,
}

impl<H: ResponseHandler + Send> Drop for Relay<H> {
    fn drop(&mut self) {
        self.handler.lock().unwrap().failed(&closed_error());
    }
}

impl<H: ResponseHandler + Send> HttpResponseHandler for Relay<H> {
    fn ok(&mut self, status: u16) {
        self.handler.lock().unwrap().inner.ok(Response::with_reason(status, reason_phrase(status).to_string()));
    }
    fn error(&mut self, status: u16) {
        self.handler.lock().unwrap().inner.error(Response::with_reason(status, reason_phrase(status).to_string()));
    }
    fn header(&mut self, name: &str, value: &str) {
        self.handler.lock().unwrap().inner.header(name, value);
    }
    fn start_response_body(&mut self) {
        self.handler.lock().unwrap().inner.start_body();
    }
    fn response_body_content(&mut self, data: &[u8]) {
        self.handler.lock().unwrap().inner.body_chunk(data);
    }
    fn end_response_body(&mut self) {
        self.handler.lock().unwrap().inner.end_body();
    }
    fn response_trailers(&mut self, headers: &Headers) {
        let mut g = self.handler.lock().unwrap();
        for f in headers.iter() {
            g.inner.header(&f.name, &f.value);
        }
    }
    fn close(&mut self) {
        self.handler.lock().unwrap().complete();
    }
    fn failed(&mut self, err: io::Error) {
        self.handler.lock().unwrap().failed(&err);
    }
}

fn client_err(e: hopf_http::HttpClientError) -> io::Error {
    let kind = if e.to_string().contains("not open") {
        io::ErrorKind::ConnectionReset
    } else {
        io::ErrorKind::Other
    };
    io::Error::new(kind, e.to_string())
}

fn closed_error() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionReset, "connection closed")
}

/// Request headers hopf manages itself; a consumer's copy is dropped.
fn managed_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("host")
        || name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("transfer-encoding")
}

/// A request body going out in rounds. Between rounds it is parked in a
/// [`Park`] cell that hopf's writable callback takes it back out of.
struct Pump<H: ResponseHandler + Send> {
    req: HttpRequest,
    body: Vec<u8>,
    offset: usize,
    conn: HttpConnection,
    handler: Shared<H>,
    trace: RequestBuilder,
}

/// Who holds the pump: the writer (`Running`), the cell (`Parked`), or
/// nobody because the writable callback fired while the writer was still
/// arming it (`Fired`).
enum Park<H: ResponseHandler + Send> {
    Running,
    Parked(Box<Pump<H>>),
    Fired,
}

impl<H: ResponseHandler + Send + 'static> Pump<H> {
    fn fail(&self, e: hopf_http::HttpClientError) {
        self.handler.lock().unwrap().failed(&client_err(e));
    }

    /// Write as much as hopf takes; on a short write, park until writable.
    fn run(mut self: Box<Self>, cell: Arc<Mutex<Park<H>>>) {
        loop {
            if self.offset >= self.body.len() {
                if let Some(ref t) = self.trace.matrix_outbound {
                    matrix_trace::log_outbound_body_chunk(&t.method, &t.path, 0, &self.body);
                }
                if let Some(ref t) = self.trace.api_outbound {
                    api_trace::log_outbound_body_chunk(t, 0, &self.body);
                }
                if let Err(e) = self.req.end_request_body() {
                    self.fail(e);
                    return;
                }
                self.conn.poke();
                return;
            }
            match self.req.request_body_content(&self.body[self.offset..]) {
                Ok(n) => self.offset += n,
                Err(e) => {
                    self.fail(e);
                    return;
                }
            }
            self.conn.poke();
            if self.offset >= self.body.len() {
                continue;
            }
            let resume = Arc::clone(&cell);
            let armed = self.req.on_body_writable(Box::new(move || {
                let next = {
                    let mut g = resume.lock().unwrap();
                    match std::mem::replace(&mut *g, Park::Running) {
                        Park::Parked(p) => Some(p),
                        _ => {
                            *g = Park::Fired;
                            None
                        }
                    }
                };
                if let Some(p) = next {
                    p.run(resume);
                }
            }));
            if let Err(e) = armed {
                self.fail(e);
                return;
            }
            self.conn.poke();
            let mut g = cell.lock().unwrap();
            if matches!(*g, Park::Fired) {
                *g = Park::Running;
                drop(g);
                continue;
            }
            *g = Park::Parked(self);
            return;
        }
    }
}

/// One slot of a parallel-GET batch.
struct Slot {
    batch: Arc<Mutex<Batch>>,
    index: usize,
    status: u16,
    body: Vec<u8>,
}

struct Batch {
    results: Vec<Option<GetResult>>,
    pending: usize,
    on_done: Option<OnBatch>,
}

impl Batch {
    fn deliver(batch: &Arc<Mutex<Batch>>, index: usize, r: GetResult) {
        let done = {
            let mut g = batch.lock().unwrap();
            if g.results[index].is_some() {
                return;
            }
            g.results[index] = Some(r);
            g.pending -= 1;
            if g.pending == 0 {
                let results = g.results.drain(..).map(|r| r.expect("every slot delivered")).collect();
                g.on_done.take().map(|k| (k, results))
            } else {
                None
            }
        };
        if let Some((k, results)) = done {
            k(results);
        }
    }
}

impl ResponseHandler for Slot {
    fn ok(&mut self, r: Response) {
        self.status = r.code;
    }
    fn error(&mut self, r: Response) {
        self.status = r.code;
    }
    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}
    fn body_chunk(&mut self, data: &[u8]) {
        self.body.extend_from_slice(data);
    }
    fn end_body(&mut self) {}
    fn complete(&mut self) {
        Batch::deliver(&self.batch, self.index, Ok((self.status, std::mem::take(&mut self.body))));
    }
    fn failed(&mut self, err: &io::Error) {
        Batch::deliver(&self.batch, self.index, Err(io::Error::new(err.kind(), err.to_string())));
    }
}

impl HttpConnection {
    pub(crate) fn new(
        session: HttpClientSessionHandle,
        host: String,
        port: u16,
        secure: bool,
        version: HttpVersion,
        alive: Arc<AtomicBool>,
    ) -> Self {
        Self { inner: Arc::new(Inner { session, host, port, secure, version, alive }) }
    }

    pub fn version(&self) -> HttpVersion {
        self.inner.version
    }

    pub fn host(&self) -> &str {
        &self.inner.host
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    /// Whether the connection is still open as far as hopf has told us.
    pub fn is_alive(&self) -> bool {
        self.inner.alive.load(Ordering::Acquire)
    }

    pub fn request(&self, method: Method, path: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(method, path.into())
    }

    fn handle(&self) -> Option<ConnHandle> {
        self.inner.session.conn_handle()
    }

    fn poke(&self) {
        if let Some(h) = self.handle() {
            h.poke();
        }
    }

    fn build(&self, request: &RequestBuilder) -> io::Result<HttpRequest> {
        if !self.is_alive() {
            return Err(closed_error());
        }
        let mut req = self.inner.session.clone().method(request.method.as_str(), &request.path);
        for (name, value) in &request.headers {
            if managed_header(name) {
                continue;
            }
            req.header(name, value).map_err(client_err)?;
        }
        Ok(req)
    }

    /// Send the request. Returns at once; every outcome reaches `handler`,
    /// ending with exactly one of `complete` (the response was read in
    /// full) or `failed` (the request could not be sent, the connection
    /// went away, or hopf gave up on it).
    pub fn send(&self, mut request: RequestBuilder, handler: impl ResponseHandler + Send + 'static) {
        let shared: Shared<_> = Arc::new(Mutex::new(Guarded { inner: handler, finished: false }));
        let mut req = match self.build(&request) {
            Ok(r) => r,
            Err(e) => {
                shared.lock().unwrap().failed(&e);
                return;
            }
        };
        let relay = Box::new(Relay { handler: Arc::clone(&shared) });
        let body = request.body.take();
        let body_len = body.as_ref().map(|b| b.len());
        match body {
            None => {
                if let Err(e) = req.send(relay) {
                    shared.lock().unwrap().failed(&client_err(e));
                    return;
                }
                self.poke();
                self.trace_headers_sent(&request, body_len);
            }
            Some(body) => {
                if let Err(e) = req.start_request_body(relay) {
                    shared.lock().unwrap().failed(&client_err(e));
                    return;
                }
                self.poke();
                self.trace_headers_sent(&request, body_len);
                let pump = Box::new(Pump { req, body, offset: 0, conn: self.clone(), handler: shared, trace: request });
                pump.run(Arc::new(Mutex::new(Park::Running)));
            }
        }
    }

    fn trace_headers_sent(&self, request: &RequestBuilder, body_len: Option<usize>) {
        if let Some(ref t) = request.matrix_outbound {
            matrix_trace::log_outbound_headers_sent(&t.method, &t.path, t.auth_bearer, body_len);
        }
        if let Some(ref t) = request.api_outbound {
            api_trace::log_outbound_headers_sent(t, body_len);
        }
    }

    /// Multiple GET requests on one HTTP/2 connection (multiplexed streams).
    ///
    /// Each request must have no body. `weight` (1–255) orders the requests:
    /// higher values are sent first and carry a more urgent RFC 9218
    /// `Priority` header, so on-screen rows can be preferred. `on_done` gets
    /// one result per request in the **original `items` order** (index `0`
    /// … `n-1`), once all have completed or failed. Returns an error only
    /// for a misuse (not HTTP/2, or a request with a body), in which case
    /// `on_done` is not called.
    pub fn send_http2_parallel_gets(&self, items: Vec<(RequestBuilder, u8)>, on_done: OnBatch) -> io::Result<()> {
        if self.inner.version != HttpVersion::Http2 {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "send_http2_parallel_gets requires HTTP/2"));
        }
        for (req, _) in &items {
            if req.body.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "send_http2_parallel_gets only supports empty request bodies",
                ));
            }
        }
        let n = items.len();
        if n == 0 {
            on_done(Vec::new());
            return Ok(());
        }
        let batch = Arc::new(Mutex::new(Batch { results: (0..n).map(|_| None).collect(), pending: n, on_done: Some(on_done) }));
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(items[i].1));
        for i in order {
            let (request, weight) = &items[i];
            let mut req = match self.build(request) {
                Ok(r) => r,
                Err(e) => {
                    Batch::deliver(&batch, i, Err(e));
                    continue;
                }
            };
            // weight 255 → u=0 (most urgent), weight 1 → u=7.
            let urgency = 7u8.saturating_sub(((*weight as u16) * 7 / 255) as u8);
            if !request.headers.keys().any(|k| k.eq_ignore_ascii_case(PRIORITY_HEADER)) {
                let _ = req.header(PRIORITY_HEADER, PriorityParams::new(urgency, false).encode());
            }
            let slot = Slot { batch: Arc::clone(&batch), index: i, status: 0, body: Vec::new() };
            let shared: Shared<Slot> = Arc::new(Mutex::new(Guarded { inner: slot, finished: false }));
            if let Err(e) = req.send(Box::new(Relay { handler: Arc::clone(&shared) })) {
                shared.lock().unwrap().failed(&client_err(e));
            }
        }
        self.poke();
        Ok(())
    }
}
