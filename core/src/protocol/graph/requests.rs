/*
 * requests.rs
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

//! JSON request body builders for Microsoft Graph API calls.
//! All JSON bodies are generated using `JsonWriter` — no serde_json.
//!
//! `POST /me/sendMail` uses MIME mode: the HTTP body is base64 (see [`build_send_mail_mime_body`]).

use crate::json::JsonWriter;

use super::base64_encode;

/// Build the JSON body for creating a mail folder: `{"displayName":"…"}`.
pub fn build_create_folder_body(name: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("displayName");
    w.write_string(name);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Build the JSON body for renaming a mail folder: `{"displayName":"…"}`.
pub fn build_rename_folder_body(new_name: &str) -> Vec<u8> {
    build_create_folder_body(new_name)
}

/// Build the JSON body for copy/move: `{"destinationId":"…"}`.
pub fn build_copy_move_body(dest_id: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("destinationId");
    w.write_string(dest_id);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Build the JSON body for PATCH flag updates.
///
/// Maps Seen → isRead, Flagged → importance, and ignores other flags.
pub fn build_flag_patch_body(
    add: &[crate::store::Flag],
    remove: &[crate::store::Flag],
) -> Option<Vec<u8>> {
    use crate::store::Flag;

    let mut has_content = false;
    let mut w = JsonWriter::new();
    w.write_start_object();

    for flag in add {
        match flag {
            Flag::Seen => {
                w.write_key("isRead");
                w.write_bool(true);
                has_content = true;
            }
            Flag::Flagged => {
                w.write_key("importance");
                w.write_string("high");
                has_content = true;
            }
            _ => {}
        }
    }
    for flag in remove {
        match flag {
            Flag::Seen => {
                w.write_key("isRead");
                w.write_bool(false);
                has_content = true;
            }
            Flag::Flagged => {
                w.write_key("importance");
                w.write_string("normal");
                has_content = true;
            }
            _ => {}
        }
    }

    w.write_end_object();

    if has_content {
        Some(w.take_buffer().to_vec())
    } else {
        None
    }
}

/// Build the HTTP body for `POST /me/sendMail` in MIME mode.
///
/// Microsoft Graph expects `Content-Type: text/plain` and a body that is the standard base64
/// encoding of the full RFC 822 message (not JSON).
pub fn build_send_mail_mime_body(rfc822: &[u8]) -> Vec<u8> {
    base64_encode(rfc822).into_bytes()
}
