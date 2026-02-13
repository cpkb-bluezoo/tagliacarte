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

//! HTTP client: generic HTTP/1.1 and HTTP/2 client with push-parsed responses.
//!
//! Design (see doc/HTTP_CLIENT_AND_MATRIX_PLAN.md):
//! - Callback-based response API (Gumdrop-shaped): `ResponseHandler` with `ok`/`error`, `header`, `start_body`, `body_chunk`, `end_body`, `complete`, `failed`.
//! - Buffers: `bytes` crate (BytesMut for parse buffer, Bytes for payload slices).
//! - HTTP/1.1: state-machine response parser. HTTP/2: our own frame parser + HPACK (no external h2 crate).
//! - TLS with ALPN `h2`, `http/1.1`. Plaintext: h2c upgrade and optional prior knowledge.
//! - Multipart: HTTP layer only delivers raw body; consumer feeds `MimeParser` when needed.

mod handler;
mod request;
mod response;

pub mod h1;
pub mod h2;
pub mod hpack;

pub use handler::ResponseHandler;
pub use h1::H1ResponseHandler;
pub use request::{Method, RequestBuilder};
pub use response::Response;

pub mod client;
pub mod connection;

pub use client::HttpClient;
pub use connection::{HttpConnection, HttpStream, HttpVersion};
