/*
 * util.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

//! Helpers for one-shot UTF-8 JSON documents (full string in memory).

use bytes::BytesMut;

use crate::json::error::JsonError;
use crate::json::handler::JsonContentHandler;
use crate::json::parser::JsonParser;
use crate::json::writer::JsonWriter;

/// Parse a complete JSON document from `input`. Fails if any non-whitespace bytes remain after the value.
pub fn parse_str_complete<H: JsonContentHandler>(input: &str, handler: &mut H) -> Result<(), JsonError> {
    let mut parser = JsonParser::new();
    let mut buf = BytesMut::from(input.as_bytes());
    parser.receive(&mut buf, handler)?;
    parser.close(handler)?;
    if !buf.is_empty() {
        return Err(JsonError::new("trailing data after JSON document"));
    }
    Ok(())
}

/// Finish a [`JsonWriter`] into a `String`. Writer output is always valid UTF-8 for JSON we emit.
pub fn writer_into_string(mut writer: JsonWriter) -> String {
    String::from_utf8(writer.take_buffer().to_vec()).expect("json writer UTF-8")
}
