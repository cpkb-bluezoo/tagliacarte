/*
 * parser.rs
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

//! Push-model UTF-8 XML parser: feed bytes via [`XmlParser::receive`], finish with [`XmlParser::close`].
//! For a document already fully in memory, [`XmlParser::parse_buffer_complete`] or
//! [`XmlParser::parse_document_from_str`] runs `receive` until the buffer is drained, then `close`.
//!
//! Start tags are accumulated until a closing `>`/`/>` (bounded by [`MAX_TAG_BUFFER`]) so parsing is
//! safe across chunk boundaries; [`XmlContentHandler::attribute`] is still invoked per attribute
//! (not as a batched map).
//!
//! # Buffer contract
//!
//! Same as [`crate::json::parser::JsonParser`]: only complete constructs consume bytes;
//! incomplete markup leaves **zero** bytes consumed; compact `BytesMut` and append more data.

use bytes::Buf;
use bytes::BytesMut;
use std::collections::HashMap;
use std::io::Read;

use crate::xml::error::XmlError;
use crate::xml::handler::XmlContentHandler;

/// XML Namespace URI for xmlns declarations (attribute events).
pub const XMLNS_NAMESPACE_URI: &str = "http://www.w3.org/2000/xmlns/";
/// XML namespace URI for `xml:*` attributes.
pub const XML_NAMESPACE_URI: &str = "http://www.w3.org/XML/1998/namespace";

const MAX_TAG_BUFFER: usize = 256 * 1024;
const MAX_TOKEN: usize = 64 * 1024;

/// Push XML parser.
pub struct XmlParser {
    namespace_aware: bool,
    bom_checked: bool,
    closed: bool,
    document_started: bool,
    /// Root: before first element; after that we use element stack.
    phase: Phase,
    element_stack: Vec<OpenElement>,
    ns_stack: Vec<NsFrame>,
}

#[derive(Debug, Clone)]
struct OpenElement {
    namespace_uri: Option<String>,
    local_name: String,
}

#[derive(Debug, Clone)]
struct NsFrame {
    default_ns: Option<String>,
    /// prefix -> uri (xml, xmlns are special but stored same way)
    prefixes: HashMap<String, String>,
}

impl Default for NsFrame {
    fn default() -> Self {
        let mut prefixes = HashMap::new();
        prefixes.insert("xml".to_string(), XML_NAMESPACE_URI.to_string());
        Self {
            default_ns: None,
            prefixes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Whitespace / XML decl / misc before first `<` of root element.
    Prolog,
    /// Inside document element tree.
    Content,
}

impl XmlParser {
    pub fn new(namespace_aware: bool) -> Self {
        Self {
            namespace_aware,
            bom_checked: false,
            closed: false,
            document_started: false,
            phase: Phase::Prolog,
            element_stack: Vec::new(),
            ns_stack: vec![NsFrame::default()],
        }
    }

    pub fn namespace_aware(&self) -> bool {
        self.namespace_aware
    }

    pub fn receive<H: XmlContentHandler + ?Sized>(
        &mut self,
        buf: &mut BytesMut,
        handler: &mut H,
    ) -> Result<(), XmlError> {
        if self.closed {
            return Err(XmlError::new("cannot receive after close"));
        }
        if buf.is_empty() {
            return Ok(());
        }
        if !self.bom_checked {
            if buf.len() >= 3 && buf[0] == 0xef && buf[1] == 0xbb && buf[2] == 0xbf {
                buf.advance(3);
            } else if buf[0] == 0xef && buf.len() < 3 {
                return Ok(());
            }
            self.bom_checked = true;
        }
        while !buf.is_empty() {
            let consumed = match self.parse_step(buf.as_ref(), handler)? {
                Some(n) => n,
                None => return Ok(()),
            };
            buf.advance(consumed);
        }
        Ok(())
    }

    pub fn close<H: XmlContentHandler + ?Sized>(&mut self, handler: &mut H) -> Result<(), XmlError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        if self.document_started && !self.element_stack.is_empty() {
            return Err(XmlError::new("unclosed element"));
        }
        if self.document_started {
            handler.end_document();
        }
        Ok(())
    }

    /// Read from `reader` in chunks, push each through [`Self::receive`], require the buffer to be
    /// empty after EOF, then [`Self::close`]. Use for files and HTTP bodies without buffering the
    /// whole document in a [`String`].
    pub fn parse_reader_to_close<R: Read, H: XmlContentHandler + ?Sized>(
        &mut self,
        reader: &mut R,
        handler: &mut H,
    ) -> Result<(), XmlError> {
        const CHUNK: usize = 8192;
        let mut buf = BytesMut::new();
        let mut chunk = vec![0u8; CHUNK];
        loop {
            let n = reader
                .read(&mut chunk)
                .map_err(|e| XmlError::new(format!("I/O error: {e}")))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            self.receive(&mut buf, handler)?;
        }
        if !buf.is_empty() {
            return Err(XmlError::new("unexpected trailing bytes after XML parse"));
        }
        self.close(handler)
    }

    /// Like [`Self::parse_reader_to_close`], but reads from [`tokio::io::AsyncRead`] (e.g. [`tokio::fs::File`]).
    pub async fn parse_async_read_to_close<R, H>(&mut self, reader: &mut R, handler: &mut H) -> Result<(), XmlError>
    where
        R: tokio::io::AsyncRead + Unpin,
        H: XmlContentHandler + ?Sized,
    {
        use tokio::io::AsyncReadExt;
        const CHUNK: usize = 8192;
        let mut buf = BytesMut::new();
        let mut chunk = vec![0u8; CHUNK];
        loop {
            let n = reader
                .read(&mut chunk)
                .await
                .map_err(|e| XmlError::new(format!("I/O error: {e}")))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            self.receive(&mut buf, handler)?;
        }
        if !buf.is_empty() {
            return Err(XmlError::new("unexpected trailing bytes after XML parse"));
        }
        self.close(handler)
    }

    /// Feed all bytes in `buf` through [`Self::receive`], require the buffer to be fully consumed
    /// (a complete document), then [`Self::close`]. Use this when the entire XML is already in
    /// memory; for streaming input, call [`Self::receive`] repeatedly and finish with [`Self::close`].
    pub fn parse_buffer_complete<H: XmlContentHandler + ?Sized>(
        &mut self,
        buf: &mut BytesMut,
        handler: &mut H,
    ) -> Result<(), XmlError> {
        self.receive(buf, handler)?;
        if !buf.is_empty() {
            return Err(XmlError::new("unexpected trailing bytes after XML parse"));
        }
        self.close(handler)
    }

    /// Parse a full UTF-8 document from `input` using [`Self::parse_reader_to_close`].
    pub fn parse_document_from_str<H: XmlContentHandler + ?Sized>(
        namespace_aware: bool,
        input: &str,
        handler: &mut H,
    ) -> Result<(), XmlError> {
        let mut p = Self::new(namespace_aware);
        let mut cursor = std::io::Cursor::new(input.as_bytes());
        p.parse_reader_to_close(&mut cursor, handler)
    }

    pub fn reset(&mut self) {
        self.bom_checked = false;
        self.closed = false;
        self.document_started = false;
        self.phase = Phase::Prolog;
        self.element_stack.clear();
        self.ns_stack = vec![NsFrame::default()];
    }

    /// One step: returns bytes consumed from the **front** of `data`, or `None` if need more input.
    fn parse_step<H: XmlContentHandler + ?Sized>(
        &mut self,
        data: &[u8],
        handler: &mut H,
    ) -> Result<Option<usize>, XmlError> {
        if data.is_empty() {
            return Ok(None);
        }
        match self.phase {
            Phase::Prolog => self.parse_prolog(data, handler),
            Phase::Content => self.parse_content(data, handler),
        }
    }

    fn ensure_document_started<H: XmlContentHandler + ?Sized>(&mut self, handler: &mut H) {
        if !self.document_started {
            handler.start_document();
            self.document_started = true;
        }
    }

    fn parse_prolog<H: XmlContentHandler + ?Sized>(
        &mut self,
        data: &[u8],
        handler: &mut H,
    ) -> Result<Option<usize>, XmlError> {
        let mut i = 0;
        while i < data.len() && is_xml_whitespace_byte(data[i]) {
            i += 1;
        }
        if i > 0 {
            return Ok(Some(i));
        }
        if data.is_empty() {
            return Ok(None);
        }
        if data[0] != b'<' {
            return Err(XmlError::new("expected '<'"));
        }
        if starts_with(data, b"<?xml") {
            return skip_xml_declaration(data);
        }
        if starts_with(data, b"<?") {
            return skip_pi(data);
        }
        self.ensure_document_started(handler);
        self.phase = Phase::Content;
        self.parse_content(data, handler)
    }

    fn parse_content<H: XmlContentHandler + ?Sized>(
        &mut self,
        data: &[u8],
        handler: &mut H,
    ) -> Result<Option<usize>, XmlError> {
        if data[0] != b'<' {
            return self.parse_text(data, handler);
        }
        if starts_with(data, b"<!--") {
            return parse_comment(data, handler);
        }
        if starts_with(data, b"<![CDATA[") {
            return parse_cdata(data, handler);
        }
        if starts_with(data, b"<!DOCTYPE") {
            return skip_doctype(data);
        }
        if starts_with(data, b"<?") {
            return skip_pi(data);
        }
        if starts_with(data, b"</") {
            return self.parse_end_tag(data, handler);
        }
        // start tag (buffer full tag for chunk safety; attribute events still ordered)
        self.parse_start_tag_buffered(data, handler)
    }

    fn parse_text<H: XmlContentHandler + ?Sized>(
        &mut self,
        data: &[u8],
        handler: &mut H,
    ) -> Result<Option<usize>, XmlError> {
        let end = find_next_lt(data).unwrap_or(data.len());
        if end == 0 {
            return Ok(None);
        }
        let chunk = &data[..end];
        if !is_valid_utf8_boundary(chunk) {
            return Ok(None);
        }
        let s = expand_entities_in_text(chunk)?;
        if !s.is_empty() {
            handler.characters(&s);
        }
        Ok(Some(end))
    }

    fn parse_start_tag_buffered<H: XmlContentHandler + ?Sized>(
        &mut self,
        data: &[u8],
        handler: &mut H,
    ) -> Result<Option<usize>, XmlError> {
        if data.len() < 2 {
            return Ok(None);
        }
        let tag_len = match scan_complete_start_tag(data) {
            Some(n) => n,
            None => {
                if data.len() > MAX_TAG_BUFFER {
                    return Err(XmlError::new("start tag exceeds buffer limit"));
                }
                return Ok(None);
            }
        };
        let tag_bytes = &data[..tag_len];
        let inner = &tag_bytes[1..tag_bytes.len() - 1];
        dispatch_start_tag(self, inner, handler)?;
        Ok(Some(tag_len))
    }

    fn pop_element<H: XmlContentHandler + ?Sized>(
        &mut self,
        handler: &mut H,
    ) -> Result<(), XmlError> {
        let el = self
            .element_stack
            .pop()
            .ok_or_else(|| XmlError::new("internal stack underflow"))?;
        if self.namespace_aware {
            self.ns_stack.pop();
        }
        let ns = el.namespace_uri.as_deref();
        handler.end_element(ns, &el.local_name);
        Ok(())
    }
}

struct ParsedStartTag {
    raw_name: String,
    attrs: Vec<(String, String)>,
    self_closing: bool,
}

fn parse_start_tag_attributes(inner: &[u8]) -> Result<ParsedStartTag, XmlError> {
    let mut pos = 0usize;
    while pos < inner.len() && is_xml_whitespace_byte(inner[pos]) {
        pos += 1;
    }
    let name_end = scan_name(inner, pos).ok_or_else(|| XmlError::new("bad element name"))?;
    if name_end - pos > MAX_TOKEN {
        return Err(XmlError::new("element name too long"));
    }
    let raw_name = std::str::from_utf8(&inner[pos..name_end])
        .map_err(|_| XmlError::new("invalid UTF-8 in element name"))?
        .to_string();
    pos = name_end;
    let mut attrs = Vec::new();
    loop {
        while pos < inner.len() && is_xml_whitespace_byte(inner[pos]) {
            pos += 1;
        }
        if pos >= inner.len() {
            return Ok(ParsedStartTag {
                raw_name,
                attrs,
                self_closing: false,
            });
        }
        if inner[pos] == b'/' {
            pos += 1;
            while pos < inner.len() && is_xml_whitespace_byte(inner[pos]) {
                pos += 1;
            }
            if pos < inner.len() {
                return Err(XmlError::new("junk after start tag"));
            }
            return Ok(ParsedStartTag {
                raw_name,
                attrs,
                self_closing: true,
            });
        }
        let aname_end = scan_name(inner, pos).ok_or_else(|| XmlError::new("bad attribute name"))?;
        let aname = std::str::from_utf8(&inner[pos..aname_end])
            .map_err(|_| XmlError::new("invalid UTF-8 in attribute name"))?;
        pos = aname_end;
        while pos < inner.len() && is_xml_whitespace_byte(inner[pos]) {
            pos += 1;
        }
        if pos >= inner.len() || inner[pos] != b'=' {
            return Err(XmlError::new("expected '=' in attribute"));
        }
        pos += 1;
        while pos < inner.len() && is_xml_whitespace_byte(inner[pos]) {
            pos += 1;
        }
        if pos >= inner.len() {
            return Err(XmlError::new("truncated attribute"));
        }
        let q = inner[pos];
        if q != b'"' && q != b'\'' {
            return Err(XmlError::new("expected quoted attribute value"));
        }
        pos += 1;
        let vstart = pos;
        while pos < inner.len() && inner[pos] != q {
            pos += 1;
        }
        if pos >= inner.len() {
            return Err(XmlError::new("unclosed attribute value"));
        }
        let value = expand_entities_in_attr(&inner[vstart..pos])?;
        attrs.push((aname.to_string(), value));
        pos += 1;
    }
}

fn dispatch_start_tag<H: XmlContentHandler + ?Sized>(
    parser: &mut XmlParser,
    inner: &[u8],
    handler: &mut H,
) -> Result<(), XmlError> {
    let parsed = parse_start_tag_attributes(inner)?;
    parser.ensure_document_started(handler);
    if !parser.namespace_aware {
        handler.start_element(None, &parsed.raw_name);
        for (n, v) in &parsed.attrs {
            handler.attribute(None, n, v);
        }
        parser.element_stack.push(OpenElement {
            namespace_uri: None,
            local_name: parsed.raw_name.clone(),
        });
        if parsed.self_closing {
            parser.pop_element(handler)?;
        }
        return Ok(());
    }

    let parent = parser.ns_stack.last().cloned().unwrap_or_default();
    let mut frame = parent;
    for (n, v) in &parsed.attrs {
        if n == "xmlns" {
            frame.default_ns = Some(v.clone());
        } else if let Some(prefix) = n.strip_prefix("xmlns:") {
            if !prefix.is_empty() {
                frame.prefixes.insert(prefix.to_string(), v.clone());
            }
        }
    }
    let (elu, ell) = resolve_element_qname(&frame, &parsed.raw_name)?;
    handler.start_element(elu.as_deref(), &ell);
    for (n, v) in &parsed.attrs {
        if n == "xmlns" {
            handler.attribute(Some(XMLNS_NAMESPACE_URI), "xmlns", v);
        } else if let Some(prefix) = n.strip_prefix("xmlns:") {
            if !prefix.is_empty() {
                handler.attribute(Some(XMLNS_NAMESPACE_URI), prefix, v);
            }
        } else {
            let (au, al) = resolve_attribute_qname(&frame, n)?;
            handler.attribute(au.as_deref(), &al, v);
        }
    }
    parser.element_stack.push(OpenElement {
        namespace_uri: elu,
        local_name: ell,
    });
    parser.ns_stack.push(frame);
    if parsed.self_closing {
        parser.pop_element(handler)?;
    }
    Ok(())
}

fn resolve_element_qname(frame: &NsFrame, qname: &str) -> Result<(Option<String>, String), XmlError> {
    if let Some(i) = qname.find(':') {
        let prefix = &qname[..i];
        let local = &qname[i + 1..];
        let uri = frame
            .prefixes
            .get(prefix)
            .cloned()
            .ok_or_else(|| XmlError::new("undefined namespace prefix"))?;
        Ok((Some(uri), local.to_string()))
    } else {
        let ns = frame.default_ns.clone();
        Ok((ns, qname.to_string()))
    }
}

fn resolve_attribute_qname(frame: &NsFrame, aname: &str) -> Result<(Option<String>, String), XmlError> {
    if let Some(i) = aname.find(':') {
        let prefix = &aname[..i];
        let local = &aname[i + 1..];
        let uri = frame
            .prefixes
            .get(prefix)
            .cloned()
            .ok_or_else(|| XmlError::new("undefined namespace prefix in attribute"))?;
        Ok((Some(uri), local.to_string()))
    } else {
        Ok((None, aname.to_string()))
    }
}

impl XmlParser {
    fn parse_end_tag<H: XmlContentHandler + ?Sized>(
        &mut self,
        data: &[u8],
        handler: &mut H,
    ) -> Result<Option<usize>, XmlError> {
        if data.len() < 3 {
            return Ok(None);
        }
        let pos = 2;
        let name_end = match scan_name(data, pos) {
            Some(e) => e,
            None => return Ok(None),
        };
        let mut p = name_end;
        while p < data.len() && is_xml_whitespace_byte(data[p]) {
            p += 1;
        }
        if p >= data.len() || data[p] != b'>' {
            return Ok(None);
        }
        if !is_valid_utf8_boundary(&data[..name_end]) {
            return Ok(None);
        }
        let raw = std::str::from_utf8(&data[2..name_end])
            .map_err(|_| XmlError::new("invalid UTF-8 in end tag"))?;
        let expected = self.element_stack.last().ok_or_else(|| XmlError::new("unexpected end tag"))?;
        if self.namespace_aware {
            let frame = self.ns_stack.last().cloned().unwrap_or_default();
            let (enu, elocal) = resolve_element_qname(&frame, raw)?;
            if expected.local_name != elocal
                || expected.namespace_uri.as_ref() != enu.as_ref()
            {
                return Err(XmlError::new("end tag name mismatch"));
            }
        } else {
            if expected.local_name != raw {
                return Err(XmlError::new("end tag name mismatch"));
            }
        }
        self.pop_element(handler)?;
        Ok(Some(p + 1))
    }
}

fn parse_comment<H: XmlContentHandler + ?Sized>(
    data: &[u8],
    handler: &mut H,
) -> Result<Option<usize>, XmlError> {
    if data.len() < 4 {
        return Ok(None);
    }
    let needle = b"-->";
    if let Some(idx) = find_subslice(data, needle) {
        let inner = &data[4..idx];
        if inner.windows(2).any(|w| w == [b'-', b'-']) {
            return Err(XmlError::new("invalid comment content"));
        }
        let s = std::str::from_utf8(inner).map_err(|_| XmlError::new("invalid UTF-8 in comment"))?;
        handler.comment(s);
        return Ok(Some(idx + 3));
    }
    Ok(None)
}

fn parse_cdata<H: XmlContentHandler + ?Sized>(
    data: &[u8],
    handler: &mut H,
) -> Result<Option<usize>, XmlError> {
    let start = b"<![CDATA[";
    if data.len() < start.len() {
        return Ok(None);
    }
    let needle = b"]]>";
    if let Some(idx) = find_subslice(data, needle) {
        let inner = &data[start.len()..idx];
        let s = std::str::from_utf8(inner).map_err(|_| XmlError::new("invalid UTF-8 in CDATA"))?;
        handler.characters(s);
        return Ok(Some(idx + 3));
    }
    Ok(None)
}

fn skip_doctype(data: &[u8]) -> Result<Option<usize>, XmlError> {
    if data.len() < 9 {
        return Ok(None);
    }
    let mut i = 9;
    while i < data.len() && is_xml_whitespace_byte(data[i]) {
        i += 1;
    }
    if i < data.len() && data[i] == b'[' {
        return Err(XmlError::new("internal DTD subset not supported"));
    }
    let mut quote: Option<u8> = None;
    while i < data.len() {
        let b = data[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'"' || b == b'\'' {
            quote = Some(b);
        } else if b == b'>' {
            return Ok(Some(i + 1));
        }
        i += 1;
    }
    Ok(None)
}

fn skip_pi(data: &[u8]) -> Result<Option<usize>, XmlError> {
    if data.len() < 2 {
        return Ok(None);
    }
    if let Some(idx) = find_subslice(data, b"?>") {
        return Ok(Some(idx + 2));
    }
    Ok(None)
}

fn skip_xml_declaration(data: &[u8]) -> Result<Option<usize>, XmlError> {
    skip_pi(data)
}

fn scan_complete_start_tag(data: &[u8]) -> Option<usize> {
    if data.is_empty() || data[0] != b'<' {
        return None;
    }
    let mut quote: Option<u8> = None;
    let mut i = 1;
    while i < data.len() {
        let b = data[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'"' || b == b'\'' {
            quote = Some(b);
        } else if b == b'>' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

fn find_next_lt(data: &[u8]) -> Option<usize> {
    data.iter().position(|&b| b == b'<')
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn starts_with(data: &[u8], prefix: &[u8]) -> bool {
    data.len() >= prefix.len() && &data[..prefix.len()] == prefix
}

fn is_xml_whitespace_byte(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_valid_utf8_boundary(slice: &[u8]) -> bool {
    if slice.is_empty() {
        return true;
    }
    let last = slice[slice.len() - 1];
    !((0x80..=0xBF).contains(&last) && slice.len() < 4)
}

fn scan_name(data: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    if i >= data.len() {
        return None;
    }
    let first = data[i];
    if !is_name_start_byte(first) {
        return None;
    }
    i += 1;
    while i < data.len() && is_name_byte(data[i]) {
        i += 1;
    }
    Some(i)
}

fn is_name_start_byte(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b':'
}

fn is_name_byte(b: u8) -> bool {
    is_name_start_byte(b) || b.is_ascii_digit() || b == b'-' || b == b'.'
}

fn expand_entities_in_attr(raw: &[u8]) -> Result<String, XmlError> {
    expand_entities(raw, false)
}

fn expand_entities_in_text(raw: &[u8]) -> Result<String, XmlError> {
    expand_entities(raw, true)
}

/// `allow_unescaped_lt_gt` — in text content `<` must be `&lt;` per XML; we still expand entities.
fn expand_entities(raw: &[u8], _text_mode: bool) -> Result<String, XmlError> {
    if raw.iter().all(|&b| b != b'&') {
        return std::str::from_utf8(raw)
            .map(|s| s.to_string())
            .map_err(|_| XmlError::new("invalid UTF-8"));
    }
    let mut out = String::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] != b'&' {
            let rest = &raw[i..];
            let ch = utf8_first_char(rest)?;
            let len = ch.len_utf8();
            out.push(ch);
            i += len;
            continue;
        }
        let semi = raw[i + 1..]
            .iter()
            .position(|&b| b == b';')
            .ok_or_else(|| XmlError::new("unterminated entity reference"))?;
        let body = &raw[i + 1..i + 1 + semi];
        let rep = resolve_entity(body)?;
        out.push(rep);
        i += 1 + semi + 1;
    }
    Ok(out)
}

fn utf8_first_char(slice: &[u8]) -> Result<char, XmlError> {
    let s = std::str::from_utf8(slice).map_err(|_| XmlError::new("invalid UTF-8"))?;
    s.chars()
        .next()
        .ok_or_else(|| XmlError::new("empty UTF-8"))
}

fn resolve_entity(body: &[u8]) -> Result<char, XmlError> {
    if body.first() == Some(&b'#') {
        let rest = &body[1..];
        let code = if rest.first() == Some(&b'x') || rest.first() == Some(&b'X') {
            let hex = std::str::from_utf8(&rest[1..])
                .map_err(|_| XmlError::new("bad char ref"))?;
            u32::from_str_radix(hex, 16).map_err(|_| XmlError::new("bad char ref"))?
        } else {
            let dec = std::str::from_utf8(rest).map_err(|_| XmlError::new("bad char ref"))?;
            dec.parse::<u32>().map_err(|_| XmlError::new("bad char ref"))?
        };
        return char::from_u32(code).ok_or_else(|| XmlError::new("invalid code point"));
    }
    match body {
        b"amp" => Ok('&'),
        b"lt" => Ok('<'),
        b"gt" => Ok('>'),
        b"apos" => Ok('\''),
        b"quot" => Ok('"'),
        _ => Err(XmlError::new("unknown entity reference")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    struct Capture {
        events: Vec<String>,
    }

    impl XmlContentHandler for Capture {
        fn start_document(&mut self) {
            self.events.push("sd".into());
        }
        fn end_document(&mut self) {
            self.events.push("ed".into());
        }
        fn start_element(&mut self, ns: Option<&str>, name: &str) {
            self.events
                .push(format!("SE:{}:{}", ns.unwrap_or(""), name));
        }
        fn attribute(&mut self, ns: Option<&str>, name: &str, v: &str) {
            self.events
                .push(format!("AT:{}:{}={}", ns.unwrap_or(""), name, v));
        }
        fn characters(&mut self, t: &str) {
            self.events.push(format!("CH:{t}"));
        }
        fn comment(&mut self, t: &str) {
            self.events.push(format!("CO:{t}"));
        }
        fn end_element(&mut self, ns: Option<&str>, name: &str) {
            self.events
                .push(format!("EE:{}:{}", ns.unwrap_or(""), name));
        }
    }

    fn parse_all(xml: &str, ns_aware: bool) -> Result<Vec<String>, XmlError> {
        let mut p = XmlParser::new(ns_aware);
        let mut buf = BytesMut::from(xml);
        let mut c = Capture { events: Vec::new() };
        p.parse_buffer_complete(&mut buf, &mut c)?;
        Ok(c.events)
    }

    fn parse_chunks(chunks: &[&str], ns_aware: bool) -> Result<Vec<String>, XmlError> {
        let mut p = XmlParser::new(ns_aware);
        let mut c = Capture { events: Vec::new() };
        for ch in chunks {
            let mut buf = BytesMut::from(*ch);
            p.receive(&mut buf, &mut c)?;
        }
        p.close(&mut c)?;
        Ok(c.events)
    }

    #[test]
    fn simple_non_ns() {
        let e = parse_all("<a x=\"1\">t</a>", false).unwrap();
        assert!(e.iter().any(|s| s == "SE::a"));
        assert!(e.iter().any(|s| s == "AT::x=1"));
        assert!(e.iter().any(|s| s == "CH:t"));
        assert!(e.iter().any(|s| s == "EE::a"));
    }

    #[test]
    fn self_closing_non_ns() {
        let e = parse_all("<a/>", false).unwrap();
        assert!(e.iter().any(|s| s == "SE::a"));
        assert!(e.iter().any(|s| s == "EE::a"));
    }

    #[test]
    fn chunks_non_ns() {
        let e = parse_chunks(&["<a>", "te", "st</a>"], false).unwrap();
        let text: String = e
            .iter()
            .filter_map(|s| s.strip_prefix("CH:"))
            .collect();
        assert_eq!(text, "test");
    }

    #[test]
    fn comment_and_cdata() {
        let e = parse_all("<!-- hi --><a><![CDATA[&x]]></a>", false).unwrap();
        assert!(e.iter().any(|s| s == "CO: hi "));
        assert!(e.iter().any(|s| s.contains("CH:&x")));
    }

    #[test]
    fn entity_in_text() {
        let e = parse_all("<a>&amp;&lt;</a>", false).unwrap();
        assert!(e.iter().any(|s| s == "CH:&<"));
    }

    #[test]
    fn ns_aware_default_xmlns() {
        let xml = r#"<root xmlns="urn:x"><e/></root>"#;
        let e = parse_all(xml, true).unwrap();
        assert!(e.iter().any(|s| s.contains("SE:urn:x:root")));
        assert!(e.iter().any(|s| s.contains("AT:http://www.w3.org/2000/xmlns/:xmlns=urn:x")));
    }

    #[test]
    fn skip_pi() {
        let e = parse_all("<?foo bar?><a/>", false).unwrap();
        assert!(e.iter().any(|s| s == "SE::a"));
    }

    #[test]
    fn skip_doctype_external() {
        let e = parse_all("<!DOCTYPE r SYSTEM \"u\"><a/>", false).unwrap();
        assert!(e.iter().any(|s| s == "SE::a"));
    }

    #[test]
    fn unknown_entity_errors() {
        let r = parse_all("<a>&bogus;</a>", false);
        assert!(r.is_err());
    }

    #[test]
    fn parse_document_from_str_matches_parse_buffer_complete() {
        let xml = "<a k=\"v\">z</a>";
        let mut c1 = Capture { events: Vec::new() };
        XmlParser::parse_document_from_str(false, xml, &mut c1).unwrap();
        let mut c2 = Capture { events: Vec::new() };
        let mut p = XmlParser::new(false);
        let mut buf = BytesMut::from(xml);
        p.parse_buffer_complete(&mut buf, &mut c2).unwrap();
        assert_eq!(c1.events, c2.events);
    }

    #[test]
    fn writer_then_parser_round_trip() {
        use crate::xml::XmlWriter;
        let mut w = XmlWriter::new();
        w.write_start_element(None, "a");
        w.write_attribute(None, "k", "v");
        w.write_characters("z");
        w.write_end_element();
        let mut buf = w.take_buffer();
        let mut p = XmlParser::new(false);
        let mut c = Capture { events: Vec::new() };
        p.parse_buffer_complete(&mut buf, &mut c).unwrap();
        assert!(c.events.iter().any(|s| s.contains("SE::a")));
        assert!(c.events.iter().any(|s| s.contains("AT::k=v")));
        assert!(c.events.iter().any(|s| s == "CH:z"));
    }
}
