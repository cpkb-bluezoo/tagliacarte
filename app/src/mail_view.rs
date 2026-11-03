/*
 * mail_view.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Message content for a UI that serves it over its own protocol (the wry
 * GUI): the HTML body of a message, its inline `cid:` parts, and the
 * envelope and attachment list as a sequence of events.
 *
 * Nothing here knows about HTTP or a window. Each entry point returns at
 * once and finishes through a callback, from the store's own callbacks or
 * from the dedicated IMAP mail-body session (one literal at a time,
 * decoded as it arrives). A transport that can only answer whole responses
 * gets the finished bytes; one that can stream could be given the pieces.
 */

use std::io::Write;
use std::sync::{Arc, Mutex};

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

use tagliacarte_core::config::set_credentials_backend;
use tagliacarte_core::mime::{StreamingCteDecoder, Utf8StreamAssembler, extract_structured_body};
use tagliacarte_core::protocol::imap::{
    BodyFetchPlan, DisplayFetch, ImapClientError, ImapStore, Lease, LiteralSink, MailBodySession, Reads,
    plan_body_fetch,
};
use tagliacarte_core::store::{Address, Envelope, message_for_display_from_raw};

use crate::frb_api::resolve_mail_account;
use crate::mail_html::{StreamingCidRewriter, escape_html_text, load_message_raw};
use crate::mail_store::{DynStore, open_cached_store};
use crate::remote_rewrite::RemoteRewriter;

/// Which message: an account, a folder in it, a message id in the folder
/// (a numeric UID for IMAP).
#[derive(Debug, Clone)]
pub struct MessageRef {
    pub account_id: String,
    pub folder: String,
    pub message_id: String,
}

impl MessageRef {
    /// The URL prefix under which a UI serves this message's parts; `cid:`
    /// references in the body are rewritten to `{prefix}/cid/{id}`.
    pub fn url_prefix(&self) -> String {
        let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
        format!("/mail/{}/{}/{}", enc(&self.account_id), enc(&self.folder), enc(&self.message_id))
    }
}

/// A finished resource.
pub struct ViewResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    /// For a message body: how many remote images (and other remote pictures)
    /// it refers to, whether they were allowed or not. A UI offers to load them
    /// only when there are some.
    pub remote_images: usize,
}

/// One address, split for display and for replying.
#[derive(Debug, Clone)]
pub struct Mailbox {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, Default)]
pub struct Headers {
    pub from: Vec<Mailbox>,
    pub to: Vec<Mailbox>,
    pub cc: Vec<Mailbox>,
    pub subject: Option<String>,
    pub date_ms: Option<i64>,
    pub message_id: Option<String>,
    /// The `References` header as written, for threading a reply.
    pub references: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    /// Position in the message's attachment list.
    pub index: u32,
    pub filename: Option<String>,
    pub content_type: String,
    pub size: u64,
    /// Present for parts the HTML body refers to as `cid:`.
    pub content_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum OpenEvent {
    Headers(Headers),
    Attachment(AttachmentInfo),
    /// What kind of body the message has, so that a UI can offer to load its
    /// remote images only when there is HTML to hold them.
    Body { html: bool },
    /// The message is signed and this is whether its signature checks out against
    /// the sender's key in the address book: `valid`, `invalid` or `unknown`. Not
    /// sent for a message that is not signed or whose sender has no key.
    Signature(String),
}

// ---------------------------------------------------------------------------
// Completion plumbing

struct Failure {
    status: u16,
    message: String,
}

impl Failure {
    fn new(status: u16, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }
}

/// A completion that many callback paths may reach for but only the first
/// uses; clones share it.
struct Once<T>(Arc<Mutex<Option<Box<dyn FnOnce(Result<T, Failure>) + Send>>>>);

impl<T> Clone for Once<T> {
    fn clone(&self) -> Self {
        Once(Arc::clone(&self.0))
    }
}

impl<T> Once<T> {
    fn new(f: impl FnOnce(Result<T, Failure>) + Send + 'static) -> Self {
        Once(Arc::new(Mutex::new(Some(Box::new(f)))))
    }

