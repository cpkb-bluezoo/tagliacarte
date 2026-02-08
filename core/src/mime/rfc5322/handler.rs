/*
 * handler.rs
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

//! RFC 5322 message handler: envelope and structured header callbacks.

use crate::mime::content_id::ContentID;
use crate::mime::handler::MimeParseError;
use chrono::{DateTime, FixedOffset};

use super::email_address::EmailAddress;
use super::obsolete::ObsoleteStructureType;

/// Handler for RFC 5322 message parsing events (extends MIME with envelope headers).
pub trait MessageHandler: crate::mime::MimeHandler {
    /// Unstructured header (e.g. Subject, Comments).
    fn header(&mut self, _name: &str, _value: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    /// Structured header that could not be parsed.
    fn unexpected_header(&mut self, _name: &str, _value: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    /// Date header (Date, Resent-Date).
    fn date_header(&mut self, _name: &str, _date: DateTime<FixedOffset>) -> Result<(), MimeParseError> {
        Ok(())
    }

    /// Address header (From, To, Cc, etc.).
    fn address_header(&mut self, _name: &str, _addresses: &[EmailAddress]) -> Result<(), MimeParseError> {
        Ok(())
    }

    /// Message-ID header (Message-ID, References, In-Reply-To).
    fn message_id_header(&mut self, _name: &str, _ids: &[ContentID]) -> Result<(), MimeParseError> {
        Ok(())
    }

    /// Obsolete but recoverable syntax was used.
    fn obsolete_structure(&mut self, _kind: ObsoleteStructureType) -> Result<(), MimeParseError> {
        Ok(())
    }
}
