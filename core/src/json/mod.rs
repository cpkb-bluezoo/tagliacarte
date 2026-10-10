/*
 * mod.rs
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

//! JSON on rjsonparser: the push parser's [`ContentHandler`] is implemented
//! directly by the REST, Matrix and Nostr code; this module adds the
//! one-shot parse helpers and an in-memory [`JsonWriter`] whose writes
//! cannot fail.

use bytes::BytesMut;

pub use rjsonparser::{ContentHandler, IndentConfig, Number, ParseError, ParseResult, Parser};

/// A non-negative integer value as `u64`; `None` for negatives, fractions
/// and numbers beyond `i64`.
pub fn number_u64(n: &Number) -> Option<u64> {
    n.as_i64().and_then(|v| u64::try_from(v).ok())
}

/// Parse a complete JSON document from `input`.
pub fn parse_bytes_complete<H: ContentHandler>(input: &[u8], handler: &mut H) -> ParseResult<()> {
    let mut parser = Parser::new(handler);
    let mut data = input;
    parser.receive(&mut data)?;
    parser.close()
}

/// Parse a complete JSON document from `input`.
pub fn parse_str_complete<H: ContentHandler>(input: &str, handler: &mut H) -> ParseResult<()> {
    parse_bytes_complete(input.as_bytes(), handler)
}

/// Streaming JSON writer into memory. Writing to a `Vec` cannot fail, so
/// the methods return nothing; a structural misuse (an end without a
/// start) is remembered and surfaces from [`Self::finish`].
pub struct JsonWriter {
    inner: rjsonparser::Writer<Vec<u8>>,
    error: Option<rjsonparser::WriteError>,
}

impl Default for JsonWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonWriter {
    pub fn new() -> Self {
        Self { inner: rjsonparser::Writer::new(Vec::new()), error: None }
    }

    pub fn with_indent(indent: IndentConfig) -> Self {
        Self { inner: rjsonparser::Writer::with_indent(Vec::new(), indent), error: None }
    }

    fn record(&mut self, r: rjsonparser::WriteResult<()>) {
        if let Err(e) = r {
            if self.error.is_none() {
                self.error = Some(e);
            }
        }
    }

    pub fn write_start_object(&mut self) {
        let r = self.inner.write_start_object();
        self.record(r);
    }

    pub fn write_end_object(&mut self) {
        let r = self.inner.write_end_object();
        self.record(r);
    }

    pub fn write_start_array(&mut self) {
        let r = self.inner.write_start_array();
        self.record(r);
    }

    pub fn write_end_array(&mut self) {
        let r = self.inner.write_end_array();
        self.record(r);
    }

    pub fn write_key(&mut self, key: &str) {
        let r = self.inner.write_key(key);
        self.record(r);
    }

    pub fn write_string(&mut self, value: &str) {
        let r = self.inner.write_string(value);
        self.record(r);
    }

    pub fn write_number(&mut self, value: &Number) {
        let r = self.inner.write_number(value);
        self.record(r);
    }

    pub fn write_i64(&mut self, value: i64) {
        let r = self.inner.write_i64(value);
        self.record(r);
    }

    pub fn write_f64(&mut self, value: f64) {
        let r = self.inner.write_f64(value);
        self.record(r);
    }

    pub fn write_bool(&mut self, value: bool) {
        let r = self.inner.write_boolean(value);
        self.record(r);
    }

    pub fn write_null(&mut self) {
        let r = self.inner.write_null();
        self.record(r);
    }

    /// The document so far, reporting any earlier structural misuse.
    pub fn finish(mut self) -> Result<Vec<u8>, ParseError> {
        let r = self.inner.flush();
        self.record(r);
        match self.error.take() {
            Some(e) => Err(ParseError::new(e.to_string())),
            None => Ok(std::mem::take(self.inner.inner_mut())),
        }
    }

    /// The document so far, as a buffer; see [`Self::finish`] for errors.
    pub fn take_buffer(&mut self) -> BytesMut {
        let r = self.inner.flush();
        self.record(r);
        BytesMut::from(std::mem::take(self.inner.inner_mut()).as_slice())
    }
}

/// Finish `writer` and return the document as a string.
pub fn writer_into_string(mut writer: JsonWriter) -> String {
    String::from_utf8(writer.take_buffer().to_vec()).expect("json writer UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Collect(Vec<String>);

    impl ContentHandler for Collect {
        fn key(&mut self, key: &str) -> ParseResult<()> {
            self.0.push(format!("key {key}"));
            Ok(())
        }
        fn string_value(&mut self, value: &str) -> ParseResult<()> {
            self.0.push(format!("str {value}"));
            Ok(())
        }
        fn number_value(&mut self, value: &Number) -> ParseResult<()> {
            self.0.push(format!("num {:?}", value.as_f64()));
            Ok(())
        }
    }

    #[test]
    fn write_then_parse() {
        let mut w = JsonWriter::new();
        w.write_start_object();
        w.write_key("a");
        w.write_string("x\"y");
        w.write_key("n");
        w.write_i64(42);
        w.write_key("f");
        w.write_f64(1.5);
        w.write_key("b");
        w.write_bool(true);
        w.write_key("z");
        w.write_null();
        w.write_end_object();
        let json = writer_into_string(w);
        assert_eq!(json, r#"{"a":"x\"y","n":42,"f":1.5,"b":true,"z":null}"#);
        let mut c = Collect::default();
        parse_str_complete(&json, &mut c).unwrap();
        assert_eq!(c.0, vec!["key a", "str x\"y", "key n", "num Some(42.0)", "key f", "num Some(1.5)", "key b", "key z"]);
    }

    #[test]
    fn trailing_garbage_and_truncation_are_errors() {
        let mut c = Collect::default();
        assert!(parse_str_complete("{\"a\": 1} x", &mut c).is_err());
        assert!(parse_str_complete("{\"a\": ", &mut c).is_err());
    }
}
