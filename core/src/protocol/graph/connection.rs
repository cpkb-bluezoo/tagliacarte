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

use std::sync::Arc;

use bytes::BytesMut;
use tokio::sync::mpsc;

use crate::json::JsonParser;
use crate::protocol::http::client::HttpClient;
use crate::protocol::http::connection::HttpConnection;
use crate::protocol::http::{Method, RequestBuilder, Response, ResponseHandler};
use crate::store::{ConversationSummary, FolderInfo, Message, StoreError};

use super::json_handlers::{
    FolderListHandler, GraphFolderEntry, MessageCountHandler,
    MessageListHandler, SingleMessageHandler,
};
use super::requests;

const GRAPH_HOST: &str = "graph.microsoft.com";
const GRAPH_PORT: u16 = 443;
const GRAPH_BASE_PATH: &str = "/v1.0";

// ── GraphCommand ──────────────────────────────────────────────────────

/// Commands sent from Store/Folder/Transport methods to the pipeline task.
/// Each variant carries the request parameters and callback(s).
pub enum GraphCommand {
    /// GET /me/mailFolders
    ListFolders {
        token: String,
        on_complete: Box<dyn FnOnce(Result<Vec<(FolderInfo, GraphFolderEntry)>, StoreError>) + Send>,
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
        on_complete: Box<dyn FnOnce(Result<Vec<ConversationSummary>, StoreError>) + Send>,
    },
    /// GET /me/messages/{id}?$expand=attachments
    GetMessage {
        token: String,
        message_id: String,
        on_complete: Box<dyn FnOnce(Result<Option<Message>, StoreError>) + Send>,
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
            GraphCommand::ListFolders { token, on_complete } => {
                on_complete(handle_list_folders(&mut conn, &token).await);
            }
            GraphCommand::MessageCount { token, folder_id, on_complete } => {
                on_complete(handle_message_count(&mut conn, &token, &folder_id).await);
            }
            GraphCommand::ListMessages { token, folder_id, top, skip, on_complete } => {
                on_complete(handle_list_messages(&mut conn, &token, &folder_id, top, skip).await);
            }
            GraphCommand::GetMessage { token, message_id, on_complete } => {
                on_complete(handle_get_message(&mut conn, &token, &message_id).await);
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

async fn handle_list_folders(
    conn: &mut HttpConnection,
    token: &str,
) -> Result<Vec<(FolderInfo, GraphFolderEntry)>, StoreError> {
    let path = format!("{}/me/mailFolders?$top=100", GRAPH_BASE_PATH);
    let req = build_get(conn, &path, token);

    let results = Arc::new(std::sync::Mutex::new(Vec::new()));
    let results_clone = results.clone();
    let handler = CollectJsonHandler::new(move |data| {
        let mut folder_handler = FolderListHandler::new(move |entry| {
            let info = entry.to_folder_info();
            if let Ok(mut v) = results_clone.lock() {
                v.push((info, entry));
            }
        });
        let mut parser = JsonParser::new();
        let mut buf = BytesMut::from(data);
        let _ = parser.receive(&mut buf, &mut folder_handler);
        let _ = parser.close(&mut folder_handler);
    });

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph list folders failed: {}", e)))?;

    let result = Arc::try_unwrap(results)
        .map(|mutex| mutex.into_inner().unwrap_or_default())
        .unwrap_or_else(|arc| arc.lock().unwrap().clone());
    Ok(result)
}

async fn handle_message_count(
    conn: &mut HttpConnection,
    token: &str,
    folder_id: &str,
) -> Result<u64, StoreError> {
    let path = format!("{}/me/mailFolders/{}?$select=totalItemCount", GRAPH_BASE_PATH, folder_id);
    let req = build_get(conn, &path, token);

    let count = Arc::new(std::sync::Mutex::new(0u64));
    let count_clone = count.clone();
    let handler = CollectJsonHandler::new(move |data| {
        let mut h = MessageCountHandler::new();
        let mut parser = JsonParser::new();
        let mut buf = BytesMut::from(data);
        let _ = parser.receive(&mut buf, &mut h);
        let _ = parser.close(&mut h);
        if let Ok(mut c) = count_clone.lock() {
            *c = h.total;
        }
    });

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph message count failed: {}", e)))?;

    let result = *count.lock().unwrap();
    Ok(result)
}

async fn handle_list_messages(
    conn: &mut HttpConnection,
    token: &str,
    folder_id: &str,
    top: u64,
    skip: u64,
) -> Result<Vec<ConversationSummary>, StoreError> {
    let path = format!(
        "{}/me/mailFolders/{}/messages?$top={}&$skip={}&$select=id,subject,from,toRecipients,ccRecipients,receivedDateTime,isRead,isDraft,importance,size,internetMessageId&$orderby=receivedDateTime desc",
        GRAPH_BASE_PATH, folder_id, top, skip
    );
    let req = build_get(conn, &path, token);

    let summaries = Arc::new(std::sync::Mutex::new(Vec::new()));
    let summaries_clone = summaries.clone();
    let handler = CollectJsonHandler::new(move |data| {
        let summaries_inner = summaries_clone.clone();
        let mut msg_handler = MessageListHandler::new(move |summary| {
            if let Ok(mut v) = summaries_inner.lock() {
                v.push(summary);
            }
        });
        let mut parser = JsonParser::new();
        let mut buf = BytesMut::from(data);
        let _ = parser.receive(&mut buf, &mut msg_handler);
        let _ = parser.close(&mut msg_handler);
    });

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph list messages failed: {}", e)))?;

    let result = Arc::try_unwrap(summaries)
        .map(|mutex| mutex.into_inner().unwrap_or_default())
        .unwrap_or_else(|arc| arc.lock().unwrap().clone());
    Ok(result)
}

async fn handle_get_message(
    conn: &mut HttpConnection,
    token: &str,
    message_id: &str,
) -> Result<Option<Message>, StoreError> {
    let path = format!("{}/me/messages/{}?$expand=attachments", GRAPH_BASE_PATH, message_id);
    let req = build_get(conn, &path, token);

    let result = Arc::new(std::sync::Mutex::new(None::<Message>));
    let result_clone = result.clone();
    let handler = CollectJsonHandler::new(move |data| {
        let mut msg_handler = SingleMessageHandler::new();
        let mut parser = JsonParser::new();
        let mut buf = BytesMut::from(data);
        let _ = parser.receive(&mut buf, &mut msg_handler);
        let _ = parser.close(&mut msg_handler);
        if let Ok(mut r) = result_clone.lock() {
            *r = msg_handler.result;
        }
    });

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph get message failed: {}", e)))?;

    let result = result.lock().unwrap().take();
    Ok(result)
}

async fn handle_delete(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
) -> Result<(), StoreError> {
    let req = build_delete(conn, path, token);

    let state = Arc::new(std::sync::Mutex::new((false, 0u16, String::new())));
    let state_clone = state.clone();
    let wrapper = StatusWrapper { state: state_clone };

    conn.send(req, wrapper)
        .await
        .map_err(|e| StoreError::new(format!("Graph delete failed: {}", e)))?;

    let (success, status_code, error_body) = {
        let s = state.lock().unwrap();
        (s.0, s.1, s.2.clone())
    };

    if !success {
        return Err(StoreError::new(format!("Graph API error ({}): {}", status_code, error_body)));
    }
    Ok(())
}

async fn handle_json_post(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
    body: &[u8],
) -> Result<(), StoreError> {
    let req = build_json_post(conn, path, token, body);

    let state = Arc::new(std::sync::Mutex::new((false, 0u16, String::new())));
    let state_clone = state.clone();
    let handler = StatusWrapper { state: state_clone };

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph POST failed: {}", e)))?;

    let (success, status_code, error_body) = {
        let s = state.lock().unwrap();
        (s.0, s.1, s.2.clone())
    };

    if !success {
        return Err(StoreError::new(format!("Graph API error ({}): {}", status_code, error_body)));
    }
    Ok(())
}

async fn handle_json_patch(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
    body: &[u8],
) -> Result<(), StoreError> {
    let req = build_json_patch(conn, path, token, body);

    let state = Arc::new(std::sync::Mutex::new((false, 0u16, String::new())));
    let state_clone = state.clone();
    let handler = StatusWrapper { state: state_clone };

    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Graph PATCH failed: {}", e)))?;

    let (success, status_code, error_body) = {
        let s = state.lock().unwrap();
        (s.0, s.1, s.2.clone())
    };

    if !success {
        return Err(StoreError::new(format!("Graph API error ({}): {}", status_code, error_body)));
    }
    Ok(())
}

// ── Handler wrappers ──────────────────────────────────────────────────

/// ResponseHandler that collects body, then runs a parse closure on completion.
struct CollectJsonHandler {
    success: bool,
    status_code: u16,
    body_buf: BytesMut,
    error_body: String,
    on_parse: Option<Box<dyn FnOnce(&[u8]) + Send>>,
}

impl CollectJsonHandler {
    fn new(on_parse: impl FnOnce(&[u8]) + Send + 'static) -> Self {
        Self {
            success: false,
            status_code: 0,
            body_buf: BytesMut::with_capacity(8192),
            error_body: String::new(),
            on_parse: Some(Box::new(on_parse)),
        }
    }
}

impl ResponseHandler for CollectJsonHandler {
    fn ok(&mut self, response: Response) {
        self.success = true;
        self.status_code = response.code;
    }

    fn error(&mut self, response: Response) {
        self.success = false;
        self.status_code = response.code;
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        if self.success {
            self.body_buf.extend_from_slice(data);
        } else {
            if let Ok(s) = std::str::from_utf8(data) {
                self.error_body.push_str(s);
            }
        }
    }

    fn end_body(&mut self) {
        if self.success {
            if let Some(parse_fn) = self.on_parse.take() {
                parse_fn(&self.body_buf);
            }
        }
    }

    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        self.success = false;
        self.error_body = error.to_string();
    }
}

/// A ResponseHandler that records status into an Arc<Mutex<(bool, u16, String)>>.
struct StatusWrapper {
    state: Arc<std::sync::Mutex<(bool, u16, String)>>,
}

impl ResponseHandler for StatusWrapper {
    fn ok(&mut self, response: Response) {
        if let Ok(mut s) = self.state.lock() {
            s.0 = true;
            s.1 = response.code;
        }
    }

    fn error(&mut self, response: Response) {
        if let Ok(mut s) = self.state.lock() {
            s.0 = false;
            s.1 = response.code;
        }
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}

    fn body_chunk(&mut self, data: &[u8]) {
        if let Ok(mut s) = self.state.lock() {
            if !s.0 {
                if let Ok(text) = std::str::from_utf8(data) {
                    s.2.push_str(text);
                }
            }
        }
    }

    fn end_body(&mut self) {}
    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        if let Ok(mut s) = self.state.lock() {
            s.0 = false;
            s.2 = error.to_string();
        }
    }
}
