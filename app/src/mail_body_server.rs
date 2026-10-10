/*
 * mail_body_server.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Local mTLS HTTPS server for streaming mail HTML / cid parts to the WebView.
 *
 * Every request is answered from hopf's reactor through callbacks: the
 * store's own callback API for local messages, and the dedicated IMAP
 * mail-body session (one literal at a time, reads paused while the WebView
 * catches up) for IMAP ones. Nothing here blocks.
 */

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex};

use once_cell::sync::OnceCell;
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

use tagliacarte_core::config::set_credentials_backend;
use tagliacarte_core::mime::{StreamingCteDecoder, Utf8StreamAssembler, extract_structured_body};
use tagliacarte_core::protocol::http::mail_view_server::{
    start_mail_view_server, Backlog, MtlsMaterial, ParsedRequest, RequestHandler, ResponseSink,
};
use tagliacarte_core::protocol::imap::{
    plan_body_fetch, DisplayFetch, ImapClientError, ImapStore, Lease, LiteralSink, MailBodySession, Reads,
};
use tagliacarte_core::message_id::MessageId;
use tagliacarte_core::store::{message_for_display_from_raw, Folder};

use crate::frb_api::resolve_mail_account;
use crate::mail_store::{open_cached_store, DynStore};

#[derive(Clone)]
struct StoreReg {
    account_id: String,
    use_keychain: bool,
}

struct MailBodyServerInner {
    mtls: MtlsMaterial,
    addr: SocketAddr,
    registrations: Arc<StdMutex<HashMap<String, StoreReg>>>,
}

static GLOBAL: OnceCell<Arc<MailBodyServerInner>> = OnceCell::new();

/// HTTPS origin including port, e.g. `https://127.0.0.1:54321`.
pub fn mail_body_server_base_url() -> Option<String> {
    GLOBAL
        .get()
        .map(|g| format!("https://127.0.0.1:{}", g.addr.port()))
}

/// Start the server once; returns listen URL and PEM material for WebView mTLS.
pub fn ensure_mail_body_server() -> Result<MailBodyServerInit, String> {
    if let Some(g) = GLOBAL.get() {
        return Ok(MailBodyServerInit {
            base_url: format!("https://127.0.0.1:{}", g.addr.port()),
            ca_cert_pem: g.mtls.ca_cert_pem.clone(),
            client_cert_pem: g.mtls.client_cert_pem.clone(),
            client_key_pem: g.mtls.client_key_pem.clone(),
            enforces_client_cert: g.mtls.enforces_client_cert,
        });
    }
    let mtls = MtlsMaterial::generate().map_err(|e| e.to_string())?;
    let registrations = Arc::new(StdMutex::new(HashMap::<String, StoreReg>::new()));
    let regs = registrations.clone();
    // hopf calls this on its reactor for every request; the work continues
    // through callbacks from there, never blocking the reactor.
    let handler: RequestHandler = Arc::new(move |req: ParsedRequest, sink: ResponseSink| {
        handle_request(req, sink, &regs);
    });
    let addr = start_mail_view_server(mtls.tls_acceptor.clone(), handler).map_err(|e| e.to_string())?;
    let inner = Arc::new(MailBodyServerInner {
        mtls,
        addr,
        registrations,
    });
    let _ = GLOBAL.set(inner);
    let g = GLOBAL.get().expect("just set");
    Ok(MailBodyServerInit {
        base_url: format!("https://127.0.0.1:{}", g.addr.port()),
        ca_cert_pem: g.mtls.ca_cert_pem.clone(),
        client_cert_pem: g.mtls.client_cert_pem.clone(),
        client_key_pem: g.mtls.client_key_pem.clone(),
        enforces_client_cert: g.mtls.enforces_client_cert,
    })
}

