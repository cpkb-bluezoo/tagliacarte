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

//! Graph API pipeline connection.
//!
//! Mirrors the IMAP pipeline pattern: a single persistent HTTPS connection to
//! `graph.microsoft.com`, with commands queued via an `mpsc::UnboundedSender`.
//! A tokio::spawn'd loop processes them one at a time on the existing connection.

use std::collections::VecDeque;
use std::sync::Arc;

use bytes::BytesMut;
use tokio::sync::mpsc;

use crate::json::{JsonContentHandler, JsonParser};
use crate::protocol::http::client::HttpClient;
use crate::protocol::http::connection::HttpConnection;
use crate::protocol::http::{Method, RequestBuilder, Response, ResponseHandler};
use crate::store::{ConversationSummary, Envelope, FolderInfo, Message, StoreError};

use super::json_handlers::{
    FolderListHandler, GraphFolderEntry, MessageCountHandler,
    MessageListHandler, SingleMessageHandler,
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
    /// POST /me/sendMail
    SendMail {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
}

// ── GraphConnection ───────────────────────────────────────────────────

/// Handle to the Graph pipeline task. Cheaply cloneable.
#[derive(Clone)]
pub struct GraphConnection {
    command_tx: mpsc::UnboundedSender<GraphCommand>,
}

impl GraphConnection {
    /// Queue a command for the pipeline. Returns immediately (fire-and-forget).
    pub fn send(&self, cmd: GraphCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Returns true if the pipeline task is still running.
    pub fn is_alive(&self) -> bool {
        !self.command_tx.is_closed()
    }
}

/// Connect to graph.microsoft.com and start the pipeline task.
/// Returns a `GraphConnection` handle.
pub async fn connect_and_start_pipeline() -> Result<GraphConnection, StoreError> {
    let conn = HttpClient::connect(GRAPH_HOST, GRAPH_PORT, true)
        .await
        .map_err(|e| StoreError::new(format!("Graph connect failed: {}", e)))?;

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    tokio::spawn(graph_pipeline_loop(conn, cmd_rx));

    Ok(GraphConnection { command_tx: cmd_tx })
}

// ── Pipeline loop ─────────────────────────────────────────────────────

/// Async pipeline loop: processes commands one at a time over a persistent HTTP connection.
async fn graph_pipeline_loop(
    mut conn: HttpConnection,
    mut cmd_rx: mpsc::UnboundedReceiver<GraphCommand>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            GraphCommand::ListFolders { token, on_folder, on_complete } => {
                on_complete(handle_list_folders(&mut conn, &token, &on_folder).await);
            }
            GraphCommand::MessageCount { token, folder_id, on_complete } => {
                on_complete(handle_message_count(&mut conn, &token, &folder_id).await);
            }
            GraphCommand::ListMessages { token, folder_id, top, skip, on_summary, on_complete } => {
                on_complete(handle_list_messages(&mut conn, &token, &folder_id, top, skip, &on_summary).await);
            }
            GraphCommand::GetMessage { token, message_id, on_metadata, on_content_chunk, on_complete } => {
                on_complete(handle_get_message(&mut conn, &token, &message_id, &*on_metadata, &on_content_chunk).await);
            }
            GraphCommand::DeleteMessage { token, message_id, on_complete } => {
                let path = format!("{}/me/messages/{}", GRAPH_BASE_PATH, message_id);
                on_complete(handle_delete(&mut conn, &token, &path).await);
            }
            GraphCommand::CreateFolder { token, name, on_complete } => {
                let body = requests::build_create_folder_body(&name);
                on_complete(handle_json_post(&mut conn, &token, &format!("{}/me/mailFolders", GRAPH_BASE_PATH), &body).await);
            }
            GraphCommand::RenameFolder { token, folder_id, new_name, on_complete } => {
                let body = requests::build_rename_folder_body(&new_name);
                let path = format!("{}/me/mailFolders/{}", GRAPH_BASE_PATH, folder_id);
                on_complete(handle_json_patch(&mut conn, &token, &path, &body).await);
            }
            GraphCommand::DeleteFolder { token, folder_id, on_complete } => {
                let path = format!("{}/me/mailFolders/{}", GRAPH_BASE_PATH, folder_id);
                on_complete(handle_delete(&mut conn, &token, &path).await);
            }
            GraphCommand::CopyMessages { token, message_ids, dest_folder_id, on_complete } => {
                let mut result = Ok(());
                for msg_id in &message_ids {
                    let body = requests::build_copy_move_body(&dest_folder_id);
                    let path = format!("{}/me/messages/{}/copy", GRAPH_BASE_PATH, msg_id);
                    if let Err(e) = handle_json_post(&mut conn, &token, &path, &body).await {
                        result = Err(e);
                        break;
                    }
                }
                on_complete(result);
            }
            GraphCommand::MoveMessages { token, message_ids, dest_folder_id, on_complete } => {
                let mut result = Ok(());
                for msg_id in &message_ids {
                    let body = requests::build_copy_move_body(&dest_folder_id);
                    let path = format!("{}/me/messages/{}/move", GRAPH_BASE_PATH, msg_id);
                    if let Err(e) = handle_json_post(&mut conn, &token, &path, &body).await {
                        result = Err(e);
                        break;
                    }
                }
                on_complete(result);
            }
            GraphCommand::StoreFlags { token, message_ids, body, on_complete } => {
                let mut result = Ok(());
                for msg_id in &message_ids {
                    let path = format!("{}/me/messages/{}", GRAPH_BASE_PATH, msg_id);
                    if let Err(e) = handle_json_patch(&mut conn, &token, &path, &body).await {
                        result = Err(e);
                        break;
                    }
                }
                on_complete(result);
            }
            GraphCommand::SendMail { token, body, on_complete } => {
                let path = format!("{}/me/sendMail", GRAPH_BASE_PATH);
                on_complete(handle_json_post(&mut conn, &token, &path, &body).await);
            }
        }
    }
}

