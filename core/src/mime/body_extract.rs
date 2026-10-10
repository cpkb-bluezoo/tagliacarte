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

/// Bytes after the first RFC 5322 header/body separator (`\r\n\r\n`, else `\n\n`).
/// Used when structured MIME walk finds no text parts but the message is a single raw RFC822 blob.
pub fn body_bytes_after_rfc822_headers(raw: &[u8]) -> Option<&[u8]> {
    if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some(&raw[i + 4..]);
    }
    if let Some(i) = raw.windows(2).position(|w| w == b"\n\n") {
        return Some(&raw[i + 2..]);
    }
    None
}

/// Lossy UTF-8 of [body_bytes_after_rfc822_headers], with one leading CRLF or LF stripped if present.
pub fn utf8_body_after_rfc822_headers(raw: &[u8]) -> Option<String> {
    let b = body_bytes_after_rfc822_headers(raw)?;
    let b = b
        .strip_prefix(b"\r\n")
        .or_else(|| b.strip_prefix(b"\n"))
        .unwrap_or(b);
    if b.is_empty() {
        return None;
    }
    Some(bytes_to_utf8_string(b))
}

use rmimeparser::{ContentDisposition, ContentId, ContentType, MimeHandler, MimeParseError, MimeParser, ParseResult};

/// Result of structured extraction: body_plain, body_html, attachments
/// `(filename, content_id, mime_type, content)` — `content_id` is normalized (no angle brackets).
pub fn extract_structured_body(
    raw: &[u8],
) -> Result<
    (
        Option<String>,
        Option<String>,
        Vec<(Option<String>, Option<String>, String, Vec<u8>)>,
    ),
    MimeParseError,
> {
    let mut handler = StructuredBodyCollector::default();
    parse_whole(raw, &mut handler)?;
    Ok(handler.into_result())
}

/// Feed a complete message to rmimeparser's parser.
fn parse_whole<H: MimeHandler>(raw: &[u8], handler: &mut H) -> Result<(), MimeParseError> {
    let mut parser = MimeParser::new(handler);
    let mut data = raw;
    parser.finish(&mut data)
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
    let mut handler = PartEmitter { on_part, entity: Entity::default() };
    parse_whole(raw, &mut handler)
}

/// What the parser told us about the entity whose body is arriving.
#[derive(Default)]
struct Entity {
    content_type: Option<ContentType>,
    disposition: Option<ContentDisposition>,
    content_id: Option<String>,
    body: Vec<u8>,
}

impl Entity {
    fn reset(&mut self) {
        *self = Entity::default();
    }

    fn content_type_value(&self) -> String {
        self.content_type.as_ref().map(ContentType::to_header_value).unwrap_or_else(|| "application/octet-stream".to_string())
    }

    fn is_attachment(&self) -> bool {
        self.disposition
            .as_ref()
            .map(|cd| cd.is_disposition_type("attachment") || cd.has_parameter("filename"))
            .unwrap_or(false)
    }

    fn filename(&self) -> Option<String> {
        self.disposition.as_ref().and_then(|cd| cd.get_parameter("filename").map(str::to_string))
    }
}

/// Handler that emits a callback for each MIME part (content-type + content + optional filename).
struct PartEmitter<F> {
    on_part: F,
    entity: Entity,
}

impl<F: FnMut(&str, &[u8], Option<&str>)> MimeHandler for PartEmitter<F> {
    fn start_entity(&mut self, _boundary: Option<&str>) -> ParseResult<()> {
        self.entity.reset();
        Ok(())
    }
    fn content_type(&mut self, ct: &ContentType) -> ParseResult<()> {
        self.entity.content_type = Some(ct.clone());
        Ok(())
    }
    fn content_disposition(&mut self, cd: &ContentDisposition) -> ParseResult<()> {
        self.entity.disposition = Some(cd.clone());
        Ok(())
    }
    fn body_content(&mut self, data: &[u8]) -> ParseResult<()> {
        self.entity.body.extend_from_slice(data);
        Ok(())
    }
    fn end_entity(&mut self, _boundary: Option<&str>) -> ParseResult<()> {
        if !self.entity.body.is_empty() {
            let ct_value = self.entity.content_type_value();
            let filename = self.entity.filename();
            let is_multipart = self.entity.content_type.as_ref().map(|ct| ct.is_primary_type("multipart")).unwrap_or(false);
            if self.entity.is_attachment() || !is_multipart {
                (self.on_part)(&ct_value, &self.entity.body, filename.as_deref());
            }
        }
        self.entity.reset();
        Ok(())
    }
}

