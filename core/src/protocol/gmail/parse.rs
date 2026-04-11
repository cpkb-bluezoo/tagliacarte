/*
 * parse.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

//! Gmail API JSON: [`JsonContentHandler`] state machines only — no DOM, no serde_json.
//! Wire shapes are mapped into small typed structs used by [`super`].

use std::collections::HashMap;

use crate::json::{parse_bytes_complete, JsonContentHandler, JsonNumber};
use crate::store::{MessageAttachmentRef, StoreError};

use super::GMAIL_ATTACHMENT_SECTION_PREFIX;

fn json_err(e: crate::json::JsonError) -> StoreError {
    StoreError::new(format!("gmail json: {e}"))
}

// ── Attachment `{"data":"..."}` ─────────────────────────────────────────

struct AttachmentDataHandler {
    key: Option<String>,
    data: Option<String>,
}

impl JsonContentHandler for AttachmentDataHandler {
    fn start_object(&mut self) {}
    fn end_object(&mut self) {}
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, key: &str) {
        self.key = Some(key.to_string());
    }
    fn string_value(&mut self, value: &str) {
        if self.key.as_deref() == Some("data") {
            self.data = Some(value.to_string());
        }
        self.key = None;
    }
    fn number_value(&mut self, _: JsonNumber) {
        self.key = None;
    }
    fn boolean_value(&mut self, _: bool) {
        self.key = None;
    }
    fn null_value(&mut self) {
        self.key = None;
    }
}

pub(crate) fn parse_attachment_data(bytes: &[u8]) -> Result<String, StoreError> {
    let mut h = AttachmentDataHandler {
        key: None,
        data: None,
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    h.data.ok_or_else(|| StoreError::new("gmail attachment: missing data"))
}

// ── Labels list `{ "labels": [ {"id","name"}, ... ] }` ───────────────────

struct LabelsListHandler {
    depth: usize,
    key: Option<String>,
    in_labels_array: bool,
    in_label_obj: bool,
    cur_id: Option<String>,
    cur_name: Option<String>,
    pub(crate) labels: Vec<(String, String)>,
}

impl JsonContentHandler for LabelsListHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.in_labels_array {
            self.in_label_obj = true;
        }
    }
    fn end_object(&mut self) {
        if self.depth == 2 && self.in_label_obj {
            if let (Some(id), Some(name)) = (self.cur_id.take(), self.cur_name.take()) {
                if !id.is_empty() && !name.is_empty() {
                    self.labels.push((id, name));
                }
            }
            self.in_label_obj = false;
        }
        self.depth -= 1;
    }
    fn start_array(&mut self) {
        if self.depth == 1 && self.key.as_deref() == Some("labels") {
            self.in_labels_array = true;
        }
    }
    fn end_array(&mut self) {
        if self.in_labels_array {
            self.in_labels_array = false;
        }
    }
    fn key(&mut self, key: &str) {
        self.key = Some(key.to_string());
    }
    fn string_value(&mut self, value: &str) {
        if self.depth == 2 && self.in_label_obj {
            match self.key.as_deref() {
                Some("id") => self.cur_id = Some(value.to_string()),
                Some("name") => self.cur_name = Some(value.to_string()),
                _ => {}
            }
        }
        self.key = None;
    }
    fn number_value(&mut self, _: JsonNumber) {
        self.key = None;
    }
    fn boolean_value(&mut self, _: bool) {
        self.key = None;
    }
    fn null_value(&mut self) {
        self.key = None;
    }
}

pub(crate) fn parse_labels_list(bytes: &[u8]) -> Result<Vec<(String, String)>, StoreError> {
    let mut h = LabelsListHandler {
        depth: 0,
        key: None,
        in_labels_array: false,
        in_label_obj: false,
        cur_id: None,
        cur_name: None,
        labels: Vec::new(),
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    Ok(h.labels)
}

// ── messages.list ───────────────────────────────────────────────────────

struct MessagesListHandler {
    depth: usize,
    key: Option<String>,
    in_messages_array: bool,
    in_message_obj: bool,
    pub(crate) ids: Vec<String>,
    pub(crate) next_page_token: Option<String>,
}

impl JsonContentHandler for MessagesListHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.depth == 2 && self.in_messages_array {
            self.in_message_obj = true;
        }
    }
    fn end_object(&mut self) {
        if self.depth == 2 {
            self.in_message_obj = false;
        }
        self.depth -= 1;
    }
    fn start_array(&mut self) {
        if self.depth == 1 && self.key.as_deref() == Some("messages") {
            self.in_messages_array = true;
        }
    }
    fn end_array(&mut self) {
        if self.in_messages_array {
            self.in_messages_array = false;
        }
    }
    fn key(&mut self, key: &str) {
        self.key = Some(key.to_string());
    }
    fn string_value(&mut self, value: &str) {
        if self.depth == 2 && self.in_message_obj && self.key.as_deref() == Some("id") {
            self.ids.push(value.to_string());
        } else if self.depth == 1 && self.key.as_deref() == Some("nextPageToken") {
            self.next_page_token = Some(value.to_string());
        }
        self.key = None;
    }
    fn number_value(&mut self, _: JsonNumber) {
        self.key = None;
    }
    fn boolean_value(&mut self, _: bool) {
        self.key = None;
    }
    fn null_value(&mut self) {
        self.key = None;
    }
}

pub(crate) fn parse_messages_list_page(
    bytes: &[u8],
) -> Result<(Vec<String>, Option<String>), StoreError> {
    let mut h = MessagesListHandler {
        depth: 0,
        key: None,
        in_messages_array: false,
        in_message_obj: false,
        ids: Vec::new(),
        next_page_token: None,
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    Ok((h.ids, h.next_page_token))
}

// ── labels.get `messagesTotal` ──────────────────────────────────────────

struct LabelTotalHandler {
    depth: usize,
    key: Option<String>,
    pub(crate) messages_total: u64,
}

impl JsonContentHandler for LabelTotalHandler {
    fn start_object(&mut self) {
        self.depth += 1;
    }
    fn end_object(&mut self) {
        self.depth -= 1;
    }
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, key: &str) {
        self.key = Some(key.to_string());
    }
    fn string_value(&mut self, value: &str) {
        if self.depth == 1 && self.key.as_deref() == Some("messagesTotal") {
            if let Ok(n) = value.parse::<u64>() {
                self.messages_total = n;
            }
        }
        self.key = None;
    }
    fn number_value(&mut self, n: JsonNumber) {
        if self.depth == 1 && self.key.as_deref() == Some("messagesTotal") {
            self.messages_total = n.as_u64().unwrap_or(0);
        }
        self.key = None;
    }
    fn boolean_value(&mut self, _: bool) {
        self.key = None;
    }
    fn null_value(&mut self) {
        self.key = None;
    }
}

pub(crate) fn parse_label_messages_total(bytes: &[u8]) -> Result<u64, StoreError> {
    let mut h = LabelTotalHandler {
        depth: 0,
        key: None,
        messages_total: 0,
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    Ok(h.messages_total)
}

// ── format=metadata message (summary + envelope headers) ────────────────

pub(crate) struct GmailMessageMetadataParsed {
    pub(crate) label_ids: Vec<String>,
    pub(crate) size_estimate: u64,
    pub(crate) internal_date_ms: Option<i64>,
    pub(crate) headers: HashMap<String, String>,
}

struct MetadataHandler {
    depth: usize,
    key: Option<String>,
    expect_payload: bool,
    in_label_ids_array: bool,
    in_payload: bool,
    payload_depth: usize,
    in_headers_array: bool,
    in_header_obj: bool,
    header_name: Option<String>,
    pub(crate) out: GmailMessageMetadataParsed,
}

impl Default for GmailMessageMetadataParsed {
    fn default() -> Self {
        Self {
            label_ids: Vec::new(),
            size_estimate: 0,
            internal_date_ms: None,
            headers: HashMap::new(),
        }
    }
}

impl JsonContentHandler for MetadataHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.expect_payload {
            self.expect_payload = false;
            self.in_payload = true;
            self.payload_depth = self.depth;
        }
        if self.in_headers_array {
            self.in_header_obj = true;
        }
    }
    fn end_object(&mut self) {
        if self.in_header_obj {
            self.in_header_obj = false;
        }
        if self.in_payload && self.depth == self.payload_depth {
            self.in_payload = false;
        }
        self.depth -= 1;
    }
    fn start_array(&mut self) {
        if self.depth == 1 && self.key.as_deref() == Some("labelIds") {
            self.in_label_ids_array = true;
        }
        if self.in_payload
            && self.depth == self.payload_depth
            && self.key.as_deref() == Some("headers")
        {
            self.in_headers_array = true;
        }
    }
    fn end_array(&mut self) {
        if self.in_label_ids_array {
            self.in_label_ids_array = false;
        }
        if self.in_headers_array {
            self.in_headers_array = false;
        }
    }
    fn key(&mut self, key: &str) {
        if key == "payload" {
            self.expect_payload = true;
        }
        self.key = Some(key.to_string());
    }
    fn string_value(&mut self, value: &str) {
        if self.in_label_ids_array && self.depth == 2 {
            self.out.label_ids.push(value.to_string());
        } else if self.depth == 1 {
            match self.key.as_deref() {
                Some("internalDate") => {
                    self.out.internal_date_ms = value.parse::<i64>().ok();
                }
                _ => {}
            }
        } else if self.in_header_obj {
            match self.key.as_deref() {
                Some("name") => self.header_name = Some(value.to_string()),
                Some("value") => {
                    if let Some(n) = self.header_name.take() {
                        if !n.is_empty() {
                            self.out.headers.insert(n, value.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        self.key = None;
    }
    fn number_value(&mut self, n: JsonNumber) {
        if self.depth == 1 && self.key.as_deref() == Some("sizeEstimate") {
            self.out.size_estimate = n.as_u64().unwrap_or(0);
        } else if self.depth == 1 && self.key.as_deref() == Some("internalDate") {
            self.out.internal_date_ms = n.as_i64();
        }
        self.key = None;
    }
    fn boolean_value(&mut self, _: bool) {
        self.key = None;
    }
    fn null_value(&mut self) {
        self.key = None;
    }
}

pub(crate) fn parse_message_metadata(bytes: &[u8]) -> Result<GmailMessageMetadataParsed, StoreError> {
    let mut h = MetadataHandler {
        depth: 0,
        key: None,
        expect_payload: false,
        in_label_ids_array: false,
        in_payload: false,
        payload_depth: 0,
        in_headers_array: false,
        in_header_obj: false,
        header_name: None,
        out: GmailMessageMetadataParsed::default(),
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    Ok(h.out)
}

// ── `{"raw":"..."}` ─────────────────────────────────────────────────────

struct RawFieldHandler {
    depth: usize,
    key: Option<String>,
    raw: Option<String>,
}

impl JsonContentHandler for RawFieldHandler {
    fn start_object(&mut self) {
        self.depth += 1;
    }
    fn end_object(&mut self) {
        self.depth -= 1;
    }
    fn start_array(&mut self) {}
    fn end_array(&mut self) {}
    fn key(&mut self, key: &str) {
        self.key = Some(key.to_string());
    }
    fn string_value(&mut self, value: &str) {
        if self.depth == 1 && self.key.as_deref() == Some("raw") {
            self.raw = Some(value.to_string());
        }
        self.key = None;
    }
    fn number_value(&mut self, _: JsonNumber) {
        self.key = None;
    }
    fn boolean_value(&mut self, _: bool) {
        self.key = None;
    }
    fn null_value(&mut self) {
        self.key = None;
    }
}

pub(crate) fn parse_message_raw_field(bytes: &[u8]) -> Result<String, StoreError> {
    let mut h = RawFieldHandler {
        depth: 0,
        key: None,
        raw: None,
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    h.raw.ok_or_else(|| StoreError::new("gmail: missing raw field"))
}

// ── format=full MIME tree (payload.parts recursion) ──────────────────────

#[derive(Debug, Clone, Default)]
pub(crate) struct GmailWirePart {
    pub mime_type: Option<String>,
    pub filename: Option<String>,
    pub body_data: Option<String>,
    pub body_attachment_id: Option<String>,
    pub body_size: u64,
    pub parts: Vec<GmailWirePart>,
}

#[derive(Default)]
struct GmailWirePartBuilder {
    mime_type: Option<String>,
    filename: Option<String>,
    body_data: Option<String>,
    body_attachment_id: Option<String>,
    body_size: u64,
    parts: Vec<GmailWirePart>,
}

impl GmailWirePartBuilder {
    fn finish(self) -> GmailWirePart {
        GmailWirePart {
            mime_type: self.mime_type,
            filename: self.filename,
            body_data: self.body_data,
            body_attachment_id: self.body_attachment_id,
            body_size: self.body_size,
            parts: self.parts,
        }
    }
}

struct FullMessageHandler {
    depth: usize,
    key: Option<String>,
    msg_id: String,
    internal_date_ms: Option<i64>,
    expect_payload_object: bool,
    payload_depth: usize,
    in_parts_array: bool,
    in_body: bool,
    part_builders: Vec<GmailWirePartBuilder>,
    in_root_headers_array: bool,
    in_header_obj: bool,
    header_name: Option<String>,
    envelope_headers: HashMap<String, String>,
}

impl JsonContentHandler for FullMessageHandler {
    fn start_object(&mut self) {
        self.depth += 1;
        if self.expect_payload_object {
            self.expect_payload_object = false;
            self.payload_depth = self.depth;
            self.part_builders.push(GmailWirePartBuilder::default());
            return;
        }
        if self.in_parts_array {
            self.part_builders.push(GmailWirePartBuilder::default());
            return;
        }
        if self.key.as_deref() == Some("body") {
            self.in_body = true;
            self.key = None;
            return;
        }
        if self.in_root_headers_array {
            self.in_header_obj = true;
        }
        self.key = None;
    }

    fn end_object(&mut self) {
        if self.in_header_obj {
            self.in_header_obj = false;
            self.depth -= 1;
            return;
        }
        if self.in_body {
            self.in_body = false;
            self.depth -= 1;
            return;
        }
        if self.part_builders.len() > 1 && self.depth > self.payload_depth {
            let child = self.part_builders.pop().unwrap().finish();
            if let Some(parent) = self.part_builders.last_mut() {
                parent.parts.push(child);
            }
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        if self.key.as_deref() == Some("parts") {
            self.in_parts_array = true;
        }
        if self.key.as_deref() == Some("headers") && self.part_builders.len() == 1 {
            self.in_root_headers_array = true;
        }
        self.key = None;
    }

    fn end_array(&mut self) {
        if self.in_parts_array {
            self.in_parts_array = false;
        }
        if self.in_root_headers_array {
            self.in_root_headers_array = false;
        }
    }

    fn key(&mut self, key: &str) {
        if key == "payload" {
            self.expect_payload_object = true;
        }
        self.key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.in_header_obj {
            match self.key.as_deref() {
                Some("name") => self.header_name = Some(value.to_string()),
                Some("value") => {
                    if let Some(n) = self.header_name.take() {
                        if !n.is_empty() {
                            self.envelope_headers.insert(n, value.to_string());
                        }
                    }
                }
                _ => {}
            }
            self.key = None;
            return;
        }
        if self.part_builders.is_empty() && self.depth == 1 {
            match self.key.as_deref() {
                Some("id") => self.msg_id = value.to_string(),
                Some("internalDate") => self.internal_date_ms = value.parse().ok(),
                _ => {}
            }
            self.key = None;
            return;
        }
        let Some(b) = self.part_builders.last_mut() else {
            self.key = None;
            return;
        };
        if self.in_body {
            match self.key.as_deref() {
                Some("data") => b.body_data = Some(value.to_string()),
                Some("attachmentId") => b.body_attachment_id = Some(value.to_string()),
                _ => {}
            }
        } else {
            match self.key.as_deref() {
                Some("mimeType") => b.mime_type = Some(value.to_string()),
                Some("filename") => b.filename = Some(value.to_string()),
                _ => {}
            }
        }
        self.key = None;
    }

    fn number_value(&mut self, n: JsonNumber) {
        if self.part_builders.is_empty() && self.depth == 1 && self.key.as_deref() == Some("internalDate")
        {
            self.internal_date_ms = n.as_i64();
        } else if self.in_body {
            if let Some(b) = self.part_builders.last_mut() {
                if self.key.as_deref() == Some("size") {
                    b.body_size = n.as_u64().unwrap_or(0);
                }
            }
        }
        self.key = None;
    }

    fn boolean_value(&mut self, _: bool) {
        self.key = None;
    }

    fn null_value(&mut self) {
        self.key = None;
    }
}

pub(crate) struct GmailMessageFullParsed {
    #[allow(dead_code)]
    pub id: String,
    pub internal_date_ms: Option<i64>,
    pub envelope_headers: HashMap<String, String>,
    pub payload: GmailWirePart,
}

pub(crate) fn parse_message_full(bytes: &[u8]) -> Result<GmailMessageFullParsed, StoreError> {
    let mut h = FullMessageHandler {
        depth: 0,
        key: None,
        msg_id: String::new(),
        internal_date_ms: None,
        expect_payload_object: false,
        payload_depth: 0,
        in_parts_array: false,
        in_body: false,
        part_builders: Vec::new(),
        in_root_headers_array: false,
        in_header_obj: false,
        header_name: None,
        envelope_headers: HashMap::new(),
    };
    parse_bytes_complete(bytes, &mut h).map_err(json_err)?;
    let payload = h
        .part_builders
        .pop()
        .map(|b| b.finish())
        .unwrap_or_default();
    Ok(GmailMessageFullParsed {
        id: h.msg_id,
        internal_date_ms: h.internal_date_ms,
        envelope_headers: h.envelope_headers,
        payload,
    })
}

impl GmailWirePart {
    pub(crate) fn find_part_body(&self, mime: &str) -> Option<String> {
        find_part_body_inner(self, mime)
    }

    pub(crate) fn collect_attachments(&self, out: &mut Vec<MessageAttachmentRef>) {
        find_attachments_inner(self, out);
    }
}

fn find_part_body_inner(p: &GmailWirePart, mime: &str) -> Option<String> {
    if p
        .mime_type
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case(mime))
        .unwrap_or(false)
    {
        let data = p.body_data.as_deref().unwrap_or("");
        if !data.is_empty() {
            return String::from_utf8(super::base64url_decode(data)).ok();
        }
    }
    for c in &p.parts {
        if let Some(v) = find_part_body_inner(c, mime) {
            return Some(v);
        }
    }
    None
}

fn find_attachments_inner(p: &GmailWirePart, out: &mut Vec<MessageAttachmentRef>) {
    let filename = p.filename.as_deref().unwrap_or("").trim();
    let attachment_id = p.body_attachment_id.as_deref().unwrap_or("");
    if !attachment_id.is_empty() {
        let sec = format!("{GMAIL_ATTACHMENT_SECTION_PREFIX}{attachment_id}");
        out.push(MessageAttachmentRef {
            filename: if filename.is_empty() {
                None
            } else {
                Some(filename.to_string())
            },
            content_type: p
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream")
                .to_string(),
            size_bytes: p.body_size,
            transfer_encoding: "BASE64".to_string(),
            imap_section: Some(sec),
            content_id: None,
            data: None,
        });
    }
    for c in &p.parts {
        find_attachments_inner(c, out);
    }
}
