/*
 * request.rs
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

//! HTTP request: method, path, headers, optional body.
//!
//! Built via RequestBuilder; sending is done by the connection (send with handler).

use std::collections::HashMap;

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Patch,
    Other(&'static str),
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
            Method::Patch => "PATCH",
            Method::Other(s) => s,
        }
    }
}

/// Mutable request builder: method, path, headers, body.
///
/// Obtain from `HttpConnection::request(method, path)` or `HttpClient::get(path)` etc.
/// Add headers, optionally set body, then call `send(handler)` to execute.
pub struct RequestBuilder {
    pub method: Method,
    pub path: String,
    pub headers: HashMap<String, String>,
    /// If set, body will be sent (chunked or with Content-Length if set).
    pub body: Option<Vec<u8>>,
}

impl RequestBuilder {
    pub fn new(method: Method, path: String) -> Self {
        Self {
            method,
            path,
            headers: HashMap::new(),
            body: None,
        }
    }

    /// Add or replace a header. Name is stored as given; comparison is case-insensitive per HTTP.
    pub fn header(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Set request body. If Content-Length is not set, chunked encoding will be used when sent.
    pub fn body(&mut self, data: Vec<u8>) -> &mut Self {
        self.body = Some(data);
        self
    }

    /// Set body from a slice (copied).
    pub fn body_slice(&mut self, data: &[u8]) -> &mut Self {
        self.body = Some(data.to_vec());
        self
    }
}