    fn finish(&self, r: Result<T, Failure>) {
        let f = self.0.lock().expect("once lock").take();
        if let Some(f) = f {
            f(r);
        }
    }
}

fn error_response(f: Failure) -> ViewResponse {
    ViewResponse {
        status: f.status,
        content_type: "text/plain; charset=utf-8".to_owned(),
        body: f.message.into_bytes(),
        remote_images: 0,
    }
}

// ---------------------------------------------------------------------------
// Where a message lives

enum Source {
    Local(DynStore),
    Imap { session: MailBodySession, uid: u32 },
}

/// The UID in an IMAP message id: the number itself, or the last segment of an
/// `imap(s)://…/{uid}` id.
fn imap_uid(message_id: &str) -> Option<u32> {
    let t = message_id.trim();
    let tail = match t.strip_prefix("imap://").or_else(|| t.strip_prefix("imaps://")) {
        Some(rest) => rest.rsplit('/').next().unwrap_or(""),
        None => t,
    };
    // `parse` takes a leading `+`; a UID is digits only.
    if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    tail.parse().ok()
}

fn open(msg: &MessageRef) -> Result<Source, Failure> {
    let (acc, use_keychain) =
        resolve_mail_account(msg.account_id.trim()).map_err(|e| Failure::new(404, e))?;
    set_credentials_backend(use_keychain);
    let store = open_cached_store(&acc, use_keychain)
        .map_err(|e| Failure::new(500, format!("open store: {e}")))?;
    let session = match store.as_ref().as_any().downcast_ref::<ImapStore>() {
        Some(imap) => Some(imap.mail_body_session().map_err(|e| Failure::new(500, e.to_string()))?),
        None => None,
    };
    match session {
        Some(session) => {
            let uid = imap_uid(&msg.message_id)
                .ok_or_else(|| Failure::new(400, "an IMAP message id must be a numeric UID"))?;
            Ok(Source::Imap { session, uid })
        }
        None => Ok(Source::Local(store)),
    }
}

/// SELECT, then the BODYSTRUCTURE line, with the session leased until the
/// plan's consumer lets the lease go.
fn with_plan<T: 'static>(
    session: MailBodySession,
    folder: &str,
    uid: u32,
    once: Once<T>,
    then: Box<dyn FnOnce(MailBodySession, BodyFetchPlan, Lease, Once<T>) + Send>,
) {
    let folder = folder.to_owned();
    let s = session.clone();
    session.acquire(Box::new(move |lease| {
        let s2 = s.clone();
        s.select(
            &folder,
            Box::new(move |r| {
                if let Err(e) = r {
                    drop(lease);
                    return once.finish(Err(Failure::new(500, e.to_string())));
                }
                let s3 = s2.clone();
                s2.fetch_bodystructure(
                    uid,
                    Box::new(move |r| match r {
                        Ok(bs) => then(s3, plan_body_fetch(&bs), lease, once),
                        Err(e) => {
                            drop(lease);
                            once.finish(Err(Failure::new(500, e.to_string())));
                        }
                    }),
                );
            }),
        );
    }));
}

// ---------------------------------------------------------------------------
// HTML body

/// Starts every body document: follow the system light or dark scheme, keep
/// images inside the pane, and send every link to a new window, where the
/// shell can intercept it (a link that navigated the frame itself would be
/// blocked silently by the page's frame-src). The message's own HTML follows.
const PREAMBLE: &str = "<!doctype html><meta charset=\"utf-8\">\
    <base target=\"_blank\">\
    <meta name=\"color-scheme\" content=\"light dark\">\
    <style>html,body{background:Canvas;color:CanvasText;font:15px system-ui,sans-serif;\
    margin:8px;overflow-wrap:anywhere}a{color:LinkText}img{max-width:100%;height:auto}</style>";

/// One line for the terminal when remote images were allowed, so that "nothing
/// to load" can be told from "loading failed" (see the proxy's own log lines).
fn log_rewritten(n: usize) {
    eprintln!("tagliacarte: remote images allowed: {n} address(es) pointed at the proxy");
}

fn html_response(body: Vec<u8>, remote_images: usize) -> ViewResponse {
    ViewResponse { status: 200, content_type: "text/html; charset=utf-8".to_owned(), body, remote_images }
}