#[derive(Clone)]
pub struct MailBodyServerInit {
    pub base_url: String,
    pub ca_cert_pem: String,
    pub client_cert_pem: String,
    pub client_key_pem: String,
    /// True when the TLS stack requires a client certificate (mutual TLS).
    pub enforces_client_cert: bool,
}

pub fn register_mail_body_store(account_id: String, use_keychain: bool) -> Result<String, String> {
    let g = GLOBAL
        .get()
        .ok_or_else(|| "mail body server not started".to_string())?;
    let mut id = [0u8; 16];
    getrandom::getrandom(&mut id).map_err(|e| e.to_string())?;
    let key = hex_lower(&id);
    let mut regs = g.registrations.lock().map_err(|e| e.to_string())?;
    regs.insert(
        key.clone(),
        StoreReg {
            account_id,
            use_keychain,
        },
    );
    Ok(key)
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn header_value<'a>(req: &'a ParsedRequest, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}


/// A short error reply that closes the connection.
fn reply(sink: &mut ResponseSink, status: u16, text: &str) {
    tagliacarte_core::trace_log!("mail_body", "reply {status} {}", text.trim_end());
    let _ = sink.bytes(status, &[("Connection", "close")], text.as_bytes());
}

fn local_view_prefix(store_key: &str, folder: &str, msg_seg: &str) -> String {
    let port = GLOBAL.get().map(|g| g.addr.port()).unwrap_or(0);
    let folder_enc = utf8_percent_encode(folder, NON_ALPHANUMERIC).to_string();
    let msg_seg = utf8_percent_encode(msg_seg, NON_ALPHANUMERIC).to_string();
    format!("https://127.0.0.1:{}/view/{}/{}/{}", port, store_key, folder_enc, msg_seg)
}

fn handle_request(req: ParsedRequest, mut sink: ResponseSink, registrations: &Arc<StdMutex<HashMap<String, StoreReg>>>) {
    tagliacarte_core::trace_log!("mail_body", "request {} {}", req.method, req.target);
    if req.method != "GET" {
        return reply(&mut sink, 405, "Method Not Allowed\r\n");
    }
    if header_value(&req, "host").is_none() {
        return reply(&mut sink, 400, "Missing Host\r\n");
    }
    let path = req.target.split('?').next().unwrap_or(&req.target).to_string();
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() < 5 || parts[0] != "view" {
        return reply(&mut sink, 404, "Not Found\r\n");
    }
    let store_key = parts[1].to_string();
    let folder = percent_decode_str(parts[2]).decode_utf8_lossy().into_owned();
    let message_id = percent_decode_str(parts[3]).decode_utf8_lossy().into_owned();
    let reg = registrations.lock().ok().and_then(|g| g.get(&store_key).cloned());
    let Some(reg) = reg else {
        return reply(&mut sink, 404, "Unknown store key\r\n");
    };
    let conn_close = header_value(&req, "connection")
        .map(|v| v.eq_ignore_ascii_case("close"))
        .unwrap_or(false);
    set_credentials_backend(reg.use_keychain);
    let store: DynStore = match resolve_mail_account(reg.account_id.trim()).and_then(|(acc, uk)| {
        set_credentials_backend(uk);
        open_cached_store(&acc, uk)
    }) {
        Ok(s) => s,
        Err(e) => return reply(&mut sink, 500, &format!("open store: {}\r\n", e)),
    };
    // IMAP messages stream from the dedicated mail-body session; anything
    // else is loaded through the store's own callbacks.
    let imap = match store.as_ref().as_any().downcast_ref::<ImapStore>() {
        Some(imap) => match imap.mail_body_session() {
            Ok(s) => Some(s),
            Err(e) => return reply(&mut sink, 500, &format!("{}\r\n", e)),
        },
        None => None,
    };
    let uid: Option<u32> = if imap.is_some() {
        match message_id.parse() {
            Ok(u) => Some(u),
            Err(_) => return reply(&mut sink, 400, "IMAP message id in path must be numeric UID\r\n"),
        }
    } else {
        None
    };
    if parts[4] == "body" {
        match imap {
            Some(session) => serve_imap_body(sink, &store_key, session, &folder, uid.unwrap_or(0), &req.target, conn_close),
            None => serve_local_body(sink, &store_key, store, &folder, &message_id, &req.target, conn_close),
        }
    } else if parts[4] == "cid" && parts.len() >= 6 {
        let cid = percent_decode_str(parts[5]).decode_utf8_lossy().into_owned();
        match imap {
            Some(session) => serve_imap_cid(sink, session, &folder, uid.unwrap_or(0), &cid, conn_close),
            None => serve_local_cid(sink, store, &folder, &message_id, &cid, conn_close),
        }
    } else {
        reply(&mut sink, 404, "Not Found\r\n")
    }
}

