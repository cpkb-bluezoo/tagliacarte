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
//! All bodies are generated using `JsonWriter` — no serde_json.

use crate::json::JsonWriter;
use crate::store::{Address, SendPayload};

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

/// Build the JSON body for `POST /me/sendMail`.
pub fn build_send_mail_body(payload: &SendPayload) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();

    // "message": { ... }
    w.write_key("message");
    w.write_start_object();

    // subject
    w.write_key("subject");
    w.write_string(payload.subject.as_deref().unwrap_or(""));

    // body
    let (content_type, content) = payload
        .body_html
        .as_ref()
        .map(|h| ("HTML", h.as_str()))
        .or_else(|| payload.body_plain.as_ref().map(|p| ("Text", p.as_str())))
        .unwrap_or(("Text", ""));

    w.write_key("body");
    w.write_start_object();
    w.write_key("contentType");
    w.write_string(content_type);
    w.write_key("content");
    w.write_string(content);
    w.write_end_object();

    // toRecipients
    w.write_key("toRecipients");
    write_recipient_array(&mut w, &payload.to);

    // ccRecipients
    w.write_key("ccRecipients");
    write_recipient_array(&mut w, &payload.cc);

    // from
    if let Some(from) = payload.from.first() {
        w.write_key("from");
        write_recipient(&mut w, from);
    }

    // attachments
    if !payload.attachments.is_empty() {
        w.write_key("attachments");
        w.write_start_array();
        for att in &payload.attachments {
            w.write_start_object();
            w.write_key("@odata.type");
            w.write_string("#microsoft.graph.fileAttachment");
            w.write_key("name");
            w.write_string(att.filename.as_deref().unwrap_or("attachment"));
            w.write_key("contentType");
            w.write_string(&att.mime_type);
            w.write_key("contentBytes");
            w.write_string(&base64_encode(&att.content));
            w.write_end_object();
        }
        w.write_end_array();
    }

    w.write_end_object(); // end message

    // "saveToSentItems": true
    w.write_key("saveToSentItems");
    w.write_bool(true);

    w.write_end_object(); // end root
    w.take_buffer().to_vec()
}

// ── Helpers ───────────────────────────────────────────────────────────

fn write_recipient_array(w: &mut JsonWriter, addrs: &[Address]) {
    w.write_start_array();
    for addr in addrs {
        write_recipient(w, addr);
    }
    w.write_end_array();
}

fn write_recipient(w: &mut JsonWriter, addr: &Address) {
    let email_addr = match &addr.domain {
        Some(d) if !d.is_empty() => format!("{}@{}", addr.local_part, d),
        _ => addr.local_part.clone(),
    };
    w.write_start_object();
    w.write_key("emailAddress");
    w.write_start_object();
    w.write_key("name");
    w.write_string(addr.display_name.as_deref().unwrap_or(""));
    w.write_key("address");
    w.write_string(&email_addr);
    w.write_end_object();
    w.write_end_object();
}
