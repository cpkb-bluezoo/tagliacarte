/*
 * parser.rs
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

//! MIME parser: receive(buffer) contract, consume complete lines only, leave remainder for next call.

use crate::mime::base64;
use crate::mime::content_type::parse_content_type;
use crate::mime::content_disposition::parse_content_disposition;
use crate::mime::content_id::parse_content_id;
use crate::mime::handler::{MimeHandler, MimeLocator, MimeParseError};
use crate::mime::mime_version::MimeVersion;
use crate::mime::quoted_printable;
use crate::mime::utils::is_valid_boundary;

/// Optional header value decoder: (header_name, value_bytes) -> decoded string.
/// When set, used instead of from_utf8_lossy(value).trim() for every header.
pub type HeaderValueDecoder = Box<dyn Fn(&str, &[u8]) -> String + Send>;

/// Event-driven MIME parser. Feed data via receive(); handler gets callbacks.
pub struct MimeParser<H> {
    handler: H,
    state: ParserState,
    /// Incomplete line carried over from previous receive()
    line_buffer: Vec<u8>,
    /// Header unfolding: current header name and value (merged continuation lines)
    header_name_buf: Option<Vec<u8>>,
    header_value_buf: Vec<u8>,
    /// Optional decoder for header values (e.g. selective RFC 2047 + SMTPUTF8)
    header_value_decoder: Option<HeaderValueDecoder>,
    /// Current entity: boundary (for multipart), content-type, cte
    boundary: Option<String>,
    /// Stack of parent boundaries for nested multipart
    boundary_stack: Vec<Option<String>>,
    content_transfer_encoding: Option<String>,
    /// For multipart: buffer body lines until we see boundary (then flush minus trailing CRLF)
    body_line_buffer: Vec<u8>,
    /// Base64 decode: input buffer (incomplete quantum)
    b64_src_pos: usize,
    b64_src: Vec<u8>,
    /// QP decode: true if previous line ended with soft break (= at end of line)
    qp_soft_break: bool,
    /// True when the current entity's Content-Type set a boundary (i.e. this entity is multipart).
    /// Used to choose FirstBoundary vs BoundaryOrContent after end_headers.
    entered_multipart: bool,
    locator: MimeLocator,
}

#[derive(Clone, Copy, PartialEq)]
enum ParserState {
    Init,
    Header,
    Body,
    FirstBoundary,
    BoundaryOrContent,
}

impl Default for ParserState {
    fn default() -> Self {
        ParserState::Init
    }
}

impl<H: MimeHandler> MimeParser<H> {
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            state: ParserState::Init,
            line_buffer: Vec::new(),
            header_name_buf: None,
            header_value_buf: Vec::new(),
            header_value_decoder: None,
            boundary: None,
            boundary_stack: Vec::new(),
            content_transfer_encoding: None,
            body_line_buffer: Vec::new(),
            b64_src_pos: 0,
            b64_src: Vec::new(),
            qp_soft_break: false,
            entered_multipart: false,
            locator: MimeLocator {
                offset: 0,
                line: 1,
                column: 1,
            },
        }
    }

    /// Set an optional decoder for header values. When set, it is called with (name, value_bytes) for every header.
    pub fn set_header_value_decoder(&mut self, decoder: Option<HeaderValueDecoder>) {
        self.header_value_decoder = decoder;
    }

    /// Process as much as possible from buf. Consumes only complete lines; unconsumed tail remains.
    /// Returns number of bytes consumed from buf.
    pub fn receive(&mut self, buf: &[u8]) -> Result<usize, MimeParseError> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Combine pending incomplete line with new data
        let pending_len = self.line_buffer.len();
        let combined_len = pending_len + buf.len();
        let mut combined = Vec::with_capacity(combined_len);
        combined.extend_from_slice(&self.line_buffer);
        combined.extend_from_slice(buf);
        self.line_buffer.clear();

        let last_newline = combined.iter().rposition(|&b| b == b'\n');
        let (to_process, incomplete) = match last_newline {
            Some(i) => {
                let end = i + 1;
                (&combined[..end], &combined[end..])
            }
            None => {
                // No complete line yet; keep all combined data for next call
                self.line_buffer = combined;
                return Ok(0);
            }
        };

        let total_consumed = to_process.len();
        let consumed_from_buf = if pending_len == 0 {
            total_consumed
        } else {
            (total_consumed - pending_len).min(buf.len())
        };

        self.line_buffer.clear();
        self.line_buffer.extend_from_slice(incomplete);

        let mut line_start = 0;
        for (i, &b) in to_process.iter().enumerate() {
            if b == b'\n' {
                let line = &to_process[line_start..=i];
                self.locator.offset += line.len() as u64;
                self.process_line(line)?;
                self.locator.line += 1;
                self.locator.column = 1;
                line_start = i + 1;
            }
        }

        Ok(consumed_from_buf)
    }

    fn process_line(&mut self, line: &[u8]) -> Result<(), MimeParseError> {
        let line = trim_crlf(line);
        match self.state {
            ParserState::Init => {
                self.handler.set_locator(self.locator.clone());
                self.handler.start_entity(None)?;
                self.state = ParserState::Header;
                if line.is_empty() {
                    return Ok(());
                }
                self.flush_pending_header()?;
                if let Some((name, value)) = split_header(line) {
                    self.header_name_buf = Some(name.to_vec());
                    self.header_value_buf = value.to_vec();
                }
            }
            ParserState::Header => {
                if line.is_empty() {
                    self.flush_pending_header()?;
                    self.handler.end_headers()?;
                    if self.boundary.is_some() {
                        self.state = if self.entered_multipart {
                            // This entity's Content-Type set a boundary; look for the first delimiter.
                            self.entered_multipart = false;
                            ParserState::FirstBoundary
                        } else {
                            // Child of an outer multipart; body lines delimited by parent boundary.
                            ParserState::BoundaryOrContent
                        };
                    } else {
                        self.state = ParserState::Body;
                    }
                    return Ok(());
                }
                if line.first().map(|&b| b == b' ' || b == b'\t').unwrap_or(false) {
                    if self.header_name_buf.is_some() {
                        self.header_value_buf.push(b' ');
                        self.header_value_buf.extend_from_slice(trim_lwsp_start(line));
                    }
                    return Ok(());
                }
                self.flush_pending_header()?;
                if let Some((name, value)) = split_header(line) {
                    self.header_name_buf = Some(name.to_vec());
                    self.header_value_buf = value.to_vec();
                }
            }
            ParserState::Body => {
                // One-line look-ahead: deliver previous line (with inter-line CRLF), buffer current
                self.flush_body_line_look_ahead(false)?;
                self.body_line_buffer = line.to_vec();
            }
            ParserState::FirstBoundary => {
                let boundary_str = self.boundary.as_deref().unwrap_or("").to_string();
                let boundary = boundary_str.as_str();
                if is_closing_boundary(line, boundary) {
                    self.handler.end_entity(Some(boundary))?;
                    self.boundary = self.boundary_stack.pop().flatten();
                    self.reset_cte_state();
                } else if is_boundary_line(line, boundary) {
                    self.reset_cte_state();
                    self.handler.start_entity(Some(boundary))?;
                    self.state = ParserState::Header;
                } else {
                    self.handler.unexpected_content(line)?;
                }
            }
            ParserState::BoundaryOrContent => {
                let boundary_str = self.boundary.as_deref().unwrap_or("").to_string();
                let boundary = boundary_str.as_str();
                if is_closing_boundary(line, boundary) {
                    // Last line before boundary: deliver without trailing CRLF, flush base64
                    self.flush_body_line_look_ahead(true)?;
                    self.flush_b64_end_of_entity()?;
                    self.handler.end_entity(Some(boundary))?;
                    self.boundary = self.boundary_stack.pop().flatten();
                    self.reset_cte_state();
                    self.state = if self.boundary.is_some() {
                        ParserState::BoundaryOrContent
                    } else {
                        ParserState::FirstBoundary
                    };
                } else if is_boundary_line(line, boundary) {
                    // End current child entity, then start the next one
                    self.flush_body_line_look_ahead(true)?;
                    self.flush_b64_end_of_entity()?;
                    self.handler.end_entity(Some(boundary))?;
                    self.reset_cte_state();
                    self.handler.start_entity(Some(boundary))?;
                    self.state = ParserState::Header;
                } else {
                    // Content line: deliver previous look-ahead (not last), buffer this line
                    self.flush_body_line_look_ahead(false)?;
                    self.body_line_buffer = line.to_vec();
                }
            }
        }
        Ok(())
    }

    fn flush_pending_header(&mut self) -> Result<(), MimeParseError> {
        let name = self.header_name_buf.clone();
        let value = self.header_value_buf.clone();
        self.header_name_buf = None;
        self.header_value_buf.clear();
        if let Some(ref name_buf) = name {
            self.process_header_line(name_buf, &value)?;
        }
        Ok(())
    }

    fn process_header_line(&mut self, name: &[u8], value: &[u8]) -> Result<(), MimeParseError> {
        let name_lower = String::from_utf8_lossy(name).to_lowercase();
        let value_str = if let Some(ref decoder) = self.header_value_decoder {
            decoder(&name_lower, value)
        } else {
            String::from_utf8_lossy(value).to_string()
        };
        let value_str = value_str.trim().to_string();
        match name_lower.as_str() {
            "content-type" => {
                if let Some(ct) = parse_content_type(&value_str) {
                    if let Some(b) = ct.get_parameter("boundary") {
                        if is_valid_boundary(b) {
                            self.boundary_stack.push(self.boundary.take());
                            self.boundary = Some(b.to_string());
                            self.entered_multipart = true;
                        }
                    }
                }
                self.handler.content_type(&value_str)?;
            }
            "content-disposition" => {
                let _ = parse_content_disposition(&value_str);
                self.handler.content_disposition(&value_str)?;
            }
            "content-transfer-encoding" => {
                self.content_transfer_encoding = Some(value_str.clone());
                self.handler.content_transfer_encoding(&value_str)?;
            }
            "content-id" => {
                let _ = parse_content_id(&value_str);
                self.handler.content_id(&value_str)?;
            }
            "content-description" => {
                self.handler.content_description(&value_str)?;
            }
            "mime-version" => {
                let _ = MimeVersion::parse(&value_str);
                self.handler.mime_version(&value_str)?;
            }
            _ => {
                let name_str = String::from_utf8_lossy(name);
                self.handler.header(&name_str, &value_str)?;
            }
        }
        Ok(())
    }

    /// Decode a single body line through the current Content-Transfer-Encoding.
    fn deliver_body_line(&mut self, line: &[u8]) -> Result<(), MimeParseError> {
        let cte = self.content_transfer_encoding.as_deref().unwrap_or("");
        if cte.eq_ignore_ascii_case("base64") {
            self.b64_src.extend_from_slice(line);
            let mut out = [0u8; 1024];
            let mut dst_pos = 0;
            base64::decode(
                &self.b64_src,
                &mut self.b64_src_pos,
                &mut out,
                &mut dst_pos,
                1024,
                false,
            );
            if dst_pos > 0 {
                self.handler.body_content(&out[..dst_pos])?;
            }
            if self.b64_src_pos > 0 {
                self.b64_src.drain(..self.b64_src_pos);
                self.b64_src_pos = 0;
            }
        } else if cte.eq_ignore_ascii_case("quoted-printable") {
            let mut out = vec![0u8; line.len() * 2 + 2];
            let out_len = out.len();
            let mut src_pos = 0;
            let mut dst_pos = 0;
            quoted_printable::decode(line, &mut src_pos, &mut out, &mut dst_pos, out_len, false);
            if dst_pos > 0 {
                self.handler.body_content(&out[..dst_pos])?;
            }
            // Track soft break: unconsumed '=' at end means continuation on next line
            self.qp_soft_break = src_pos < line.len() && line[src_pos] == b'=';
        } else {
            self.handler.body_content(line)?;
        }
        Ok(())
    }

    /// Deliver the one-line look-ahead buffer through CTE decoding.
    /// If `is_last` is false, emit inter-line CRLF for non-base64/non-soft-break content.
    fn flush_body_line_look_ahead(&mut self, is_last: bool) -> Result<(), MimeParseError> {
        if self.body_line_buffer.is_empty() {
            return Ok(());
        }
        let line = std::mem::take(&mut self.body_line_buffer);
        self.deliver_body_line(&line)?;
        if !is_last {
            self.emit_inter_line_crlf()?;
        }
        Ok(())
    }

    /// Emit CRLF between body content lines for non-base64, non-QP-soft-break content.
    fn emit_inter_line_crlf(&mut self) -> Result<(), MimeParseError> {
        let cte = self.content_transfer_encoding.as_deref().unwrap_or("");
        if cte.eq_ignore_ascii_case("base64") {
            // Base64: whitespace between lines is handled by the decoder
        } else if cte.eq_ignore_ascii_case("quoted-printable") {
            // QP: CRLF only on hard breaks, not after soft break (=)
            if !self.qp_soft_break {
                self.handler.body_content(b"\r\n")?;
            }
        } else {
            // 7bit / 8bit / binary: CRLF is part of content
            self.handler.body_content(b"\r\n")?;
        }
        Ok(())
    }

    /// Flush remaining base64 data at end of entity (partial quantum decoded with end_of_stream).
    fn flush_b64_end_of_entity(&mut self) -> Result<(), MimeParseError> {
        let cte = self.content_transfer_encoding.as_deref().unwrap_or("");
        if !cte.eq_ignore_ascii_case("base64") || self.b64_src.is_empty() {
            return Ok(());
        }
        loop {
            let mut out = [0u8; 1024];
            let mut dst_pos = 0;
            let prev_pos = self.b64_src_pos;
            base64::decode(
                &self.b64_src,
                &mut self.b64_src_pos,
                &mut out,
                &mut dst_pos,
                1024,
                true,
            );
            if dst_pos > 0 {
                self.handler.body_content(&out[..dst_pos])?;
            }
            if self.b64_src_pos > 0 {
                self.b64_src.drain(..self.b64_src_pos);
                self.b64_src_pos = 0;
            }
            if self.b64_src.is_empty() || (dst_pos == 0 && self.b64_src_pos == prev_pos) {
                break;
            }
        }
        self.b64_src.clear();
        self.b64_src_pos = 0;
        Ok(())
    }

    /// Reset per-entity CTE decode state (call between entities at boundary transitions).
    fn reset_cte_state(&mut self) {
        self.content_transfer_encoding = None;
        self.b64_src.clear();
        self.b64_src_pos = 0;
        self.qp_soft_break = false;
    }

    /// Return the handler (e.g. after close) for inspection in tests.
    pub fn into_inner(self) -> H {
        self.handler
    }

    /// End of input; flush any pending state.
    pub fn close(&mut self) -> Result<(), MimeParseError> {
        self.handler.set_locator(self.locator.clone());
        let remaining = if !self.line_buffer.is_empty() {
            Some(std::mem::take(&mut self.line_buffer))
        } else {
            None
        };
        if self.state == ParserState::Header {
            self.flush_pending_header()?;
            if let Some(ref line) = remaining {
                if !line.is_empty() {
                    if let Some((name, value)) = split_header(line) {
                        self.process_header_line(name, value)?;
                    }
                }
            }
        } else if self.state == ParserState::Body
            || self.state == ParserState::BoundaryOrContent
            || self.state == ParserState::FirstBoundary
        {
            if let Some(line) = remaining {
                // Route through process_line so boundary lines in BoundaryOrContent /
                // FirstBoundary states are recognised instead of being delivered as
                // body content.
                self.process_line(&line)?;
            }
            // Flush any remaining body content (last line in look-ahead buffer).
            if self.state == ParserState::Body || self.state == ParserState::BoundaryOrContent {
                self.flush_body_line_look_ahead(true)?;
                self.flush_b64_end_of_entity()?;
            }
        }
        self.handler.end_entity(self.boundary.as_deref())?;
        Ok(())
    }
}

fn trim_crlf(line: &[u8]) -> &[u8] {
    trim_trailing_crlf(line)
}

fn trim_lwsp_start(b: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    &b[i..]
}

fn trim_trailing_crlf(s: &[u8]) -> &[u8] {
    let mut end = s.len();
    if end >= 2 && s[end - 2] == b'\r' && s[end - 1] == b'\n' {
        end -= 2;
    } else if end >= 1 && s[end - 1] == b'\n' {
        end -= 1;
    } else if end >= 1 && s[end - 1] == b'\r' {
        end -= 1;
    }
    &s[..end]
}

fn split_header(line: &[u8]) -> Option<(&[u8], &[u8])> {
    let colon = line.iter().position(|&b| b == b':')?;
    if colon == 0 {
        return None;
    }
    let name = &line[..colon];
    let value = line.get(colon + 1..).unwrap_or(&[]);
    let value = value.strip_prefix(b" ").unwrap_or(value);
    Some((name, value))
}

fn is_boundary_line(line: &[u8], boundary: &str) -> bool {
    let prefix = b"--";
    if line.len() < prefix.len() + boundary.len() {
        return false;
    }
    if !line.starts_with(prefix) {
        return false;
    }
    let rest = &line[prefix.len()..];
    if !rest.starts_with(boundary.as_bytes()) {
        return false;
    }
    let after = &rest[boundary.len()..];
    let after = trim_trailing_crlf(after);
    after.is_empty()
}

fn is_closing_boundary(line: &[u8], boundary: &str) -> bool {
    let full = format!("--{}--", boundary);
    let full_bytes = full.as_bytes();
    if line.len() < full_bytes.len() {
        return false;
    }
    let check = trim_crlf(line);
    check == full_bytes
}

#[allow(dead_code)]
trait ByteSliceExt {
    fn to_ascii_lowercase(&self) -> String;
}

impl ByteSliceExt for [u8] {
    fn to_ascii_lowercase(&self) -> String {
        self.iter().map(|&b| (b as char).to_ascii_lowercase()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::handler::MimeParseError;

    struct CollectingHandler {
        content_types: Vec<String>,
        body_chunks: Vec<Vec<u8>>,
        entities_started: u32,
        entities_ended: u32,
    }

    impl Default for CollectingHandler {
        fn default() -> Self {
            Self {
                content_types: Vec::new(),
                body_chunks: Vec::new(),
                entities_started: 0,
                entities_ended: 0,
            }
        }
    }

    impl MimeHandler for CollectingHandler {
        fn start_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
            self.entities_started += 1;
            Ok(())
        }
        fn content_type(&mut self, content_type: &str) -> Result<(), MimeParseError> {
            self.content_types.push(content_type.to_string());
            Ok(())
        }
        fn body_content(&mut self, data: &[u8]) -> Result<(), MimeParseError> {
            self.body_chunks.push(data.to_vec());
            Ok(())
        }
        fn end_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
            self.entities_ended += 1;
            Ok(())
        }
    }

    #[test]
    fn plain_text_message() {
        let msg = b"MIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello, world.\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        let n = parser.receive(msg).unwrap();
        assert_eq!(n, msg.len());
        parser.close().unwrap();
        let h = parser.into_inner();
        assert_eq!(h.content_types.len(), 1);
        assert_eq!(h.content_types[0], "text/plain; charset=utf-8");
        assert_eq!(h.entities_started, 1);
        assert_eq!(h.entities_ended, 1);
        assert_eq!(h.body_chunks.len(), 1);
        assert_eq!(h.body_chunks[0], b"Hello, world.");
    }

    #[test]
    fn multipart_single_part() {
        let msg = b"MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=sep\r\n\r\n--sep\r\nContent-Type: text/plain\r\n\r\nPart one.\r\n--sep--\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        let n = parser.receive(msg).unwrap();
        assert_eq!(n, msg.len());
        parser.close().unwrap();
        let h = parser.into_inner();
        assert_eq!(h.content_types.len(), 2); // root + part
        assert_eq!(h.entities_started, 2);
        assert_eq!(h.entities_ended, 2);
        assert_eq!(h.body_chunks.len(), 1);
        assert_eq!(h.body_chunks[0], b"Part one.");
    }

    #[test]
    fn base64_in_multipart() {
        // "Hello" in base64 = "SGVsbG8="
        let msg = b"Content-Type: multipart/mixed; boundary=sep\r\n\r\n--sep\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\nSGVs\r\nbG8=\r\n--sep--\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        parser.receive(msg).unwrap();
        parser.close().unwrap();
        let h = parser.into_inner();
        let decoded: Vec<u8> = h.body_chunks.iter().flat_map(|c| c.iter().cloned()).collect();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn base64_partial_quantum_at_boundary() {
        // "Hello" = 48 65 6C 6C 6F → base64: SGVsbG8=
        // Split so partial quantum spans two lines
        let msg = b"Content-Type: multipart/mixed; boundary=sep\r\n\r\n--sep\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\nSGVsbG\r\n8=\r\n--sep--\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        parser.receive(msg).unwrap();
        parser.close().unwrap();
        let h = parser.into_inner();
        let decoded: Vec<u8> = h.body_chunks.iter().flat_map(|c| c.iter().cloned()).collect();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn qp_soft_break_in_multipart() {
        // QP: "Hello World" encoded with soft break: "Hello=\r\n World" → "Hello World"
        let msg = b"Content-Type: multipart/mixed; boundary=sep\r\n\r\n--sep\r\nContent-Type: text/plain\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nHello=\r\n World\r\n--sep--\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        parser.receive(msg).unwrap();
        parser.close().unwrap();
        let h = parser.into_inner();
        let decoded: Vec<u8> = h.body_chunks.iter().flat_map(|c| c.iter().cloned()).collect();
        assert_eq!(decoded, b"Hello World");
    }

    #[test]
    fn qp_hex_escape_in_multipart() {
        // QP: "caf=C3=A9" → "café" (UTF-8)
        let msg = b"Content-Type: multipart/mixed; boundary=sep\r\n\r\n--sep\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\ncaf=C3=A9\r\n--sep--\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        parser.receive(msg).unwrap();
        parser.close().unwrap();
        let h = parser.into_inner();
        let decoded: Vec<u8> = h.body_chunks.iter().flat_map(|c| c.iter().cloned()).collect();
        assert_eq!(decoded, "café".as_bytes());
    }

    #[test]
    fn multiline_plain_text_preserves_crlf() {
        let msg = b"Content-Type: text/plain\r\n\r\nLine one.\r\nLine two.\r\nLine three.\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        parser.receive(msg).unwrap();
        parser.close().unwrap();
        let h = parser.into_inner();
        let all: Vec<u8> = h.body_chunks.iter().flat_map(|c| c.iter().cloned()).collect();
        assert_eq!(all, b"Line one.\r\nLine two.\r\nLine three.");
    }

    #[test]
    fn multiline_plain_in_multipart() {
        let msg = b"Content-Type: multipart/mixed; boundary=sep\r\n\r\n--sep\r\nContent-Type: text/plain\r\n\r\nLine one.\r\nLine two.\r\n--sep--\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        parser.receive(msg).unwrap();
        parser.close().unwrap();
        let h = parser.into_inner();
        let all: Vec<u8> = h.body_chunks.iter().flat_map(|c| c.iter().cloned()).collect();
        assert_eq!(all, b"Line one.\r\nLine two.");
    }

    #[test]
    fn streaming_receive_base64_multipart() {
        // Feed data in small chunks to test streaming decode
        let msg = b"Content-Type: multipart/mixed; boundary=sep\r\n\r\n--sep\r\nContent-Transfer-Encoding: base64\r\n\r\nSGVsbG8g\r\nd29ybGQ=\r\n--sep--\r\n";
        let handler = CollectingHandler::default();
        let mut parser = MimeParser::new(handler);
        // Feed 20 bytes at a time; receive() stores all input (processes or buffers)
        let mut pos = 0;
        while pos < msg.len() {
            let end = (pos + 20).min(msg.len());
            parser.receive(&msg[pos..end]).unwrap();
            pos = end;
        }
        parser.close().unwrap();
        let h = parser.into_inner();
        let decoded: Vec<u8> = h.body_chunks.iter().flat_map(|c| c.iter().cloned()).collect();
        assert_eq!(decoded, b"Hello world");
    }
}