fn query_param<'a>(target: &'a str, key: &str) -> Option<&'a str> {
    let q = target.split('?').nth(1)?;
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?;
        if k == key {
            return it.next();
        }
    }
    None
}

fn escape_html_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Incremental `cid:` → local HTTPS URL rewrite. Defers output only when a `cid:` URL may span chunks.
struct StreamingCidRewriter {
    local_view_prefix: String,
    pending: String,
}

impl StreamingCidRewriter {
    fn new(local_view_prefix: String) -> Self {
        Self {
            local_view_prefix,
            pending: String::new(),
        }
    }

    fn feed(&mut self, text: &str) -> String {
        self.pending.push_str(text);
        let mut out = String::new();

        loop {
            let buf = self.pending.as_str();
            let Some(cid_start) = buf.find("cid:") else {
                let keep = if buf.ends_with("cid") {
                    3
                } else if buf.ends_with("ci") {
                    2
                } else if buf.ends_with('c') {
                    1
                } else {
                    0
                };
                if buf.len() > keep {
                    let emit_len = buf.len() - keep;
                    out.push_str(&buf[..emit_len]);
                    self.pending.drain(..emit_len);
                }
                break;
            };

            out.push_str(&buf[..cid_start]);
            self.pending.drain(..cid_start);

            let buf = self.pending.as_str();
            debug_assert!(buf.starts_with("cid:"));
            let after = &buf[4..];
            let end = after
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '>' | '&'))
                .unwrap_or(after.len());

            if end < after.len() {
                let cid_raw = &after[..end];
                let cid_enc = utf8_percent_encode(cid_raw, NON_ALPHANUMERIC).to_string();
                out.push_str(&format!("{}/cid/{}", self.local_view_prefix, cid_enc));
                self.pending.drain(..4 + end);
                continue;
            }

            // No terminator yet — hold from `cid:` onward for the next chunk (or finish).
            break;
        }

        out
    }

    /// Remaining tail (incomplete `cid:` URLs are emitted verbatim).
    fn finish(self) -> String {
        self.pending
    }
}

fn html_view_csp(port: u16, allow_remote: bool) -> String {
    if allow_remote {
        format!(
            "default-src 'none'; img-src https://127.0.0.1:{} https: http:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'",
            port
        )
    } else {
        format!(
            "default-src 'none'; img-src https://127.0.0.1:{}; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'",
            port
        )
    }
}

fn html_view_style_chunk(target: &str) -> String {
    let fg = query_param(target, "fg").unwrap_or("202020");
    let bg = query_param(target, "bg").unwrap_or("f5f5f5");
    let link = query_param(target, "link").unwrap_or("1565c0");
    let fs = query_param(target, "fs").unwrap_or("15");
    format!(
        "<style>html,body{{background:#{};color:#{};font-family:system-ui,sans-serif;font-size:{}px;margin:8px;word-wrap:break-word;}}a{{color:#{};}}img{{max-width:100%;height:auto;}}</style>",
        bg, fg, fs, link
    )
}

