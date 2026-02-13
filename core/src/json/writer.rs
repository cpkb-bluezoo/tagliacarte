/*
 * writer.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Streaming JSON writer: build JSON via write_* methods, output in a BytesMut (bytes crate).

use bytes::{BufMut, BytesMut};

use crate::json::indent::IndentConfig;
use crate::json::number::JsonNumber;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Init,        // before first value
    AfterValue,  // after a value, next may be comma
    AfterKey,    // after key, need colon then value
    InArray,
    InObject,
}

/// JSON writer that appends to a BytesMut. Same interface as jsonparser JSONWriter.
pub struct JsonWriter {
    buf: BytesMut,
    indent: Option<IndentConfig>,
    state: State,
    depth: usize,
}

impl JsonWriter {
    pub fn new() -> Self {
        Self {
            buf: BytesMut::with_capacity(4096),
            indent: None,
            state: State::Init,
            depth: 0,
        }
    }

    pub fn with_indent(indent: IndentConfig) -> Self {
        Self {
            buf: BytesMut::with_capacity(4096),
            indent: Some(indent),
            state: State::Init,
            depth: 0,
        }
    }

    /// Access the buffer (e.g. to append to another writer or get bytes).
    pub fn buffer(&self) -> &BytesMut {
        &self.buf
    }

    /// Take the buffer, leaving the writer with an empty buffer (for reuse).
    pub fn take_buffer(&mut self) -> BytesMut {
        std::mem::take(&mut self.buf)
    }

    fn value_separator(&mut self) {
        match self.state {
            State::AfterValue => {
                self.buf.put_u8(b',');
                if let Some(ref ind) = self.indent {
                    self.buf.put_slice(ind.indent_for_depth(self.depth).as_bytes());
                }
            }
            State::Init | State::InArray | State::InObject => {
                if let Some(ref ind) = self.indent {
                    if self.state != State::Init {
                        self.buf.put_slice(ind.indent_for_depth(self.depth).as_bytes());
                    }
                }
            }
            State::AfterKey => {
                if self.indent.is_some() {
                    self.buf.put_u8(b' ');
                }
            }
        }
    }

    pub fn write_start_object(&mut self) {
        self.value_separator();
        self.buf.put_u8(b'{');
        self.state = State::InObject;
        self.depth += 1;
    }

    pub fn write_end_object(&mut self) {
        self.depth -= 1;
        if let Some(ref ind) = self.indent {
            self.buf.put_slice(ind.indent_for_depth(self.depth).as_bytes());
        }
        self.buf.put_u8(b'}');
        self.state = State::AfterValue;
    }

    pub fn write_start_array(&mut self) {
        self.value_separator();
        self.buf.put_u8(b'[');
        self.state = State::InArray;
        self.depth += 1;
    }

    pub fn write_end_array(&mut self) {
        self.depth -= 1;
        if let Some(ref ind) = self.indent {
            self.buf.put_slice(ind.indent_for_depth(self.depth).as_bytes());
        }
        self.buf.put_u8(b']');
        self.state = State::AfterValue;
    }

    pub fn write_key(&mut self, key: &str) {
        if self.state == State::AfterValue {
            self.buf.put_u8(b',');
        }
        if let Some(ref ind) = self.indent {
            self.buf.put_slice(ind.indent_for_depth(self.depth).as_bytes());
        }
        write_escaped_string(&mut self.buf, key);
        self.buf.put_u8(b':');
        if self.indent.is_some() {
            self.buf.put_u8(b' ');
        }
        self.state = State::AfterKey;
    }

    pub fn write_string(&mut self, value: &str) {
        self.value_separator();
        write_escaped_string(&mut self.buf, value);
        self.state = State::AfterValue;
    }

    pub fn write_number(&mut self, num: JsonNumber) {
        self.value_separator();
        match num {
            JsonNumber::I64(n) => {
                self.buf.extend_from_slice(format!("{}", n).as_bytes());
            }
            JsonNumber::F64(f) => {
                self.buf.extend_from_slice(format!("{}", f).as_bytes());
            }
        }
        self.state = State::AfterValue;
    }

    pub fn write_bool(&mut self, value: bool) {
        self.value_separator();
        self.buf.put_slice(if value {
            b"true"
        } else {
            b"false"
        });
        self.state = State::AfterValue;
    }

    pub fn write_null(&mut self) {
        self.value_separator();
        self.buf.put_slice(b"null");
        self.state = State::AfterValue;
    }
}

impl Default for JsonWriter {
    fn default() -> Self {
        Self::new()
    }
}

fn write_escaped_string(buf: &mut BytesMut, s: &str) {
    buf.put_u8(b'"');
    for ch in s.chars() {
        match ch {
            '"' => buf.extend_from_slice(b"\\\""),
            '\\' => buf.extend_from_slice(b"\\\\"),
            '\u{8}' => buf.extend_from_slice(b"\\b"),
            '\u{c}' => buf.extend_from_slice(b"\\f"),
            '\n' => buf.extend_from_slice(b"\\n"),
            '\r' => buf.extend_from_slice(b"\\r"),
            '\t' => buf.extend_from_slice(b"\\t"),
            c if c.is_ascii_control() => {
                buf.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes());
            }
            c => buf.extend_from_slice(c.to_string().as_bytes()),
        }
    }
    buf.put_u8(b'"');
}
