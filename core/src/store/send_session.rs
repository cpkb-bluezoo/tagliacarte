/*
 * send_session.rs
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
 * along with Tagliacarte. If not, see <http://www.gnu.org/licenses/>.
 */

//! Streaming send session: non-blocking send with metadata, body chunks, and attachment chunks.
//! UI drives: send_metadata → body chunks → (start_attachment → attachment chunks → end_attachment)* → end_send.
//! Completion is reported asynchronously (Future or callback); send never blocks the UI.

use crate::store::error::StoreError;
use crate::store::message::Envelope;
use std::future::Future;
use std::pin::Pin;

/// Streaming send session. Created by `Transport::start_send()`.
/// Order: send_metadata (once) → send_body_plain_chunk / send_body_html_chunk → for each attachment:
/// start_attachment → send_attachment_chunk (zero or more) → end_attachment → end_send.
pub trait SendSession: Send + Sync {
    /// Set envelope (from, to, cc) and subject. Must be called first, once.
    fn send_metadata(&mut self, envelope: &Envelope, subject: Option<&str>) -> Result<(), StoreError>;

    /// Append a chunk of plain-text body. Call any number of times; order with html is preserved (plain first, then html if both).
    fn send_body_plain_chunk(&mut self, data: &[u8]) -> Result<(), StoreError>;

    /// Append a chunk of HTML body. Call any number of times.
    fn send_body_html_chunk(&mut self, data: &[u8]) -> Result<(), StoreError>;

    /// Start an attachment (filename, MIME type). Next chunks go to this attachment until end_attachment.
    fn start_attachment(&mut self, filename: Option<&str>, mime_type: &str) -> Result<(), StoreError>;

    /// Append a chunk of the current attachment's content.
    fn send_attachment_chunk(&mut self, data: &[u8]) -> Result<(), StoreError>;

    /// End the current attachment.
    fn end_attachment(&mut self) -> Result<(), StoreError>;

    /// Finish the message and send. Returns a Future that completes when the send has finished (success or error).
    /// Callbacks (e.g. from FFI) should be invoked when this Future completes; the UI must not block on it.
    fn end_send(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<(), StoreError>> + Send>>;
}
