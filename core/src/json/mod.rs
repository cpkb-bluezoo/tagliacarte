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

//! JSON on rjsonparser: the push parser's events are handled by the REST,
//! Matrix and Nostr code through [`ContentHandler`], which is rjsonparser's
//! with one difference: a string value arrives whole, in
//! [`ContentHandler::string_value`], where rjsonparser streams it in pieces.
//! Every string those handlers see is a key, an id, a token or a short
//! field, so holding one at a time costs nothing; bodies that are large (a
//! message's content) are fetched as bytes, not through JSON. This module
//! also adds the one-shot parse helpers and an in-memory [`JsonWriter`]
//! whose writes cannot fail.

use std::io::Read;

use bytes::BytesMut;
use loopbuf::{LoopBuf, WriteResult};

pub use rjsonparser::{IndentConfig, Number, ParseError, ParseResult};

/// Events of a JSON document, strings whole. See the module documentation.
pub trait ContentHandler {
    fn start_object(&mut self) -> ParseResult<()> {
        Ok(())
    }

    fn end_object(&mut self) -> ParseResult<()> {
        Ok(())
    }

    fn start_array(&mut self) -> ParseResult<()> {
        Ok(())
    }

    fn end_array(&mut self) -> ParseResult<()> {
        Ok(())
    }

    fn number_value(&mut self, _value: &Number) -> ParseResult<()> {
        Ok(())
    }

    fn string_value(&mut self, _value: &str) -> ParseResult<()> {
        Ok(())
    }

    fn boolean_value(&mut self, _value: bool) -> ParseResult<()> {
        Ok(())
    }

    fn null_value(&mut self) -> ParseResult<()> {
        Ok(())
    }

    fn key(&mut self, _key: &str) -> ParseResult<()> {
        Ok(())
    }
}

macro_rules! forward_content_handler {
    ($target:ty) => {
        impl<T: ContentHandler + ?Sized> ContentHandler for $target {
            fn start_object(&mut self) -> ParseResult<()> {
                (**self).start_object()
            }
            fn end_object(&mut self) -> ParseResult<()> {
                (**self).end_object()
            }
            fn start_array(&mut self) -> ParseResult<()> {
                (**self).start_array()
            }
            fn end_array(&mut self) -> ParseResult<()> {
                (**self).end_array()
            }
            fn number_value(&mut self, value: &Number) -> ParseResult<()> {
                (**self).number_value(value)
            }
            fn string_value(&mut self, value: &str) -> ParseResult<()> {
                (**self).string_value(value)
            }
            fn boolean_value(&mut self, value: bool) -> ParseResult<()> {
                (**self).boolean_value(value)
            }
            fn null_value(&mut self) -> ParseResult<()> {
                (**self).null_value()
            }
            fn key(&mut self, key: &str) -> ParseResult<()> {
                (**self).key(key)
            }
        }
    };
}

forward_content_handler!(&mut T);
forward_content_handler!(Box<T>);

/// Gives a [`ContentHandler`] to rjsonparser: collects each string's pieces
/// and delivers it whole when it ends.
struct WholeStrings<H> {
    handler: H,
    text: String,
}

impl<H: ContentHandler> rjsonparser::ContentHandler for WholeStrings<H> {
    fn start_object(&mut self) -> ParseResult<()> {
        self.handler.start_object()
    }

    fn end_object(&mut self) -> ParseResult<()> {
        self.handler.end_object()
    }

    fn start_array(&mut self) -> ParseResult<()> {
        self.handler.start_array()
    }

    fn end_array(&mut self) -> ParseResult<()> {
        self.handler.end_array()
    }

    fn number_value(&mut self, value: &Number) -> ParseResult<()> {
        self.handler.number_value(value)
    }

    fn start_string_value(&mut self) -> ParseResult<()> {
        self.text.clear();
        Ok(())
    }

    fn string_value_content(&mut self, chunk: &str) -> ParseResult<()> {
        self.text.push_str(chunk);
        Ok(())
    }

    fn end_string_value(&mut self) -> ParseResult<()> {
        let text = std::mem::take(&mut self.text);
        self.handler.string_value(&text)
    }

