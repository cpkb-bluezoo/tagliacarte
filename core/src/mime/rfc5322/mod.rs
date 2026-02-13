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

//! RFC 5322 message parser: envelope headers (Date, From, To, Cc, Subject, Message-ID).

mod address_parser;
mod date_time;
mod email_address;
mod handler;
mod message_id_list;
mod obsolete;
mod thread_headers;

use crate::mime::content_id::ContentID;
use crate::mime::handler::{MimeHandler, MimeParseError};
use crate::mime::parser::{HeaderValueDecoder, MimeParser};
use crate::mime::{bytes_to_string, decode_header_value_bytes};
use chrono::{DateTime, FixedOffset};

pub use email_address::{format_mailbox, EmailAddress};
pub use handler::MessageHandler;
pub use obsolete::ObsoleteStructureType;
pub use thread_headers::parse_thread_headers;

use address_parser::parse_email_address_list;
use date_time::parse_rfc5322_date;
use message_id_list::parse_message_id_list;

/// RFC 5322 envelope headers (top-level message only).
#[derive(Debug, Default)]
pub struct EnvelopeHeaders {
    pub date: Option<DateTime<FixedOffset>>,
    pub from: Vec<EmailAddress>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub subject: Option<String>,
    pub message_id: Option<ContentID>,
}

/// Adapter that implements MimeHandler and dispatches to a MessageHandler for RFC 5322 headers.
pub(crate) struct Rfc5322Adapter<H> {
    pub(crate) inner: H,
}

impl<H: MessageHandler> MimeHandler for Rfc5322Adapter<H> {
    fn set_locator(&mut self, locator: crate::mime::MimeLocator) {
        self.inner.set_locator(locator);
    }
    fn start_entity(&mut self, boundary: Option<&str>) -> Result<(), MimeParseError> {
        self.inner.start_entity(boundary)
    }
    fn content_type(&mut self, v: &str) -> Result<(), MimeParseError> {
        self.inner.content_type(v)
    }
    fn content_disposition(&mut self, v: &str) -> Result<(), MimeParseError> {
        self.inner.content_disposition(v)
    }
    fn content_transfer_encoding(&mut self, v: &str) -> Result<(), MimeParseError> {
        self.inner.content_transfer_encoding(v)
    }
    fn content_id(&mut self, v: &str) -> Result<(), MimeParseError> {
        self.inner.content_id(v)
    }
    fn content_description(&mut self, v: &str) -> Result<(), MimeParseError> {
        self.inner.content_description(v)
    }
    fn mime_version(&mut self, v: &str) -> Result<(), MimeParseError> {
        self.inner.mime_version(v)
    }
    fn header(&mut self, name: &str, value: &str) -> Result<(), MimeParseError> {
        let name_lower = name.to_ascii_lowercase();
        match name_lower.as_str() {
            "date" | "resent-date" => {
                if let Some(dt) = parse_rfc5322_date(value) {
                    self.inner.date_header(name, dt)?;
                } else {
                    self.inner.unexpected_header(name, value)?;
                }
            }
            "from" | "sender" | "to" | "cc" | "bcc" | "reply-to"
            | "resent-from" | "return-path" | "resent-sender" | "resent-to"
            | "resent-cc" | "resent-bcc" | "resent-reply-to" | "envelope-to"
            | "delivered-to" | "x-original-to" | "errors-to" | "apparently-to" => {
                if let Some(addrs) = parse_email_address_list(value) {
                    self.inner.address_header(name, &addrs)?;
                } else {
                    self.inner.unexpected_header(name, value)?;
                }
            }
            "message-id" | "in-reply-to" | "references" | "resent-message-id" => {
                if let Some(ids) = parse_message_id_list(value) {
                    if !ids.is_empty() {
                        self.inner.message_id_header(name, &ids)?;
                    } else {
                        self.inner.unexpected_header(name, value)?;
                    }
                } else {
                    self.inner.unexpected_header(name, value)?;
                }
            }
            _ => MimeHandler::header(&mut self.inner, name, value)?,
        }
        Ok(())
    }
    fn end_headers(&mut self) -> Result<(), MimeParseError> {
        self.inner.end_headers()
    }
    fn body_content(&mut self, data: &[u8]) -> Result<(), MimeParseError> {
        self.inner.body_content(data)
    }
    fn unexpected_content(&mut self, data: &[u8]) -> Result<(), MimeParseError> {
        self.inner.unexpected_content(data)
    }
    fn end_entity(&mut self, boundary: Option<&str>) -> Result<(), MimeParseError> {
        self.inner.end_entity(boundary)
    }
}

#[allow(dead_code)]
trait ToAsciiLowercase {
    fn to_ascii_lowercase(&self) -> String;
}
impl ToAsciiLowercase for str {
    fn to_ascii_lowercase(&self) -> String {
        self.chars().map(|c| c.to_ascii_lowercase()).collect()
    }
}

/// Parser for RFC 5322 email messages. Wraps MimeParser and dispatches envelope headers to a MessageHandler.
pub struct MessageParser<H: MessageHandler> {
    parser: MimeParser<Rfc5322Adapter<H>>,
}

