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

//! XML on Tractrix: the push parser and streaming writer this crate's
//! config files use, behind a small content-handler API (element, attribute
//! and whole-text events with names as written) that hides Tractrix's
//! chunked attribute and text delivery.

mod error;
mod handler;
mod parser;
mod writer;

pub use error::XmlError;
pub use handler::XmlContentHandler;
pub use parser::XmlParser;
pub use tractrix::IndentConfig;
pub use writer::XmlWriter;