/// Handler that collects display bodies and attachments per entity.
#[derive(Default)]
struct StructuredBodyCollector {
    entity: Entity,
    body_plain: Option<String>,
    body_html: Option<String>,
    attachments: Vec<(Option<String>, Option<String>, String, Vec<u8>)>,
}

impl MimeHandler for StructuredBodyCollector {
    fn start_entity(&mut self, _boundary: Option<&str>) -> ParseResult<()> {
        self.entity.reset();
        Ok(())
    }
    fn content_type(&mut self, ct: &ContentType) -> ParseResult<()> {
        self.entity.content_type = Some(ct.clone());
        Ok(())
    }
    fn content_disposition(&mut self, cd: &ContentDisposition) -> ParseResult<()> {
        self.entity.disposition = Some(cd.clone());
        Ok(())
    }
    fn content_id(&mut self, id: &ContentId) -> ParseResult<()> {
        self.entity.content_id = Some(format!("{}@{}", id.local_part(), id.domain()));
        Ok(())
    }
    fn body_content(&mut self, data: &[u8]) -> ParseResult<()> {
        self.entity.body.extend_from_slice(data);
        Ok(())
    }
    fn end_entity(&mut self, _boundary: Option<&str>) -> ParseResult<()> {
        if self.entity.body.is_empty() {
            self.entity.reset();
            return Ok(());
        }
        let ct_value = self.entity.content_type_value();
        let body = std::mem::take(&mut self.entity.body);
        let filename = self.entity.filename();
        let cid = self.entity.content_id.clone();
        if self.entity.is_attachment() {
            self.attachments.push((filename, cid, ct_value, body));
        } else if let Some(ct) = self.entity.content_type.as_ref() {
            if ct.is_mime_type("message", "rfc822") {
                // An embedded message: look inside it for the bodies.
                let inner = extract_structured_body(&body).unwrap_or((None, None, vec![]));
                if self.body_plain.is_none() {
                    self.body_plain = inner.0;
                }
                if self.body_html.is_none() {
                    self.body_html = inner.1;
                }
            } else {
                let s = bytes_to_utf8_string(&body);
                if ct.is_mime_type("text", "html") && self.body_html.is_none() {
                    self.body_html = Some(s);
                } else if ct.is_mime_type("text", "plain") && self.body_plain.is_none() {
                    self.body_plain = Some(s);
                } else if cid.is_some() {
                    // Inline image (or other) part referenced by cid:
                    self.attachments.push((filename, cid, ct_value, body));
                }
            }
        } else {
            // No Content-Type at all: RFC 2045 says text/plain.
            if self.body_plain.is_none() {
                self.body_plain = Some(bytes_to_utf8_string(&body));
            }
        }
        self.entity.reset();
        Ok(())
    }
}

impl StructuredBodyCollector {
    fn into_result(
        self,
    ) -> (
        Option<String>,
        Option<String>,
        Vec<(Option<String>, Option<String>, String, Vec<u8>)>,
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

    #[test]
    fn body_after_headers_skips_top_rfc822_headers() {
        let raw = b"From: a@b\r\nTo: c@d\r\nSubject: x\r\nContent-Type: text/x-weird\r\n\r\nJust the payload.";
        let (plain, html) = extract_display_body(raw).unwrap();
        assert!(plain.is_none() && html.is_none());
        let fb = utf8_body_after_rfc822_headers(raw).expect("fallback");
        assert_eq!(fb, "Just the payload.");
    }

    #[test]
    fn extract_collects_content_id_inline_part() {
        let raw = b"MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=b\r\n\r\n--b\r\nContent-Type: text/html\r\n\r\n<img src=\"cid:foo\">\r\n--b\r\nContent-Type: image/png\r\nContent-ID: <foo@bar>\r\n\r\nfake\r\n--b--";
        let (_, html, parts) = extract_structured_body(raw).expect("parse");
        assert!(html.is_some());
        let cid_part = parts
            .iter()
            .find(|(_, cid, _, _)| cid.as_deref() == Some("foo@bar"));
        assert!(cid_part.is_some(), "expected inline cid part");
    }
}