    fn boolean_value(&mut self, value: bool) -> ParseResult<()> {
        self.handler.boolean_value(value)
    }

    fn null_value(&mut self) -> ParseResult<()> {
        self.handler.null_value()
    }

    fn key(&mut self, key: &str) -> ParseResult<()> {
        self.handler.key(key)
    }
}

/// A non-negative integer value as `u64`; `None` for negatives, fractions
/// and numbers beyond `i64`.
pub fn number_u64(n: &Number) -> Option<u64> {
    n.as_i64().and_then(|v| u64::try_from(v).ok())
}

/// Room for the part of the document the parser has not finished with: a
/// key or a number has to fit whole, nothing else does.
const PARSE_RING: usize = 16 * 1024;

/// Push JSON parser driving a [`ContentHandler`]: feed it slices as they
/// arrive, then [`close`](Self::close).
pub struct Parser<H: ContentHandler> {
    inner: rjsonparser::Parser<WholeStrings<H>>,
    ring: LoopBuf,
}

// SAFETY: rjsonparser's parser keeps an `Rc` locator, which is what stops it
// being `Send`. Every clone of that `Rc` lives inside the parser itself: it
// is shared with the handler only through `set_locator`, which `WholeStrings`
// does not forward, so no `Rc` is reachable from outside this struct. A
// `Parser` moves between threads whole (an HTTP response handler is built on
// one thread and driven on a reactor), with all its counts, and is only ever
// used from one thread at a time.
unsafe impl<H: ContentHandler + Send> Send for Parser<H> {}

impl<H: ContentHandler> Parser<H> {
    pub fn new(handler: H) -> Self {
        Self {
            inner: rjsonparser::Parser::new(WholeStrings { handler, text: String::new() }),
            ring: LoopBuf::with_capacity(PARSE_RING),
        }
    }

    /// Parse `data`, consuming all of it.
    pub fn receive(&mut self, data: &mut &[u8]) -> ParseResult<()> {
        while !data.is_empty() {
            let taken = self.ring.extend_from_slice(data);
            *data = &data[taken..];
            self.inner.receive(&mut self.ring)?;
            if taken == 0 && self.ring.is_full() {
                return Err(ParseError::new("JSON token larger than the parse buffer"));
            }
        }
        Ok(())
    }

    /// The document is complete.
    pub fn close(&mut self) -> ParseResult<()> {
        self.inner.close(&mut self.ring)
    }
}

/// Parse a complete JSON document from `input`.
pub fn parse_bytes_complete<H: ContentHandler>(input: &[u8], handler: &mut H) -> ParseResult<()> {
    parse_reader(input, handler)
}

fn parse_reader<R: Read, H: ContentHandler>(reader: R, handler: &mut H) -> ParseResult<()> {
    rjsonparser::Parser::new(WholeStrings { handler, text: String::new() }).parse(reader)
}

/// Parse a complete JSON document from `input`.
pub fn parse_str_complete<H: ContentHandler>(input: &str, handler: &mut H) -> ParseResult<()> {
    parse_bytes_complete(input.as_bytes(), handler)
}

/// Streaming JSON writer into memory. Writing to a `Vec` cannot fail, so
/// the methods return nothing; a structural misuse (an end without a
/// start) is remembered and surfaces from [`Self::finish`].
pub struct JsonWriter {
    inner: rjsonparser::Writer,
    ring: LoopBuf,
    out: Vec<u8>,
    error: Option<String>,
}

impl Default for JsonWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Room for the output of one call; the ring is drained after each.
const WRITE_RING: usize = 16 * 1024;

impl JsonWriter {
    pub fn new() -> Self {
        Self::build(rjsonparser::Writer::new())
    }

    pub fn with_indent(indent: IndentConfig) -> Self {
        Self::build(rjsonparser::Writer::with_indent(indent))
    }

    fn build(inner: rjsonparser::Writer) -> Self {
        Self { inner, ring: LoopBuf::with_capacity(WRITE_RING), out: Vec::new(), error: None }
    }

    /// Move what the ring holds into `out`; whether there was anything.
    fn drain(&mut self) -> bool {
        let (head, tail) = self.ring.readable_spans();
        self.out.extend_from_slice(head);
        self.out.extend_from_slice(tail);
        let taken = self.ring.len();
        self.ring.advance(taken);
        taken > 0
    }

