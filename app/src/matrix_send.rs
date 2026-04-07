/*
 * matrix_send.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
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
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Send Matrix room messages via [`MatrixTransport`], paired with the session [`MatrixStore`].
//!
//! **Stores** load and list messages through the folder abstraction; **transports** send.
//! The Matrix transport shares token and E2EE state with its store. Routing by account type
//! lives in [`crate::session`].

use std::sync::mpsc;

use tagliacarte_core::protocol::matrix::MatrixStore;
use tagliacarte_core::store::{Address, SendPayload, StoreError, Transport};

use crate::frb_api::FrbAccount;
use crate::mail_kind::is_matrix_store;
use crate::mail_store::open_cached_store;

/// Send a text (and optional HTML) message to a Matrix room.
///
/// Uses the cached [`MatrixStore`] for the account so the transport shares login and crypto
/// with the rest of the session.
pub fn send_matrix_room_message(
    acc: &FrbAccount,
    room_id: &str,
    body_plain: &str,
    body_html: Option<&str>,
    use_keychain: bool,
) -> Result<(), String> {
    if !is_matrix_store(acc.backend_type.as_str()) {
        return Err("send_matrix_room_message: not a Matrix account".to_owned());
    }
    let store = open_cached_store(acc, use_keychain)?;
    let matrix: &MatrixStore = store.as_any().downcast_ref::<MatrixStore>().ok_or_else(|| {
        "send_matrix_room_message: store is not MatrixStore (internal error)".to_owned()
    })?;
    let transport = matrix.paired_transport().map_err(|e| e.to_string())?;
    let mut payload = SendPayload::default();
    payload.to.push(Address {
        display_name: None,
        local_part: room_id.trim().to_string(),
        domain: None,
    });
    payload.body_plain = Some(body_plain.to_string());
    payload.body_html = body_html.map(|s| s.to_string());
    let (tx, rx) = mpsc::channel::<Result<(), StoreError>>();
    transport.send(
        &payload,
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    rx.recv()
        .map_err(|_| "Matrix send: internal channel closed".to_string())?
        .map_err(|e| e.to_string())
}