/// Writes 200 + CSP + chunked + Connection, then the injected `<style>` chunk (first body chunk).
fn write_html_mail_view_preamble(sink: &mut ResponseSink, target: &str, conn_close: bool) -> std::io::Result<()> {
    let port = GLOBAL.get().map(|g| g.addr.port()).unwrap_or(0);
    let allow_remote = query_param(target, "allowRemote")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let csp = html_view_csp(port, allow_remote);
    let style = html_view_style_chunk(target);
    let conn_hdr = if conn_close { "close" } else { "keep-alive" };
    let headers = [
        ("Content-Type", "text/html; charset=utf-8"),
        ("Transfer-Encoding", "chunked"),
        ("Content-Security-Policy", csp.as_str()),
        ("Connection", conn_hdr),
    ];
    sink.head(200, &headers)?;
    sink.chunk(style.as_bytes())?;
    Ok(())
}

/// Ends the response once the body has gone out.
fn finish_body(r: std::io::Result<ResponseSink>) {
    if let Ok(mut s) = r {
        let _ = s.end();
    }
}

fn send_html_mail_view(mut sink: ResponseSink, html: String, local_view_prefix: &str, target: &str, conn_close: bool) {
    let html = rewrite_cid_urls(&html, local_view_prefix);
    if write_html_mail_view_preamble(&mut sink, target, conn_close).is_err() {
        return;
    }
    sink.write_all(html.into_bytes(), Box::new(finish_body));
}

/// The whole raw message through the store's callbacks.
fn load_message_raw(store: DynStore, folder: &str, message_id: &str, done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>) {
    let id = MessageId::new(message_id.to_owned());
    store.open_folder(
        folder,
        Box::new(|_| {}),
        Box::new(move |r| {
            let folder: Arc<Box<dyn Folder>> = match r {
                Ok(f) => Arc::new(f),
                Err(e) => return done(Err(e.to_string())),
            };
            let raw: Arc<StdMutex<Vec<u8>>> = Arc::new(StdMutex::new(Vec::new()));
            let (raw_chunks, keep) = (Arc::clone(&raw), Arc::clone(&folder));
            folder.get_message(
                &id,
                Box::new(|_| {}),
                Box::new(move |chunk| raw_chunks.lock().expect("raw lock").extend_from_slice(chunk)),
                Box::new(move |r| {
                    // The folder lives until its fetch has reported.
                    drop(keep);
                    done(r.map(|()| std::mem::take(&mut *raw.lock().expect("raw lock"))).map_err(|e| e.to_string()))
                }),
            );
        }),
    );
}

fn serve_local_body(sink: ResponseSink, store_key: &str, store: DynStore, folder: &str, message_id: &str, target: &str, conn_close: bool) {
    let prefix = local_view_prefix(store_key, folder, message_id);
    let target = target.to_string();
    load_message_raw(
        store,
        folder,
        message_id,
        Box::new(move |r| {
            let mut sink = sink;
            let raw = match r {
                Ok(bytes) => bytes,
                Err(e) => return reply(&mut sink, 500, &format!("{}\r\n", e)),
            };
            let disp = message_for_display_from_raw(&raw);
            let html = disp
                .body_html
                .or_else(|| disp.body_plain.map(|p| format!("<pre>{}</pre>", escape_html_text(&p))))
                .unwrap_or_else(|| "<pre>(no displayable body)</pre>".to_string());
            send_html_mail_view(sink, html, &prefix, &target, conn_close)
        }),
    );
}

