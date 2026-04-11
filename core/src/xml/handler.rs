/*
 * handler.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! SAX-like content handler for the push XML parser.

/// Handler for XML parse events. String slices are valid only for the duration of each call.
pub trait XmlContentHandler {
    fn start_document(&mut self) {}
    fn end_document(&mut self) {}

    /// Opening tag fully known: `namespace_uri` is `None` when the parser is not namespace-aware.
    fn start_element(&mut self, namespace_uri: Option<&str>, local_name: &str);

    /// One attribute in document order (including `xmlns` when namespace-aware).
    fn attribute(&mut self, namespace_uri: Option<&str>, local_name: &str, value: &str);

    fn characters(&mut self, text: &str);
    fn comment(&mut self, _text: &str) {}

    fn end_element(&mut self, namespace_uri: Option<&str>, local_name: &str);
}
