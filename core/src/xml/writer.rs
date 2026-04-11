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

//! Streaming XML writer: `BytesMut` output, optional indentation, deferred `>`.

use bytes::{BufMut, BytesMut};
use std::collections::HashMap;

use crate::json::IndentConfig;
use crate::xml::parser::XML_NAMESPACE_URI;
use crate::xml::XMLNS_NAMESPACE_URI;

#[derive(Debug, Clone)]
struct OpenElem {
    qname: String,
    had_child_or_text: bool,
}

/// Streaming XML serializer (subset: no DTD/PI writers).
pub struct XmlWriter {
    buf: BytesMut,
    indent: Option<IndentConfig>,
    depth: usize,
    stack: Vec<OpenElem>,
    /// URI → prefix for the current element (cloned per depth).
    ns_uri_to_prefix: Vec<HashMap<String, String>>,
    next_auto_prefix: usize,
    pending_start: bool,
    /// `xmlns:*` to emit before `>` when closing the opening tag (prefix, uri).
    pending_xmlns: Vec<(String, String)>,
}

impl XmlWriter {
    pub fn new() -> Self {
        Self {
            buf: BytesMut::with_capacity(4096),
            indent: None,
            depth: 0,
            stack: Vec::new(),
            ns_uri_to_prefix: vec![HashMap::new()],
            next_auto_prefix: 0,
            pending_start: false,
            pending_xmlns: Vec::new(),
        }
    }

    pub fn with_indent(indent: IndentConfig) -> Self {
        Self {
            buf: BytesMut::with_capacity(4096),
            indent: Some(indent),
            depth: 0,
            stack: Vec::new(),
            ns_uri_to_prefix: vec![HashMap::new()],
            next_auto_prefix: 0,
            pending_start: false,
            pending_xmlns: Vec::new(),
        }
    }

    pub fn buffer(&self) -> &BytesMut {
        &self.buf
    }

    pub fn take_buffer(&mut self) -> BytesMut {
        std::mem::take(&mut self.buf)
    }

