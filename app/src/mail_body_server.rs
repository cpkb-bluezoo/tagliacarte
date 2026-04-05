/*
 * mail_body_server.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Local mTLS HTTPS server for streaming mail HTML / cid parts to the WebView.
 */

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex};

use once_cell::sync::OnceCell;
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use tagliacarte_core::json::{JsonWriter, writer_into_string};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_rustls::server::TlsStream;

use tagliacarte_core::config::set_credentials_backend;
use tagliacarte_core::mime::{StreamingCteDecoder, Utf8StreamAssembler, extract_structured_body};
use tagliacarte_core::protocol::http::mail_view_server::{
    MtlsMaterial, ParsedRequest, read_http_request, write_chunk, write_chunk_end,
    write_response_bytes, write_response_head,
};
use tagliacarte_core::protocol::imap::{DisplayFetch, ImapClientError, ImapStore, plan_body_fetch};
use tagliacarte_core::store::message_for_display_from_raw;

use crate::frb_api::resolve_mail_account;
use crate::mail_store::{blocking_get_message_raw, mail_runtime_handle, open_cached_store, DynStore};

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
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let addr = std_listener.local_addr().map_err(|e| e.to_string())?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| e.to_string())?;
    let acceptor = mtls.tls_acceptor.clone();
    let registrations = Arc::new(StdMutex::new(HashMap::<String, StoreReg>::new()));
    let regs = registrations.clone();
    // `TcpListener::from_std` must run on the Tokio runtime (registers with its reactor).
    mail_runtime_handle().spawn(async move {
        let listener = match TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[mail body server] TcpListener::from_std: {e}");
                return;
            }
        };
        server_accept_loop(listener, acceptor, regs).await;
    });
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

pub fn mail_body_server_init_json(init: &MailBodyServerInit) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("baseUrl");
    w.write_string(&init.base_url);
    w.write_key("caCertPem");
    w.write_string(&init.ca_cert_pem);
    w.write_key("clientCertPem");
    w.write_string(&init.client_cert_pem);
    w.write_key("clientKeyPem");
    w.write_string(&init.client_key_pem);
    w.write_key("enforcesClientCert");
    w.write_bool(init.enforces_client_cert);
    w.write_end_object();
    writer_into_string(w)
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

async fn server_accept_loop(
    listener: TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
    registrations: Arc<StdMutex<HashMap<String, StoreReg>>>,
) {
    loop {
        let Ok((tcp, _)) = listener.accept().await else {
            continue;
        };
        let acceptor = acceptor.clone();
        let regs = registrations.clone();
        tokio::spawn(async move {
            let Ok(tls) = acceptor.accept(tcp).await else {
                return;
            };
            let mut tls = tls;
            loop {
                let Ok(opt) = read_http_request(&mut tls).await else {
                    break;
                };
                let Some(req) = opt else {
                    break;
                };
                let close = handle_one_request(&mut tls, &req, &regs)
                    .await
                    .unwrap_or(true);
                if close {
                    break;
                }
            }
        });
    }
}

fn header_value<'a>(req: &'a ParsedRequest, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

