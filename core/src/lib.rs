/*
 * lib.rs
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

//! Tagliacarte core: Store/Folder/Message/Transport abstraction, protocols, local storage, MIME.
//!
//! Protocol debug logging: set **`TAGLIACARTE_TRACE`** to a comma- or space-separated list of
//! providers (`imap`, `nostr`, `mail_body`, `gmail`, `graph`, `http`, …) or `all`. See [`trace`] module.

pub mod config;
pub mod config_xml;
pub mod json;
pub mod localstorage;
pub mod message_id;
pub mod mime;
pub mod net;
pub mod oauth;
pub mod protocol;
mod rustls_init;
pub mod sasl;
pub mod store;
pub mod tagliacarte_config_xml;
pub mod trace;
pub mod uri;
pub mod xml;
