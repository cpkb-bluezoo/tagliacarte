/*
 * connection.rs
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
//! Graph API pipeline connection on hopf.
//!
//! One persistent HTTPS connection to `graph.microsoft.com`, dialled on
//! first use and redialled when it drops. Commands queue behind each other
//! and run one at a time; every HTTP request is one step, and a step that
//! fails because the connection went away is retried once on a fresh
//! connection. Nothing here blocks: each step continues from the response
//! handler on hopf's reactor thread.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bytes::BytesMut;

use crate::json::{ContentHandler, Number, ParseResult, Parser};
use crate::protocol::http::api_trace::{
    log_http_error_response_body, log_http_response_size_only, log_http_stream_summary,
    log_inbound_body_chunk, log_inbound_body_complete_full, log_inbound_response_head,
};
use crate::protocol::http::client::HttpClient;
use crate::protocol::http::connection::HttpConnection;
use crate::protocol::http::{Method, RequestBuilder, Response, ResponseHandler};
use crate::store::{ConversationSummary, Envelope, FolderInfo, Message, StoreError};

use super::json_handlers::{
    FolderListHandler, GraphFolderEntry, MessageCountHandler, MessageListHandler,
    SingleMessageHandler,
};
use super::requests;

const GRAPH_HOST: &str = "graph.microsoft.com";
const GRAPH_PORT: u16 = 443;
const GRAPH_BASE_PATH: &str = "/v1.0";

type SharedNextLink = Arc<std::sync::Mutex<Option<String>>>;

/// Extract the path+query from a full `@odata.nextLink` URL.
/// e.g. `"https://graph.microsoft.com/v1.0/me/..."` -> `"/v1.0/me/..."`
fn extract_graph_path(url: &str) -> Option<String> {
    url.find(GRAPH_HOST)
        .map(|i| url[i + GRAPH_HOST.len()..].to_string())
}

// ── GraphCommand ──────────────────────────────────────────────────────

/// Commands sent from Store/Folder/Transport methods to the pipeline task.
/// Each variant carries the request parameters and callback(s).
pub enum GraphCommand {
    /// GET /me/mailFolders (with recursive child folder fetching)
    ListFolders {
        token: String,
        on_folder: Arc<dyn Fn(FolderInfo, GraphFolderEntry) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// GET /me/mailFolders/{id}?$select=totalItemCount
    MessageCount {
        token: String,
        folder_id: String,
        on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>,
    },
    /// GET /me/mailFolders/{id}/messages with paging
    ListMessages {
        token: String,
        folder_id: String,
        top: u64,
        skip: u64,
        on_summary: Arc<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// GET /me/messages/{id} (metadata) + GET /me/messages/{id}/$value (raw MIME)
    GetMessage {
        token: String,
        message_id: String,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Arc<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// DELETE /me/messages/{id}
    DeleteMessage {
        token: String,
        message_id: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// POST /me/mailFolders  (create folder)
    CreateFolder {
        token: String,
        name: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// PATCH /me/mailFolders/{id}  (rename folder)
    RenameFolder {
        token: String,
        folder_id: String,
        new_name: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// DELETE /me/mailFolders/{id}
    DeleteFolder {
        token: String,
        folder_id: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// POST /me/messages/{id}/copy
    CopyMessages {
        token: String,
        message_ids: Vec<String>,
        dest_folder_id: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// POST /me/messages/{id}/move
    MoveMessages {
        token: String,
        message_ids: Vec<String>,
        dest_folder_id: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// PATCH /me/messages/{id}  (store flags)
    StoreFlags {
        token: String,
        message_ids: Vec<String>,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// POST /me/sendMail (MIME mode: `text/plain` body = base64 RFC 822)
    SendMail {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
}

// ── Pipeline ──────────────────────────────────────────────────────────

/// Hears one outcome.
type Done<T> = Box<dyn FnOnce(Result<T, StoreError>) + Send>;
/// Issues one request on a connection; called again on a fresh
/// connection when the first attempt lost its connection.
type Start<T> = Arc<dyn Fn(&HttpConnection, Done<T>) + Send + Sync>;
/// What runs after a step, with the pipeline to continue on.
type Then<T> = Box<dyn FnOnce(Pipe, Result<T, StoreError>) + Send>;
type Pipe = Arc<Mutex<Pipeline>>;

struct Pipeline {
    conn: Option<HttpConnection>,
    busy: bool,
    queue: VecDeque<GraphCommand>,
}

/// Handle to the Graph pipeline. Cheaply cloneable.
#[derive(Clone)]
pub struct GraphConnection {
    pipe: Pipe,
}

impl Default for GraphConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphConnection {
    /// A pipeline with no connection yet; the first command dials.
    pub fn new() -> Self {
        Self { pipe: Arc::new(Mutex::new(Pipeline { conn: None, busy: false, queue: VecDeque::new() })) }
    }

    /// Queue a command for the pipeline. Returns immediately.
    pub fn send(&self, cmd: GraphCommand) {
        self.pipe.lock().unwrap().queue.push_back(cmd);
        run_next(Arc::clone(&self.pipe));
    }
}

fn run_next(pipe: Pipe) {
    let cmd = {
        let mut g = pipe.lock().unwrap();
        if g.busy {
            return;
        }
        match g.queue.pop_front() {
            Some(c) => {
                g.busy = true;
                c
            }
            None => return,
        }
    };
    execute(pipe, cmd);
}

/// Report a command's outcome and move on to the next one.
fn finish<T>(pipe: Pipe, on_complete: Done<T>, r: Result<T, StoreError>) {
    on_complete(r);
    pipe.lock().unwrap().busy = false;
    run_next(pipe);
}

fn with_conn(pipe: Pipe, k: Box<dyn FnOnce(Pipe, Result<HttpConnection, StoreError>) + Send>) {
    let live = pipe.lock().unwrap().conn.clone().filter(|c| c.is_alive());
    match live {
        Some(c) => k(pipe, Ok(c)),
        None => dial(pipe, k),
    }
}

fn dial(pipe: Pipe, k: Box<dyn FnOnce(Pipe, Result<HttpConnection, StoreError>) + Send>) {
    if crate::trace::enabled("graph") {
        eprintln!("[graph trace] connecting to {}", GRAPH_HOST);
    }
    HttpClient::connect(
        GRAPH_HOST,
        GRAPH_PORT,
        true,
        Box::new(move |r| {
            let r = r.map_err(|e| StoreError::new(format!("Graph connect failed: {}", e)));
            if let Ok(c) = &r {
                pipe.lock().unwrap().conn = Some(c.clone());
                if crate::trace::enabled("graph") {
                    eprintln!("[graph trace] connected");
                }
            }
            k(pipe, r)
        }),
    );
}

fn is_connection_error(e: &StoreError) -> bool {
    let msg = e.to_string();
    if msg.contains("interrupted after") {
        // Part of a streamed body already reached the consumer: a retry
        // would deliver it twice.
        return false;
    }
    msg.contains("broken pipe")
        || msg.contains("connection reset")
        || msg.contains("connection closed")
        || msg.contains("UnexpectedEof")
}

/// One request as a pipeline step: on the live connection, dialled if
/// needed, and once more on a fresh one if the connection went away.
fn step<T: Send + 'static>(pipe: Pipe, start: Start<T>, then: Then<T>) {
    with_conn(
        pipe,
        Box::new(move |pipe, conn| match conn {
            Ok(c) => attempt(pipe, c, start, then, false),
            Err(e) => then(pipe, Err(e)),
        }),
    );
}

fn attempt<T: Send + 'static>(pipe: Pipe, conn: HttpConnection, start: Start<T>, then: Then<T>, retried: bool) {
    let again = Arc::clone(&start);
    let p = Arc::clone(&pipe);
    start(
        &conn,
        Box::new(move |r| match r {
            Err(e) if !retried && is_connection_error(&e) => {
                if crate::trace::enabled("graph") {
                    eprintln!("[graph trace] connection lost, reconnecting...");
                }
                p.lock().unwrap().conn = None;
                dial(
                    p,
                    Box::new(move |pipe, c| match c {
                        Ok(c) => attempt(pipe, c, again, then, true),
                        Err(e2) => then(pipe, Err(e2)),
                    }),
                );
            }
            r => then(p, r),
        }),
    );
}

/// A single-request command.
fn simple(pipe: Pipe, start: Start<()>, on_complete: Done<()>) {
    step(pipe, start, Box::new(|pipe, r| finish(pipe, on_complete, r)));
}

/// One request per item, in order; stops at the first failure.
fn each<X: Send + 'static>(
    pipe: Pipe,
    mut items: VecDeque<X>,
    make: Arc<dyn Fn(&X) -> Start<()> + Send + Sync>,
    on_complete: Done<()>,
) {
    let Some(item) = items.pop_front() else {
        return finish(pipe, on_complete, Ok(()));
    };
    let start = make(&item);
    step(
        pipe,
        start,
        Box::new(move |pipe, r| match r {
            Ok(()) => each(pipe, items, make, on_complete),
            Err(e) => finish(pipe, on_complete, Err(e)),
        }),
    );
}

fn execute(pipe: Pipe, cmd: GraphCommand) {
    match cmd {
        GraphCommand::ListFolders { token, on_folder, on_complete } => {
            let start = format!("{}/me/mailFolders?$top=100", GRAPH_BASE_PATH);
            folder_pages(pipe, token, start, String::new(), on_folder, VecDeque::new(), on_complete)
        }
        GraphCommand::MessageCount { token, folder_id, on_complete } => {
            let path = format!("{}/me/mailFolders/{}?$select=totalItemCount", GRAPH_BASE_PATH, folder_id);
            let count = Arc::new(std::sync::Mutex::new(0u64));
            let c2 = Arc::clone(&count);
            let start: Start<()> = Arc::new(move |conn, done| {
                let json = MessageCountHandler::new(Arc::clone(&c2));
                request_json(conn, build_get(conn, &path, &token), Box::new(json), "message count", done)
            });
            step(
                pipe,
                start,
                Box::new(move |pipe, r| {
                    let n = *count.lock().unwrap();
                    finish(pipe, on_complete, r.map(|()| n))
                }),
            )
        }
        GraphCommand::ListMessages { token, folder_id, top, skip, on_summary, on_complete } => {
            let page_size = top.min(100);
            let select = "id,subject,from,toRecipients,ccRecipients,receivedDateTime,isRead,isDraft,importance,internetMessageId";
            let path = format!(
                "{}/me/mailFolders/{}/messages?$top={}&$skip={}&$select={}&$orderby=receivedDateTime desc",
                GRAPH_BASE_PATH, folder_id, page_size, skip, select
            );
            if crate::trace::enabled("graph") {
                eprintln!(
                    "[graph trace] list messages: top={} skip={} folder_id={}",
                    top, skip, folder_id
                );
            }
            message_pages(pipe, token, path, top, 0, on_summary, on_complete)
        }
        GraphCommand::GetMessage { token, message_id, on_metadata, on_content_chunk, on_complete } => {
            get_message(pipe, token, message_id, Arc::from(on_metadata), on_content_chunk, on_complete)
        }
        GraphCommand::DeleteMessage { token, message_id, on_complete } => {
            let path = format!("{}/me/messages/{}", GRAPH_BASE_PATH, message_id);
            simple(
                pipe,
                Arc::new(move |conn, done| request_status(conn, build_delete(conn, &path, &token), "delete", done)),
                on_complete,
            )
        }
        GraphCommand::CreateFolder { token, name, on_complete } => {
            let body = requests::build_create_folder_body(&name);
            let path = format!("{}/me/mailFolders", GRAPH_BASE_PATH);
            simple(
                pipe,
                Arc::new(move |conn, done| request_status(conn, build_json_post(conn, &path, &token, &body), "POST", done)),
                on_complete,
            )
        }
        GraphCommand::RenameFolder { token, folder_id, new_name, on_complete } => {
            let body = requests::build_rename_folder_body(&new_name);
            let path = format!("{}/me/mailFolders/{}", GRAPH_BASE_PATH, folder_id);
            simple(
                pipe,
                Arc::new(move |conn, done| request_status(conn, build_json_patch(conn, &path, &token, &body), "PATCH", done)),
                on_complete,
            )
        }
        GraphCommand::DeleteFolder { token, folder_id, on_complete } => {
            let path = format!("{}/me/mailFolders/{}", GRAPH_BASE_PATH, folder_id);
            simple(
                pipe,
                Arc::new(move |conn, done| request_status(conn, build_delete(conn, &path, &token), "delete", done)),
                on_complete,
            )
        }
        GraphCommand::CopyMessages { token, message_ids, dest_folder_id, on_complete } => {
            copy_or_move(pipe, token, message_ids, dest_folder_id, "copy", on_complete)
        }
        GraphCommand::MoveMessages { token, message_ids, dest_folder_id, on_complete } => {
            copy_or_move(pipe, token, message_ids, dest_folder_id, "move", on_complete)
        }
        GraphCommand::StoreFlags { token, message_ids, body, on_complete } => {
            let token = Arc::new(token);
            let body = Arc::new(body);
            let make = Arc::new(move |msg_id: &String| -> Start<()> {
                let path = format!("{}/me/messages/{}", GRAPH_BASE_PATH, msg_id);
                let (token, body) = (Arc::clone(&token), Arc::clone(&body));
                Arc::new(move |conn, done| request_status(conn, build_json_patch(conn, &path, &token, &body), "PATCH", done))
            });
            each(pipe, message_ids.into(), make, on_complete)
        }
        GraphCommand::SendMail { token, body, on_complete } => {
            let path = format!("{}/me/sendMail", GRAPH_BASE_PATH);
            simple(
                pipe,
                Arc::new(move |conn, done| {
                    request_status(conn, build_text_plain_post(conn, &path, &token, &body), "sendMail POST", done)
                }),
                on_complete,
            )
        }
    }
}

fn copy_or_move(pipe: Pipe, token: String, message_ids: Vec<String>, dest_folder_id: String, verb: &'static str, on_complete: Done<()>) {
    let token = Arc::new(token);
    let body = Arc::new(requests::build_copy_move_body(&dest_folder_id));
    let make = Arc::new(move |msg_id: &String| -> Start<()> {
        let path = format!("{}/me/messages/{}/{}", GRAPH_BASE_PATH, msg_id, verb);
        let (token, body) = (Arc::clone(&token), Arc::clone(&body));
        Arc::new(move |conn, done| request_status(conn, build_json_post(conn, &path, &token, &body), "POST", done))
    });
    each(pipe, message_ids.into(), make, on_complete)
}

// ── HTTP request helpers ──────────────────────────────────────────────

/// Build a GET request with the Bearer token.
fn build_get(conn: &HttpConnection, path: &str, token: &str) -> RequestBuilder {
    let mut req = conn.request(Method::Get, path);
    req.header("Authorization", &format!("Bearer {}", token));
    req.set_api_outbound_trace("graph", "GET", path);
    req
}

/// Build a POST request with JSON body and the Bearer token.
fn build_json_post(conn: &HttpConnection, path: &str, token: &str, body: &[u8]) -> RequestBuilder {
    let mut req = conn.request(Method::Post, path);
    req.header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Content-Length", &body.len().to_string())
        .body(body.to_vec());
    req.set_api_outbound_trace("graph", "POST", path);
    req
}

/// Build a POST with `Content-Type: text/plain` (Graph `sendMail` MIME mode: base64 RFC 822 body).
fn build_text_plain_post(conn: &HttpConnection, path: &str, token: &str, body: &[u8]) -> RequestBuilder {
    let mut req = conn.request(Method::Post, path);
    req.header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", "text/plain")
        .header("Content-Length", &body.len().to_string())
        .body(body.to_vec());
    req.set_api_outbound_trace("graph", "POST", path);
    req
}

/// Build a PATCH request with JSON body and the Bearer token.
fn build_json_patch(conn: &HttpConnection, path: &str, token: &str, body: &[u8]) -> RequestBuilder {
    let mut req = conn.request(Method::Patch, path);
    req.header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Content-Length", &body.len().to_string())
        .body(body.to_vec());
    req.set_api_outbound_trace("graph", "PATCH", path);
    req
}

/// Build a DELETE request with the Bearer token.
fn build_delete(conn: &HttpConnection, path: &str, token: &str) -> RequestBuilder {
    let mut req = conn.request(Method::Delete, path);
    req.header("Authorization", &format!("Bearer {}", token));
    req.set_api_outbound_trace("graph", "DELETE", path);
    req
}

/// Send `req`, feeding a 2xx body to `json`; `done` hears Ok once the
/// response is fully read, or the Graph error.
fn request_json(conn: &HttpConnection, req: RequestBuilder, json: Box<dyn ContentHandler + Send>, context: &'static str, done: Done<()>) {
    let http_tr = Some((req.method.as_str().to_string(), req.path.clone()));
    conn.send(req, GraphResponseHandler::new(json, http_tr, context, done));
}

/// Send `req` where only the status (and any error body) matters.
fn request_status(conn: &HttpConnection, req: RequestBuilder, context: &'static str, done: Done<()>) {
    request_json(conn, req, Box::new(NoOpJsonHandler), context, done)
}

// ── Multi-step commands ───────────────────────────────────────────────

/// One page of a folder listing, then its next page, then the next folder
/// with children (breadth first), streaming each folder to `on_folder`.
fn folder_pages(
    pipe: Pipe,
    token: String,
    path: String,
    prefix: String,
    on_folder: Arc<dyn Fn(FolderInfo, GraphFolderEntry) + Send + Sync>,
    mut pending: VecDeque<(String, String)>,
    on_complete: Done<()>,
) {
    let entries: Arc<std::sync::Mutex<Vec<GraphFolderEntry>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let next_link: SharedNextLink = Arc::new(std::sync::Mutex::new(None));
    let (tok, p) = (token.clone(), path.clone());
    let (e2, n2) = (Arc::clone(&entries), Arc::clone(&next_link));
    let start: Start<()> = Arc::new(move |conn, done| {
        // A retry starts the page afresh.
        e2.lock().unwrap().clear();
        *n2.lock().unwrap() = None;
        let e3 = Arc::clone(&e2);
        let json = FolderListHandler::new(
            move |entry| {
                if let Ok(mut v) = e3.lock() {
                    v.push(entry);
                }
            },
            Arc::clone(&n2),
        );
        request_json(conn, build_get(conn, &p, &tok), Box::new(json), "list folders", done)
    });
    step(
        pipe,
        start,
        Box::new(move |pipe, r| {
            if let Err(e) = r {
                return finish(pipe, on_complete, Err(e));
            }
            let page_entries = entries.lock().unwrap().drain(..).collect::<Vec<_>>();
            for entry in page_entries {
                let full_name = if prefix.is_empty() {
                    entry.display_name.clone()
                } else {
                    format!("{}/{}", prefix, entry.display_name)
                };
                let info = FolderInfo {
                    name: full_name.clone(),
                    delimiter: Some('/'),
                    attributes: Vec::new(),
                };
                if entry.child_folder_count > 0 {
                    pending.push_back((entry.id.clone(), full_name.clone()));
                }
                on_folder(info, entry);
            }
            let next = next_link.lock().unwrap().take().and_then(|u| extract_graph_path(&u));
            match next {
                Some(n) => folder_pages(pipe, token, n, prefix, on_folder, pending, on_complete),
                None => match pending.pop_front() {
                    Some((folder_id, parent_path)) => {
                        let child_path = format!("{}/me/mailFolders/{}/childFolders?$top=100", GRAPH_BASE_PATH, folder_id);
                        folder_pages(pipe, token, child_path, parent_path, on_folder, pending, on_complete)
                    }
                    None => finish(pipe, on_complete, Ok(())),
                },
            }
        }),
    );
}

/// One page of a message listing, then the next until `top` are in.
fn message_pages(
    pipe: Pipe,
    token: String,
    path: String,
    top: u64,
    collected: u64,
    on_summary: Arc<dyn Fn(ConversationSummary) + Send + Sync>,
    on_complete: Done<()>,
) {
    let next_link: SharedNextLink = Arc::new(std::sync::Mutex::new(None));
    let page_count = Arc::new(std::sync::Mutex::new(0u64));
    let (tok, p) = (token.clone(), path.clone());
    let (n2, c2, s2) = (Arc::clone(&next_link), Arc::clone(&page_count), Arc::clone(&on_summary));
    let start: Start<()> = Arc::new(move |conn, done| {
        *n2.lock().unwrap() = None;
        *c2.lock().unwrap() = 0;
        let (c3, s3) = (Arc::clone(&c2), Arc::clone(&s2));
        let json = MessageListHandler::new(
            move |summary| {
                s3(summary);
                if let Ok(mut c) = c3.lock() {
                    *c += 1;
                }
            },
            Arc::clone(&n2),
        );
        request_json(conn, build_get(conn, &p, &tok), Box::new(json), "list messages", done)
    });
    step(
        pipe,
        start,
        Box::new(move |pipe, r| {
            if let Err(e) = r {
                if crate::trace::enabled("graph") {
                    eprintln!("[graph trace] list messages error: {}", e);
                }
                return finish(pipe, on_complete, Err(e));
            }
            let page = *page_count.lock().unwrap();
            if crate::trace::enabled("graph") {
                eprintln!(
                    "[graph trace] list messages page: {} messages (total so far: {})",
                    page,
                    collected + page
                );
            }
            let collected = collected + page;
            if collected >= top {
                return finish(pipe, on_complete, Ok(()));
            }
            let next = next_link.lock().unwrap().take().and_then(|u| extract_graph_path(&u));
            match next {
                Some(n) => message_pages(pipe, token, n, top, collected, on_summary, on_complete),
                None => finish(pipe, on_complete, Ok(())),
            }
        }),
    );
}

/// Envelope metadata (JSON), then the raw MIME (`$value`) streamed to
/// `on_content_chunk`, then a non-fatal mark-as-read.
fn get_message(
    pipe: Pipe,
    token: String,
    message_id: String,
    on_metadata: Arc<dyn Fn(Envelope) + Send + Sync>,
    on_content_chunk: Arc<dyn Fn(&[u8]) + Send + Sync>,
    on_complete: Done<()>,
) {
    if crate::trace::enabled("graph") {
        eprintln!("[graph trace] get message: fetching metadata for {}", message_id);
    }
    let select = "subject,from,toRecipients,ccRecipients,receivedDateTime,internetMessageId";
    let meta_path = format!("{}/me/messages/{}?$select={}", GRAPH_BASE_PATH, message_id, select);
    let result = Arc::new(std::sync::Mutex::new(None::<Message>));
    let (tok, r2) = (token.clone(), Arc::clone(&result));
    let start: Start<()> = Arc::new(move |conn, done| {
        let json = SingleMessageHandler::new(Arc::clone(&r2));
        request_json(conn, build_get(conn, &meta_path, &tok), Box::new(json), "get message metadata", done)
    });
    step(
        pipe,
        start,
        Box::new(move |pipe, r| {
            if let Err(e) = r {
                return finish(pipe, on_complete, Err(e));
            }
            if crate::trace::enabled("graph") {
                eprintln!("[graph trace] get message: metadata complete");
            }
            if let Some(msg) = result.lock().unwrap().take() {
                on_metadata(msg.envelope);
            }
            if crate::trace::enabled("graph") {
                eprintln!("[graph trace] get message: fetching MIME content");
            }
            let value_path = format!("{}/me/messages/{}/$value", GRAPH_BASE_PATH, message_id);
            let tok = token.clone();
            let start: Start<()> = Arc::new(move |conn, done| {
                let req = build_get(conn, &value_path, &tok);
                let http_tr = Some((req.method.as_str().to_string(), req.path.clone()));
                conn.send(req, MimeStreamHandler::new(Arc::clone(&on_content_chunk), http_tr, done));
            });
            step(
                pipe,
                start,
                Box::new(move |pipe, r| {
                    if let Err(e) = r {
                        return finish(pipe, on_complete, Err(e));
                    }
                    if crate::trace::enabled("graph") {
                        eprintln!("[graph trace] get message: MIME content complete; marking as read...");
                    }
                    let patch_path = format!("{}/me/messages/{}", GRAPH_BASE_PATH, message_id);
                    let start: Start<()> = Arc::new(move |conn, done| {
                        request_status(conn, build_json_patch(conn, &patch_path, &token, b"{\"isRead\":true}"), "mark as read", done)
                    });
                    step(
                        pipe,
                        start,
                        Box::new(move |pipe, r| {
                            if crate::trace::enabled("graph") {
                                match r {
                                    Ok(()) => eprintln!("[graph trace] mark-as-read complete"),
                                    Err(e) => eprintln!("[graph trace] mark-as-read failed: {}", e),
                                }
                            }
                            finish(pipe, on_complete, Ok(()))
                        }),
                    );
                }),
            );
        }),
    );
}

// ── Handler wrappers ──────────────────────────────────────────────────

/// Parsed Graph API error: HTTP status + structured code/message from JSON body.
struct GraphError {
    status: u16,
    code: String,
    message: String,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.code.is_empty() && self.message.is_empty() {
            write!(f, "HTTP {}", self.status)
        } else if self.code.is_empty() {
            write!(f, "HTTP {}: {}", self.status, self.message)
        } else {
            write!(f, "HTTP {} {}: {}", self.status, self.code, self.message)
        }
    }
}


/// Parsed error detail shared between GraphErrorJsonHandler and GraphResponseHandler.
type SharedErrorDetail = Arc<std::sync::Mutex<(String, String)>>;

/// ContentHandler that parses Graph API error responses:
/// `{"error": {"code": "...", "message": "..."}}`.
/// Writes parsed code and message to a shared Arc as they are encountered.
struct GraphErrorJsonHandler {
    depth: usize,
    in_error: bool,
    current_key: Option<String>,
    detail: SharedErrorDetail,
}

impl GraphErrorJsonHandler {
    fn new(detail: SharedErrorDetail) -> Self {
        Self {
            depth: 0,
            in_error: false,
            current_key: None,
            detail,
        }
    }
}

impl ContentHandler for GraphErrorJsonHandler {
    fn start_object(&mut self) -> ParseResult<()> {
        self.depth += 1;
        if self.depth == 2 && self.current_key.as_deref() == Some("error") {
            self.in_error = true;
        }
        Ok(())
    }

    fn end_object(&mut self) -> ParseResult<()> {
        if self.depth == 2 {
            self.in_error = false;
        }
        self.depth -= 1;
        self.current_key = None;
        Ok(())
    }

    fn start_array(&mut self) -> ParseResult<()> { Ok(()) }
    fn end_array(&mut self) -> ParseResult<()> { Ok(()) }

    fn key(&mut self, key: &str) -> ParseResult<()> {
        self.current_key = Some(key.to_string());
        Ok(())
    }

    fn string_value(&mut self, value: &str) -> ParseResult<()> {
        if self.in_error {
            if let Ok(mut d) = self.detail.lock() {
                match self.current_key.as_deref() {
                    Some("code") => d.0 = value.to_string(),
                    Some("message") => d.1 = value.to_string(),
                    _ => {}
                }
            }
        }
        self.current_key = None;
        Ok(())
    }

    fn number_value(&mut self, _number: &Number) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
    }
    fn boolean_value(&mut self, _value: bool) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
    }
    fn null_value(&mut self) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
    }
}

/// No-op ContentHandler for endpoints where we only care about the HTTP
/// status and error body (POST/PATCH/DELETE with empty or irrelevant success bodies).
struct NoOpJsonHandler;

impl ContentHandler for NoOpJsonHandler {
    fn start_object(&mut self) -> ParseResult<()> { Ok(()) }
    fn end_object(&mut self) -> ParseResult<()> { Ok(()) }
    fn start_array(&mut self) -> ParseResult<()> { Ok(()) }
    fn end_array(&mut self) -> ParseResult<()> { Ok(()) }
    fn key(&mut self, _key: &str) -> ParseResult<()> { Ok(()) }
    fn string_value(&mut self, _value: &str) -> ParseResult<()> { Ok(()) }
    fn number_value(&mut self, _number: &Number) -> ParseResult<()> { Ok(()) }
    fn boolean_value(&mut self, _value: bool) -> ParseResult<()> { Ok(()) }
    fn null_value(&mut self) -> ParseResult<()> { Ok(()) }
}

/// ResponseHandler that streams raw body bytes to a callback.
/// Used for `$value` endpoints that return raw MIME content.
struct MimeStreamHandler {
    on_chunk: Arc<dyn Fn(&[u8]) + Send + Sync>,
    is_error: bool,
    status_code: u16,
    streamed_ok_bytes: usize,
    err_body_preview: Vec<u8>,
    /// `(method, path)` for stderr trace lines.
    http_trace: Option<(String, String)>,
    response_headers: Vec<(String, String)>,
    inbound_chunk_seq: usize,
    done: Option<Done<()>>,
}

impl MimeStreamHandler {
    fn new(on_chunk: Arc<dyn Fn(&[u8]) + Send + Sync>, http_trace: Option<(String, String)>, done: Done<()>) -> Self {
        Self {
            on_chunk,
            is_error: false,
            status_code: 0,
            streamed_ok_bytes: 0,
            err_body_preview: Vec::new(),
            http_trace,
            response_headers: Vec::new(),
            inbound_chunk_seq: 0,
            done: Some(done),
        }
    }
}

impl ResponseHandler for MimeStreamHandler {
    fn ok(&mut self, response: Response) {
        self.status_code = response.code;
    }

    fn error(&mut self, response: Response) {
        self.is_error = true;
        self.status_code = response.code;
    }

    fn header(&mut self, name: &str, value: &str) {
        if self.response_headers.len() < 128 {
            self.response_headers
                .push((name.to_string(), value.to_string()));
        }
    }

    fn start_body(&mut self) {
        if let Some((ref m, ref p)) = self.http_trace {
            if crate::trace::enabled("graph") {
                log_inbound_response_head("graph", m, p, self.status_code, &self.response_headers);
            }
            self.response_headers.clear();
        }
    }

    fn body_chunk(&mut self, data: &[u8]) {
        if self.is_error {
            const CAP: usize = 65536;
            if self.err_body_preview.len() < CAP {
                let take = (CAP - self.err_body_preview.len()).min(data.len());
                self.err_body_preview.extend_from_slice(&data[..take]);
            }
        } else {
            (self.on_chunk)(data);
            self.streamed_ok_bytes += data.len();
            if let Some((ref meth, ref p)) = self.http_trace {
                if crate::trace::full_enabled("graph") {
                    log_inbound_body_chunk(
                        "graph",
                        meth,
                        p,
                        self.inbound_chunk_seq,
                        data,
                    );
                    self.inbound_chunk_seq += 1;
                }
            }
        }
    }

    fn end_body(&mut self) {
        if self.is_error {
            log_http_error_response_body("graph", self.status_code, &self.err_body_preview);
        } else if let Some((ref m, ref p)) = self.http_trace {
            if crate::trace::full_enabled("graph") {
                log_inbound_body_complete_full("graph", m, p, self.status_code, self.streamed_ok_bytes);
            } else {
                log_http_stream_summary("graph", self.status_code, self.streamed_ok_bytes);
            }
        } else {
            log_http_stream_summary("graph", self.status_code, self.streamed_ok_bytes);
        }
    }

    fn complete(&mut self) {
        if let Some((ref m, ref p)) = self.http_trace {
            if crate::trace::enabled("graph") && !self.response_headers.is_empty() {
                log_inbound_response_head("graph", m, p, self.status_code, &self.response_headers);
                self.response_headers.clear();
            }
        }
        let Some(done) = self.done.take() else { return };
        if self.is_error {
            let ge = GraphError { status: self.status_code, code: String::new(), message: format!("HTTP {}", self.status_code) };
            eprintln!("[graph] get message content error: {}", ge);
            done(Err(StoreError::new(format!("Graph get message content: {}", ge))))
        } else {
            done(Ok(()))
        }
    }

    fn failed(&mut self, error: &std::io::Error) {
        let Some(done) = self.done.take() else { return };
        if self.streamed_ok_bytes > 0 {
            done(Err(StoreError::new(format!(
                "Graph get message content interrupted after {} bytes: {}",
                self.streamed_ok_bytes, error
            ))))
        } else {
            done(Err(StoreError::new(format!("Graph get message content failed: {}", error))))
        }
    }
}

/// Streaming ResponseHandler that feeds body chunks into one rjsonparser `Parser`.
///
/// The active ContentHandler is selected based on the HTTP status code:
/// - 2xx: the endpoint-specific handler passed at construction
/// - 4xx/5xx: swapped to GraphErrorJsonHandler to parse the standard error body
///
/// Only one parser and one handler are active at any time. The outcome
/// reaches `done` once: Ok after a 2xx body is fully read, the parsed
/// Graph error otherwise.
struct GraphResponseHandler {
    status_code: u16,
    is_error: bool,
    parser: Parser<Box<dyn ContentHandler + Send>>,
    /// Error bodies only, for the trace line.
    buf: BytesMut,
    error: Option<GraphError>,
    err_detail: SharedErrorDetail,
    http_trace: Option<(String, String)>,
    response_headers: Vec<(String, String)>,
    inbound_chunk_seq: usize,
    body_total_len: usize,
    context: &'static str,
    done: Option<Done<()>>,
}

impl GraphResponseHandler {
    fn new(handler: Box<dyn ContentHandler + Send>, http_trace: Option<(String, String)>, context: &'static str, done: Done<()>) -> Self {
        Self {
            status_code: 0,
            is_error: false,
            parser: Parser::new(handler),
            buf: BytesMut::new(),
            error: None,
            err_detail: Arc::new(std::sync::Mutex::new((String::new(), String::new()))),
            http_trace,
            response_headers: Vec::new(),
            inbound_chunk_seq: 0,
            body_total_len: 0,
            context,
            done: Some(done),
        }
    }
}

impl ResponseHandler for GraphResponseHandler {
    fn ok(&mut self, response: Response) {
        self.status_code = response.code;
    }

    fn error(&mut self, response: Response) {
        self.status_code = response.code;
        self.is_error = true;
        self.parser = Parser::new(Box::new(GraphErrorJsonHandler::new(self.err_detail.clone())));
    }

    fn header(&mut self, name: &str, value: &str) {
        if self.response_headers.len() < 128 {
            self.response_headers
                .push((name.to_string(), value.to_string()));
        }
    }

    fn start_body(&mut self) {
        if let Some((ref m, ref p)) = self.http_trace {
            if crate::trace::enabled("graph") {
                log_inbound_response_head("graph", m, p, self.status_code, &self.response_headers);
            }
            self.response_headers.clear();
        }
    }

    fn body_chunk(&mut self, data: &[u8]) {
        self.body_total_len = self.body_total_len.saturating_add(data.len());
        if let Some((ref meth, ref p)) = self.http_trace {
            if crate::trace::full_enabled("graph") {
                log_inbound_body_chunk(
                    "graph",
                    meth,
                    p,
                    self.inbound_chunk_seq,
                    data,
                );
                self.inbound_chunk_seq += 1;
            }
        }
        if self.is_error {
            self.buf.extend_from_slice(data);
        }
        let mut chunk = data;
        let _ = self.parser.receive(&mut chunk);
    }

    fn end_body(&mut self) {
        let _ = self.parser.close();
        if self.is_error {
            let (code, message) = {
                let d = self.err_detail.lock().unwrap();
                (d.0.clone(), d.1.clone())
            };
            self.error = Some(GraphError { status: self.status_code, code, message });
            log_http_error_response_body("graph", self.status_code, &self.buf);
        } else if let Some((ref m, ref p)) = self.http_trace {
            if crate::trace::full_enabled("graph") {
                log_inbound_body_complete_full("graph", m, p, self.status_code, self.body_total_len);
            } else {
                log_http_response_size_only("graph", self.status_code, self.body_total_len);
            }
        }
    }

    fn complete(&mut self) {
        if let Some((ref m, ref p)) = self.http_trace {
            if crate::trace::enabled("graph") && !self.response_headers.is_empty() {
                log_inbound_response_head("graph", m, p, self.status_code, &self.response_headers);
                self.response_headers.clear();
            }
        }
        let Some(done) = self.done.take() else { return };
        // An error status whose body never came (no `end_body`) is still an error.
        if self.is_error && self.error.is_none() {
            self.error = Some(GraphError { status: self.status_code, code: String::new(), message: String::new() });
        }
        match self.error.take() {
            Some(ge) => {
                eprintln!("[graph] {} error: {}", self.context, ge);
                done(Err(StoreError::new(format!("Graph {}: {}", self.context, ge))))
            }
            None => done(Ok(())),
        }
    }

    fn failed(&mut self, error: &std::io::Error) {
        if let Some(done) = self.done.take() {
            done(Err(StoreError::new(format!("Graph {} failed: {}", self.context, error))));
        }
    }
}