// ── HTTP request helpers ──────────────────────────────────────────────

/// Build a GET request with the Bearer token.
fn build_get(conn: &mut HttpConnection, path: &str, token: &str) -> RequestBuilder {
    let mut req = conn.request(Method::Get, path);
    req.header("Authorization", &format!("Bearer {}", token));
    req
}

/// Build a POST request with JSON body and the Bearer token.
fn build_json_post(
    conn: &mut HttpConnection,
    path: &str,
    token: &str,
    body: &[u8],
) -> RequestBuilder {
    let mut req = conn.request(Method::Post, path);
    req.header("Authorization", &format!("Bearer {}", token))
       .header("Content-Type", "application/json")
       .header("Content-Length", &body.len().to_string())
       .body(body.to_vec());
    req
}

/// Build a PATCH request with JSON body and the Bearer token.
fn build_json_patch(
    conn: &mut HttpConnection,
    path: &str,
    token: &str,
    body: &[u8],
) -> RequestBuilder {
    let mut req = conn.request(Method::Patch, path);
    req.header("Authorization", &format!("Bearer {}", token))
       .header("Content-Type", "application/json")
       .header("Content-Length", &body.len().to_string())
       .body(body.to_vec());
    req
}

/// Build a DELETE request with the Bearer token.
fn build_delete(conn: &mut HttpConnection, path: &str, token: &str) -> RequestBuilder {
    let mut req = conn.request(Method::Delete, path);
    req.header("Authorization", &format!("Bearer {}", token));
    req
}

// ── Command handlers ──────────────────────────────────────────────────

/// Fetch all pages of a folder list endpoint, streaming results to `on_folder`.
/// Returns `(folder_id, full_path)` pairs for folders that have children (for BFS).
async fn fetch_folder_pages(
    conn: &mut HttpConnection,
    token: &str,
    start_path: &str,
    path_prefix: &str,
    on_folder: &Arc<dyn Fn(FolderInfo, GraphFolderEntry) + Send + Sync>,
) -> Result<Vec<(String, String)>, StoreError> {
    let mut path = start_path.to_string();
    let mut children_to_visit: Vec<(String, String)> = Vec::new();
    loop {
        let error: SharedError = Arc::new(std::sync::Mutex::new(None));
        let next_link: SharedNextLink = Arc::new(std::sync::Mutex::new(None));
        let entries: Arc<std::sync::Mutex<Vec<GraphFolderEntry>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let entries_clone = entries.clone();
        let json_handler = FolderListHandler::new(
            move |entry| {
                if let Ok(mut v) = entries_clone.lock() {
                    v.push(entry);
                }
            },
            next_link.clone(),
        );
        let handler = GraphResponseHandler::new(error.clone(), Box::new(json_handler));
        let req = build_get(conn, &path, token);
        conn.send(req, handler)
            .await
            .map_err(|e| StoreError::new(format!("Graph list folders failed: {}", e)))?;
        check_graph_error(&error, "list folders")?;

        let page_entries = entries.lock().unwrap().drain(..).collect::<Vec<_>>();
        for entry in page_entries {
            let full_name = if path_prefix.is_empty() {
                entry.display_name.clone()
            } else {
                format!("{}/{}", path_prefix, entry.display_name)
            };
            let info = FolderInfo {
                name: full_name.clone(),
                delimiter: Some('/'),
                attributes: Vec::new(),
            };
            if entry.child_folder_count > 0 {
                children_to_visit.push((entry.id.clone(), full_name.clone()));
            }
            on_folder(info, entry);
        }

        let next = next_link.lock().unwrap().take().and_then(|u| extract_graph_path(&u));
        match next {
            Some(n) => path = n,
            None => break,
        }
    }
    Ok(children_to_visit)
}

