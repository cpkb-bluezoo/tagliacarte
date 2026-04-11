/*
 * mod.rs
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

//! Push UTF-8 XML parser and streaming serializer (subset; no DTD).

mod error;
mod handler;
mod href_collector;
mod parser;
mod writer;

pub use error::XmlError;
pub use handler::XmlContentHandler;
pub use href_collector::{collect_href_texts, collect_href_texts_from_reader};
pub use parser::{XmlParser, XMLNS_NAMESPACE_URI, XML_NAMESPACE_URI};
pub use writer::XmlWriter;