    fn fail(&mut self, message: String) {
        if self.error.is_none() {
            self.error = Some(message);
        }
    }

    /// Run one rjsonparser write, draining the ring and repeating the same
    /// call if the ring had no room for it.
    fn write<T>(&mut self, mut call: impl FnMut(&mut rjsonparser::Writer, &mut LoopBuf) -> WriteResult<T>) -> Option<T> {
        loop {
            match call(&mut self.inner, &mut self.ring) {
                Ok(v) => {
                    self.drain();
                    return Some(v);
                }
                Err(e) if e.is_would_block() => {
                    if !self.drain() {
                        self.fail(format!("JSON write needs more room than the {WRITE_RING}-byte ring has"));
                        return None;
                    }
                }
                Err(e) => {
                    self.fail(e.to_string());
                    return None;
                }
            }
        }
    }

    pub fn write_start_object(&mut self) {
        self.write(|w, ring| w.write_start_object(ring));
    }

    pub fn write_end_object(&mut self) {
        self.write(|w, ring| w.write_end_object(ring));
    }

    pub fn write_start_array(&mut self) {
        self.write(|w, ring| w.write_start_array(ring));
    }

    pub fn write_end_array(&mut self) {
        self.write(|w, ring| w.write_end_array(ring));
    }

    pub fn write_key(&mut self, key: &str) {
        self.write(|w, ring| w.write_key(ring, key));
    }

    /// Written in pieces, so a string of any length gets through the ring.
    pub fn write_string(&mut self, value: &str) {
        if self.write(|w, ring| w.write_start_string(ring)).is_none() {
            return;
        }
        let mut rest = value;
        while !rest.is_empty() {
            let Some(taken) = self.write(|w, ring| w.write_string_content(ring, rest)) else { return };
            rest = &rest[taken..];
        }
        self.write(|w, ring| w.write_end_string(ring));
    }

    pub fn write_number(&mut self, value: &Number) {
        self.write(|w, ring| w.write_number(ring, value));
    }

    pub fn write_i64(&mut self, value: i64) {
        self.write(|w, ring| w.write_i64(ring, value));
    }

    pub fn write_f64(&mut self, value: f64) {
        self.write(|w, ring| w.write_f64(ring, value));
    }

    pub fn write_bool(&mut self, value: bool) {
        self.write(|w, ring| w.write_boolean(ring, value));
    }

    pub fn write_null(&mut self) {
        self.write(|w, ring| w.write_null(ring));
    }

    /// The document so far, reporting any earlier structural misuse.
    pub fn finish(mut self) -> Result<Vec<u8>, ParseError> {
        match self.error.take() {
            Some(e) => Err(ParseError::new(e)),
            None => Ok(std::mem::take(&mut self.out)),
        }
    }

    /// The document so far, as a buffer; see [`Self::finish`] for errors.
    pub fn take_buffer(&mut self) -> BytesMut {
        BytesMut::from(std::mem::take(&mut self.out).as_slice())
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

    #[test]
    fn chunked_receive_gives_the_same_events_with_strings_whole() {
        let json = r#"{"name":"caf\u00e9 au lait","n":[1,2],"ok":true}"#;
        let mut whole = Collect::default();
        parse_str_complete(json, &mut whole).unwrap();
        let mut parser = Parser::new(Collect::default());
        for byte in json.as_bytes().chunks(1) {
            let mut chunk = byte;
            parser.receive(&mut chunk).unwrap();
        }
        parser.close().unwrap();
        assert_eq!(parser.inner.handler().handler.0, whole.0);
        assert!(whole.0.contains(&"str caf\u{e9} au lait".to_string()));
    }

    #[test]
    fn strings_longer_than_the_ring_round_trip() {
        let long = "x".repeat(WRITE_RING * 3 + 7);
        let mut w = JsonWriter::new();
        w.write_start_array();
        w.write_string(&long);
        w.write_end_array();
        let json = writer_into_string(w);
        let mut c = Collect::default();
        parse_str_complete(&json, &mut c).unwrap();
        assert_eq!(c.0, vec![format!("str {long}")]);
    }
}