async fn handle_list_folders(
    conn: &mut HttpConnection,
    token: &str,
    on_folder: &Arc<dyn Fn(FolderInfo, GraphFolderEntry) + Send + Sync>,
) -> Result<(), StoreError> {
    let mut queue: VecDeque<(String, String)> = VecDeque::new();

    // Step 1: fetch top-level folders
    let start = format!("{}/me/mailFolders?$top=100", GRAPH_BASE_PATH);
    let children = fetch_folder_pages(conn, token, &start, "", on_folder).await?;
    queue.extend(children);

    // Step 2: BFS through child folders
    while let Some((folder_id, parent_path)) = queue.pop_front() {
        let child_path = format!(
            "{}/me/mailFolders/{}/childFolders?$top=100",
            GRAPH_BASE_PATH, folder_id
        );
        let children =
            fetch_folder_pages(conn, token, &child_path, &parent_path, on_folder).await?;
        queue.extend(children);
    }

    Ok(())
}

async fn handle_message_count(
    conn: &mut HttpConnection,
    token: &str,
    folder_id: &str,
) -> Result<u64, StoreError> {
    let path = format!("{}/me/mailFolders/{}?$select=totalItemCount", GRAPH_BASE_PATH, folder_id);
    let req = build_get(conn, &path, token);

    let error: SharedError = Arc::new(std::sync::Mutex::new(None));
    let count = Arc::new(std::sync::Mutex::new(0u64));
    let json_handler = MessageCountHandler::new(count.clone());
    let handler = GraphResponseHandler::new(error.clone(), Box::new(json_handler));

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph message count failed: {}", e)))?;

    check_graph_error(&error, "message count")?;

    let result = *count.lock().unwrap();
    Ok(result)
}

async fn handle_list_messages(
    conn: &mut HttpConnection,
    token: &str,
    folder_id: &str,
    top: u64,
    skip: u64,
    on_summary: &Arc<dyn Fn(ConversationSummary) + Send + Sync>,
) -> Result<(), StoreError> {
    let page_size = top.min(100);
    let select = "id,subject,from,toRecipients,ccRecipients,receivedDateTime,isRead,isDraft,importance,internetMessageId";
    let mut path = format!(
        "{}/me/mailFolders/{}/messages?$top={}&$skip={}&$select={}&$orderby=receivedDateTime desc",
        GRAPH_BASE_PATH, folder_id, page_size, skip, select
    );
    eprintln!("[graph] list messages: top={} skip={} folder_id={}", top, skip, folder_id);
    let mut collected: u64 = 0;

    loop {
        let error: SharedError = Arc::new(std::sync::Mutex::new(None));
        let next_link: SharedNextLink = Arc::new(std::sync::Mutex::new(None));
        let page_count = Arc::new(std::sync::Mutex::new(0u64));
        let page_count_clone = page_count.clone();
        let on_summary_clone = on_summary.clone();
        let json_handler = MessageListHandler::new(
            move |summary| {
                on_summary_clone(summary);
                if let Ok(mut c) = page_count_clone.lock() {
                    *c += 1;
                }
            },
            next_link.clone(),
        );
        let handler = GraphResponseHandler::new(error.clone(), Box::new(json_handler));
        let req = build_get(conn, &path, token);
        conn.send(req, handler)
            .await
            .map_err(|e| {
                eprintln!("[graph] list messages send error: {}", e);
                StoreError::new(format!("Graph list messages failed: {}", e))
            })?;
        check_graph_error(&error, "list messages")?;

        let page = *page_count.lock().unwrap();
        eprintln!("[graph] list messages page: {} messages (total so far: {})", page, collected + page);
        collected += page;
        if collected >= top {
            break;
        }

        let next = next_link.lock().unwrap().take().and_then(|u| extract_graph_path(&u));
        match next {
            Some(n) => path = n,
            None => break,
        }
    }
    Ok(())
}