fn serve_local_cid(sink: ResponseSink, store: DynStore, folder: &str, message_id: &str, cid: &str, conn_close: bool) {
    let cid_norm = cid.trim().trim_matches(|c| c == '<' || c == '>').to_string();
    load_message_raw(
        store,
        folder,
        message_id,
        Box::new(move |r| {
            let mut sink = sink;
            let raw = match r {
                Ok(bytes) => bytes,
                Err(e) => return reply(&mut sink, 500, &format!("{}\r\n", e)),
            };
            let parts = match extract_structured_body(&raw) {
                Ok((_, _, p)) => p,
                Err(e) => return reply(&mut sink, 500, &format!("MIME parse: {}\r\n", e)),
            };
            let found = parts.into_iter().find(|(_filename, content_id, _ct, _body)| {
                content_id.as_deref().map(|x| x.eq_ignore_ascii_case(&cid_norm)).unwrap_or(false)
            });
            let Some((_filename, _cid, ct, body)) = found else {
                return reply(&mut sink, 404, "cid not found\r\n");
            };
            let len = body.len().to_string();
            let conn = if conn_close { "close" } else { "keep-alive" };
            let headers = [("Content-Type", ct.as_str()), ("Content-Length", len.as_str()), ("Connection", conn)];
            if sink.head(200, &headers).is_err() {
                return;
            }
            sink.write_all(body, Box::new(finish_body));
        }),
    );
}

/// SELECT, then the BODYSTRUCTURE line, with the session leased to this
/// request; `then` gets the plan and the sink.
fn with_plan(
    sink: ResponseSink,
    session: MailBodySession,
    folder: &str,
    uid: u32,
    then: Box<dyn FnOnce(ResponseSink, MailBodySession, tagliacarte_core::protocol::imap::BodyFetchPlan, Lease) + Send>,
) {
    let folder = folder.to_string();
    let s = session.clone();
    session.acquire(Box::new(move |lease| {
        let s2 = s.clone();
        s.select(
            &folder,
            Box::new(move |r| {
                let mut sink = sink;
                if let Err(e) = r {
                    drop(lease);
                    return reply(&mut sink, 500, &format!("{}\r\n", e));
                }
                let s3 = s2.clone();
                s2.fetch_bodystructure_line(
                    uid,
                    Box::new(move |r| {
                        let line = match r {
                            Ok(l) => l,
                            Err(e) => {
                                drop(lease);
                                return reply(&mut sink, 500, &format!("{}\r\n", e));
                            }
                        };
                        match plan_body_fetch(&line) {
                            Some(plan) => then(sink, s3, plan, lease),
                            None => {
                                drop(lease);
                                reply(&mut sink, 500, "BODYSTRUCTURE plan failed\r\n")
                            }
                        }
                    }),
                );
            }),
        );
    }));
}

