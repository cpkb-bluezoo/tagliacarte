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

//! JSON request body builders for Matrix Client-Server API calls.
//! All bodies are generated using `JsonWriter` — no serde_json.

use crate::json::JsonWriter;

/// Login body: `{"type":"m.login.password","identifier":{"type":"m.id.user","user":"..."},"password":"..."}`.
pub fn build_login_body(user: &str, password: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();

    w.write_key("type");
    w.write_string("m.login.password");

    w.write_key("identifier");
    w.write_start_object();
    w.write_key("type");
    w.write_string("m.id.user");
    w.write_key("user");
    w.write_string(user);
    w.write_end_object();

    w.write_key("password");
    w.write_string(password);

    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Send text message body: `{"msgtype":"m.text","body":"..."}`.
pub fn build_text_message_body(body: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("msgtype");
    w.write_string("m.text");
    w.write_key("body");
    w.write_string(body);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Send media message body (after upload):
/// `{"msgtype":"m.image","body":"filename","url":"mxc://...","info":{"mimetype":"..."}}`.
pub fn build_media_message_body(msgtype: &str, filename: &str, mxc_url: &str, mime_type: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("msgtype");
    w.write_string(msgtype);
    w.write_key("body");
    w.write_string(filename);
    w.write_key("url");
    w.write_string(mxc_url);
    w.write_key("info");
    w.write_start_object();
    w.write_key("mimetype");
    w.write_string(mime_type);
    w.write_end_object();
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Set display name body: `{"displayname":"..."}`.
pub fn build_display_name_body(display_name: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("displayname");
    w.write_string(display_name);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Set avatar URL body: `{"avatar_url":"mxc://..."}`.
pub fn build_avatar_url_body(mxc_url: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("avatar_url");
    w.write_string(mxc_url);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Empty body for POST endpoints that require no payload (join, leave).
pub fn build_empty_body() -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_end_object();
    w.take_buffer().to_vec()
}