async fn handle_get_message(
    conn: &mut HttpConnection,
    token: &str,
    message_id: &str,
    on_metadata: &(dyn Fn(Envelope) + Send + Sync),
    on_content_chunk: &Arc<dyn Fn(&[u8]) + Send + Sync>,
) -> Result<(), StoreError> {
    // Step 1: Fetch envelope metadata via JSON
    let select = "subject,from,toRecipients,ccRecipients,receivedDateTime,internetMessageId";
    let meta_path = format!(
        "{}/me/messages/{}?$select={}",
        GRAPH_BASE_PATH, message_id, select
    );
    let error: SharedError = Arc::new(std::sync::Mutex::new(None));
    let result = Arc::new(std::sync::Mutex::new(None::<Message>));
    let json_handler = SingleMessageHandler::new(result.clone());
    let handler = GraphResponseHandler::new(error.clone(), Box::new(json_handler));
    let req = build_get(conn, &meta_path, token);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph get message metadata failed: {}", e)))?;
    check_graph_error(&error, "get message metadata")?;

    if let Some(msg) = result.lock().unwrap().take() {
        on_metadata(msg.envelope);
    }

    // Step 2: Fetch raw MIME content via $value
    let value_path = format!("{}/me/messages/{}/$value", GRAPH_BASE_PATH, message_id);
    let error: SharedError = Arc::new(std::sync::Mutex::new(None));
    let stream_handler = MimeStreamHandler::new(error.clone(), on_content_chunk.clone());
    let req = build_get(conn, &value_path, token);
    conn.send(req, stream_handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph get message content failed: {}", e)))?;
    check_graph_error(&error, "get message content")?;

    // Step 3: Mark as read (non-fatal)
    let patch_path = format!("{}/me/messages/{}", GRAPH_BASE_PATH, message_id);
    let body = b"{\"isRead\":true}";
    let error: SharedError = Arc::new(std::sync::Mutex::new(None));
    let handler = GraphResponseHandler::new_status_only(error.clone());
    let req = build_json_patch(conn, &patch_path, token, body);
    eprintln!("[graph] marking message as read...");
    match conn.send(req, handler).await {
        Ok(()) => eprintln!("[graph] mark-as-read complete"),
        Err(e) => eprintln!("[graph] mark-as-read failed: {}", e),
    }

    Ok(())
}

async fn handle_delete(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
) -> Result<(), StoreError> {
    let req = build_delete(conn, path, token);

    let error: SharedError = Arc::new(std::sync::Mutex::new(None));
    let handler = GraphResponseHandler::new_status_only(error.clone());

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph delete failed: {}", e)))?;

    check_graph_error(&error, "delete")?;
    Ok(())
}

async fn handle_json_post(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
    body: &[u8],
) -> Result<(), StoreError> {
    let req = build_json_post(conn, path, token, body);

    let error: SharedError = Arc::new(std::sync::Mutex::new(None));
    let handler = GraphResponseHandler::new_status_only(error.clone());

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph POST failed: {}", e)))?;

    check_graph_error(&error, "POST")?;
    Ok(())
}

async fn handle_json_patch(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
    body: &[u8],
) -> Result<(), StoreError> {
    let req = build_json_patch(conn, path, token, body);

    let error: SharedError = Arc::new(std::sync::Mutex::new(None));
    let handler = GraphResponseHandler::new_status_only(error.clone());

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph PATCH failed: {}", e)))?;

    check_graph_error(&error, "PATCH")?;
    Ok(())
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

type SharedError = Arc<std::sync::Mutex<Option<GraphError>>>;

/// Parsed error detail shared between GraphErrorJsonHandler and GraphResponseHandler.
type SharedErrorDetail = Arc<std::sync::Mutex<(String, String)>>;

/// JsonContentHandler that parses Graph API error responses:
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

impl JsonContentHandler for GraphErrorJsonHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.current_key.as_deref() == Some("error") {
            self.in_error = true;
        }
    }

    fn end_object(&mut self) {
        if self.depth == 2 {
            self.in_error = false;
        }
        self.depth -= 1;
        self.current_key = None;
    }

    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
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
    }

    fn number_value(&mut self, _number: crate::json::JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _value: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

/// No-op JsonContentHandler for endpoints where we only care about the HTTP
/// status and error body (POST/PATCH/DELETE with empty or irrelevant success bodies).
struct NoOpJsonHandler;

impl JsonContentHandler for NoOpJsonHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, _key: &str) {}
    fn string_value(&mut self, _value: &str) {}
    fn number_value(&mut self, _number: crate::json::JsonNumber) {}
    fn boolean_value(&mut self, _value: bool) {}
    fn null_value(&mut self) {}
}

