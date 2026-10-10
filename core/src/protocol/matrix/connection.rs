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
//! Matrix pipeline connection on hopf.
//!
//! Mirrors the Graph pipeline: one persistent HTTPS connection to the
//! homeserver, dialled on first use and redialled when it drops. Commands
//! queue behind each other and run one at a time; every HTTP request is one
//! step, and a step that fails because the connection went away is retried
//! once on a fresh connection. Nothing here blocks: each step continues from
//! the response handler on hopf's reactor thread.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bytes::BytesMut;

use crate::json::{ContentHandler, Number, ParseResult, Parser};
use crate::protocol::http::client::HttpClient;
use crate::protocol::http::connection::HttpConnection;
use crate::protocol::http::{Method, RequestBuilder, Response, ResponseHandler};
use crate::store::{FolderInfo, StoreError};

use super::crypto::CryptoMachine;
use super::device::{self, DeviceTracker};
use super::json_handlers::*;
use super::requests;
use super::trace;
use super::types::*;

/// `(HTTP method, path)` for [`MatrixResponseHandler`] stderr tracing when `TAGLIACARTE_TRACE=matrix`.
type MatrixHttpTrace = Option<(String, String)>;

#[inline]
fn mtx_tr(method: &'static str, path: impl Into<String>) -> MatrixHttpTrace {
    Some((method.to_string(), path.into()))
}

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
        /// For DM folder labels: ignore our own `m.room.member` when inferring display name.
        own_user_id: Option<String>,
        on_room: Arc<dyn Fn(RoomSummary) + Send + Sync>,
        on_event: Arc<dyn Fn(RoomEvent) + Send + Sync>,
        /// E2EE: one-time key count from server
        otk_count: Arc<Mutex<Option<usize>>>,
        /// E2EE: users whose device lists changed
        device_lists_changed: Arc<Mutex<Vec<String>>>,
        /// E2EE: callback for to-device events (event_type, sender, content_json)
        on_to_device: Arc<dyn Fn(String, String, String) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Option<String>, StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/joined_rooms
    JoinedRooms {
        token: String,
        on_complete: Box<dyn FnOnce(Result<Vec<String>, StoreError>) + Send>,
    },
    /// GET `/joined_rooms`, then GET `m.room.name` per room and finish [`Store::list_folders`],
    /// then full `/sync` (E2EE / `next_batch`) without blocking on a long initial sync body.
    ListFoldersAfterJoinedRooms {
        token: String,
        own_user_id: String,
        on_folder: Arc<dyn Fn(FolderInfo) + Send + Sync>,
        on_room: Arc<dyn Fn(RoomSummary) + Send + Sync>,
        on_event: Arc<dyn Fn(RoomEvent) + Send + Sync>,
        otk_count: Arc<Mutex<Option<usize>>>,
        device_lists_changed: Arc<Mutex<Vec<String>>>,
        on_to_device: Arc<dyn Fn(String, String, String) + Send + Sync>,
        user_on_complete: Arc<Mutex<Option<Box<dyn FnOnce(Result<(), StoreError>) + Send>>>>,
        sync_token: Arc<Mutex<Option<String>>>,
        conn_for_keys: MatrixConnection,
        token_for_keys: String,
        crypto_for_keys: Option<Arc<CryptoMachine>>,
        device_tracker: Arc<DeviceTracker>,
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
    /// PUT /_matrix/client/v3/profile/{userId}/avatar_url
    SetAvatar {
        user_id: String,
        token: String,
        mxc_url: String,
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
    /// POST /_matrix/client/v3/publicRooms
    PublicRooms {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<Vec<u8>, StoreError>) + Send>,
    },
    /// POST /_matrix/media/v3/upload
    UploadMedia {
        token: String,
        content_type: String,
        data: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<String, StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/keys/upload
    UploadKeys {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<KeyUploadCounts, StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/keys/query
    QueryKeys {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<KeyQueryResult, StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/keys/claim
    ClaimKeys {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<KeyClaimResult, StoreError>) + Send>,
    },
    /// PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}
    SendToDevice {
        token: String,
        event_type: String,
        txn_id: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// PUT /_matrix/client/v3/user/{userId}/account_data/{type}
    SetAccountData {
        token: String,
        user_id: String,
        event_type: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/user/{userId}/account_data/{type}
    GetAccountData {
        token: String,
        user_id: String,
        event_type: String,
        on_complete: Box<dyn FnOnce(Result<Vec<u8>, StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/room_keys/version
    CreateKeyBackup {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<String, StoreError>) + Send>,
    },
    /// PUT /_matrix/client/v3/room_keys/keys
    UploadRoomKeys {
        token: String,
        version: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/room_keys/keys?version={ver}
    DownloadRoomKeys {
        token: String,
        version: String,
        on_complete: Box<dyn FnOnce(Result<Vec<u8>, StoreError>) + Send>,
    },
    /// GET /_matrix/client/v3/room_keys/version — returns (version, algorithm) or None if no backup
    GetKeyBackupVersion {
        token: String,
        on_complete: Box<dyn FnOnce(Result<Option<(String, String)>, StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/keys/device_signing/upload
    UploadSigningKeys {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
    /// POST /_matrix/client/v3/keys/signatures/upload
    UploadSignatures {
        token: String,
        body: Vec<u8>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    },
}

// ── Pipeline ─────────────────────────────────────────────────────────

/// Hears one outcome.
type Done<T> = Box<dyn FnOnce(Result<T, StoreError>) + Send>;
/// Issues one request on a connection; called again on a fresh
/// connection when the first attempt lost its connection.
type Start = Arc<dyn Fn(&HttpConnection, Done<()>) + Send + Sync>;
/// What runs after a step, with the pipeline to continue on.
type Then<T> = Box<dyn FnOnce(Pipe, Result<T, StoreError>) + Send>;
type Pipe = Arc<Mutex<Pipeline>>;

struct Pipeline {
    host: String,
    port: u16,
    tls: bool,
    conn: Option<HttpConnection>,
    busy: bool,
    queue: VecDeque<MatrixCommand>,
}

/// Handle to the Matrix pipeline. Cheaply cloneable.
#[derive(Clone)]
pub struct MatrixConnection {
    pipe: Pipe,
}

impl MatrixConnection {
    /// A pipeline for `homeserver_url` with no connection yet; the first
    /// command dials.
    pub fn new(homeserver_url: &str) -> Result<Self, StoreError> {
        let (host, port, tls) = parse_homeserver_url(homeserver_url)?;
        Ok(Self {
            pipe: Arc::new(Mutex::new(Pipeline { host, port, tls, conn: None, busy: false, queue: VecDeque::new() })),
        })
    }

    /// Queue a command for the pipeline. Returns immediately.
    pub fn send(&self, cmd: MatrixCommand) {
        self.pipe.lock().unwrap().queue.push_back(cmd);
        run_next(Arc::clone(&self.pipe));
    }
}

/// Parse host and port from a homeserver URL like "https://matrix.org" or "https://matrix.org:8448".
/// If no scheme is given, assumes HTTPS.
fn parse_homeserver_url(url: &str) -> Result<(String, u16, bool), StoreError> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        (true, url)
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
    let (host, port, tls) = {
        let g = pipe.lock().unwrap();
        (g.host.clone(), g.port, g.tls)
    };
    let host_for_err = host.clone();
    HttpClient::connect(
        &host,
        port,
        tls,
        Box::new(move |r| {
            let r = r.map_err(|e| StoreError::new(format!("Matrix connect to {} failed: {}", host_for_err, e)));
            if let Ok(c) = &r {
                pipe.lock().unwrap().conn = Some(c.clone());
            }
            k(pipe, r)
        }),
    );
}

fn is_connection_error(e: &StoreError) -> bool {
    let msg = e.to_string();
    msg.contains("broken pipe")
        || msg.contains("connection reset")
        || msg.contains("connection closed")
        || msg.contains("UnexpectedEof")
}

/// One request as a pipeline step: on the live connection, dialled if
/// needed, and once more on a fresh one if the connection went away.
fn step(pipe: Pipe, start: Start, then: Then<()>) {
    with_conn(
        pipe,
        Box::new(move |pipe, conn| match conn {
            Ok(c) => attempt(pipe, c, start, then, false),
            Err(e) => then(pipe, Err(e)),
        }),
    );
}

fn attempt(pipe: Pipe, conn: HttpConnection, start: Start, then: Then<()>, retried: bool) {
    let again = Arc::clone(&start);
    let p = Arc::clone(&pipe);
    start(
        &conn,
        Box::new(move |r| match r {
            Err(e) if !retried && is_connection_error(&e) => {
                eprintln!("[matrix] connection lost, reconnecting...");
                p.lock().unwrap().conn = None;
                dial(
                    p,
                    Box::new(move |pipe, c| match c {
                        Ok(c) => {
                            eprintln!("[matrix] reconnected");
                            attempt(pipe, c, again, then, true)
                        }
                        Err(e2) => then(pipe, Err(e2)),
                    }),
                );
            }
            r => then(p, r),
        }),
    );
}

/// A single-request command whose result is read out of a slot the
/// response handler filled.
fn one<T: Send + 'static>(
    pipe: Pipe,
    start: Start,
    out: impl FnOnce() -> Result<T, StoreError> + Send + 'static,
    on_complete: Done<T>,
) {
    step(pipe, start, Box::new(move |pipe, r| finish(pipe, on_complete, r.and_then(|()| out()))));
}

/// A single-request command where only success matters.
fn status_only(pipe: Pipe, start: Start, on_complete: Done<()>) {
    step(pipe, start, Box::new(|pipe, r| finish(pipe, on_complete, r)));
}

/// Send `req`, feeding a 2xx body to `json`; `done` hears Ok once the
/// response is fully read, or the Matrix error. With `not_found_ok`, a 404
/// is Ok too (the slot stays empty).
fn request(
    conn: &HttpConnection,
    req: RequestBuilder,
    json: Box<dyn ContentHandler + Send>,
    http_trace: MatrixHttpTrace,
    context: impl Into<String>,
    not_found_ok: bool,
    done: Done<()>,
) {
    conn.send(req, MatrixResponseHandler::new(json, http_trace, context.into(), not_found_ok, done));
}

fn execute(pipe: Pipe, cmd: MatrixCommand) {
    match cmd {
        MatrixCommand::Login { user, password, on_complete } => {
            let body = requests::build_login_body(&user, &password);
            let result: Arc<Mutex<Option<LoginResponse>>> = Arc::new(Mutex::new(None));
            let slot = Arc::clone(&result);
            let start: Start = Arc::new(move |conn, done| {
                let json = LoginResponseHandler::new(Arc::clone(&slot));
                request(conn, build_json_post_no_auth(conn, PATH_LOGIN, &body), Box::new(json), mtx_tr("POST", PATH_LOGIN), "login", false, done)
            });
            one(
                pipe,
                start,
                move || result.lock().unwrap().take().ok_or_else(|| StoreError::new("Matrix login: no response")),
                on_complete,
            )
        }
        MatrixCommand::WellKnown { on_complete } => {
            let result: Arc<Mutex<Option<WellKnown>>> = Arc::new(Mutex::new(None));
            let slot = Arc::clone(&result);
            let start: Start = Arc::new(move |conn, done| {
                let json = WellKnownHandler::new(Arc::clone(&slot));
                let mut req = conn.request(Method::Get, WELL_KNOWN_PATH);
                req.set_matrix_outbound_trace("GET", WELL_KNOWN_PATH, false);
                // 404 is not an error for well-known: no delegation.
                request(conn, req, Box::new(json), mtx_tr("GET", WELL_KNOWN_PATH), "well-known", true, done)
            });
            one(pipe, start, move || Ok(result.lock().unwrap().take()), on_complete)
        }
        MatrixCommand::Sync { token, since, own_user_id, on_room, on_event, otk_count, device_lists_changed, on_to_device, on_complete } => {
            let next_batch: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let start = sync_start(token, since, own_user_id, on_room, on_event, otk_count, device_lists_changed, on_to_device, Arc::clone(&next_batch));
            one(pipe, start, move || Ok(next_batch.lock().unwrap().take()), on_complete)
        }
        MatrixCommand::JoinedRooms { token, on_complete } => {
            let rooms: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let start = joined_rooms_start(token, Arc::clone(&rooms));
            one(pipe, start, move || Ok(rooms.lock().unwrap().drain(..).collect()), on_complete)
        }
        MatrixCommand::ListFoldersAfterJoinedRooms {
            token,
            own_user_id,
            on_folder,
            on_room,
            on_event,
            otk_count,
            device_lists_changed,
            on_to_device,
            user_on_complete,
            sync_token,
            conn_for_keys,
            token_for_keys,
            crypto_for_keys,
            device_tracker,
        } => list_folders_after_joined_rooms(
            pipe,
            ListFoldersJob {
                token,
                own_user_id,
                on_folder,
                on_room,
                on_event,
                otk_count,
                device_lists_changed,
                on_to_device,
                user_on_complete,
                sync_token,
                conn_for_keys,
                token_for_keys,
                crypto_for_keys,
                device_tracker,
            },
        ),
        MatrixCommand::GetProfile { user_id, token, on_complete } => {
            let path = path_profile(&user_id);
            let profile: Arc<Mutex<Profile>> = Arc::new(Mutex::new(Profile::default()));
            let slot = Arc::clone(&profile);
            let start: Start = Arc::new(move |conn, done| {
                let json = ProfileHandler::new(Arc::clone(&slot));
                request(conn, build_get(conn, &path, &token), Box::new(json), mtx_tr("GET", path.clone()), "get profile", false, done)
            });
            one(pipe, start, move || Ok(profile.lock().unwrap().clone()), on_complete)
        }
        MatrixCommand::SetDisplayName { user_id, token, display_name, on_complete } => {
            let body = requests::build_display_name_body(&display_name);
            status_only(pipe, json_put_start(token, path_display_name(&user_id), body), on_complete)
        }
        MatrixCommand::SetAvatar { user_id, token, mxc_url, on_complete } => {
            let body = requests::build_avatar_url_body(&mxc_url);
            status_only(pipe, json_put_start(token, path_avatar_url(&user_id), body), on_complete)
        }
        MatrixCommand::SendMessage { token, room_id, body, txn_id, on_complete } => {
            status_only(pipe, json_put_start(token, path_send_message(&room_id, &txn_id), body), on_complete)
        }
        MatrixCommand::RoomMessages { token, room_id, limit, from, on_event, on_complete } => {
            let path = path_room_messages(&room_id, limit, from.as_deref());
            let end_token: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let slot = Arc::clone(&end_token);
            let start: Start = Arc::new(move |conn, done| {
                let on_event = Arc::clone(&on_event);
                let json = RoomMessagesHandler::new(room_id.clone(), move |event| on_event(event), Arc::clone(&slot));
                request(conn, build_get(conn, &path, &token), Box::new(json), mtx_tr("GET", path.clone()), "room messages", false, done)
            });
            one(pipe, start, move || Ok(end_token.lock().unwrap().take()), on_complete)
        }
        MatrixCommand::GetEvent { token, room_id, event_id, on_complete } => {
            let path = path_room_event(&room_id, &event_id);
            let result: Arc<Mutex<Option<RoomEvent>>> = Arc::new(Mutex::new(None));
            let slot = Arc::clone(&result);
            let start: Start = Arc::new(move |conn, done| {
                let json = SingleEventHandler::new(room_id.clone(), Arc::clone(&slot));
                request(conn, build_get(conn, &path, &token), Box::new(json), mtx_tr("GET", path.clone()), "get event", false, done)
            });
            one(pipe, start, move || Ok(result.lock().unwrap().take()), on_complete)
        }
        MatrixCommand::JoinRoom { token, room_id_or_alias, on_complete } => {
            status_only(pipe, json_post_start(token, path_join(&room_id_or_alias), requests::build_empty_body()), on_complete)
        }
        MatrixCommand::LeaveRoom { token, room_id, on_complete } => {
            status_only(pipe, json_post_start(token, path_leave(&room_id), requests::build_empty_body()), on_complete)
        }
        MatrixCommand::PublicRooms { token, body, on_complete } => {
            let raw: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
            let slot = Arc::clone(&raw);
            let start: Start = Arc::new(move |conn, done| {
                let json = RawBodyHandler::new(Arc::clone(&slot));
                request(conn, build_json_post(conn, PATH_PUBLIC_ROOMS, &token, &body), Box::new(json), mtx_tr("POST", PATH_PUBLIC_ROOMS), PATH_PUBLIC_ROOMS, false, done)
            });
            one(
                pipe,
                start,
                move || raw.lock().unwrap().take().ok_or_else(|| StoreError::new(format!("Matrix POST {}: empty response", PATH_PUBLIC_ROOMS))),
                on_complete,
            )
        }
        MatrixCommand::UploadMedia { token, content_type, data, on_complete } => {
            let path = path_media_upload();
            let mxc: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let slot = Arc::clone(&mxc);
            let start: Start = Arc::new(move |conn, done| {
                let json = MediaUploadHandler::new(Arc::clone(&slot));
                let mut req = conn.request(Method::Post, &path);
                req.header("Authorization", &format!("Bearer {}", token))
                    .header("Content-Type", &content_type)
                    .header("Content-Length", &data.len().to_string())
                    .body(data.clone());
                req.set_matrix_outbound_trace("POST", &path, true);
                request(conn, req, Box::new(json), mtx_tr("POST", path.clone()), "media upload", false, done)
            });
            one(
                pipe,
                start,
                move || mxc.lock().unwrap().take().ok_or_else(|| StoreError::new("Matrix media upload: no content_uri in response")),
                on_complete,
            )
        }
        MatrixCommand::UploadKeys { token, body, on_complete } => {
            let counts: Arc<Mutex<KeyUploadCounts>> = Arc::new(Mutex::new(KeyUploadCounts::default()));
            let slot = Arc::clone(&counts);
            let start: Start = Arc::new(move |conn, done| {
                let json = KeyUploadResponseHandler::new(Arc::clone(&slot));
                request(conn, build_json_post(conn, PATH_KEYS_UPLOAD, &token, &body), Box::new(json), mtx_tr("POST", PATH_KEYS_UPLOAD), "keys/upload", false, done)
            });
            one(pipe, start, move || Ok(counts.lock().unwrap().clone()), on_complete)
        }
        MatrixCommand::QueryKeys { token, body, on_complete } => {
            let result: Arc<Mutex<KeyQueryResult>> = Arc::new(Mutex::new(KeyQueryResult::default()));
            let slot = Arc::clone(&result);
            let start: Start = Arc::new(move |conn, done| {
                let json = KeyQueryResponseHandler::new(Arc::clone(&slot));
                request(conn, build_json_post(conn, PATH_KEYS_QUERY, &token, &body), Box::new(json), mtx_tr("POST", PATH_KEYS_QUERY), "keys/query", false, done)
            });
            one(pipe, start, move || Ok(result.lock().unwrap().clone()), on_complete)
        }
        MatrixCommand::ClaimKeys { token, body, on_complete } => {
            let result: Arc<Mutex<KeyClaimResult>> = Arc::new(Mutex::new(KeyClaimResult::default()));
            let slot = Arc::clone(&result);
            let start: Start = Arc::new(move |conn, done| {
                let json = KeyClaimResponseHandler::new(Arc::clone(&slot));
                request(conn, build_json_post(conn, PATH_KEYS_CLAIM, &token, &body), Box::new(json), mtx_tr("POST", PATH_KEYS_CLAIM), "keys/claim", false, done)
            });
            one(pipe, start, move || Ok(result.lock().unwrap().clone()), on_complete)
        }
        MatrixCommand::SendToDevice { token, event_type, txn_id, body, on_complete } => {
            status_only(pipe, json_put_start(token, path_send_to_device(&event_type, &txn_id), body), on_complete)
        }
        MatrixCommand::SetAccountData { token, user_id, event_type, body, on_complete } => {
            status_only(pipe, json_put_start(token, path_account_data(&user_id, &event_type), body), on_complete)
        }
        MatrixCommand::GetAccountData { token, user_id, event_type, on_complete } => {
            let path = path_account_data(&user_id, &event_type);
            let (start, raw) = get_raw_body_start(token, path.clone());
            one(pipe, start, move || take_raw(&raw, &path), on_complete)
        }
        MatrixCommand::CreateKeyBackup { token, body, on_complete } => {
            let version: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let slot = Arc::clone(&version);
            let start: Start = Arc::new(move |conn, done| {
                let json = VersionResponseHandler::new(Arc::clone(&slot));
                request(conn, build_json_post(conn, PATH_ROOM_KEYS_VERSION, &token, &body), Box::new(json), mtx_tr("POST", PATH_ROOM_KEYS_VERSION), "room_keys/version", false, done)
            });
            one(
                pipe,
                start,
                move || version.lock().unwrap().take().ok_or_else(|| StoreError::new("Matrix room_keys/version: no version in response")),
                on_complete,
            )
        }
        MatrixCommand::UploadRoomKeys { token, version, body, on_complete } => {
            status_only(pipe, json_put_start(token, path_room_keys(&version), body), on_complete)
        }
        MatrixCommand::DownloadRoomKeys { token, version, on_complete } => {
            let path = path_room_keys(&version);
            let (start, raw) = get_raw_body_start(token, path.clone());
            one(pipe, start, move || take_raw(&raw, &path), on_complete)
        }
        MatrixCommand::GetKeyBackupVersion { token, on_complete } => {
            let path = PATH_ROOM_KEYS_VERSION.to_string();
            let (start, raw) = get_raw_body_start(token, path.clone());
            step(
                pipe,
                start,
                Box::new(move |pipe, r| {
                    let parsed = match r.and_then(|()| take_raw(&raw, &path)) {
                        Ok(body) => parse_backup_version(&body),
                        Err(e) => {
                            let msg = e.to_string();
                            if msg.contains("404") || msg.contains("M_NOT_FOUND") {
                                Ok(None)
                            } else {
                                Err(e)
                            }
                        }
                    };
                    finish(pipe, on_complete, parsed)
                }),
            )
        }
        MatrixCommand::UploadSigningKeys { token, body, on_complete } => {
            status_only(pipe, json_post_start(token, PATH_DEVICE_SIGNING_UPLOAD.to_string(), body), on_complete)
        }
        MatrixCommand::UploadSignatures { token, body, on_complete } => {
            status_only(pipe, json_post_start(token, PATH_SIGNATURES_UPLOAD.to_string(), body), on_complete)
        }
    }
}

// ── Step builders ────────────────────────────────────────────────────

fn json_post_start(token: String, path: String, body: Vec<u8>) -> Start {
    Arc::new(move |conn, done| {
        request(conn, build_json_post(conn, &path, &token, &body), Box::new(NoOpHandler), mtx_tr("POST", path.clone()), "POST", false, done)
    })
}

fn json_put_start(token: String, path: String, body: Vec<u8>) -> Start {
    Arc::new(move |conn, done| {
        request(conn, build_json_put(conn, &path, &token, &body), Box::new(NoOpHandler), mtx_tr("PUT", path.clone()), "PUT", false, done)
    })
}

/// GET `path` keeping the raw body in the returned slot.
fn get_raw_body_start(token: String, path: String) -> (Start, Arc<Mutex<Option<Vec<u8>>>>) {
    let raw: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&raw);
    let start: Start = Arc::new(move |conn, done| {
        let json = RawBodyHandler::new(Arc::clone(&slot));
        request(conn, build_get(conn, &path, &token), Box::new(json), mtx_tr("GET", path.clone()), path.clone(), false, done)
    });
    (start, raw)
}

fn take_raw(raw: &Arc<Mutex<Option<Vec<u8>>>>, path: &str) -> Result<Vec<u8>, StoreError> {
    raw.lock().unwrap().take().ok_or_else(|| StoreError::new(format!("Matrix GET {}: empty response", path)))
}

fn joined_rooms_start(token: String, rooms: Arc<Mutex<Vec<String>>>) -> Start {
    Arc::new(move |conn, done| {
        rooms.lock().unwrap().clear();
        let json = JoinedRoomsHandler::new(Arc::clone(&rooms));
        request(conn, build_get(conn, PATH_JOINED_ROOMS, &token), Box::new(json), mtx_tr("GET", PATH_JOINED_ROOMS), "joined_rooms", false, done)
    })
}

#[allow(clippy::too_many_arguments)]
fn sync_start(
    token: String,
    since: Option<String>,
    own_user_id: Option<String>,
    on_room: Arc<dyn Fn(RoomSummary) + Send + Sync>,
    on_event: Arc<dyn Fn(RoomEvent) + Send + Sync>,
    otk_count: Arc<Mutex<Option<usize>>>,
    device_lists_changed: Arc<Mutex<Vec<String>>>,
    on_to_device: Arc<dyn Fn(String, String, String) + Send + Sync>,
    next_batch: Arc<Mutex<Option<String>>>,
) -> Start {
    let mut path = PATH_SYNC.to_string();
    let mut sep = '?';
    if let Some(s) = since {
        path.push_str(&format!("{}since={}", sep, s));
        sep = '&';
        path.push_str(&format!("{}timeout=30000", sep));
    }
    Arc::new(move |conn, done| {
        let (on_room, on_event, otk, dl, on_to_device) = (
            Arc::clone(&on_room),
            Arc::clone(&on_event),
            Arc::clone(&otk_count),
            Arc::clone(&device_lists_changed),
            Arc::clone(&on_to_device),
        );
        let json = SyncResponseHandler::with_e2ee(
            move |room| on_room(room),
            move |event| on_event(event),
            Arc::clone(&next_batch),
            otk,
            dl,
            Box::new(move |etype, sender, content| on_to_device(etype, sender, content)),
            own_user_id.clone(),
        );
        request(conn, build_get(conn, &path, &token), Box::new(json), mtx_tr("GET", path.clone()), "sync", false, done)
    })
}

/// [`GET /state/…`](https://spec.matrix.org/latest/client-server-api/#get_matrixclientv3roomsroomidstateeventtypestatekey)
/// via [`RoomStateDisplayLabelHandler`](super::json_handlers::RoomStateDisplayLabelHandler); a 404 leaves the slot empty.
fn room_state_label_start(token: String, path: String, trace_label: &'static str, out: Arc<Mutex<Option<String>>>) -> Start {
    Arc::new(move |conn, done| {
        *out.lock().unwrap() = None;
        let json = RoomStateDisplayLabelHandler::new(Arc::clone(&out));
        request(conn, build_get(conn, &path, &token), Box::new(json), mtx_tr("GET", path.clone()), trace_label, true, done)
    })
}

// ── list_folders: /joined_rooms, labels, then the first /sync ───────

struct ListFoldersJob {
    token: String,
    own_user_id: String,
    on_folder: Arc<dyn Fn(FolderInfo) + Send + Sync>,
    on_room: Arc<dyn Fn(RoomSummary) + Send + Sync>,
    on_event: Arc<dyn Fn(RoomEvent) + Send + Sync>,
    otk_count: Arc<Mutex<Option<usize>>>,
    device_lists_changed: Arc<Mutex<Vec<String>>>,
    on_to_device: Arc<dyn Fn(String, String, String) + Send + Sync>,
    user_on_complete: Arc<Mutex<Option<Box<dyn FnOnce(Result<(), StoreError>) + Send>>>>,
    sync_token: Arc<Mutex<Option<String>>>,
    conn_for_keys: MatrixConnection,
    token_for_keys: String,
    crypto_for_keys: Option<Arc<CryptoMachine>>,
    device_tracker: Arc<DeviceTracker>,
}

impl ListFoldersJob {
    fn user_complete(&self, r: Result<(), StoreError>) -> bool {
        match self.user_on_complete.lock().unwrap().take() {
            Some(f) => {
                f(r);
                true
            }
            None => false,
        }
    }
}

fn list_folders_after_joined_rooms(pipe: Pipe, job: ListFoldersJob) {
    let rooms: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let start = joined_rooms_start(job.token.clone(), Arc::clone(&rooms));
    step(
        pipe,
        start,
        Box::new(move |pipe, r| {
            let joined: Result<Vec<String>, StoreError> = r.map(|()| rooms.lock().unwrap().drain(..).collect());
            eprintln!(
                "[matrix] list_folders: /joined_rooms -> {:?} room id(s); fetching m.room.name, then completing list_folders (full /sync follows)",
                joined.as_ref().map(|v| v.len())
            );
            match joined {
                Ok(ids) => {
                    for id in &ids {
                        (job.on_folder)(FolderInfo { name: id.clone(), delimiter: None, attributes: vec![] });
                    }
                    label_rooms(pipe, job, ids.into(), false)
                }
                Err(e) => {
                    eprintln!("[matrix] list_folders: /joined_rooms failed ({}); folder discovery from /sync only", e);
                    initial_sync(pipe, job, false)
                }
            }
        }),
    );
}

/// Label each joined room (`m.room.name`, then `m.room.canonical_alias`)
/// in turn, then complete the user's list_folders and start the sync.
fn label_rooms(pipe: Pipe, job: ListFoldersJob, mut ids: VecDeque<String>, user_done: bool) {
    let Some(room_id) = ids.pop_front() else {
        let user_done = user_done || job.user_complete(Ok(()));
        return initial_sync(pipe, job, user_done);
    };
    let label: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let start = room_state_label_start(job.token.clone(), path_room_m_room_name(&room_id), "m.room.name", Arc::clone(&label));
    step(
        pipe,
        start,
        Box::new(move |pipe, r| {
            if let Err(e) = r {
                eprintln!("[matrix] list_folders: GET m.room.name for {}: {}", room_id, e);
            }
            let found = label.lock().unwrap().take();
            if let Some(found) = found {
                labelled(&job, &room_id, found);
                return label_rooms(pipe, job, ids, user_done);
            }
            let start = room_state_label_start(job.token.clone(), path_room_m_room_canonical_alias(&room_id), "m.room.canonical_alias", Arc::clone(&label));
            step(
                pipe,
                start,
                Box::new(move |pipe, r| {
                    if let Err(e) = r {
                        eprintln!("[matrix] list_folders: GET m.room.canonical_alias for {}: {}", room_id, e);
                    }
                    let found = label.lock().unwrap().take();
                    if let Some(found) = found {
                        labelled(&job, &room_id, found);
                    }
                    label_rooms(pipe, job, ids, user_done)
                }),
            );
        }),
    );
}

fn labelled(job: &ListFoldersJob, room_id: &str, label: String) {
    (job.on_folder)(FolderInfo {
        name: room_id.to_string(),
        delimiter: None,
        attributes: vec![format!("display_name={}", label)],
    });
}

/// The full initial `/sync` (E2EE state and `next_batch`), then one-time
/// key upload if the crypto machine needs it.
fn initial_sync(pipe: Pipe, job: ListFoldersJob, user_done: bool) {
    let next_batch: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let start = sync_start(
        job.token.clone(),
        None,
        Some(job.own_user_id.clone()),
        Arc::clone(&job.on_room),
        Arc::clone(&job.on_event),
        Arc::clone(&job.otk_count),
        Arc::clone(&job.device_lists_changed),
        Arc::clone(&job.on_to_device),
        Arc::clone(&next_batch),
    );
    step(
        pipe,
        start,
        Box::new(move |pipe, r| {
            match r {
                Ok(()) => {
                    if let Some(nb) = next_batch.lock().unwrap().take() {
                        if let Ok(mut st) = job.sync_token.lock() {
                            *st = Some(nb);
                        }
                    }
                    if let Ok(dl) = job.device_lists_changed.lock() {
                        if !dl.is_empty() {
                            job.device_tracker.mark_users_dirty(&dl);
                        }
                    }
                    if let Some(ref cm) = job.crypto_for_keys {
                        let count = job.otk_count.lock().unwrap().take();
                        let otk = cm.generate_one_time_keys_if_needed(count.unwrap_or(0));
                        if !otk.is_empty() {
                            let otk_json = cm.one_time_keys_json(&otk);
                            let body = device::build_keys_upload_body(None, Some(&otk_json));
                            let cm_clone = cm.clone();
                            job.conn_for_keys.send(MatrixCommand::UploadKeys {
                                token: job.token_for_keys.clone(),
                                body,
                                on_complete: Box::new(move |result| {
                                    if result.is_ok() {
                                        cm_clone.mark_keys_as_published();
                                    }
                                }),
                            });
                        }
                    }
                    if !user_done {
                        job.user_complete(Ok(()));
                    }
                }
                Err(e) => {
                    eprintln!("[matrix] list_folders: /sync failed: {}", e);
                    if !user_done {
                        job.user_complete(Err(e));
                    }
                }
            }
            pipe.lock().unwrap().busy = false;
            run_next(pipe);
        }),
    );
}

// ── HTTP request helpers ─────────────────────────────────────────────

fn build_get(conn: &HttpConnection, path: &str, token: &str) -> RequestBuilder {
    let mut req = conn.request(Method::Get, path);
    req.header("Authorization", &format!("Bearer {}", token));
    req.set_matrix_outbound_trace("GET", path, true);
    req
}

fn build_json_post(conn: &HttpConnection, path: &str, token: &str, body: &[u8]) -> RequestBuilder {
    let mut req = conn.request(Method::Post, path);
    req.header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Content-Length", &body.len().to_string())
        .body(body.to_vec());
    req.set_matrix_outbound_trace("POST", path, true);
    req
}

fn build_json_put(conn: &HttpConnection, path: &str, token: &str, body: &[u8]) -> RequestBuilder {
    let mut req = conn.request(Method::Put, path);
    req.header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Content-Length", &body.len().to_string())
        .body(body.to_vec());
    req.set_matrix_outbound_trace("PUT", path, true);
    req
}

fn build_json_post_no_auth(conn: &HttpConnection, path: &str, body: &[u8]) -> RequestBuilder {
    let mut req = conn.request(Method::Post, path);
    req.header("Content-Type", "application/json")
        .header("Content-Length", &body.len().to_string())
        .body(body.to_vec());
    req.set_matrix_outbound_trace("POST", path, false);
    req
}

// ── Response handler ─────────────────────────────────────────────────

type SharedErrorDetail = Arc<Mutex<(String, String)>>;

/// Streaming ResponseHandler that feeds body chunks directly into a JsonParser.
///
/// On 2xx: uses the endpoint-specific handler.
/// On 4xx/5xx: swaps to MatrixErrorHandler.
///
/// The outcome reaches `done` once: Ok after a 2xx body is fully read (or
/// a 404 when `not_found_ok`), the parsed Matrix error otherwise.
struct MatrixResponseHandler {
    status_code: u16,
    is_error: bool,
    parser: Parser<Box<dyn ContentHandler + Send>>,
    /// Error bodies only, for the trace line.
    buf: BytesMut,
    error: Option<MatrixApiError>,
    err_detail: SharedErrorDetail,
    /// When set, [`end_body`](ResponseHandler::end_body) logs the aggregated response for `TAGLIACARTE_TRACE=matrix`.
    http_trace: MatrixHttpTrace,
    /// Total response body length (for stderr line even when we do not retain full bytes).
    body_total_len: usize,
    /// Response headers (for `TAGLIACARTE_TRACE` line when the response head completes).
    response_headers: Vec<(String, String)>,
    /// Sequence number for [`trace::log_inbound_body_chunk`].
    inbound_body_chunk_seq: usize,
    context: String,
    not_found_ok: bool,
    done: Option<Done<()>>,
}

impl MatrixResponseHandler {
    fn new(
        handler: Box<dyn ContentHandler + Send>,
        http_trace: MatrixHttpTrace,
        context: String,
        not_found_ok: bool,
        done: Done<()>,
    ) -> Self {
        Self {
            status_code: 0,
            is_error: false,
            parser: Parser::new(handler),
            buf: BytesMut::new(),
            error: None,
            err_detail: Arc::new(Mutex::new((String::new(), String::new()))),
            http_trace,
            body_total_len: 0,
            response_headers: Vec::new(),
            inbound_body_chunk_seq: 0,
            context,
            not_found_ok,
            done: Some(done),
        }
    }
}

impl ResponseHandler for MatrixResponseHandler {
    fn ok(&mut self, response: Response) {
        self.status_code = response.code;
    }

    fn error(&mut self, response: Response) {
        self.status_code = response.code;
        self.is_error = true;
        self.parser = Parser::new(Box::new(MatrixErrorJsonHandler::new(self.err_detail.clone())));
    }

    fn header(&mut self, name: &str, value: &str) {
        if self.response_headers.len() < 128 {
            self.response_headers
                .push((name.to_string(), value.to_string()));
        }
    }

    fn start_body(&mut self) {
        if let Some((ref method, ref path)) = self.http_trace {
            if crate::trace::enabled("matrix") {
                trace::log_inbound_response_head(
                    method,
                    path,
                    self.status_code,
                    &self.response_headers,
                );
            }
            self.response_headers.clear();
        }
    }

    fn body_chunk(&mut self, data: &[u8]) {
        self.body_total_len = self.body_total_len.saturating_add(data.len());
        if let Some((ref method, ref path)) = self.http_trace {
            if crate::trace::full_enabled("matrix") {
                trace::log_inbound_body_chunk(
                    method,
                    path,
                    self.inbound_body_chunk_seq,
                    data,
                );
                self.inbound_body_chunk_seq += 1;
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
            let (errcode, error_msg) = {
                let d = self.err_detail.lock().unwrap();
                (d.0.clone(), d.1.clone())
            };
            self.error = Some(MatrixApiError { status: self.status_code, errcode, error: error_msg });
        }
        if let Some((ref method, ref path)) = self.http_trace {
            if crate::trace::full_enabled("matrix") {
                trace::log_inbound_body_complete_full(method, path, self.status_code, self.body_total_len);
            } else {
                trace::log_response(method, path, self.status_code, &[], self.body_total_len);
            }
        }
    }

    fn complete(&mut self) {
        // e.g. HTTP/2 response with HEADERS+END_STREAM and no body: no `start_body` / `end_body`.
        if let Some((ref method, ref path)) = self.http_trace {
            if crate::trace::enabled("matrix") && !self.response_headers.is_empty() {
                trace::log_inbound_response_head(
                    method,
                    path,
                    self.status_code,
                    &self.response_headers,
                );
                self.response_headers.clear();
            }
        }
        let Some(done) = self.done.take() else { return };
        if self.is_error && self.error.is_none() {
            self.error = Some(MatrixApiError { status: self.status_code, errcode: String::new(), error: String::new() });
        }
        match self.error.take() {
            Some(me) if me.status == 404 && self.not_found_ok => done(Ok(())),
            Some(me) => {
                eprintln!("[matrix] {} error: {}", self.context, me);
                done(Err(StoreError::new(format!("Matrix {}: {}", self.context, me))))
            }
            None => done(Ok(())),
        }
    }

    fn failed(&mut self, error: &std::io::Error) {
        if crate::trace::enabled("matrix") {
            if let Some((ref method, ref path)) = self.http_trace {
                eprintln!(
                    "[matrix trace] << {method} {path} transport error: {error}"
                );
            }
        }
        if let Some(done) = self.done.take() {
            done(Err(StoreError::new(format!("Matrix {} failed: {}", self.context, error))));
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
        Self {
            current_key: None,
            detail,
        }
    }
}

impl ContentHandler for MatrixErrorJsonHandler {
    fn start_object(&mut self) -> ParseResult<()> { Ok(()) }
    fn end_object(&mut self) -> ParseResult<()> { Ok(()) }
    fn start_array(&mut self) -> ParseResult<()> { Ok(()) }
    fn end_array(&mut self) -> ParseResult<()> { Ok(()) }

    fn key(&mut self, key: &str) -> ParseResult<()> {
        self.current_key = Some(key.to_string());
        Ok(())
    }

    fn string_value(&mut self, value: &str) -> ParseResult<()> {
        if let Ok(mut d) = self.detail.lock() {
            match self.current_key.as_deref() {
                Some("errcode") => d.0 = value.to_string(),
                Some("error") => d.1 = value.to_string(),
                _ => {}
            }
        }
        self.current_key = None;
        Ok(())
    }

    fn number_value(&mut self, _: &Number) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
    }
    fn boolean_value(&mut self, _: bool) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
    }
    fn null_value(&mut self) -> ParseResult<()> {
        self.current_key = None;
        Ok(())
    }
}

fn parse_backup_version(body: &[u8]) -> Result<Option<(String, String)>, StoreError> {
    let text = std::str::from_utf8(body)
        .map_err(|_| StoreError::new("backup version response is not UTF-8"))?;
    let version = super::extract_json_string(text, "version");
    let algorithm = super::extract_json_string(text, "algorithm");
    match (version, algorithm) {
        (Some(v), Some(a)) => Ok(Some((v, a))),
        _ => Ok(None),
    }
}
