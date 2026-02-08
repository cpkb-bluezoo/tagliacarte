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

//! MIME handler trait: receives parsing events (entity, headers, body chunks).

/// Handler for MIME parsing events (push model). Parser calls these as it reads.
pub trait MimeHandler {
    fn set_locator(&mut self, _locator: MimeLocator) {}

    fn start_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn content_type(&mut self, _content_type: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn content_disposition(&mut self, _value: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn content_transfer_encoding(&mut self, _encoding: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn content_id(&mut self, _id: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn content_description(&mut self, _description: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn mime_version(&mut self, _version: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    /// Unstructured or unknown header (RFC 5322). Called for headers not handled by content_type, etc.
    fn header(&mut self, _name: &str, _value: &str) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn end_headers(&mut self) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn body_content(&mut self, _data: &[u8]) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn unexpected_content(&mut self, _data: &[u8]) -> Result<(), MimeParseError> {
        Ok(())
    }

    fn end_entity(&mut self, _boundary: Option<&str>) -> Result<(), MimeParseError> {
        Ok(())
    }
}

/// Position within the MIME entity for error reporting.
#[derive(Debug, Clone)]
pub struct MimeLocator {
    pub offset: u64,
    pub line: u64,
    pub column: u64,
}

#[derive(Debug)]
pub struct MimeParseError {
    pub message: String,
    pub locator: Option<MimeLocator>,
}

impl std::fmt::Display for MimeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MimeParseError {}
