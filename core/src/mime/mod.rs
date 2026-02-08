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

//! Event-driven MIME and RFC 5322 message parsing (push/handler model, non-blocking buffer contract).

mod base64;
mod body_extract;
mod content_disposition;
mod content_id;
mod content_type;
mod handler;
mod mime_version;
mod parameter;
mod parser;
mod quoted_printable;
mod rfc5322;
mod utils;

pub use body_extract::{extract_display_body, extract_structured_body};
pub use content_disposition::{parse_content_disposition, ContentDisposition};
pub use content_id::{parse_content_id, ContentID};
pub use content_type::{parse_content_type, ContentType};
pub use handler::{MimeHandler, MimeLocator, MimeParseError};
pub use mime_version::MimeVersion;
pub use parameter::Parameter;
pub use parser::MimeParser;
pub use rfc5322::{
    EmailAddress, EnvelopeHeaders, MessageHandler, MessageParser, ObsoleteStructureType,
    format_mailbox, parse_envelope, parse_thread_headers,
};
pub use utils::{is_boundary_char, is_token, is_token_char, is_valid_boundary};
