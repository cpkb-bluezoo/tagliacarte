/*
 * body_extract.rs
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

//! Extract displayable body (text/html or text/plain) and attachments from raw RFC 822 / MIME bytes.

use crate::mime::content_disposition::parse_content_disposition;
use crate::mime::content_type::parse_content_type;
use crate::mime::handler::{MimeHandler, MimeParseError};
use crate::mime::parser::MimeParser;

/// Result of structured extraction: body_plain, body_html, attachments (filename, mime_type, content).
pub fn extract_structured_body(
    raw: &[u8],
) -> Result<
    (
        Option<String>,
        Option<String>,
        Vec<(Option<String>, String, Vec<u8>)>,
    ),
    MimeParseError,
> {
    let handler = StructuredBodyCollector::default();
    let mut parser = MimeParser::new(handler);
    // Single receive() call: parser processes all complete lines and buffers any
    // incomplete tail in line_buffer. close() flushes the buffer.
    parser.receive(raw)?;
    parser.close()?;
    let handler = parser.into_inner();
    Ok(handler.into_result())
}

/// Result of body extraction: (html_body, plain_body). One or both may be None.
pub fn extract_display_body(raw: &[u8]) -> Result<(Option<String>, Option<String>), MimeParseError> {
    let (plain, html, _) = extract_structured_body(raw)?;
    Ok((html, plain))
}

/// Callback for each MIME part: (content_type, content, filename).
/// content_type is the full value e.g. "text/plain; charset=utf-8".
/// filename is from Content-Disposition, or None.
pub fn emit_message_parts<F>(raw: &[u8], on_part: F) -> Result<(), MimeParseError>
where
    F: FnMut(&str, &[u8], Option<&str>),
{
    let handler = PartEmitter {
        on_part,
        current_content_type: None,
        current_content_disposition: None,
        current_body: Vec::new(),
    };
    let mut parser = MimeParser::new(handler);
    parser.receive(raw)?;
    parser.close()?;
    Ok(())
}

/// Handler that emits a callback for each MIME part (content-type + content + optional filename).
struct PartEmitter<F> {
    on_part: F,
    current_content_type: Option<String>,
    current_content_disposition: Option<String>,
    current_body: Vec<u8>,
}

impl<F: FnMut(&str, &[u8], Option<&str>)> MimeHandler for PartEmitter<F> {
    fn start_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
        self.current_content_type = None;
        self.current_content_disposition = None;
        self.current_body.clear();
        Ok(())
    }

    fn content_type(&mut self, value: &str) -> Result<(), MimeParseError> {
        self.current_content_type = Some(value.to_string());
        Ok(())
    }

    fn content_disposition(&mut self, value: &str) -> Result<(), MimeParseError> {
        self.current_content_disposition = Some(value.to_string());
        Ok(())
    }

    fn body_content(&mut self, data: &[u8]) -> Result<(), MimeParseError> {
        self.current_body.extend_from_slice(data);
        Ok(())
    }

    fn end_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
        if self.current_body.is_empty() {
            self.current_content_type = None;
            self.current_content_disposition = None;
            return Ok(());
        }
        let ct_value = self
            .current_content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let body = std::mem::take(&mut self.current_body);
        let cd = self.current_content_disposition.as_deref();
        let is_attachment = cd
            .and_then(parse_content_disposition)
            .map(|cd| cd.is_disposition_type("attachment") || cd.has_parameter("filename"))
            .unwrap_or(false);
        let filename = cd
            .and_then(parse_content_disposition)
            .and_then(|cd| cd.get_parameter("filename").map(|s| s.to_string()));

        if is_attachment {
            (self.on_part)(&ct_value, &body, filename.as_deref());
        } else if let Some(ct) = parse_content_type(ct_value.trim()) {
            if !ct.is_primary_type("multipart") {
                (self.on_part)(&ct_value, &body, filename.as_deref());
            }
        }
        self.current_content_type = None;
        self.current_content_disposition = None;
        Ok(())
    }
}

/// Handler that collects display bodies and attachments per entity.
#[derive(Default)]
struct StructuredBodyCollector {
    current_content_type: Option<String>,
    current_content_disposition: Option<String>,
    current_body: Vec<u8>,
    body_plain: Option<String>,
    body_html: Option<String>,
    attachments: Vec<(Option<String>, String, Vec<u8>)>,
}

impl MimeHandler for StructuredBodyCollector {
    fn start_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
        self.current_content_type = None;
        self.current_content_disposition = None;
        self.current_body.clear();
        Ok(())
    }

    fn content_type(&mut self, value: &str) -> Result<(), MimeParseError> {
        self.current_content_type = Some(value.to_string());
        Ok(())
    }

    fn content_disposition(&mut self, value: &str) -> Result<(), MimeParseError> {
        self.current_content_disposition = Some(value.to_string());
        Ok(())
    }

    fn body_content(&mut self, data: &[u8]) -> Result<(), MimeParseError> {
        self.current_body.extend_from_slice(data);
        Ok(())
    }

    fn end_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
        if self.current_body.is_empty() {
            self.current_content_type = None;
            self.current_content_disposition = None;
            return Ok(());
        }
        let ct_value = self
            .current_content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let body = std::mem::take(&mut self.current_body);
        let cd = self.current_content_disposition.as_deref();
        let is_attachment = cd
            .and_then(parse_content_disposition)
            .map(|cd| cd.is_disposition_type("attachment") || cd.has_parameter("filename"))
            .unwrap_or(false);
        let filename = cd
            .and_then(parse_content_disposition)
            .and_then(|cd| cd.get_parameter("filename").map(|s| s.to_string()));

        if is_attachment {
            self.attachments.push((filename, ct_value, body));
        } else if let Some(ct) = parse_content_type(ct_value.trim()) {
            let s = bytes_to_utf8_string(&body);
            if ct.is_mime_type("text", "html") && self.body_html.is_none() {
                self.body_html = Some(s);
            } else if ct.is_mime_type("text", "plain") && self.body_plain.is_none() {
                self.body_plain = Some(s);
            }
        }
        self.current_content_type = None;
        self.current_content_disposition = None;
        Ok(())
    }
}

impl StructuredBodyCollector {
    fn into_result(
        self,
    ) -> (
        Option<String>,
        Option<String>,
        Vec<(Option<String>, String, Vec<u8>)>,
    ) {
        (self.body_plain, self.body_html, self.attachments)
    }
}

fn bytes_to_utf8_string(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(b).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_plain_only() {
        let raw = b"From: a@b.com\r\nTo: c@d.com\r\nSubject: Hi\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello, world.";
        let (html, plain) = extract_display_body(raw).unwrap();
        assert!(html.is_none());
        assert_eq!(plain.as_deref(), Some("Hello, world."));
    }

    #[test]
    fn extract_html_and_plain() {
        let raw = b"MIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nPlain.\r\n--x\r\nContent-Type: text/html\r\n\r\n<b>HTML</b>\r\n--x--";
        let (html, plain) = extract_display_body(raw).unwrap();
        assert_eq!(plain.as_deref(), Some("Plain."));
        assert_eq!(html.as_deref(), Some("<b>HTML</b>"));
    }
}