fn is_unstructured_header(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(n.as_str(), "subject" | "comments" | "keywords" | "received")
        || n.starts_with("x-")
}

fn is_address_header(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "from" | "sender" | "to" | "cc" | "bcc" | "reply-to"
            | "resent-from" | "return-path" | "resent-sender" | "resent-to"
            | "resent-cc" | "resent-bcc" | "resent-reply-to" | "envelope-to"
            | "delivered-to" | "x-original-to" | "errors-to" | "apparently-to"
    )
}

fn is_mime_header_no_rfc2047(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "content-type"
            | "content-disposition"
            | "content-transfer-encoding"
            | "content-id"
            | "mime-version"
            | "content-description"
    )
}

impl<H: MessageHandler> MessageParser<H> {
    pub fn new(handler: H) -> Self {
        Self::new_with_smtp_utf8(handler, false)
    }

    pub fn new_with_smtp_utf8(handler: H, smtp_utf8: bool) -> Self {
        let mut parser = MimeParser::new(Rfc5322Adapter { inner: handler });
        let decoder: HeaderValueDecoder = Box::new(move |name, value| {
            let raw = bytes_to_string(value, smtp_utf8);
            let raw = raw.trim();
            if is_mime_header_no_rfc2047(name) {
                raw.to_string()
            } else if is_unstructured_header(name) || is_address_header(name) {
                decode_header_value_bytes(value, smtp_utf8)
            } else {
                raw.to_string()
            }
        });
        parser.set_header_value_decoder(Some(decoder));
        Self { parser }
    }

    /// Process bytes; returns number of bytes consumed.
    pub fn receive(&mut self, buf: &[u8]) -> Result<usize, MimeParseError> {
        self.parser.receive(buf)
    }

    /// End of input.
    pub fn close(&mut self) -> Result<(), MimeParseError> {
        self.parser.close()
    }

    pub fn into_inner(self) -> H {
        self.parser.into_inner().inner
    }
}

/// Handler that collects only envelope headers (for parse_envelope).
struct EnvelopeCollector {
    envelope: EnvelopeHeaders,
}

impl MimeHandler for EnvelopeCollector {
    fn header(&mut self, name: &str, value: &str) -> Result<(), MimeParseError> {
        if name.eq_ignore_ascii_case("subject") {
            self.envelope.subject = Some(value.to_string());
        }
        Ok(())
    }
}

impl MessageHandler for EnvelopeCollector {
    fn date_header(&mut self, name: &str, date: DateTime<FixedOffset>) -> Result<(), MimeParseError> {
        if name.eq_ignore_ascii_case("date") {
            self.envelope.date = Some(date);
        }
        Ok(())
    }
    fn address_header(&mut self, name: &str, addresses: &[EmailAddress]) -> Result<(), MimeParseError> {
        let addrs: Vec<EmailAddress> = addresses.to_vec();
        match name.to_ascii_lowercase().as_str() {
            "from" | "sender" => {
                if self.envelope.from.is_empty() {
                    self.envelope.from = addrs;
                }
            }
            "to" => self.envelope.to = addrs,
            "cc" => self.envelope.cc = addrs,
            _ => {}
        }
        Ok(())
    }
    fn message_id_header(&mut self, name: &str, ids: &[ContentID]) -> Result<(), MimeParseError> {
        if name.eq_ignore_ascii_case("message-id") && self.envelope.message_id.is_none() {
            if let Some(id) = ids.first() {
                self.envelope.message_id = Some(ContentID::new(
                    id.get_local_part(),
                    id.get_domain(),
                ));
            }
        }
        Ok(())
    }
}

/// Parse envelope headers only from raw message bytes (stops after headers; does not require full body).
pub fn parse_envelope(raw: &[u8]) -> Result<EnvelopeHeaders, MimeParseError> {
    let collector = EnvelopeCollector {
        envelope: EnvelopeHeaders::default(),
    };
    let mut parser = MessageParser::new(collector);
    parser.receive(raw)?;
    parser.close()?;
    Ok(parser.into_inner().envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_envelope_simple() {
        let raw = b"From: alice@example.com\r\nTo: bob@example.com\r\nSubject: Hello\r\nDate: Fri, 21 Nov 1997 09:55:06 -0600\r\nMessage-ID: <id@host>\r\n\r\nBody";
        let env = parse_envelope(raw).unwrap();
        assert_eq!(env.from.len(), 1);
        assert_eq!(env.from[0].address(), "alice@example.com");
        assert_eq!(env.to.len(), 1);
        assert_eq!(env.to[0].address(), "bob@example.com");
        assert_eq!(env.subject.as_deref(), Some("Hello"));
        assert!(env.date.is_some());
        assert!(env.message_id.is_some());
        assert_eq!(env.message_id.as_ref().unwrap().get_local_part(), "id");
        assert_eq!(env.message_id.as_ref().unwrap().get_domain(), "host");
    }
}
