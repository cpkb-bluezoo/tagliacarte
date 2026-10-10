/*
 * rfc5322.rs
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

//! Envelope and threading headers, parsed with rmimeparser's RFC 5322
//! message parser and handed to the stores as plain values.

use chrono::{DateTime, FixedOffset, NaiveDate};
use rmimeparser::{
    Address, EmailAddressParser, MessageHandler, MessageParser, MimeHandler, MimeParseError,
    OffsetDateTime, ParseResult,
};

use super::ContentID;

/// RFC 5322 mailbox as the stores use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAddress {
    pub display_name: Option<String>,
    pub local_part: String,
    pub domain: String,
}

impl EmailAddress {
    pub fn new(display_name: Option<impl Into<String>>, local_part: impl Into<String>, domain: impl Into<String>) -> Self {
        Self { display_name: display_name.map(Into::into), local_part: local_part.into(), domain: domain.into() }
    }

    /// Display name (canonical form: no surrounding double-quotes).
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn local_part(&self) -> &str {
        &self.local_part
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Full mailbox address: local-part@domain.
    pub fn address(&self) -> String {
        format!("{}@{}", self.local_part, self.domain)
    }

    /// Envelope address for SMTP (same as address).
    pub fn envelope_address(&self) -> String {
        self.address()
    }
}

impl From<&rmimeparser::EmailAddress> for EmailAddress {
    fn from(a: &rmimeparser::EmailAddress) -> Self {
        Self {
            display_name: a.display_name().map(str::to_string),
            local_part: a.local_part().to_string(),
            domain: a.domain().to_string(),
        }
    }
}

impl std::fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref dn) = self.display_name {
            if !dn.is_empty() {
                write!(f, "{} ", dn)?;
            }
        }
        write!(f, "<{}>", self.address())
    }
}

/// Format a mailbox for RFC 5322 headers (same format as [`EmailAddress`]'s
/// `Display`). Use when building messages; domain may be empty for local-only.
pub fn format_mailbox(display_name: Option<&str>, local_part: &str, domain: &str) -> String {
    let addr = if domain.is_empty() { local_part.to_string() } else { format!("{}@{}", local_part, domain) };
    match display_name {
        Some(dn) if !dn.is_empty() => format!("{} <{}>", dn, addr),
        _ => format!("<{}>", addr),
    }
}

/// Parse an address-list header value; group members are flattened.
pub fn parse_email_address_list(value: &str) -> Option<Vec<EmailAddress>> {
    let list = EmailAddressParser::parse_email_address_list(value)?;
    Some(flatten(&list))
}

fn flatten(list: &[Address]) -> Vec<EmailAddress> {
    let mut out = Vec::new();
    for a in list {
        match a {
            Address::Mailbox(m) => out.push(EmailAddress::from(m)),
            Address::Group(g) => out.extend(g.members().iter().map(EmailAddress::from)),
        }
    }
    out
}

fn to_chrono(d: &OffsetDateTime) -> Option<DateTime<FixedOffset>> {
    let offset = FixedOffset::east_opt(d.offset_seconds)?;
    let naive = NaiveDate::from_ymd_opt(d.year, u32::from(d.month), u32::from(d.day))?
        .and_hms_opt(u32::from(d.hour), u32::from(d.minute), u32::from(d.second))?;
    naive.and_local_timezone(offset).single()
}

fn ids_in_angle_brackets(ids: &[ContentID]) -> String {
    ids.iter().map(|id| format!("<{}@{}>", id.local_part(), id.domain())).collect::<Vec<_>>().join(" ")
}

/// RFC 5322 envelope headers (top-level message only).
#[derive(Debug, Default)]
pub struct EnvelopeHeaders {
    pub date: Option<DateTime<FixedOffset>>,
    pub from: Vec<EmailAddress>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub subject: Option<String>,
    pub message_id: Option<ContentID>,
    /// Decoded `In-Reply-To` message ids (angle-bracket form, space-separated if multiple).
    pub in_reply_to: Option<String>,
    /// Decoded `References` message ids (angle-bracket form, space-separated).
    pub references: Option<String>,
}

#[derive(Default)]
struct EnvelopeCollector {
    envelope: EnvelopeHeaders,
}

impl MimeHandler for EnvelopeCollector {}

impl MessageHandler for EnvelopeCollector {
    fn header(&mut self, name: &str, value: &str) -> ParseResult<()> {
        if name.eq_ignore_ascii_case("subject") {
            self.envelope.subject = Some(value.to_string());
        }
        Ok(())
    }

    fn date_header(&mut self, name: &str, date: OffsetDateTime) -> ParseResult<()> {
        if name.eq_ignore_ascii_case("date") {
            self.envelope.date = to_chrono(&date);
        }
        Ok(())
    }

    fn address_header(&mut self, name: &str, addresses: &[rmimeparser::EmailAddress]) -> ParseResult<()> {
        let addrs: Vec<EmailAddress> = addresses.iter().map(EmailAddress::from).collect();
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

    fn message_id_header(&mut self, name: &str, ids: &[ContentID]) -> ParseResult<()> {
        let n = name.to_ascii_lowercase();
        if n == "message-id" {
            if self.envelope.message_id.is_none() {
                self.envelope.message_id = ids.first().cloned();
            }
            return Ok(());
        }
        let slot = match n.as_str() {
            "in-reply-to" => &mut self.envelope.in_reply_to,
            "references" => &mut self.envelope.references,
            _ => return Ok(()),
        };
        let part = ids_in_angle_brackets(ids);
        if !part.is_empty() {
            *slot = Some(match slot.take() {
                None => part,
                Some(existing) => format!("{existing} {part}"),
            });
        }
        Ok(())
    }
}

/// Run `handler` over `raw` with the RFC 5322 message parser.
fn parse_message<H: MessageHandler>(raw: &[u8], handler: &mut H) -> Result<(), MimeParseError> {
    let mut parser = MessageParser::new(handler);
    let mut data = raw;
    parser.finish(&mut data)
}

/// Parse the envelope headers of a raw message (body may follow).
pub fn parse_envelope(raw: &[u8]) -> Result<EnvelopeHeaders, MimeParseError> {
    let mut c = EnvelopeCollector::default();
    parse_message(raw, &mut c)?;
    Ok(c.envelope)
}

/// Subject, Message-ID, and References/In-Reply-To for thread grouping.
#[derive(Debug, Default)]
pub struct ThreadHeaders {
    pub subject: Option<String>,
    pub message_id: Option<String>,
    /// References and In-Reply-To msg-ids in order (References first, then In-Reply-To).
    pub references: Vec<String>,
}

#[derive(Default)]
struct ThreadHeadersCollector {
    out: ThreadHeaders,
}

impl MimeHandler for ThreadHeadersCollector {}

impl MessageHandler for ThreadHeadersCollector {
    fn header(&mut self, name: &str, value: &str) -> ParseResult<()> {
        if name.eq_ignore_ascii_case("subject") {
            self.out.subject = Some(value.trim().to_string());
        }
        Ok(())
    }

    fn message_id_header(&mut self, name: &str, ids: &[ContentID]) -> ParseResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        if name.eq_ignore_ascii_case("message-id") {
            self.out.message_id = Some(ids_in_angle_brackets(&ids[..1]));
        } else if name.eq_ignore_ascii_case("references") || name.eq_ignore_ascii_case("in-reply-to") {
            self.out.references.extend(ids.iter().map(|id| ids_in_angle_brackets(std::slice::from_ref(id))));
        }
        Ok(())
    }
}

/// Parse Subject, Message-ID, References, In-Reply-To from raw message bytes.
pub fn parse_thread_headers(raw: &[u8]) -> Result<ThreadHeaders, MimeParseError> {
    let mut c = ThreadHeadersCollector::default();
    parse_message(raw, &mut c)?;
    Ok(c.out)
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
        assert_eq!(env.to[0].address(), "bob@example.com");
        assert_eq!(env.subject.as_deref(), Some("Hello"));
        let d = env.date.expect("date");
        assert_eq!(d.to_rfc3339(), "1997-11-21T09:55:06-06:00");
        let id = env.message_id.expect("message id");
        assert_eq!((id.local_part(), id.domain()), ("id", "host"));
    }

    #[test]
    fn parse_envelope_references_in_reply_to_and_encoded_subject() {
        let raw = b"From: =?UTF-8?Q?Ren=C3=A9?= <a@b>\r\nSubject: =?UTF-8?B?w4l0w6k=?=\r\nMessage-ID: <m1@host>\r\nReferences: <r1@x> <r2@y>\r\nIn-Reply-To: <p@z>\r\n\r\n";
        let env = parse_envelope(raw).unwrap();
        assert_eq!(env.references.as_deref(), Some("<r1@x> <r2@y>"));
        assert_eq!(env.in_reply_to.as_deref(), Some("<p@z>"));
        assert_eq!(env.subject.as_deref(), Some("\u{c9}t\u{e9}"));
        assert_eq!(env.from[0].display_name.as_deref(), Some("Ren\u{e9}"));
    }

    #[test]
    fn thread_headers_and_groups() {
        let raw = b"Subject: Re: x\r\nMessage-ID: <m@h>\r\nReferences: <a@h> <b@h>\r\nIn-Reply-To: <b@h>\r\n\r\nbody";
        let t = parse_thread_headers(raw).unwrap();
        assert_eq!(t.message_id.as_deref(), Some("<m@h>"));
        assert_eq!(t.references, vec!["<a@h>", "<b@h>", "<b@h>"]);
        let list = parse_email_address_list("Team: a@x.org, Bob <b@x.org>;, c@y.org").unwrap();
        assert_eq!(list.iter().map(|a| a.address()).collect::<Vec<_>>(), vec!["a@x.org", "b@x.org", "c@y.org"]);
        assert_eq!(list[1].display_name.as_deref(), Some("Bob"));
        assert_eq!(format_mailbox(Some("Bob"), "b", "x.org"), "Bob <b@x.org>");
    }
}
