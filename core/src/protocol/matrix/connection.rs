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

//! Matrix pipeline connection.
//!
//! Mirrors the Graph pipeline pattern: a single persistent HTTPS connection to
//! the homeserver, with commands queued via an `mpsc::UnboundedSender`.
//! A tokio::spawn'd loop processes them one at a time on the existing connection.

use std::sync::{Arc, Mutex};

use bytes::BytesMut;
use tokio::sync::mpsc;

use crate::json::{JsonContentHandler, JsonParser};
use crate::protocol::http::client::HttpClient;
use crate::protocol::http::connection::HttpConnection;
use crate::protocol::http::{Method, RequestBuilder, Response, ResponseHandler};
use crate::store::StoreError;

use super::json_handlers::*;
use super::requests;
use super::types::*;

// ── MatrixCommand ────────────────────────────────────────────────────

/// Commands sent from Store/Folder/Transport methods to the pipeline task.
pub enum MatrixCommand {
    /// POST /_matrix/client/v3/login
    Login {
        user: String,
        password: String,
        on_complete: Box<dyn FnOnce(Result<LoginResponse, StoreError>) + Send>,
    },
    /// GET /.well-known/matrix/client
    WellKnown {
        on_complete: Box<dyn FnOnce(Result<Option<WellKnown>, StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/sync
    Sync {
        token: String,
        since: Option<String>,
        on_room: Arc<dyn Fn(RoomSummary) + Send + Sync>,
        on_event: Arc<dyn Fn(RoomEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Option<String>, StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/joined_rooms
    JoinedRooms {
        token: String,
        on_complete: Box<dyn FnOnce(Result<Vec<String>, StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/profile/{userId}
    GetProfile {
        user_id: String,
        token: String,
        on_complete: Box<dyn FnOnce(Result<Profile, StoreError>) + Send>,
    },
    /// PUT /_matrix/client/v3/profile/{userId}/displayname
    SetDisplayName {
        user_id: String,
        token: String,
        display_name: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// PUT /_matrix/client/v3/rooms/{roomId}/send/m.room.message/{txnId}
    SendMessage {
        token: String,
        room_id: String,
        body: Vec<u8>,
        txn_id: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/rooms/{roomId}/messages
    RoomMessages {
        token: String,
        room_id: String,
        limit: u64,
        from: Option<String>,
        on_event: Arc<dyn Fn(RoomEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Option<String>, StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/rooms/{roomId}/event/{eventId}
    GetEvent {
        token: String,
        room_id: String,
        event_id: String,
        on_complete: Box<dyn FnOnce(Result<Option<RoomEvent>, StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/join/{roomIdOrAlias}
    JoinRoom {
        token: String,
        room_id_or_alias: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/rooms/{roomId}/leave
    LeaveRoom {
        token: String,
        room_id: String,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// POST /_matrix/media/v3/upload
    UploadMedia {
        token: String,
        content_type: String,
        data: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<String, StoreError>) + Send>,
    },
}

// ── MatrixConnection ─────────────────────────────────────────────────

/// Handle to the Matrix pipeline task. Cheaply cloneable.
#[derive(Clone)]
pub struct MatrixConnection {
    command_tx: mpsc::UnboundedSender<MatrixCommand>,
}

impl MatrixConnection {
    pub fn send(&self, cmd: MatrixCommand) {
        let _ = self.command_tx.send(cmd);
    }

    pub fn is_alive(&self) -> bool {
        !self.command_tx.is_closed()
    }
}

/// Parse host and port from a homeserver URL like "https://matrix.org" or "https://matrix.org:8448".
fn parse_homeserver_url(url: &str) -> Result<(String, u16, bool), StoreError> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(StoreError::new(format!("invalid homeserver URL: {}", url)));
    };
    let rest = rest.trim_end_matches('/');
    let (host, port) = if let Some(colon) = rest.rfind(':') {
        let port_str = &rest[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (rest[..colon].to_string(), p)
        } else {
            (rest.to_string(), if scheme { 443 } else { 80 })
        }
    } else {
        (rest.to_string(), if scheme { 443 } else { 80 })
    };
    Ok((host, port, scheme))
}

/// Connect to the homeserver and start the pipeline task.
pub async fn connect_and_start_pipeline(
    homeserver_url: &str,
) -> Result<MatrixConnection, StoreError> {
    let (host, port, tls) = parse_homeserver_url(homeserver_url)?;
    let conn = HttpClient::connect(&host, port, tls)
        .await
        .map_err(|e| StoreError::new(format!("Matrix connect to {} failed: {}", host, e)))?;

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let host_clone = host.clone();
    tokio::spawn(matrix_pipeline_loop(conn, cmd_rx, host_clone, port, tls));

    Ok(MatrixConnection { command_tx: cmd_tx })
}

// ── Pipeline loop ────────────────────────────────────────────────────

async fn try_reconnect(
    conn: &mut HttpConnection,
    host: &str,
    port: u16,
    tls: bool,
) -> Result<(), StoreError> {
    eprintln!("[matrix] connection lost, reconnecting...");
    match HttpClient::connect(host, port, tls).await {
        Ok(new_conn) => {
            *conn = new_conn;
            eprintln!("[matrix] reconnected");
            Ok(())
        }
        Err(e) => Err(StoreError::new(format!("Matrix reconnect failed: {}", e))),
    }
}

fn is_connection_error(e: &StoreError) -> bool {
    let msg = e.to_string();
    msg.contains("broken pipe")
        || msg.contains("connection reset")
        || msg.contains("connection closed")
        || msg.contains("UnexpectedEof")
}

/// Main pipeline loop: processes commands sequentially over a persistent HTTP connection.
async fn matrix_pipeline_loop(
    mut conn: HttpConnection,
    mut cmd_rx: mpsc::UnboundedReceiver<MatrixCommand>,
    host: String,
    port: u16,
    tls: bool,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            MatrixCommand::Login { user, password, on_complete } => {
                let mut result = handle_login(&mut conn, &user, &password).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_login(&mut conn, &user, &password).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::WellKnown { on_complete } => {
                let result = handle_well_known(&mut conn).await;
                on_complete(result);
            }
            MatrixCommand::Sync { token, since, on_room, on_event, on_complete } => {
                let mut result = handle_sync(&mut conn, &token, since.as_deref(), &on_room, &on_event).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_sync(&mut conn, &token, since.as_deref(), &on_room, &on_event).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::JoinedRooms { token, on_complete } => {
                let mut result = handle_joined_rooms(&mut conn, &token).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_joined_rooms(&mut conn, &token).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::GetProfile { user_id, token, on_complete } => {
                let mut result = handle_get_profile(&mut conn, &token, &user_id).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_get_profile(&mut conn, &token, &user_id).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::SetDisplayName { user_id, token, display_name, on_complete } => {
                let body = requests::build_display_name_body(&display_name);
                let path = path_display_name(&user_id);
                let mut result = handle_json_put(&mut conn, &token, &path, &body).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_json_put(&mut conn, &token, &path, &body).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::SendMessage { token, room_id, body, txn_id, on_complete } => {
                let path = path_send_message(&room_id, &txn_id);
                let mut result = handle_json_put(&mut conn, &token, &path, &body).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_json_put(&mut conn, &token, &path, &body).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::RoomMessages { token, room_id, limit, from, on_event, on_complete } => {
                let mut result = handle_room_messages(&mut conn, &token, &room_id, limit, from.as_deref(), &on_event).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_room_messages(&mut conn, &token, &room_id, limit, from.as_deref(), &on_event).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::GetEvent { token, room_id, event_id, on_complete } => {
                let mut result = handle_get_event(&mut conn, &token, &room_id, &event_id).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_get_event(&mut conn, &token, &room_id, &event_id).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::JoinRoom { token, room_id_or_alias, on_complete } => {
                let body = requests::build_empty_body();
                let path = path_join(&room_id_or_alias);
                let mut result = handle_json_post(&mut conn, &token, &path, &body).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_json_post(&mut conn, &token, &path, &body).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::LeaveRoom { token, room_id, on_complete } => {
                let body = requests::build_empty_body();
                let path = path_leave(&room_id);
                let mut result = handle_json_post(&mut conn, &token, &path, &body).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_json_post(&mut conn, &token, &path, &body).await;
                        }
                    }
                }
                on_complete(result);
            }
            MatrixCommand::UploadMedia { token, content_type, data, on_complete } => {
                let mut result = handle_upload_media(&mut conn, &token, &content_type, &data).await;
                if let Err(ref e) = result {
                    if is_connection_error(e) {
                        if try_reconnect(&mut conn, &host, port, tls).await.is_ok() {
                            result = handle_upload_media(&mut conn, &token, &content_type, &data).await;
                        }
                    }
                }
                on_complete(result);
            }
        }
    }
}

// ── HTTP request helpers ─────────────────────────────────────────────

fn build_get(conn: &mut HttpConnection, path: &str, token: &str) -> RequestBuilder {
    let mut req = conn.request(Method::Get, path);
    req.header("Authorization", &format!("Bearer {}", token));
    req
}

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

fn build_json_put(
    conn: &mut HttpConnection,
    path: &str,
    token: &str,
    body: &[u8],
) -> RequestBuilder {
    let mut req = conn.request(Method::Put, path);
    req.header("Authorization", &format!("Bearer {}", token))
       .header("Content-Type", "application/json")
       .header("Content-Length", &body.len().to_string())
       .body(body.to_vec());
    req
}

fn build_json_post_no_auth(
    conn: &mut HttpConnection,
    path: &str,
    body: &[u8],
) -> RequestBuilder {
    let mut req = conn.request(Method::Post, path);
    req.header("Content-Type", "application/json")
       .header("Content-Length", &body.len().to_string())
       .body(body.to_vec());
    req
}

// ── Command handlers ─────────────────────────────────────────────────

type SharedError = Arc<Mutex<Option<MatrixApiError>>>;
type SharedErrorDetail = Arc<Mutex<(String, String)>>;

async fn handle_login(
    conn: &mut HttpConnection,
    user: &str,
    password: &str,
) -> Result<LoginResponse, StoreError> {
    let body = requests::build_login_body(user, password);
    let error: SharedError = Arc::new(Mutex::new(None));
    let result: Arc<Mutex<Option<LoginResponse>>> = Arc::new(Mutex::new(None));
    let json_handler = LoginResponseHandler::new(result.clone());
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let req = build_json_post_no_auth(conn, PATH_LOGIN, &body);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix login failed: {}", e)))?;
    check_matrix_error(&error, "login")?;

    let login = result.lock().unwrap().take();
    login.ok_or_else(|| StoreError::new("Matrix login: no response"))
}

async fn handle_well_known(
    conn: &mut HttpConnection,
) -> Result<Option<WellKnown>, StoreError> {
    let error: SharedError = Arc::new(Mutex::new(None));
    let result: Arc<Mutex<Option<WellKnown>>> = Arc::new(Mutex::new(None));
    let json_handler = WellKnownHandler::new(result.clone());
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let req = conn.request(Method::Get, WELL_KNOWN_PATH);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix well-known failed: {}", e)))?;

    // 404 is not an error for well-known — just means no delegation
    if let Ok(guard) = error.lock() {
        if let Some(ref e) = *guard {
            if e.status == 404 {
                return Ok(None);
            }
        }
    }
    check_matrix_error(&error, "well-known")?;

    let wk = result.lock().unwrap().take();
    Ok(wk)
}

async fn handle_sync(
    conn: &mut HttpConnection,
    token: &str,
    since: Option<&str>,
    on_room: &Arc<dyn Fn(RoomSummary) + Send + Sync>,
    on_event: &Arc<dyn Fn(RoomEvent) + Send + Sync>,
) -> Result<Option<String>, StoreError> {
    let mut path = PATH_SYNC.to_string();
    let mut sep = '?';
    if let Some(s) = since {
        path.push_str(&format!("{}since={}", sep, s));
        sep = '&';
        path.push_str(&format!("{}timeout=30000", sep));
    }

    let error: SharedError = Arc::new(Mutex::new(None));
    let next_batch: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let on_room_clone = on_room.clone();
    let on_event_clone = on_event.clone();
    let json_handler = SyncResponseHandler::new(
        move |room| on_room_clone(room),
        move |event| on_event_clone(event),
        next_batch.clone(),
    );
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let req = build_get(conn, &path, token);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix sync failed: {}", e)))?;
    check_matrix_error(&error, "sync")?;

    let nb = next_batch.lock().unwrap().take();
    Ok(nb)
}

async fn handle_joined_rooms(
    conn: &mut HttpConnection,
    token: &str,
) -> Result<Vec<String>, StoreError> {
    let error: SharedError = Arc::new(Mutex::new(None));
    let rooms: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let json_handler = JoinedRoomsHandler::new(rooms.clone());
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let req = build_get(conn, PATH_JOINED_ROOMS, token);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix joined_rooms failed: {}", e)))?;
    check_matrix_error(&error, "joined_rooms")?;

    let result = rooms.lock().unwrap().drain(..).collect();
    Ok(result)
}

async fn handle_get_profile(
    conn: &mut HttpConnection,
    token: &str,
    user_id: &str,
) -> Result<Profile, StoreError> {
    let path = path_profile(user_id);
    let error: SharedError = Arc::new(Mutex::new(None));
    let profile: Arc<Mutex<Profile>> = Arc::new(Mutex::new(Profile::default()));
    let json_handler = ProfileHandler::new(profile.clone());
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let req = build_get(conn, &path, token);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix get profile failed: {}", e)))?;
    check_matrix_error(&error, "get profile")?;

    let result = profile.lock().unwrap().clone();
    Ok(result)
}

async fn handle_room_messages(
    conn: &mut HttpConnection,
    token: &str,
    room_id: &str,
    limit: u64,
    from: Option<&str>,
    on_event: &Arc<dyn Fn(RoomEvent) + Send + Sync>,
) -> Result<Option<String>, StoreError> {
    let path = path_room_messages(room_id, limit, from);
    let error: SharedError = Arc::new(Mutex::new(None));
    let end_token: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let on_event_clone = on_event.clone();
    let json_handler = RoomMessagesHandler::new(
        room_id.to_string(),
        move |event| on_event_clone(event),
        end_token.clone(),
    );
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let req = build_get(conn, &path, token);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix room messages failed: {}", e)))?;
    check_matrix_error(&error, "room messages")?;

    let et = end_token.lock().unwrap().take();
    Ok(et)
}

async fn handle_get_event(
    conn: &mut HttpConnection,
    token: &str,
    room_id: &str,
    event_id: &str,
) -> Result<Option<RoomEvent>, StoreError> {
    let path = path_room_event(room_id, event_id);
    let error: SharedError = Arc::new(Mutex::new(None));
    let result: Arc<Mutex<Option<RoomEvent>>> = Arc::new(Mutex::new(None));
    let json_handler = SingleEventHandler::new(room_id.to_string(), result.clone());
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let req = build_get(conn, &path, token);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix get event failed: {}", e)))?;
    check_matrix_error(&error, "get event")?;

    let event = result.lock().unwrap().take();
    Ok(event)
}

async fn handle_json_post(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
    body: &[u8],
) -> Result<(), StoreError> {
    let error: SharedError = Arc::new(Mutex::new(None));
    let handler = MatrixResponseHandler::new_status_only(error.clone());
    let req = build_json_post(conn, path, token, body);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix POST failed: {}", e)))?;
    check_matrix_error(&error, "POST")
}

async fn handle_json_put(
    conn: &mut HttpConnection,
    token: &str,
    path: &str,
    body: &[u8],
) -> Result<(), StoreError> {
    let error: SharedError = Arc::new(Mutex::new(None));
    let handler = MatrixResponseHandler::new_status_only(error.clone());
    let req = build_json_put(conn, path, token, body);
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix PUT failed: {}", e)))?;
    check_matrix_error(&error, "PUT")
}

async fn handle_upload_media(
    conn: &mut HttpConnection,
    token: &str,
    content_type: &str,
    data: &[u8],
) -> Result<String, StoreError> {
    let path = path_media_upload();
    let error: SharedError = Arc::new(Mutex::new(None));
    let mxc: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let json_handler = MediaUploadHandler::new(mxc.clone());
    let handler = MatrixResponseHandler::new(error.clone(), Box::new(json_handler));

    let mut req = conn.request(Method::Post, &path);
    req.header("Authorization", &format!("Bearer {}", token))
       .header("Content-Type", content_type)
       .header("Content-Length", &data.len().to_string())
       .body(data.to_vec());
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("Matrix media upload failed: {}", e)))?;
    check_matrix_error(&error, "media upload")?;

    let content_uri = mxc.lock().unwrap().take();
    content_uri.ok_or_else(|| StoreError::new("Matrix media upload: no content_uri in response"))
}

// ── Response handler ─────────────────────────────────────────────────

/// Streaming ResponseHandler that feeds body chunks directly into a JsonParser.
///
/// On 2xx: uses the endpoint-specific handler.
/// On 4xx/5xx: swaps to MatrixErrorHandler.
struct MatrixResponseHandler {
    status_code: u16,
    is_error: bool,
    parser: JsonParser,
    handler: Box<dyn JsonContentHandler + Send>,
    buf: BytesMut,
    error: SharedError,
    err_detail: SharedErrorDetail,
}

impl MatrixResponseHandler {
    fn new(error: SharedError, handler: Box<dyn JsonContentHandler + Send>) -> Self {
        Self {
            status_code: 0,
            is_error: false,
            parser: JsonParser::new(),
            handler,
            buf: BytesMut::new(),
            error,
            err_detail: Arc::new(Mutex::new((String::new(), String::new()))),
        }
    }

    fn new_status_only(error: SharedError) -> Self {
        Self::new(error, Box::new(NoOpHandler))
    }
}

impl ResponseHandler for MatrixResponseHandler {
    fn ok(&mut self, response: Response) {
        self.status_code = response.code;
    }

    fn error(&mut self, response: Response) {
        self.status_code = response.code;
        self.is_error = true;
        self.handler = Box::new(MatrixErrorJsonHandler::new(self.err_detail.clone()));
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
            let (errcode, error_msg) = {
                let d = self.err_detail.lock().unwrap();
                (d.0.clone(), d.1.clone())
            };
            if let Ok(mut e) = self.error.lock() {
                *e = Some(MatrixApiError {
                    status: self.status_code,
                    errcode,
                    error: error_msg,
                });
            }
        }
    }

    fn complete(&mut self) {}

    fn failed(&mut self, error: &std::io::Error) {
        self.is_error = true;
        if let Ok(mut e) = self.error.lock() {
            *e = Some(MatrixApiError {
                status: 0,
                errcode: String::new(),
                error: error.to_string(),
            });
        }
    }
}

/// Handler for Matrix error response bodies: `{"errcode":"...","error":"..."}`.
struct MatrixErrorJsonHandler {
    current_key: Option<String>,
    detail: SharedErrorDetail,
}

impl MatrixErrorJsonHandler {
    fn new(detail: SharedErrorDetail) -> Self {
        Self { current_key: None, detail }
    }
}

impl JsonContentHandler for MatrixErrorJsonHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        self.current_key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if let Ok(mut d) = self.detail.lock() {
            match self.current_key.as_deref() {
                Some("errcode") => d.0 = value.to_string(),
                Some("error") => d.1 = value.to_string(),
                _ => {}
            }
        }
        self.current_key = None;
    }

    fn number_value(&mut self, _: crate::json::JsonNumber) { self.current_key = None; }
    fn boolean_value(&mut self, _: bool) { self.current_key = None; }
    fn null_value(&mut self) { self.current_key = None; }
}

fn check_matrix_error(error: &SharedError, context: &str) -> Result<(), StoreError> {
    if let Ok(guard) = error.lock() {
        if let Some(ref me) = *guard {
            eprintln!("[matrix] {} error: {}", context, me);
            return Err(StoreError::new(format!("Matrix {}: {}", context, me)));
        }
    }
    Ok(())
}
