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
use tractrix::IndentConfig;

use super::error::XmlError;

pub struct XmlWriter {
    inner: tractrix::XmlWriter<Vec<u8>>,
    indented: bool,
    error: Option<std::io::Error>,
}

impl Default for XmlWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlWriter {
    pub fn new() -> Self {
        Self { inner: tractrix::XmlWriter::new_vec(), indented: false, error: None }
    }

    pub fn with_indent(indent: IndentConfig) -> Self {
        Self { inner: tractrix::XmlWriter::with_indent(Vec::new(), indent), indented: true, error: None }
    }

    fn record(&mut self, r: std::io::Result<()>) {
        if let Err(e) = r {
            if self.error.is_none() {
                self.error = Some(e);
            }
        }
    }

    pub fn write_xml_declaration(&mut self) {
        let r = self.inner.write_raw(r#"<?xml version="1.0" encoding="utf-8"?>"#);
        self.record(r);
        if self.indented {
            let r = self.inner.write_raw("\n");
            self.record(r);
        }
    }

    /// `namespace_uri == None` → unprefixed element (no xmlns).
    pub fn write_start_element(&mut self, namespace_uri: Option<&str>, local_name: &str) {
        let r = match namespace_uri {
            Some(ns) => self.inner.write_start_element_ns(ns, local_name),
            None => self.inner.write_start_element(local_name),
        };
        self.record(r);
    }

    pub fn write_namespace(&mut self, prefix: &str, namespace_uri: &str) {
        let r = self.inner.write_namespace(prefix, namespace_uri);
        self.record(r);
    }

    pub fn write_attribute(&mut self, namespace_uri: Option<&str>, local_name: &str, value: &str) {
        let r = match namespace_uri {
            Some(ns) => self.inner.write_attribute_ns(ns, local_name, value),
            None => self.inner.write_attribute(local_name, value),
        };
        self.record(r);
    }

    pub fn write_characters(&mut self, text: &str) {
        let r = self.inner.write_characters(text);
        self.record(r);
    }

    pub fn write_cdata(&mut self, text: &str) {
        let r = self
            .inner
            .write_start_cdata()
            .and_then(|_| self.inner.write_characters(text))
            .and_then(|_| self.inner.write_end_cdata());
        self.record(r);
    }

    pub fn write_comment(&mut self, text: &str) {
        let r = self.inner.write_comment(text);
        self.record(r);
    }

    pub fn write_end_element(&mut self) {
        let r = self.inner.write_end_element();
        self.record(r);
    }

    /// The document so far. Any earlier write failure is reported here.
    pub fn finish(mut self) -> Result<Vec<u8>, XmlError> {
        let r = self.inner.flush();
        self.record(r);
        if let Some(e) = self.error.take() {
            return Err(XmlError::new(e.to_string()));
        }
        Ok(std::mem::take(self.inner.get_mut()))
    }

    /// The document so far, as a buffer; see [`Self::finish`] for errors.
    pub fn take_buffer(&mut self) -> BytesMut {
        let r = self.inner.flush();
        self.record(r);
        BytesMut::from(std::mem::take(self.inner.get_mut()).as_slice())
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