fn serve_imap_body(sink: ResponseSink, store_key: &str, session: MailBodySession, folder: &str, uid: u32, target: &str, conn_close: bool) {
    let prefix = local_view_prefix(store_key, folder, &uid.to_string());
    let target = target.to_string();
    with_plan(
        sink,
        session,
        folder,
        uid,
        Box::new(move |mut sink, session, plan, lease| match plan.display {
            DisplayFetch::TextPart { section, encoding, is_html, charset_hint: _ } => {
                if write_html_mail_view_preamble(&mut sink, &target, conn_close).is_err() {
                    return;
                }
                if !is_html && sink.chunk(b"<pre>").is_err() {
                    return;
                }
                session.fetch_body_peek_section(
                    uid,
                    &section,
                    Box::new(TextPartSink {
                        sink: Some(sink),
                        cte: StreamingCteDecoder::for_encoding(&encoding),
                        utf8: Utf8StreamAssembler::default(),
                        cid: if is_html { Some(StreamingCidRewriter::new(prefix.clone())) } else { None },
                        acc: Vec::new(),
                        _lease: lease,
                    }),
                );
            }
            DisplayFetch::NestedMessage { section, encoding } => {
                // Nested message/rfc822: the whole part is needed to MIME-parse the inner envelope.
                session.fetch_body_peek_section(
                    uid,
                    &section,
                    Box::new(Collect {
                        body: Vec::new(),
                        sink: Some(sink),
                        _lease: lease,
                        then: Some(Box::new(move |mut sink, body| {
                            let mut cte = StreamingCteDecoder::for_encoding(&encoding);
                            let mut decoded = Vec::new();
                            cte.feed(&body, &mut decoded);
                            cte.finish(&mut decoded);
                            let (plain, html, _) = match extract_structured_body(&decoded) {
                                Ok(x) => x,
                                Err(e) => return reply(&mut sink, 500, &format!("{}\r\n", e)),
                            };
                            let html = html.or(plain).unwrap_or_else(|| "<pre>(empty)</pre>".to_string());
                            send_html_mail_view(sink, html, &prefix, &target, conn_close)
                        })),
                    }),
                );
            }
            DisplayFetch::None => {
                session.fetch_body_by_uid(
                    uid,
                    Box::new(Collect {
                        body: Vec::new(),
                        sink: Some(sink),
                        _lease: lease,
                        then: Some(Box::new(move |sink, body| {
                            let mut cte = StreamingCteDecoder::for_encoding("8BIT");
                            let mut decoded = Vec::new();
                            cte.feed(&body, &mut decoded);
                            cte.finish(&mut decoded);
                            let disp = message_for_display_from_raw(&decoded);
                            let html = disp
                                .body_html
                                .or_else(|| disp.body_plain.map(|p| format!("<pre>{}</pre>", escape_html_text(&p))))
                                .unwrap_or_else(|| "<pre>(no displayable body)</pre>".to_string());
                            send_html_mail_view(sink, html, &prefix, &target, conn_close)
                        })),
                    }),
                );
            }
        }),
    );
}

/// Streams a text part: CTE decode, UTF-8 assembly, HTML escaping or
/// `cid:` rewriting, straight into the response as the literal arrives.
struct TextPartSink {
    sink: Option<ResponseSink>,
    cte: StreamingCteDecoder,
    utf8: Utf8StreamAssembler,
    cid: Option<StreamingCidRewriter>,
    acc: Vec<u8>,
    _lease: Lease,
}

impl TextPartSink {
    fn render(&mut self, text: &str) -> String {
        match self.cid.as_mut() {
            Some(cid) => cid.feed(text),
            None => escape_html_text(text),
        }
    }
}

/// Queue `out` on `sink`; when the backlog is full, pause the IMAP reads
/// until it drains. The sink is gone (and the connection closed) on error.
fn push_out(sink: &mut Option<ResponseSink>, out: &[u8], reads: &mut Reads<'_>) {
    if out.is_empty() {
        return;
    }
    let Some(s) = sink.as_mut() else { return };
    match s.chunk(out) {
        Ok(Backlog::Room) => {}
        Ok(Backlog::Full) => {
            let resume = reads.pause();
            s.when_drained(Box::new(move |_open| resume.resume()));
        }
        Err(_) => {
            if let Some(s) = sink.take() {
                s.abort();
            }
        }
    }
}

impl LiteralSink for TextPartSink {
    fn chunk(&mut self, data: &[u8], reads: &mut Reads<'_>) {
        if self.sink.is_none() {
            return;
        }
        self.cte.feed(data, &mut self.acc);
        let text = self.utf8.push_decode(&self.acc);
        self.acc.clear();
        let out = self.render(&text);
        push_out(&mut self.sink, out.as_bytes(), reads);
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        let Some(mut sink) = self.sink.take() else { return };
        if r.is_err() {
            return sink.abort();
        }
        self.cte.finish(&mut self.acc);
        let tail = self.utf8.push_decode(&self.acc);
        self.acc.clear();
        let mut out = self.render(&tail);
        let tail2 = self.utf8.flush_utf8_lossy();
        out.push_str(&self.render(&tail2));
        if let Some(cid) = self.cid.take() {
            out.push_str(&cid.finish());
        } else {
            out.push_str("</pre>");
        }
        if !out.is_empty() && sink.chunk(out.as_bytes()).is_err() {
            return sink.abort();
        }
        let _ = sink.end();
    }
}