fn document(html: &str, prefix: &str, images: Option<&str>) -> ViewResponse {
    let mut rw = StreamingCidRewriter::new(prefix.to_owned());
    let mut inline = rw.feed(html);
    inline.push_str(&rw.finish());
    let mut out = String::from(PREAMBLE);
    // Always scanned, to count what is there; its output is used only once the
    // images are allowed.
    let mut remote = RemoteRewriter::new(images.unwrap_or_default().to_owned());
    let rewritten = remote.feed(&inline);
    let count = remote.rewritten();
    match images {
        Some(_) => {
            log_rewritten(count);
            out.push_str(&rewritten);
            out.push_str(&remote.finish());
        }
        None => out.push_str(&inline),
    }
    html_response(out.into_bytes(), count)
}

fn plain_as_html(plain: &str) -> String {
    format!("<pre>{}</pre>", escape_html_text(plain))
}

/// The HTML document for a message, with `cid:` references pointing at the
/// message's own `/cid/` URLs.
///
/// `images` is `None` while remote images are blocked; the HTML then keeps its
/// remote references, which the body's content security policy refuses. With
/// `Some(prefix)` each remote image reference is pointed at the proxy at
/// `prefix` (see `remote_rewrite`).
pub fn serve_body(msg: MessageRef, images: Option<String>, done: Box<dyn FnOnce(ViewResponse) + Send>) {
    let once = Once::new(move |r: Result<ViewResponse, Failure>| {
        done(r.unwrap_or_else(error_response))
    });
    let prefix = msg.url_prefix();
    match open(&msg) {
        Err(f) => once.finish(Err(f)),
        Ok(Source::Local(store)) => {
            load_message_raw(
                store,
                &msg.folder,
                &msg.message_id,
                Box::new(move |r| match r {
                    Err(e) => once.finish(Err(Failure::new(500, e))),
                    Ok(raw) => {
                        let d = message_for_display_from_raw(&raw);
                        let html = d
                            .body_html
                            .or_else(|| d.body_plain.map(|p| plain_as_html(&p)))
                            .unwrap_or_else(|| "<pre>(no displayable body)</pre>".to_owned());
                        once.finish(Ok(document(&html, &prefix, images.as_deref())));
                    }
                }),
            );
        }
        Ok(Source::Imap { session, uid }) => {
            with_plan(
                session,
                &msg.folder,
                uid,
                once,
                Box::new(move |session, plan, lease, once| imap_body(session, uid, plan, lease, once, prefix, images)),
            );
        }
    }
}

fn imap_body(
    session: MailBodySession,
    uid: u32,
    plan: BodyFetchPlan,
    lease: Lease,
    once: Once<ViewResponse>,
    prefix: String,
    images: Option<String>,
) {
    match plan.display {
        DisplayFetch::TextPart { section, encoding, is_html, charset_hint: _ } => {
            let mut out = Vec::from(PREAMBLE.as_bytes());
            if !is_html {
                out.extend_from_slice(b"<pre>");
            }
            session.fetch_body_peek_section(
                uid,
                &section,
                Box::new(TextPartSink {
                    once,
                    out,
                    cte: StreamingCteDecoder::for_encoding(&encoding),
                    utf8: Utf8StreamAssembler::default(),
                    cid: if is_html { Some(StreamingCidRewriter::new(prefix)) } else { None },
                    remote: is_html.then(|| RemoteRewriter::new(images.clone().unwrap_or_default())),
                    proxy_images: images.is_some(),
                    acc: Vec::new(),
                    _lease: lease,
                }),
            );
        }
        DisplayFetch::NestedMessage { section, encoding } => {
            // A nested message/rfc822 needs the whole part to MIME-parse its inner envelope.
            session.fetch_body_peek_section(
                uid,
                &section,
                Box::new(Collect {
                    once,
                    body: Vec::new(),
                    lease: Some(lease),
                    then: Some(Box::new(move |body, once, _lease| {
                        let mut cte = StreamingCteDecoder::for_encoding(&encoding);
                        let mut decoded = Vec::new();
                        cte.feed(&body, &mut decoded);
                        cte.finish(&mut decoded);
                        match extract_structured_body(&decoded) {
                            Ok((plain, html, _)) => {
                                let html = html.or(plain).unwrap_or_else(|| "<pre>(empty)</pre>".to_owned());
                                once.finish(Ok(document(&html, &prefix, images.as_deref())));
                            }
                            Err(e) => once.finish(Err(Failure::new(500, e.to_string()))),
                        }
                    })),
                }),
            );
        }
        DisplayFetch::None => {
            session.fetch_body_by_uid(
                uid,
                Box::new(Collect {
                    once,
                    body: Vec::new(),
                    lease: Some(lease),
                    then: Some(Box::new(move |body, once, _lease| {
                        let mut cte = StreamingCteDecoder::for_encoding("8BIT");
                        let mut decoded = Vec::new();
                        cte.feed(&body, &mut decoded);
                        cte.finish(&mut decoded);
                        let d = message_for_display_from_raw(&decoded);
                        let html = d
                            .body_html
                            .or_else(|| d.body_plain.map(|p| plain_as_html(&p)))
                            .unwrap_or_else(|| "<pre>(no displayable body)</pre>".to_owned());
                        once.finish(Ok(document(&html, &prefix, images.as_deref())));
                    })),
                }),
            );
        }
    }
}