    pub fn write_xml_declaration(&mut self) {
        self.buf
            .put_slice(br#"<?xml version="1.0" encoding="utf-8"?>"#);
        if self.indent.is_some() {
            self.buf.put_u8(b'\n');
        }
    }

    /// `namespace_uri == None` → unprefixed element (no xmlns).
    pub fn write_start_element(&mut self, namespace_uri: Option<&str>, local_name: &str) {
        self.close_pending_start_for_child();
        self.indent_before_content_open();
        self.buf.put_u8(b'<');
        self.ns_uri_to_prefix
            .push(self.ns_uri_to_prefix.last().cloned().unwrap_or_default());
        let qname = match namespace_uri {
            None => local_name.to_string(),
            Some(uri) => {
                let prefix = self.ensure_prefix(uri);
                format!("{}:{}", prefix, local_name)
            }
        };
        self.buf.put_slice(qname.as_bytes());
        self.stack.push(OpenElem {
            qname,
            had_child_or_text: false,
        });
        self.pending_start = true;
        self.depth += 1;
    }

    /// `prefix` empty string → default namespace (`xmlns="..."`).
    pub fn write_namespace(&mut self, prefix: &str, namespace_uri: &str) {
        assert!(self.pending_start, "write_namespace outside open start tag");
        self.buf.put_u8(b' ');
        if prefix.is_empty() {
            self.buf.put_slice(b"xmlns=\"");
            escape_attr_value(&mut self.buf, namespace_uri);
            self.buf.put_u8(b'"');
        } else {
            self.buf.put_slice(b"xmlns:");
            self.buf.put_slice(prefix.as_bytes());
            self.buf.put_slice(b"=\"");
            escape_attr_value(&mut self.buf, namespace_uri);
            self.buf.put_u8(b'"');
        }
        let map = self.ns_uri_to_prefix.last_mut().unwrap();
        map.insert(namespace_uri.to_string(), prefix.to_string());
    }

    pub fn write_attribute(&mut self, namespace_uri: Option<&str>, local_name: &str, value: &str) {
        assert!(self.pending_start, "write_attribute outside open start tag");
        self.flush_pending_xmlns();
        self.buf.put_u8(b' ');
        let name = match namespace_uri {
            None => local_name.to_string(),
            Some(uri) => {
                if uri == XMLNS_NAMESPACE_URI {
                    if local_name == "xmlns" {
                        "xmlns".to_string()
                    } else {
                        format!("xmlns:{}", local_name)
                    }
                } else if uri == XML_NAMESPACE_URI {
                    format!("xml:{}", local_name)
                } else {
                    let pfx = self.ensure_prefix(uri);
                    format!("{}:{}", pfx, local_name)
                }
            }
        };
        self.buf.put_slice(name.as_bytes());
        self.buf.put_slice(b"=\"");
        escape_attr_value(&mut self.buf, value);
        self.buf.put_u8(b'"');
    }

    pub fn write_characters(&mut self, text: &str) {
        self.flush_pending_xmlns();
        self.close_pending_gt();
        if let Some(last) = self.stack.last_mut() {
            last.had_child_or_text = true;
        }
        escape_text(&mut self.buf, text);
    }

    pub fn write_cdata(&mut self, text: &str) {
        self.flush_pending_xmlns();
        self.close_pending_gt();
        if let Some(last) = self.stack.last_mut() {
            last.had_child_or_text = true;
        }
        self.buf.put_slice(b"<![CDATA[");
        self.buf.put_slice(text.as_bytes());
        self.buf.put_slice(b"]]>");
    }

    pub fn write_comment(&mut self, text: &str) {
        if text.contains("--") {
            panic!("XML comment text must not contain --");
        }
        self.flush_pending_xmlns();
        self.close_pending_gt();
        if let Some(last) = self.stack.last_mut() {
            last.had_child_or_text = true;
        }
        self.buf.put_slice(b"<!--");
        self.buf.put_slice(text.as_bytes());
        self.buf.put_slice(b"-->");
    }

    pub fn write_end_element(&mut self) {
        if self.pending_start {
            self.flush_pending_xmlns();
            self.buf.put_slice(b"/>");
            self.pending_start = false;
            self.depth -= 1;
            self.stack.pop();
            self.ns_uri_to_prefix.pop();
            if self.indent.is_some() {
                self.buf.put_u8(b'\n');
            }
            return;
        }
        let open = self.stack.pop().expect("write_end_element without matching start");
        self.ns_uri_to_prefix.pop();
        self.depth -= 1;
        if self.indent.is_some() && open.had_child_or_text {
            self.buf.put_slice(
                self.indent
                    .as_ref()
                    .unwrap()
                    .indent_for_depth(self.depth)
                    .as_bytes(),
            );
        }
        self.buf.put_slice(b"</");
        self.buf.put_slice(open.qname.as_bytes());
        self.buf.put_u8(b'>');
        if self.indent.is_some() {
            self.buf.put_u8(b'\n');
        }
    }

    fn flush_pending_xmlns(&mut self) {
        for (pfx, uri) in self.pending_xmlns.drain(..) {
            self.buf.put_u8(b' ');
            self.buf.put_slice(b"xmlns:");
            self.buf.put_slice(pfx.as_bytes());
            self.buf.put_slice(b"=\"");
            escape_attr_value(&mut self.buf, &uri);
            self.buf.put_u8(b'"');
        }
    }

    fn close_pending_gt(&mut self) {
        self.flush_pending_xmlns();
        if self.pending_start {
            self.buf.put_u8(b'>');
            self.pending_start = false;
            if self.indent.is_some() {
                self.buf.put_u8(b'\n');
            }
        }
    }

    fn close_pending_start_for_child(&mut self) {
        self.close_pending_gt();
    }

    fn indent_before_content_open(&mut self) {
        if let Some(ref ind) = self.indent {
            if !self.stack.is_empty() {
                self.buf
                    .put_slice(ind.indent_for_depth(self.depth.saturating_sub(1)).as_bytes());
            }
        }
    }

    /// Return an in-scope prefix for `uri`, declaring it on this start tag if needed.
    fn ensure_prefix(&mut self, uri: &str) -> String {
        let map = self.ns_uri_to_prefix.last_mut().unwrap();
        if let Some(p) = map.get(uri) {
            return p.clone();
        }
        let p = format!("n{}", self.next_auto_prefix);
        self.next_auto_prefix += 1;
        map.insert(uri.to_string(), p.clone());
        self.pending_xmlns.push((p.clone(), uri.to_string()));
        p
    }
}

impl Default for XmlWriter {
    fn default() -> Self {
        Self::new()
    }
}

fn escape_text(buf: &mut BytesMut, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => buf.put_slice(b"&amp;"),
            '<' => buf.put_slice(b"&lt;"),
            '>' => buf.put_slice(b"&gt;"),
            c => {
                let mut tmp = [0u8; 4];
                buf.put_slice(c.encode_utf8(&mut tmp).as_bytes());
            }
        }
    }
}

fn escape_attr_value(buf: &mut BytesMut, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => buf.put_slice(b"&amp;"),
            '<' => buf.put_slice(b"&lt;"),
            '"' => buf.put_slice(b"&quot;"),
            c => {
                let mut tmp = [0u8; 4];
                buf.put_slice(c.encode_utf8(&mut tmp).as_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_round_trip() {
        let mut w = XmlWriter::new();
        w.write_start_element(None, "a");
        w.write_attribute(None, "x", "1");
        w.write_characters("hi");
        w.write_end_element();
        let s = String::from_utf8(w.take_buffer().to_vec()).unwrap();
        assert_eq!(s, r#"<a x="1">hi</a>"#);
    }

    #[test]
    fn empty_element() {
        let mut w = XmlWriter::new();
        w.write_start_element(None, "a");
        w.write_end_element();
        let s = String::from_utf8(w.take_buffer().to_vec()).unwrap();
        assert_eq!(s, "<a/>");
    }

    #[test]
    fn indent_nesting() {
        let mut w = XmlWriter::with_indent(IndentConfig::spaces2());
        w.write_start_element(None, "r");
        w.write_start_element(None, "c");
        w.write_end_element();
        w.write_end_element();
        let s = String::from_utf8(w.take_buffer().to_vec()).unwrap();
        assert!(s.contains("<r>"));
        assert!(s.contains("</r>"));
    }

    #[test]
    fn namespaced_element_auto_xmlns() {
        let mut w = XmlWriter::new();
        w.write_start_element(Some("urn:x"), "root");
        w.write_end_element();
        let s = String::from_utf8(w.take_buffer().to_vec()).unwrap();
        assert!(s.starts_with("<n0:root"));
        assert!(s.contains("xmlns:n0=\"urn:x\""));
        assert!(s.ends_with("/>"));
    }
}