/// Collects a whole literal, then hands it on.
struct Collect {
    body: Vec<u8>,
    sink: Option<ResponseSink>,
    _lease: Lease,
    then: Option<Box<dyn FnOnce(ResponseSink, Vec<u8>) + Send>>,
}

impl LiteralSink for Collect {
    fn chunk(&mut self, data: &[u8], _reads: &mut Reads<'_>) {
        self.body.extend_from_slice(data);
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        let Some(mut sink) = self.sink.take() else { return };
        if let Err(e) = r {
            return reply(&mut sink, 500, &format!("{}\r\n", e));
        }
        if let Some(then) = self.then.take() {
            then(sink, std::mem::take(&mut self.body));
        }
    }
}

/// `local_view_prefix` is `https://127.0.0.1:port/view/{store}/{folder_enc}/{msg_seg}` (no `/body`).
fn rewrite_cid_urls(html: &str, local_view_prefix: &str) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut rest = html;
    while let Some(i) = rest.find("cid:") {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let after = rest.strip_prefix("cid:").unwrap_or(rest);
        let end = after
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '>' || c == '&')
            .unwrap_or(after.len());
        let cid_raw = &after[..end];
        let cid_enc = utf8_percent_encode(cid_raw, NON_ALPHANUMERIC).to_string();
        out.push_str(&format!("{}/cid/{}", local_view_prefix, cid_enc));
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn serve_imap_cid(sink: ResponseSink, session: MailBodySession, folder: &str, uid: u32, cid: &str, conn_close: bool) {
    let cid_norm = cid.trim().trim_matches(|c| c == '<' || c == '>').to_string();
    with_plan(
        sink,
        session,
        folder,
        uid,
        Box::new(move |mut sink, session, plan, lease| {
            let (section, encoding, ct) = if let Some(c) = plan.inline_cids.iter().find(|c| c.cid.eq_ignore_ascii_case(&cid_norm)) {
                (c.section.clone(), c.encoding.clone(), c.content_type.clone())
            } else if let Some(a) = plan
                .attachments
                .iter()
                .find(|a| a.content_id.as_deref().map(|x| x.eq_ignore_ascii_case(&cid_norm)).unwrap_or(false))
            {
                (a.section.clone(), a.encoding.clone(), a.content_type.clone())
            } else {
                drop(lease);
                return reply(&mut sink, 404, "cid not found\r\n");
            };
            let conn = if conn_close { "close" } else { "keep-alive" };
            let headers = [("Content-Type", ct.as_str()), ("Transfer-Encoding", "chunked"), ("Connection", conn)];
            if sink.head(200, &headers).is_err() {
                return;
            }
            session.fetch_body_peek_section(
                uid,
                &section,
                Box::new(CidSink { sink: Some(sink), cte: StreamingCteDecoder::for_encoding(&encoding), decoded: Vec::new(), _lease: lease }),
            );
        }),
    );
}

/// Streams one part's decoded bytes into the response.
struct CidSink {
    sink: Option<ResponseSink>,
    cte: StreamingCteDecoder,
    decoded: Vec<u8>,
    _lease: Lease,
}

impl LiteralSink for CidSink {
    fn chunk(&mut self, data: &[u8], reads: &mut Reads<'_>) {
        if self.sink.is_none() {
            return;
        }
        self.decoded.clear();
        self.cte.feed(data, &mut self.decoded);
        let out = std::mem::take(&mut self.decoded);
        push_out(&mut self.sink, &out, reads);
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        let Some(mut sink) = self.sink.take() else { return };
        if r.is_err() {
            return sink.abort();
        }
        self.decoded.clear();
        self.cte.finish(&mut self.decoded);
        if !self.decoded.is_empty() && sink.chunk(&self.decoded).is_err() {
            return sink.abort();
        }
        let _ = sink.end();
    }
}