/// A text part, decoded as the literal arrives: transfer decoding, UTF-8
/// assembly, then HTML escaping (plain) or `cid:` rewriting (HTML). Memory
/// beyond the output is one chunk.
struct TextPartSink {
    once: Once<ViewResponse>,
    out: Vec<u8>,
    cte: StreamingCteDecoder,
    utf8: Utf8StreamAssembler,
    cid: Option<StreamingCidRewriter>,
    /// Finds remote images; its output is used once the user has allowed them.
    remote: Option<RemoteRewriter>,
    /// Whether the images are allowed, so that the rewritten text is what is sent.
    proxy_images: bool,
    acc: Vec<u8>,
    _lease: Lease,
}

impl TextPartSink {
    fn render(&mut self, text: &str) -> String {
        let inline = match self.cid.as_mut() {
            Some(cid) => cid.feed(text),
            None => return escape_html_text(text),
        };
        self.through_remote(inline)
    }

    fn through_remote(&mut self, inline: String) -> String {
        match self.remote.as_mut() {
            Some(r) => {
                let rewritten = r.feed(&inline);
                if self.proxy_images { rewritten } else { inline }
            }
            None => inline,
        }
    }
}

impl LiteralSink for TextPartSink {
    fn chunk(&mut self, data: &[u8], _reads: &mut Reads<'_>) {
        self.cte.feed(data, &mut self.acc);
        let text = self.utf8.push_decode(&self.acc);
        self.acc.clear();
        let rendered = self.render(&text);
        self.out.extend_from_slice(rendered.as_bytes());
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        if let Err(e) = r {
            return self.once.finish(Err(Failure::new(502, e.to_string())));
        }
        self.cte.finish(&mut self.acc);
        let tail = self.utf8.push_decode(&self.acc);
        self.acc.clear();
        let mut remote_images = 0;
        let mut rest = self.render(&tail);
        let flushed = self.utf8.flush_utf8_lossy();
        rest.push_str(&self.render(&flushed));
        match self.cid.take() {
            Some(cid) => {
                let tail = self.through_remote(cid.finish());
                rest.push_str(&tail);
                if let Some(r) = self.remote.take() {
                    remote_images = r.rewritten();
                    if self.proxy_images {
                        log_rewritten(remote_images);
                        rest.push_str(&r.finish());
                    }
                }
            }
            None => rest.push_str("</pre>"),
        }
        self.out.extend_from_slice(rest.as_bytes());
        self.once.finish(Ok(html_response(std::mem::take(&mut self.out), remote_images)));
    }
}

/// Collects a whole literal, then hands it on.
struct Collect<T> {
    once: Once<T>,
    body: Vec<u8>,
    lease: Option<Lease>,
    /// Gets the literal, the completion, and the lease (to keep, for a second
    /// fetch on the same session, or to drop).
    then: Option<Box<dyn FnOnce(Vec<u8>, Once<T>, Lease) + Send>>,
}

impl<T: Send + 'static> LiteralSink for Collect<T> {
    fn chunk(&mut self, data: &[u8], _reads: &mut Reads<'_>) {
        self.body.extend_from_slice(data);
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        if let Err(e) = r {
            return self.once.finish(Err(Failure::new(502, e.to_string())));
        }
        if let (Some(then), Some(lease)) = (self.then.take(), self.lease.take()) {
            then(std::mem::take(&mut self.body), self.once.clone(), lease);
        }
    }
}

