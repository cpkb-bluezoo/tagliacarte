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

//! HTTP response handler trait (Gumdrop-shaped callbacks).
//!
//! Events: status → headers → start_body → body_chunk (×n) → end_body → trailer (×n) → complete / failed.

use crate::protocol::http::response::Response;

/// Handler for HTTP response events (push model). Connection drives this as data arrives.
///
/// Flow for a response with body:
/// 1. `ok(response)` or `error(response)` — status received
/// 2. `header(name, value)` — for each response header
/// 3. `start_body()` — body begins
/// 4. `body_chunk(data)` — for each chunk of body data
/// 5. `end_body()` — body complete
/// 6. `header(name, value)` — for each trailer (if any)
/// 7. `complete()` — response fully complete
///
/// On connection/protocol failure only `failed(error)` is called.
pub trait ResponseHandler {
    /// Called when a successful (2xx) status is received.
    fn ok(&mut self, response: Response);

    /// Called when an error status (4xx, 5xx) or client-detected error is received.
    fn error(&mut self, response: Response);

    /// Called for each response or trailer header. Name may repeat for multi-value headers.
    fn header(&mut self, name: &str, value: &str);

    /// Called when the response body is about to start. Not called for 204/304 etc.
    fn start_body(&mut self);

    /// Called for each chunk of body data. Data is only valid for the duration of the call.
    fn body_chunk(&mut self, data: &[u8]);

    /// Called when the response body is complete. Trailers may follow.
    fn end_body(&mut self);

    /// Called when the response is fully complete (after all headers and body).
    fn complete(&mut self);

    /// Called when the request fails (connection error, protocol error, cancellation).
    fn failed(&mut self, error: &std::io::Error);
}