async fn handle_one_request(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    req: &ParsedRequest,
    registrations: &Arc<StdMutex<HashMap<String, StoreReg>>>,
) -> io::Result<bool> {
    if req.method != "GET" {
        write_response_bytes(
            tls,
            405,
            &[("Connection", "close")],
            b"Method Not Allowed\r\n",
        )
        .await?;
        return Ok(true);
    }
    if header_value(req, "host").is_none() {
        write_response_bytes(tls, 400, &[("Connection", "close")], b"Missing Host\r\n").await?;
        return Ok(true);
    }

    let path = req.target.split('?').next().unwrap_or(&req.target);
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() < 5 || parts[0] != "view" {
        write_response_bytes(tls, 404, &[("Connection", "close")], b"Not Found\r\n").await?;
        return Ok(true);
    }
    let store_key = parts[1].to_string();
    let folder_enc = parts[2];
    let message_id_decoded = percent_decode_str(parts[3])
        .decode_utf8_lossy()
        .into_owned();
    let reg = registrations
        .lock()
        .ok()
        .and_then(|g| g.get(&store_key).cloned());
    let Some(reg) = reg else {
        write_response_bytes(
            tls,
            404,
            &[("Connection", "close")],
            b"Unknown store key\r\n",
        )
        .await?;
        return Ok(true);
    };
    let folder = percent_decode_str(folder_enc)
        .decode_utf8_lossy()
        .into_owned();

    let conn_close = header_value(req, "connection")
        .map(|v| v.eq_ignore_ascii_case("close"))
        .unwrap_or(false);

    set_credentials_backend(reg.use_keychain);
    let store: DynStore = match resolve_mail_account(reg.account_id.trim()).and_then(|(acc, uk)| {
        set_credentials_backend(uk);
        open_cached_store(&acc, uk)
    }) {
        Ok(s) => s,
        Err(e) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("open store: {}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
    };

    if parts[4] == "body" {
        if let Some(imap) = store.as_ref().as_any().downcast_ref::<ImapStore>() {
            let uid: u32 = match message_id_decoded.parse() {
                Ok(u) => u,
                Err(_) => {
                    write_response_bytes(
                        tls,
                        400,
                        &[("Connection", "close")],
                        b"IMAP message id in path must be numeric UID\r\n",
                    )
                    .await?;
                    return Ok(true);
                }
            };
            serve_imap_body(tls, &store_key, imap, &folder, uid, &req.target, conn_close).await
        } else {
            serve_local_body(
                tls,
                &store_key,
                &reg,
                &folder,
                &message_id_decoded,
                &req.target,
                conn_close,
            )
            .await
        }
    } else if parts[4] == "cid" && parts.len() >= 6 {
        let cid_enc = parts[5];
        let cid = percent_decode_str(cid_enc).decode_utf8_lossy().into_owned();
        if let Some(imap) = store.as_ref().as_any().downcast_ref::<ImapStore>() {
            let uid: u32 = match message_id_decoded.parse() {
                Ok(u) => u,
                Err(_) => {
                    write_response_bytes(
                        tls,
                        400,
                        &[("Connection", "close")],
                        b"IMAP message id in path must be numeric UID\r\n",
                    )
                    .await?;
                    return Ok(true);
                }
            };
            serve_imap_cid(tls, imap, &folder, uid, &cid, conn_close).await
        } else {
            serve_local_cid(tls, &reg, &folder, &message_id_decoded, &cid, conn_close).await
        }
    } else {
        write_response_bytes(tls, 404, &[("Connection", "close")], b"Not Found\r\n").await?;
        Ok(true)
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
async fn write_html_mail_view_preamble(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    target: &str,
    conn_close: bool,
) -> io::Result<()> {
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
    write_response_head(tls, 200, &headers).await?;
    write_chunk(tls, style.as_bytes()).await?;
    Ok(())
}

async fn send_html_mail_view(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    mut html: String,
    local_view_prefix: &str,
    target: &str,
    conn_close: bool,
) -> io::Result<bool> {
    html = rewrite_cid_urls(&html, local_view_prefix);
    write_html_mail_view_preamble(tls, target, conn_close).await?;
    for chunk in html.as_bytes().chunks(16 * 1024) {
        write_chunk(tls, chunk).await?;
    }
    write_chunk_end(tls).await?;
    Ok(conn_close)
}

async fn serve_local_body(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    store_key: &str,
    reg: &StoreReg,
    folder: &str,
    message_id: &str,
    target: &str,
    conn_close: bool,
) -> io::Result<bool> {
    let reg = reg.clone();
    let folder_for_blocking = folder.to_string();
    let mid = message_id.to_string();
    let raw = match tokio::task::spawn_blocking(move || {
        let (acc, uk) = resolve_mail_account(reg.account_id.trim())?;
        blocking_get_message_raw(&acc, uk, &folder_for_blocking, &mid)
    })
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("{}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
        Err(e) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("task join: {}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
    };

    let disp = message_for_display_from_raw(&raw);
    let html = disp
        .body_html
        .or_else(|| {
            disp.body_plain
                .map(|p| format!("<pre>{}</pre>", escape_html_text(&p)))
        })
        .unwrap_or_else(|| "<pre>(no displayable body)</pre>".to_string());

    let port = GLOBAL.get().map(|g| g.addr.port()).unwrap_or(0);
    let folder_enc = utf8_percent_encode(folder, NON_ALPHANUMERIC).to_string();
    let msg_seg = utf8_percent_encode(message_id, NON_ALPHANUMERIC).to_string();
    let local_view_prefix = format!(
        "https://127.0.0.1:{}/view/{}/{}/{}",
        port, store_key, folder_enc, msg_seg
    );

    send_html_mail_view(tls, html, &local_view_prefix, target, conn_close).await
}

async fn serve_local_cid(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    reg: &StoreReg,
    folder: &str,
    message_id: &str,
    cid: &str,
    conn_close: bool,
) -> io::Result<bool> {
    let reg = reg.clone();
    let folder_owned = folder.to_string();
    let mid = message_id.to_string();
    let raw = match tokio::task::spawn_blocking(move || {
        let (acc, uk) = resolve_mail_account(reg.account_id.trim())?;
        blocking_get_message_raw(&acc, uk, &folder_owned, &mid)
    })
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("{}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
        Err(e) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("task join: {}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
    };

    let cid_norm = cid.trim().trim_matches(|c| c == '<' || c == '>');
    let parts = match extract_structured_body(&raw) {
        Ok((_, _, p)) => p,
        Err(e) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("MIME parse: {}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
    };

    let found = parts
        .into_iter()
        .find(|(_filename, content_id, _ct, _body)| {
            content_id
                .as_deref()
                .map(|x| x.eq_ignore_ascii_case(cid_norm))
                .unwrap_or(false)
        });

    let Some((_filename, _cid, ct, body)) = found else {
        write_response_bytes(tls, 404, &[("Connection", "close")], b"cid not found\r\n").await?;
        return Ok(true);
    };

    let len = body.len().to_string();
    let conn = if conn_close { "close" } else { "keep-alive" };
    let headers = [
        ("Content-Type", ct.as_str()),
        ("Content-Length", len.as_str()),
        ("Connection", conn),
    ];
    write_response_head(tls, 200, &headers).await?;
    tls.write_all(&body).await?;
    tls.flush().await?;
    Ok(conn_close)
}

fn imap_to_io(e: ImapClientError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

async fn serve_imap_body(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    store_key: &str,
    imap: &ImapStore,
    folder: &str,
    uid: u32,
    target: &str,
    conn_close: bool,
) -> io::Result<bool> {
    let port = GLOBAL.get().map(|g| g.addr.port()).unwrap_or(0);
    let folder_enc = utf8_percent_encode(folder, NON_ALPHANUMERIC).to_string();
    let msg_seg = utf8_percent_encode(&uid.to_string(), NON_ALPHANUMERIC).to_string();
    let local_view_prefix = format!(
        "https://127.0.0.1:{}/view/{}/{}/{}",
        port, store_key, folder_enc, msg_seg
    );

    let mut slot = imap
        .lock_mail_body_streaming_session()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    let session = slot
        .as_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "mail body session missing"))?;

    if let Err(e) = session.select(folder).await {
        write_response_bytes(
            tls,
            500,
            &[("Connection", "close")],
            format!("{}\r\n", e).as_bytes(),
        )
        .await?;
        return Ok(true);
    }
    let line = match session.fetch_bodystructure_line(uid).await {
        Ok(l) => l,
        Err(e) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("{}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
    };
    let plan = match plan_body_fetch(&line) {
        Some(p) => p,
        None => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                b"BODYSTRUCTURE plan failed\r\n",
            )
            .await?;
            return Ok(true);
        }
    };

    match plan.display {
        DisplayFetch::TextPart {
            section,
            encoding,
            is_html,
            charset_hint: _,
        } => {
            let mut state = match session.begin_fetch_body_peek_section(uid, &section).await {
                Ok(s) => s,
                Err(e) => {
                    write_response_bytes(
                        tls,
                        500,
                        &[("Connection", "close")],
                        format!("{}\r\n", e).as_bytes(),
                    )
                    .await?;
                    return Ok(true);
                }
            };

            if let Err(e) = write_html_mail_view_preamble(tls, target, conn_close).await {
                let _ = session.finish_streaming_fetch(state).await;
                return Err(e);
            }

            let mut cte = StreamingCteDecoder::for_encoding(&encoding);
            let mut utf8 = Utf8StreamAssembler::default();
            let mut cid_rewriter: Option<StreamingCidRewriter> = if is_html {
                Some(StreamingCidRewriter::new(local_view_prefix.clone()))
            } else {
                None
            };
            let mut raw_buf = vec![0u8; 8192];
            let mut decoded_acc = Vec::new();

            if !is_html {
                if let Err(e) = write_chunk(tls, b"<pre>").await {
                    let _ = session.finish_streaming_fetch(state).await;
                    return Err(e);
                }
            }

            while state.remaining > 0 {
                let n = match session
                    .read_streaming_literal_chunk(&mut raw_buf, &mut state)
                    .await
                {
                    Ok(n) => n,
                    Err(e) => {
                        let _ = session.finish_streaming_fetch(state).await;
                        return Err(imap_to_io(e));
                    }
                };
                if n == 0 {
                    break;
                }
                cte.feed(&raw_buf[..n], &mut decoded_acc);
                let text = utf8.push_decode(&decoded_acc);
                decoded_acc.clear();

                if is_html {
                    let html = cid_rewriter.as_mut().expect("cid rewriter").feed(&text);
                    if !html.is_empty() {
                        if let Err(e) = write_chunk(tls, html.as_bytes()).await {
                            let _ = session.finish_streaming_fetch(state).await;
                            return Err(e);
                        }
                    }
                } else {
                    let esc = escape_html_text(&text);
                    if !esc.is_empty() {
                        if let Err(e) = write_chunk(tls, esc.as_bytes()).await {
                            let _ = session.finish_streaming_fetch(state).await;
                            return Err(e);
                        }
                    }
                }
            }

            cte.finish(&mut decoded_acc);
            let tail = utf8.push_decode(&decoded_acc);
            decoded_acc.clear();

            if is_html {
                let html = cid_rewriter.as_mut().expect("cid rewriter").feed(&tail);
                if !html.is_empty() {
                    write_chunk(tls, html.as_bytes()).await?;
                }
                let tail2 = utf8.flush_utf8_lossy();
                let html2 = cid_rewriter.as_mut().expect("cid rewriter").feed(&tail2);
                if !html2.is_empty() {
                    write_chunk(tls, html2.as_bytes()).await?;
                }
                let html3 = cid_rewriter.take().expect("cid rewriter").finish();
                if !html3.is_empty() {
                    write_chunk(tls, html3.as_bytes()).await?;
                }
            } else {
                let esc = escape_html_text(&tail);
                if !esc.is_empty() {
                    write_chunk(tls, esc.as_bytes()).await?;
                }
                let tail2 = utf8.flush_utf8_lossy();
                let esc2 = escape_html_text(&tail2);
                if !esc2.is_empty() {
                    write_chunk(tls, esc2.as_bytes()).await?;
                }
                write_chunk(tls, b"</pre>").await?;
            }

            if let Err(e) = write_chunk_end(tls).await {
                let _ = session.finish_streaming_fetch(state).await;
                return Err(e);
            }
            session
                .finish_streaming_fetch(state)
                .await
                .map_err(imap_to_io)?;
            Ok(conn_close)
        }
        DisplayFetch::NestedMessage { section, encoding } => {
            // Nested message/rfc822: need full bytes to MIME-parse the inner envelope.
            let mut state = match session.begin_fetch_body_peek_section(uid, &section).await {
                Ok(s) => s,
                Err(e) => {
                    write_response_bytes(
                        tls,
                        500,
                        &[("Connection", "close")],
                        format!("{}\r\n", e).as_bytes(),
                    )
                    .await?;
                    return Ok(true);
                }
            };
            let mut raw_buf = vec![0u8; 8192];
            let mut body = Vec::new();
            while state.remaining > 0 {
                let n = session
                    .read_streaming_literal_chunk(&mut raw_buf, &mut state)
                    .await
                    .map_err(imap_to_io)?;
                body.extend_from_slice(&raw_buf[..n]);
            }
            session
                .finish_streaming_fetch(state)
                .await
                .map_err(imap_to_io)?;

            let mut cte = StreamingCteDecoder::for_encoding(&encoding);
            let mut decoded = Vec::new();
            cte.feed(&body, &mut decoded);
            cte.finish(&mut decoded);
            let (plain, html, _) = match extract_structured_body(&decoded) {
                Ok(x) => x,
                Err(e) => {
                    write_response_bytes(
                        tls,
                        500,
                        &[("Connection", "close")],
                        format!("{}\r\n", e).as_bytes(),
                    )
                    .await?;
                    return Ok(true);
                }
            };
            let html = html
                .or(plain)
                .unwrap_or_else(|| "<pre>(empty)</pre>".to_string());
            send_html_mail_view(tls, html, &local_view_prefix, target, conn_close).await
        }
        DisplayFetch::None => {
            let mut state = match session.begin_fetch_body_by_uid(uid).await {
                Ok(s) => s,
                Err(e) => {
                    write_response_bytes(
                        tls,
                        500,
                        &[("Connection", "close")],
                        format!("{}\r\n", e).as_bytes(),
                    )
                    .await?;
                    return Ok(true);
                }
            };
            let mut raw_buf = vec![0u8; 8192];
            let mut body = Vec::new();
            while state.remaining > 0 {
                let n = session
                    .read_streaming_literal_chunk(&mut raw_buf, &mut state)
                    .await
                    .map_err(imap_to_io)?;
                body.extend_from_slice(&raw_buf[..n]);
            }
            session
                .finish_streaming_fetch(state)
                .await
                .map_err(imap_to_io)?;

            let mut cte = StreamingCteDecoder::for_encoding("8BIT");
            let mut decoded = Vec::new();
            cte.feed(&body, &mut decoded);
            cte.finish(&mut decoded);
            let disp = message_for_display_from_raw(&decoded);
            let html = disp
                .body_html
                .or_else(|| {
                    disp.body_plain
                        .map(|p| format!("<pre>{}</pre>", escape_html_text(&p)))
                })
                .unwrap_or_else(|| "<pre>(no displayable body)</pre>".to_string());
            send_html_mail_view(tls, html, &local_view_prefix, target, conn_close).await
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

async fn serve_imap_cid(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    imap: &ImapStore,
    folder: &str,
    uid: u32,
    cid: &str,
    conn_close: bool,
) -> io::Result<bool> {
    let cid_norm = cid.trim().trim_matches(|c| c == '<' || c == '>');

    let mut slot = imap
        .lock_mail_body_streaming_session()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    let session = slot
        .as_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "mail body session missing"))?;

    if let Err(e) = session.select(folder).await {
        write_response_bytes(
            tls,
            500,
            &[("Connection", "close")],
            format!("{}\r\n", e).as_bytes(),
        )
        .await?;
        return Ok(true);
    }
    let line = match session.fetch_bodystructure_line(uid).await {
        Ok(l) => l,
        Err(e) => {
            write_response_bytes(
                tls,
                500,
                &[("Connection", "close")],
                format!("{}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
    };
    let plan = match plan_body_fetch(&line) {
        Some(p) => p,
        None => {
            write_response_bytes(tls, 404, &[("Connection", "close")], b"plan failed\r\n").await?;
            return Ok(true);
        }
    };

    let (section, encoding, ct) = if let Some(c) = plan
        .inline_cids
        .iter()
        .find(|c| c.cid.eq_ignore_ascii_case(cid_norm))
    {
        (
            c.section.clone(),
            c.encoding.clone(),
            c.content_type.clone(),
        )
    } else if let Some(a) = plan.attachments.iter().find(|a| {
        a.content_id
            .as_deref()
            .map(|x| x.eq_ignore_ascii_case(cid_norm))
            .unwrap_or(false)
    }) {
        (
            a.section.clone(),
            a.encoding.clone(),
            a.content_type.clone(),
        )
    } else {
        write_response_bytes(tls, 404, &[("Connection", "close")], b"cid not found\r\n").await?;
        return Ok(true);
    };

    let mut state = match session.begin_fetch_body_peek_section(uid, &section).await {
        Ok(s) => s,
        Err(e) => {
            write_response_bytes(
                tls,
                404,
                &[("Connection", "close")],
                format!("{}\r\n", e).as_bytes(),
            )
            .await?;
            return Ok(true);
        }
    };

    let conn = if conn_close { "close" } else { "keep-alive" };
    let headers = [
        ("Content-Type", ct.as_str()),
        ("Transfer-Encoding", "chunked"),
        ("Connection", conn),
    ];
    if let Err(e) = write_response_head(tls, 200, &headers).await {
        let _ = session.finish_streaming_fetch(state).await;
        return Err(e);
    }

    let mut cte = StreamingCteDecoder::for_encoding(&encoding);
    let mut raw_buf = vec![0u8; 8192];
    let mut decoded = Vec::new();

    while state.remaining > 0 {
        let n = match session
            .read_streaming_literal_chunk(&mut raw_buf, &mut state)
            .await
        {
            Ok(n) => n,
            Err(e) => {
                let _ = session.finish_streaming_fetch(state).await;
                return Err(imap_to_io(e));
            }
        };
        if n == 0 {
            break;
        }
        decoded.clear();
        cte.feed(&raw_buf[..n], &mut decoded);
        if !decoded.is_empty() {
            if let Err(e) = write_chunk(tls, &decoded).await {
                let _ = session.finish_streaming_fetch(state).await;
                return Err(e);
            }
        }
    }

    decoded.clear();
    cte.finish(&mut decoded);
    if !decoded.is_empty() {
        write_chunk(tls, &decoded).await?;
    }
    if let Err(e) = write_chunk_end(tls).await {
        let _ = session.finish_streaming_fetch(state).await;
        return Err(e);
    }
    session
        .finish_streaming_fetch(state)
        .await
        .map_err(imap_to_io)?;
    Ok(conn_close)
}