// ---------------------------------------------------------------------------
// Inline parts (`cid:`)

/// The bytes of the part whose Content-ID is `cid`.
pub fn serve_cid(msg: MessageRef, cid: String, done: Box<dyn FnOnce(ViewResponse) + Send>) {
    let once = Once::new(move |r: Result<ViewResponse, Failure>| {
        done(r.unwrap_or_else(error_response))
    });
    let wanted = cid.trim().trim_matches(|c| c == '<' || c == '>').to_owned();
    match open(&msg) {
        Err(f) => once.finish(Err(f)),
        Ok(Source::Local(store)) => {
            load_message_raw(
                store,
                &msg.folder,
                &msg.message_id,
                Box::new(move |r| {
                    let raw = match r {
                        Ok(b) => b,
                        Err(e) => return once.finish(Err(Failure::new(500, e))),
                    };
                    let parts = match extract_structured_body(&raw) {
                        Ok((_, _, p)) => p,
                        Err(e) => return once.finish(Err(Failure::new(500, format!("MIME parse: {e}")))),
                    };
                    let found = parts.into_iter().find(|(_, content_id, _, _)| {
                        content_id.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&wanted))
                    });
                    match found {
                        Some((_, _, content_type, body)) => {
                            once.finish(Ok(ViewResponse { status: 200, content_type, body, remote_images: 0 }))
                        }
                        None => once.finish(Err(Failure::new(404, "cid not found"))),
                    }
                }),
            );
        }
        Ok(Source::Imap { session, uid }) => {
            with_plan(
                session,
                &msg.folder,
                uid,
                once,
                Box::new(move |session, plan, lease, once| {
                    let part = plan
                        .inline_cids
                        .iter()
                        .find(|c| c.cid.eq_ignore_ascii_case(&wanted))
                        .map(|c| (c.section.clone(), c.encoding.clone(), c.content_type.clone()))
                        .or_else(|| {
                            plan.attachments
                                .iter()
                                .find(|a| a.content_id.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&wanted)))
                                .map(|a| (a.section.clone(), a.encoding.clone(), a.content_type.clone()))
                        });
                    let Some((section, encoding, content_type)) = part else {
                        drop(lease);
                        return once.finish(Err(Failure::new(404, "cid not found")));
                    };
                    session.fetch_body_peek_section(
                        uid,
                        &section,
                        Box::new(PartSink {
                            once,
                            content_type,
                            cte: StreamingCteDecoder::for_encoding(&encoding),
                            out: Vec::new(),
                            _lease: lease,
                        }),
                    );
                }),
            );
        }
    }
}

/// One part's decoded bytes.
struct PartSink {
    once: Once<ViewResponse>,
    content_type: String,
    cte: StreamingCteDecoder,
    out: Vec<u8>,
    _lease: Lease,
}

impl LiteralSink for PartSink {
    fn chunk(&mut self, data: &[u8], _reads: &mut Reads<'_>) {
        self.cte.feed(data, &mut self.out);
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        if let Err(e) = r {
            return self.once.finish(Err(Failure::new(502, e.to_string())));
        }
        self.cte.finish(&mut self.out);
        self.once.finish(Ok(ViewResponse {
            status: 200,
            content_type: std::mem::take(&mut self.content_type),
            body: std::mem::take(&mut self.out),
            remote_images: 0,
        }));
    }
}

// ---------------------------------------------------------------------------
// Envelope and attachments

fn mailbox(a: &Address) -> Mailbox {
    let email = match &a.domain {
        Some(d) => format!("{}@{}", a.local_part, d),
        None => a.local_part.clone(),
    };
    Mailbox { name: a.display_name.clone().filter(|n| !n.is_empty()), email }
}

fn headers_of(e: &Envelope) -> Headers {
    Headers {
        from: e.from.iter().map(mailbox).collect(),
        to: e.to.iter().map(mailbox).collect(),
        cc: e.cc.iter().map(mailbox).collect(),
        subject: e.subject.clone(),
        date_ms: e.date.as_ref().map(|d| d.timestamp.saturating_mul(1000)),
        message_id: e.message_id.clone(),
        references: e.references.clone(),
    }
}

/// The most attachment data worth fetching only to check a signature.
const MAX_SIGNED_FETCH: u64 = 25 * 1024 * 1024;

