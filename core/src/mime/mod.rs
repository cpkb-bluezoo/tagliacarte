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

//! MIME and RFC 5322 on rmimeparser. The parser, the transfer-encoding
//! codecs and the header value parsers come from rmimeparser; what lives
//! here is the application layer: structured body extraction for the
//! reader, envelope and threading headers for the stores, streaming
//! transfer decoding for the mail-view server, and the uuencode section
//! NNTP posts carry attachments in.

mod body_extract;
mod rfc5322;
mod transfer_encoding;
mod uuencode;

pub use rmimeparser::{
    ContentDisposition, ContentDispositionParser, ContentId as ContentID, ContentIdParser,
    ContentType, ContentTypeParser, MessageHandler, MimeHandler, MimeLocator, MimeParseError,
    MimeParser, MimeVersion, Parameter,
};

/// Parse a `Content-Type` header value.
pub fn parse_content_type(value: &str) -> Option<ContentType> {
    ContentTypeParser::parse_str(value)
}

/// Parse a `Content-Disposition` header value.
pub fn parse_content_disposition(value: &str) -> Option<ContentDisposition> {
    ContentDispositionParser::parse_str(value)
}

/// Parse a `Content-ID` or `Message-ID` header value.
pub fn parse_content_id(value: &str) -> Option<ContentID> {
    ContentIdParser::parse_str(value)
}

pub use body_extract::{
    body_bytes_after_rfc822_headers, emit_message_parts, extract_display_body,
    extract_structured_body, utf8_body_after_rfc822_headers,
};
pub use rfc5322::{
    format_mailbox, parse_email_address_list, parse_envelope, parse_thread_headers, EmailAddress,
    EnvelopeHeaders, ThreadHeaders,
};
pub use transfer_encoding::{
    decode_content_transfer_encoding, StreamingBase64Decoder, StreamingCteDecoder,
    StreamingQuotedPrintableDecoder, StreamingUuencodeDecoder, Utf8StreamAssembler,
};
pub use uuencode::uuencode_file_section;
