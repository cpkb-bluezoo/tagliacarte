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

//! HTTP client on hopf-http (HTTP/1.1 and HTTP/2, negotiated by ALPN) with
//! push-parsed responses.
//!
//! - Callback-based response API (Gumdrop-shaped): `ResponseHandler` with `ok`/`error`, `header`, `start_body`, `body_chunk`, `end_body`, `complete`, `failed`.
//! - `HttpClient::connect` dials on the process-wide hopf runtime (`crate::net::hopf_runtime`) and hands the connection to a continuation; `HttpConnection::send` returns at once and every outcome reaches the handler.
//! - HTTP/2 multiplexing for batches of GETs: `HttpConnection::send_http2_parallel_gets`.
//! - Multipart: HTTP layer only delivers raw body; consumer feeds `MimeParser` when needed.

pub(crate) mod api_trace;
mod handler;
mod request;
mod response;

pub mod mail_view_server;

pub use handler::ResponseHandler;
pub use request::{ApiOutboundTrace, MatrixOutboundTrace, Method, RequestBuilder};
pub use response::Response;

pub mod client;
pub mod connection;
pub mod fetch;

pub use client::{HttpClient, OnConnect};
pub use connection::{GetResult, HttpConnection, HttpVersion, OnBatch};
pub use fetch::{fetch, fetch_blocking, send_collect, send_collect_blocking, Collected, HttpUrl, OnCollected};