/// The signature verdict for `raw`, for a message from the first address of `envelope`.
fn signature_of(raw: &[u8], envelope: &Envelope) -> Option<String> {
    let sender = envelope.from.first().map(crate::contacts_crypto::normalize_email_addr)?;
    crate::mail_crypto::incoming_signature_status(raw, &sender)
}

/// Reports a message's envelope and attachments as events, then completes.
/// For IMAP this reads the BODYSTRUCTURE and the header block only; the body
/// is not fetched.
pub fn open_message(
    msg: MessageRef,
    on_event: Arc<dyn Fn(OpenEvent) + Send + Sync>,
    done: Box<dyn FnOnce(Result<(), String>) + Send>,
) {
    let once = Once::new(move |r: Result<(), Failure>| done(r.map_err(|f| f.message)));
    match open(&msg) {
        Err(f) => once.finish(Err(f)),
        Ok(Source::Local(store)) => {
            load_message_raw(
                store,
                &msg.folder,
                &msg.message_id,
                Box::new(move |r| match r {
                    Err(e) => once.finish(Err(Failure::new(500, e))),
                    Ok(raw) => {
                        let d = message_for_display_from_raw(&raw);
                        on_event(OpenEvent::Headers(headers_of(&d.envelope)));
                        on_event(OpenEvent::Body { html: d.body_html.is_some() });
                        if let Some(s) = signature_of(&raw, &d.envelope) {
                            on_event(OpenEvent::Signature(s));
                        }
                        for (i, a) in d.attachments.iter().enumerate() {
                            on_event(OpenEvent::Attachment(AttachmentInfo {
                                index: i as u32,
                                filename: a.filename.clone(),
                                content_type: a.content_type.clone(),
                                size: a.size_bytes,
                                content_id: a.content_id.clone(),
                            }));
                        }
                        once.finish(Ok(()));
                    }
                }),
            );
        }
        Ok(Source::Imap { session, uid }) => {
            with_plan(
                session,
                &msg.folder,
                uid,
                once,
                Box::new(move |session, plan, lease, once| {
                    // Checking a signature needs the whole message, so only a signed one is
                    // fetched whole, and not when it is too large to be worth it.
                    let check_signature = plan.signed && plan.attachments.iter().map(|a| a.size).sum::<u64>() <= MAX_SIGNED_FETCH;
                    let for_signature = session.clone();
                    on_event(OpenEvent::Body {
                        html: matches!(plan.display, DisplayFetch::TextPart { is_html: true, .. }),
                    });
                    for (i, a) in plan.attachments.iter().enumerate() {
                        on_event(OpenEvent::Attachment(AttachmentInfo {
                            index: i as u32,
                            filename: a.filename.clone(),
                            content_type: a.content_type.clone(),
                            size: a.size,
                            content_id: a.content_id.clone(),
                        }));
                    }
                    session.fetch_body_peek_section(
                        uid,
                        "HEADER",
                        Box::new(Collect {
                            once,
                            body: Vec::new(),
                                lease: Some(lease),
                            then: Some(Box::new(move |header, once, lease| {
                                let d = message_for_display_from_raw(&header);
                                on_event(OpenEvent::Headers(headers_of(&d.envelope)));
                                if !check_signature {
                                    return once.finish(Ok(()));
                                }
                                let envelope = d.envelope;
                                for_signature.fetch_body_by_uid(
                                    uid,
                                    Box::new(Collect {
                                        once,
                                        body: Vec::new(),
                                        lease: Some(lease),
                                        then: Some(Box::new(move |whole, once, _lease| {
                                            if let Some(s) = signature_of(&whole, &envelope) {
                                                on_event(OpenEvent::Signature(s));
                                            }
                                            once.finish(Ok(()));
                                        })),
                                    }),
                                );
                            })),
                        }),
                    );
                }),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The original of a reply or forward

/// What replying to or forwarding a message needs from it: its envelope and
/// its text, plain and/or HTML. Attachments are not read.
#[derive(Debug, Clone, Default)]
pub struct SourceMessage {
    pub headers: Headers,
    pub body_plain: Option<String>,
    pub body_html: Option<String>,
}

fn source_from_raw(raw: &[u8]) -> SourceMessage {
    let d = message_for_display_from_raw(raw);
    SourceMessage { headers: headers_of(&d.envelope), body_plain: d.body_plain, body_html: d.body_html }
}

/// A text part decoded as it arrives: transfer decoding, then UTF-8 assembly.
struct TextCollect<T> {
    once: Once<T>,
    cte: StreamingCteDecoder,
    utf8: Utf8StreamAssembler,
    acc: Vec<u8>,
    text: String,
    is_html: bool,
    headers: Headers,
    _lease: Lease,
}

impl LiteralSink for TextCollect<SourceMessage> {
    fn chunk(&mut self, data: &[u8], _reads: &mut Reads<'_>) {
        self.cte.feed(data, &mut self.acc);
        self.text.push_str(&self.utf8.push_decode(&self.acc));
        self.acc.clear();
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        if let Err(e) = r {
            return self.once.finish(Err(Failure::new(502, e.to_string())));
        }
        self.cte.finish(&mut self.acc);
        self.text.push_str(&self.utf8.push_decode(&self.acc));
        self.text.push_str(&self.utf8.flush_utf8_lossy());
        let text = std::mem::take(&mut self.text);
        let mut src = SourceMessage { headers: std::mem::take(&mut self.headers), ..SourceMessage::default() };
        if self.is_html {
            src.body_html = Some(text);
        } else {
            src.body_plain = Some(text);
        }
        self.once.finish(Ok(src));
    }
}

/// Reads the message a reply or forward is made from. For IMAP that is the
/// header block and the text part the reader would show; nothing else is
/// fetched, so a message with large attachments costs no more than one without.
pub fn load_source(msg: MessageRef, done: Box<dyn FnOnce(Result<SourceMessage, String>) + Send>) {
    let once = Once::new(move |r: Result<SourceMessage, Failure>| done(r.map_err(|f| f.message)));
    match open(&msg) {
        Err(f) => once.finish(Err(f)),
        Ok(Source::Local(store)) => {
            load_message_raw(
                store,
                &msg.folder,
                &msg.message_id,
                Box::new(move |r| match r {
                    Err(e) => once.finish(Err(Failure::new(500, e))),
                    Ok(raw) => once.finish(Ok(source_from_raw(&raw))),
                }),
            );
        }
        Ok(Source::Imap { session, uid }) => {
            with_plan(
                session,
                &msg.folder,
                uid,
                once,
                Box::new(move |session, plan, lease, once| {
                    let s2 = session.clone();
                    session.fetch_body_peek_section(
                        uid,
                        "HEADER",
                        Box::new(Collect {
                            once,
                            body: Vec::new(),
                            lease: Some(lease),
                            then: Some(Box::new(move |header, once, lease| {
                                let headers = headers_of(&message_for_display_from_raw(&header).envelope);
                                match plan.display {
                                    DisplayFetch::TextPart { section, encoding, is_html, .. } => {
                                        s2.fetch_body_peek_section(
                                            uid,
                                            &section,
                                            Box::new(TextCollect {
                                                once,
                                                cte: StreamingCteDecoder::for_encoding(&encoding),
                                                utf8: Utf8StreamAssembler::default(),
                                                acc: Vec::new(),
                                                text: String::new(),
                                                is_html,
                                                headers,
                                                _lease: lease,
                                            }),
                                        );
                                    }
                                    // A nested message, or none the plan could pick: take the
                                    // whole message and read the text from it.
                                    _ => {
                                        s2.fetch_body_by_uid(
                                            uid,
                                            Box::new(Collect {
                                                once,
                                                body: Vec::new(),
                                                lease: Some(lease),
                                                then: Some(Box::new(move |whole, once, _lease| {
                                                    let mut cte = StreamingCteDecoder::for_encoding("8BIT");
                                                    let mut decoded = Vec::new();
                                                    cte.feed(&whole, &mut decoded);
                                                    cte.finish(&mut decoded);
                                                    let mut src = source_from_raw(&decoded);
                                                    src.headers = headers;
                                                    once.finish(Ok(src));
                                                })),
                                            }),
                                        );
                                    }
                                }
                            })),
                        }),
                    );
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imap_uid_accepts_a_number_or_an_imap_uri() {
        assert_eq!(imap_uid("42"), Some(42));
        assert_eq!(imap_uid(" 42 "), Some(42));
        assert_eq!(imap_uid("imap://user_at_example.com/INBOX/42"), Some(42));
        assert_eq!(imap_uid("imaps://user_at_example.com/Archive/2026/17"), Some(17));
        // Not UIDs.
        assert_eq!(imap_uid(""), None);
        assert_eq!(imap_uid("imap://user_at_example.com/INBOX/"), None);
        assert_eq!(imap_uid("imap://user_at_example.com/INBOX/abc"), None);
        assert_eq!(imap_uid("maildir:///x/42"), None);
        assert_eq!(imap_uid("-1"), None);
    }
}

// ---------------------------------------------------------------------------
// Saving an attachment

/// Writes the decoded bytes of attachment `index` (its position in the list
/// `open_message` reported) to `out`, and reports how many bytes that was.
///
/// From IMAP the part is fetched on its own and decoded as each piece of the
/// literal arrives, so memory is one chunk however large the file. A local
/// store has to read the whole message to find the part.
pub fn save_attachment(
    msg: MessageRef,
    index: u32,
    out: Box<dyn Write + Send>,
    done: Box<dyn FnOnce(Result<u64, String>) + Send>,
) {
    let once = Once::new(move |r: Result<u64, Failure>| done(r.map_err(|f| f.message)));
    let missing = || Failure::new(404, "attachment not found");
    match open(&msg) {
        Err(f) => once.finish(Err(f)),
        Ok(Source::Local(store)) => {
            let mut out = out;
            load_message_raw(
                store,
                &msg.folder,
                &msg.message_id,
                Box::new(move |r| {
                    let raw = match r {
                        Ok(b) => b,
                        Err(e) => return once.finish(Err(Failure::new(500, e))),
                    };
                    let parts = match extract_structured_body(&raw) {
                        Ok((_, _, p)) => p,
                        Err(e) => return once.finish(Err(Failure::new(500, format!("MIME parse: {e}")))),
                    };
                    let Some((_, _, _, body)) = parts.into_iter().nth(index as usize) else {
                        return once.finish(Err(missing()));
                    };
                    match out.write_all(&body).and_then(|()| out.flush()) {
                        Ok(()) => once.finish(Ok(body.len() as u64)),
                        Err(e) => once.finish(Err(Failure::new(500, format!("write: {e}")))),
                    }
                }),
            );
        }
        Ok(Source::Imap { session, uid }) => {
            with_plan(
                session,
                &msg.folder,
                uid,
                once,
                Box::new(move |session, plan, lease, once| {
                    let Some(a) = plan.attachments.get(index as usize) else {
                        drop(lease);
                        return once.finish(Err(missing()));
                    };
                    session.fetch_body_peek_section(
                        uid,
                        &a.section,
                        Box::new(FileSink {
                            once,
                            cte: StreamingCteDecoder::for_encoding(&a.encoding),
                            out,
                            buf: Vec::new(),
                            written: 0,
                            error: None,
                            _lease: lease,
                        }),
                    );
                }),
            );
        }
    }
}

/// A part's decoded bytes, written as they arrive. After a write fails the rest
/// of the literal is read and dropped, and the failure is reported at the end.
struct FileSink {
    once: Once<u64>,
    cte: StreamingCteDecoder,
    out: Box<dyn Write + Send>,
    buf: Vec<u8>,
    written: u64,
    error: Option<std::io::Error>,
    _lease: Lease,
}

impl FileSink {
    fn drain(&mut self) {
        if self.error.is_none() {
            match self.out.write_all(&self.buf) {
                Ok(()) => self.written += self.buf.len() as u64,
                Err(e) => self.error = Some(e),
            }
        }
        self.buf.clear();
    }
}

impl LiteralSink for FileSink {
    fn chunk(&mut self, data: &[u8], _reads: &mut Reads<'_>) {
        self.cte.feed(data, &mut self.buf);
        self.drain();
    }

    fn done(&mut self, r: Result<(), ImapClientError>) {
        if let Err(e) = r {
            return self.once.finish(Err(Failure::new(502, e.to_string())));
        }
        self.cte.finish(&mut self.buf);
        self.drain();
        if self.error.is_none() {
            if let Err(e) = self.out.flush() {
                self.error = Some(e);
            }
        }
        self.once.finish(match self.error.take() {
            Some(e) => Err(Failure::new(500, format!("write: {e}"))),
            None => Ok(self.written),
        });
    }
}