/// ResponseHandler that streams raw body bytes to a callback.
/// Used for `$value` endpoints that return raw MIME content.
struct MimeStreamHandler {
    error: SharedError,
    on_chunk: Arc<dyn Fn(&[u8]) + Send + Sync>,
    is_error: bool,
}

impl MimeStreamHandler {
    fn new(error: SharedError, on_chunk: Arc<dyn Fn(&[u8]) + Send + Sync>) -> Self {
        Self { error, on_chunk, is_error: false }
    }
}

impl ResponseHandler for MimeStreamHandler {
    fn ok(&mut self, _response: Response) {}

    fn error(&mut self, response: Response) {
        self.is_error = true;
        if let Ok(mut e) = self.error.lock() {
            *e = Some(GraphError {
                status: response.code,
                code: String::new(),
                message: format!("HTTP {}", response.code),
            });
        }
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        if !self.is_error {
            (self.on_chunk)(data);
        }
    }

    fn end_body(&mut self) {}
    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        if let Ok(mut e) = self.error.lock() {
            *e = Some(GraphError {
                status: 0,
                code: String::new(),
                message: error.to_string(),
            });
        }
    }
}

/// Streaming ResponseHandler that feeds body chunks into a single JsonParser.
///
/// The active JsonContentHandler is selected based on the HTTP status code:
/// - 2xx: the endpoint-specific handler passed at construction
/// - 4xx/5xx: swapped to GraphErrorJsonHandler to parse the standard error body
///
/// Only one parser and one handler are active at any time.
struct GraphResponseHandler {
    status_code: u16,
    is_error: bool,
    parser: JsonParser,
    handler: Box<dyn JsonContentHandler + Send>,
    buf: BytesMut,
    error: SharedError,
    err_detail: SharedErrorDetail,
}

impl GraphResponseHandler {
    fn new(error: SharedError, handler: Box<dyn JsonContentHandler + Send>) -> Self {
        Self {
            status_code: 0,
            is_error: false,
            parser: JsonParser::new(),
            handler,
            buf: BytesMut::with_capacity(4096),
            error,
            err_detail: Arc::new(std::sync::Mutex::new((String::new(), String::new()))),
        }
    }

    fn new_status_only(error: SharedError) -> Self {
        Self::new(error, Box::new(NoOpJsonHandler))
    }
}

impl ResponseHandler for GraphResponseHandler {
    fn ok(&mut self, response: Response) {
        self.status_code = response.code;
    }

    fn error(&mut self, response: Response) {
        self.status_code = response.code;
        self.is_error = true;
        self.handler = Box::new(GraphErrorJsonHandler::new(self.err_detail.clone()));
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
        let _ = self.parser.receive(&mut self.buf, &mut *self.handler);
    }

    fn end_body(&mut self) {
        let _ = self.parser.close(&mut *self.handler);
        if self.is_error {
            let (code, message) = {
                let d = self.err_detail.lock().unwrap();
                (d.0.clone(), d.1.clone())
            };
            if let Ok(mut e) = self.error.lock() {
                *e = Some(GraphError {
                    status: self.status_code,
                    code,
                    message,
                });
            }
        }
    }

    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        self.is_error = true;
        if let Ok(mut e) = self.error.lock() {
            *e = Some(GraphError {
                status: 0,
                code: String::new(),
                message: error.to_string(),
            });
        }
    }
}

/// Check the shared error after a `conn.send()` call with a `GraphResponseHandler`.
fn check_graph_error(error: &SharedError, context: &str) -> Result<(), StoreError> {
    if let Ok(guard) = error.lock() {
        if let Some(ref ge) = *guard {
            eprintln!("[graph] {} error: {}", context, ge);
            return Err(StoreError::new(format!("Graph {}: {}", context, ge)));
        }
    }
    Ok(())
}
