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

/// Event-driven MIME parser. Feed data via receive(); handler gets callbacks.
pub struct MimeParser<H> {
    handler: H,
    state: ParserState,
    /// Incomplete line carried over from previous receive()
    line_buffer: Vec<u8>,
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
    /// QP decode: no extra state beyond line
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
            boundary: None,
            boundary_stack: Vec::new(),
            content_transfer_encoding: None,
            body_line_buffer: Vec::new(),
            b64_src_pos: 0,
            b64_src: Vec::new(),
            locator: MimeLocator {
                offset: 0,
                line: 1,
                column: 1,
            },
        }
    }

    /// Process as much as possible from buf. Consumes only complete lines; unconsumed tail remains.
    /// Returns number of bytes consumed from buf.
    pub fn receive(&mut self, buf: &[u8]) -> Result<usize, MimeParseError> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Combine pending incomplete line with new data
        let combined_len = self.line_buffer.len() + buf.len();
        let mut combined = Vec::with_capacity(combined_len);
        combined.extend_from_slice(&self.line_buffer);
        combined.extend_from_slice(buf);
        self.line_buffer.clear();

        let pending_len = self.line_buffer.len();
        let last_newline = combined.iter().rposition(|&b| b == b'\n');
        let (to_process, incomplete) = match last_newline {
            Some(i) => {
                let end = i + 1;
                (&combined[..end], &combined[end..])
            }
            None => {
                self.line_buffer.extend_from_slice(buf);
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
                self.process_header_line(line)?;
            }
            ParserState::Header => {
                if line.is_empty() {
                    self.handler.end_headers()?;
                    if self.boundary.is_some() {
                        self.state = if self.boundary_stack.is_empty() {
                            ParserState::FirstBoundary
                        } else {
                            ParserState::BoundaryOrContent
                        };
                    } else {
                        self.state = ParserState::Body;
                    }
                    return Ok(());
                }
                self.process_header_line(line)?;
            }
            ParserState::Body => {
                self.deliver_body_line(line)?;
            }
            ParserState::FirstBoundary => {
                let boundary = self.boundary.as_deref().unwrap_or("");
                if is_closing_boundary(line, boundary) {
                    self.handler.end_entity(Some(boundary))?;
                    self.boundary = self.boundary_stack.pop().flatten();
                } else if is_boundary_line(line, boundary) {
                    self.body_line_buffer.clear();
                    self.content_transfer_encoding = None;
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
                    self.flush_body_buffer(false)?;
                    self.handler.end_entity(Some(boundary))?;
                    self.boundary = self.boundary_stack.pop().flatten();
                    self.state = if self.boundary.is_some() {
                        ParserState::BoundaryOrContent
                    } else {
                        ParserState::FirstBoundary
                    };
                } else if is_boundary_line(line, boundary) {
                    self.flush_body_buffer(false)?;
                    self.content_transfer_encoding = None;
                    self.handler.start_entity(Some(boundary))?;
                    self.state = ParserState::Header;
                } else {
                    if !self.body_line_buffer.is_empty() {
                        self.body_line_buffer.extend_from_slice(b"\r\n");
                    }
                    self.body_line_buffer.extend_from_slice(line);
                }
            }
        }
        Ok(())
    }

    fn process_header_line(&mut self, line: &[u8]) -> Result<(), MimeParseError> {
        let (name, value) = match split_header(line) {
            Some(p) => p,
            None => return Ok(()),
        };
        let name_lower = String::from_utf8_lossy(name).to_lowercase();
        let value_str = String::from_utf8_lossy(value).trim().to_string();
        match name_lower.as_str() {
            "content-type" => {
                if let Some(ct) = parse_content_type(&value_str) {
                    if let Some(b) = ct.get_parameter("boundary") {
                        if is_valid_boundary(b) {
                            self.boundary_stack.push(self.boundary.take());
                            self.boundary = Some(b.to_string());
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
                4096,
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
            let mut out = vec![0u8; line.len() * 2];
            let out_len = out.len();
            let mut src_pos = 0;
            let mut dst_pos = 0;
            quoted_printable::decode(line, &mut src_pos, &mut out, &mut dst_pos, out_len, false);
            if dst_pos > 0 {
                self.handler.body_content(&out[..dst_pos])?;
            }
        } else {
            self.handler.body_content(line)?;
        }
        Ok(())
    }

    fn flush_body_buffer(&mut self, include_trailing_crlf: bool) -> Result<(), MimeParseError> {
        let buf = &self.body_line_buffer[..];
        let to_send = if include_trailing_crlf || buf.is_empty() {
            buf
        } else if buf.ends_with(b"\r\n") {
            &buf[..buf.len() - 2]
        } else if buf.ends_with(b"\n") {
            &buf[..buf.len() - 1]
        } else {
            buf
        };
        if !to_send.is_empty() {
            self.handler.body_content(to_send)?;
        }
        self.body_line_buffer.clear();
        Ok(())
    }

    /// Return the handler (e.g. after close) for inspection in tests.
    pub fn into_inner(self) -> H {
        self.handler
    }

    /// End of input; flush any pending state.
    pub fn close(&mut self) -> Result<(), MimeParseError> {
        self.handler.set_locator(self.locator.clone());
        if !self.line_buffer.is_empty() {
            let line = std::mem::take(&mut self.line_buffer);
            if self.state == ParserState::Header {
                self.process_header_line(&line)?;
            } else if self.state == ParserState::Body {
                self.deliver_body_line(&line)?;
            } else if self.state == ParserState::BoundaryOrContent {
                if !self.body_line_buffer.is_empty() {
                    self.body_line_buffer.extend_from_slice(b"\r\n");
                }
                self.body_line_buffer.extend_from_slice(&line);
                self.flush_body_buffer(true)?;
            }
        }
        if self.state == ParserState::Body && self.content_transfer_encoding.as_deref() == Some("base64") && !self.b64_src.is_empty() {
            let mut out = [0u8; 1024];
            let mut dst_pos = 0;
            base64::decode(
                &self.b64_src,
                &mut self.b64_src_pos,
                &mut out,
                &mut dst_pos,
                4096,
                true,
            );
            if dst_pos > 0 {
                self.handler.body_content(&out[..dst_pos])?;
            }
        }
        self.handler.end_entity(self.boundary.as_deref())?;
        Ok(())
    }
}

fn trim_crlf(line: &[u8]) -> &[u8] {
    trim_trailing_crlf(line)
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
}
