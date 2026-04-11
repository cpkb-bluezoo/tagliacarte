/*
 * parse.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Matrix Client-Server JSON: [`JsonContentHandler`] only — no DOM.
 */

use std::collections::HashSet;

use crate::json::{parse_bytes_complete, JsonContentHandler, JsonNumber};
use crate::store::StoreError;

fn json_err(e: crate::json::JsonError) -> StoreError {
    StoreError::new(format!("matrix json: {e}"))
}

// ── Room keys backup download body ──────────────────────────────────────

/// One session entry from a key backup download (fields used when Megolm import exists).
#[allow(dead_code)]
pub(crate) struct RoomKeySessionRow {
    pub room_id: String,
    pub session_id: String,
    pub ephemeral: String,
    pub ciphertext: String,
    pub mac: String,
}

struct RoomKeysBackupHandler {
    depth: usize,
    key_field: Option<String>,
    expect_session_data_object: bool,
    session_data_depth: usize,
    pending_room_id: Option<String>,
    pending_session_id: Option<String>,
    ephemeral: Option<String>,
    ciphertext: Option<String>,
    mac: Option<String>,
    pub(crate) rows: Vec<RoomKeySessionRow>,
}

impl JsonContentHandler for RoomKeysBackupHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.expect_session_data_object {
            self.expect_session_data_object = false;
            self.session_data_depth = self.depth;
        }
    }

    fn end_object(&mut self) {
        if self.session_data_depth > 0 && self.depth == self.session_data_depth {
            if let (Some(rid), Some(sid), Some(e), Some(c), Some(m)) = (
                self.pending_room_id.clone(),
                self.pending_session_id.clone(),
                self.ephemeral.take(),
                self.ciphertext.take(),
                self.mac.take(),
            ) {
                self.rows.push(RoomKeySessionRow {
                    room_id: rid,
                    session_id: sid,
                    ephemeral: e,
                    ciphertext: c,
                    mac: m,
                });
            }
            self.session_data_depth = 0;
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {}

    fn end_array(&mut self) {}

    fn key(&mut self, key: &str) {
        if key == "session_data" {
            self.expect_session_data_object = true;
        }
        if self.depth == 2 {
            self.pending_room_id = Some(key.to_string());
        }
        if self.depth == 4 {
            self.pending_session_id = Some(key.to_string());
        }
        self.key_field = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.session_data_depth > 0 {
            match self.key_field.as_deref() {
                Some("ephemeral") => self.ephemeral = Some(value.to_string()),
                Some("ciphertext") => self.ciphertext = Some(value.to_string()),
                Some("mac") => self.mac = Some(value.to_string()),
                _ => {}
            }
        }
        self.key_field = None;
    }

    fn number_value(&mut self, _: JsonNumber) {
        self.key_field = None;
    }

    fn boolean_value(&mut self, _: bool) {
        self.key_field = None;
    }

    fn null_value(&mut self) {
        self.key_field = None;
    }
}

pub(crate) fn parse_room_keys_backup(bytes: &[u8]) -> Result<Vec<RoomKeySessionRow>, StoreError> {
    let mut h = RoomKeysBackupHandler {
        depth: 0,
        key_field: None,
        expect_session_data_object: false,
        session_data_depth: 0,
        pending_room_id: None,
        pending_session_id: None,
        ephemeral: None,
        ciphertext: None,
        mac: None,
        rows: Vec::new(),
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    Ok(h.rows)
}

// ── m.direct: `content.global` or top-level `global` — values are string arrays ──

struct MDirectHandler {
    depth: usize,
    key_field: Option<String>,
    expect_content_obj: bool,
    inside_content: bool,
    expect_global_obj: bool,
    global_depth: usize,
    global_array_depth: usize,
    out: HashSet<String>,
}

impl JsonContentHandler for MDirectHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.expect_content_obj {
            self.expect_content_obj = false;
            self.inside_content = true;
        }
        if self.expect_global_obj {
            self.expect_global_obj = false;
            self.global_depth = self.depth;
        }
    }

    fn end_object(&mut self) {
        if self.global_depth > 0 && self.depth == self.global_depth {
            self.global_depth = 0;
        }
        if self.inside_content && self.depth == 2 {
            self.inside_content = false;
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        if self.global_depth > 0 && self.depth > self.global_depth {
            self.global_array_depth = self.depth;
        }
    }

    fn end_array(&mut self) {
        if self.global_array_depth > 0 && self.depth == self.global_array_depth {
            self.global_array_depth = 0;
        }
    }

    fn key(&mut self, key: &str) {
        if key == "content" && self.depth == 1 {
            self.expect_content_obj = true;
        }
        if key == "global" && (self.depth == 1 || (self.depth == 2 && self.inside_content)) {
            self.expect_global_obj = true;
        }
        self.key_field = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.global_array_depth > 0 {
            self.out.insert(value.to_string());
        }
        self.key_field = None;
    }

    fn number_value(&mut self, _: JsonNumber) {
        self.key_field = None;
    }

    fn boolean_value(&mut self, _: bool) {
        self.key_field = None;
    }

    fn null_value(&mut self) {
        self.key_field = None;
    }
}

pub(crate) fn parse_m_direct_event_body(bytes: &[u8]) -> HashSet<String> {
    let mut h = MDirectHandler {
        depth: 0,
        key_field: None,
        expect_content_obj: false,
        inside_content: false,
        expect_global_obj: false,
        global_depth: 0,
        global_array_depth: 0,
        out: HashSet::new(),
    };
    let _ = parse_bytes_complete(bytes, &mut h);
    h.out
}

// ── `publicRooms` chunk ─────────────────────────────────────────────────

struct PublicRoomsHandler {
    depth: usize,
    key_field: Option<String>,
    in_chunk_array: bool,
    in_row: bool,
    room_id: Option<String>,
    name: Option<String>,
    pub(crate) rows: Vec<(String, Option<String>)>,
}

impl JsonContentHandler for PublicRoomsHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.in_chunk_array && self.depth == 2 {
            self.in_row = true;
            self.room_id = None;
            self.name = None;
        }
    }

    fn end_object(&mut self) {
        if self.in_row && self.depth == 2 {
            if let Some(rid) = self.room_id.take() {
                let n = self.name.take();
                self.rows.push((rid, n));
            }
            self.in_row = false;
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        if self.key_field.as_deref() == Some("chunk") {
            self.in_chunk_array = true;
        }
    }

    fn end_array(&mut self) {
        if self.in_chunk_array {
            self.in_chunk_array = false;
        }
    }

    fn key(&mut self, key: &str) {
        self.key_field = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.in_row {
            match self.key_field.as_deref() {
                Some("room_id") => self.room_id = Some(value.to_string()),
                Some("name") => self.name = Some(value.to_string()),
                _ => {}
            }
        }
        self.key_field = None;
    }

    fn number_value(&mut self, _: JsonNumber) {
        self.key_field = None;
    }

    fn boolean_value(&mut self, _: bool) {
        self.key_field = None;
    }

    fn null_value(&mut self) {
        self.key_field = None;
    }
}

pub(crate) fn parse_public_rooms_response(
    bytes: &[u8],
) -> Result<Vec<(String, Option<String>)>, StoreError> {
    let mut h = PublicRoomsHandler {
        depth: 0,
        key_field: None,
        in_chunk_array: false,
        in_row: false,
        room_id: None,
        name: None,
        rows: Vec::new(),
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    Ok(h.rows)
}
