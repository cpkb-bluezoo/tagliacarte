/*
 * writer.rs
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

//! [`XmlWriter`]: Tractrix's serializer behind the small API the config
//! writers use. Output is UTF-8 into memory.

use bytes::BytesMut;
use loopbuf::{LoopBuf, WriteResult};
use tractrix::IndentConfig;

use super::error::XmlError;

/// Room for the output of one call; tractrix writes a call's output into the
/// ring all at once or not at all, and this drains the ring after each.
const RING: usize = 16 * 1024;

pub struct XmlWriter {
    inner: tractrix::XmlWriter,
    ring: LoopBuf,
    out: Vec<u8>,
    indented: bool,
    error: Option<String>,
}

impl Default for XmlWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlWriter {
    pub fn new() -> Self {
        Self::build(tractrix::XmlWriter::new(), false)
    }

    pub fn with_indent(indent: IndentConfig) -> Self {
        Self::build(tractrix::XmlWriter::with_indent(indent), true)
    }

    fn build(inner: tractrix::XmlWriter, indented: bool) -> Self {
        Self { inner, ring: LoopBuf::with_capacity(RING), out: Vec::new(), indented, error: None }
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

    /// Run one tractrix write, draining the ring and repeating the same call
    /// if the ring had no room for it.
    fn write<T>(&mut self, mut call: impl FnMut(&mut tractrix::XmlWriter, &mut LoopBuf) -> WriteResult<T>) -> Option<T> {
        loop {
            match call(&mut self.inner, &mut self.ring) {
                Ok(v) => {
                    self.drain();
                    return Some(v);
                }
                Err(e) if e.is_would_block() => {
                    if !self.drain() {
                        self.fail(format!("XML write needs more room than the {RING}-byte ring has"));
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

    /// A write that takes as much of `text` as fits and says how much: repeat
    /// it for the rest.
    fn write_text(&mut self, text: &str, mut call: impl FnMut(&mut tractrix::XmlWriter, &mut LoopBuf, &str) -> WriteResult<usize>) {
        let mut rest = text;
        while !rest.is_empty() {
            let Some(taken) = self.write(|w, ring| call(w, ring, rest)) else { return };
            if taken == 0 {
                self.fail("XML write made no progress".to_string());
                return;
            }
            rest = &rest[taken..];
        }
    }

    pub fn write_xml_declaration(&mut self) {
        self.write_text(r#"<?xml version="1.0" encoding="utf-8"?>"#, |w, ring, t| w.write_raw(ring, t));
        if self.indented {
            self.write_text("\n", |w, ring, t| w.write_raw(ring, t));
        }
    }

    /// `namespace_uri == None` → unprefixed element (no xmlns).
    pub fn write_start_element(&mut self, namespace_uri: Option<&str>, local_name: &str) {
        self.write(|w, ring| match namespace_uri {
            Some(ns) => w.write_start_element_ns(ring, ns, local_name),
            None => w.write_start_element(ring, local_name),
        });
    }

    pub fn write_namespace(&mut self, prefix: &str, namespace_uri: &str) {
        self.write(|w, ring| w.write_namespace(ring, prefix, namespace_uri));
    }

    pub fn write_attribute(&mut self, namespace_uri: Option<&str>, local_name: &str, value: &str) {
        self.write(|w, ring| match namespace_uri {
            Some(ns) => w.write_attribute_ns(ring, ns, local_name, value),
            None => w.write_attribute(ring, local_name, value),
        });
    }

    pub fn write_characters(&mut self, text: &str) {
        self.write_text(text, |w, ring, t| w.write_characters(ring, t));
    }

    pub fn write_cdata(&mut self, text: &str) {
        self.write(|w, ring| w.write_start_cdata(ring));
        self.write_text(text, |w, ring, t| w.write_characters(ring, t));
        self.write(|w, ring| w.write_end_cdata(ring));
    }

    pub fn write_comment(&mut self, text: &str) {
        self.write(|w, ring| w.write_comment(ring, text));
    }

    pub fn write_end_element(&mut self) {
        self.write(|w, ring| w.write_end_element(ring));
    }

    /// The document so far. Any earlier write failure is reported here.
    pub fn finish(mut self) -> Result<Vec<u8>, XmlError> {
        self.write(|w, ring| w.close(ring));
        if let Some(e) = self.error.take() {
            return Err(XmlError::new(e));
        }
        Ok(std::mem::take(&mut self.out))
    }

    /// The document so far, as a buffer; see [`Self::finish`] for errors.
    pub fn take_buffer(&mut self) -> BytesMut {
        self.write(|w, ring| w.close(ring));
        BytesMut::from(std::mem::take(&mut self.out).as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_nests() {
        let mut w = XmlWriter::new();
        w.write_xml_declaration();
        w.write_start_element(None, "a");
        w.write_attribute(None, "x", "1 & \"2\"");
        w.write_start_element(None, "b");
        w.write_characters("<hi>");
        w.write_end_element();
        w.write_end_element();
        let out = String::from_utf8(w.finish().unwrap()).unwrap();
        assert!(out.starts_with(r#"<?xml version="1.0" encoding="utf-8"?><a x="1 &amp; &quot;2&quot;">"#), "{out}");
        assert!(out.ends_with("<b>&lt;hi&gt;</b></a>"), "{out}");
    }

    #[test]
    fn round_trips_through_the_parser() {
        let mut w = XmlWriter::with_indent(IndentConfig::spaces(2));
        w.write_xml_declaration();
        w.write_start_element(None, "credentials");
        w.write_start_element(None, "uri");
        w.write_characters("imap://a@b/");
        w.write_end_element();
        w.write_end_element();
        let bytes = w.take_buffer();
        struct Uri(String);
        impl crate::xml::XmlContentHandler for Uri {
            fn start_element(&mut self, _: Option<&str>, _: &str) {}
            fn attribute(&mut self, _: Option<&str>, _: &str, _: &str) {}
            fn characters(&mut self, t: &str) {
                if !t.trim().is_empty() {
                    self.0.push_str(t);
                }
            }
            fn end_element(&mut self, _: Option<&str>, _: &str) {}
        }
        let mut u = Uri(String::new());
        crate::xml::XmlParser::new(false).parse_buffer_complete(&bytes, &mut u).unwrap();
        assert_eq!(u.0, "imap://a@b/");
    }
}
