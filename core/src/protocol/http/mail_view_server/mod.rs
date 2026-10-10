/*
 * mod.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 */

//! Loopback HTTPS server for the mail-body WebView, on hopf-http.

mod mtls;
mod server;
mod sink;

pub use mtls::{
    mail_body_tls_requires_client_cert, set_mail_body_tls_require_client_cert, MtlsMaterial,
};
pub use server::{start_mail_view_server, RequestHandler};
pub use sink::{Backlog, Continue, ParsedRequest, ResponseSink};
