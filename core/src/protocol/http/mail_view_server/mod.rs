/*
 * mail_view_server/mod.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Local HTTPS (mTLS) server primitives for streaming mail body to WebView.
 */

mod h1;
mod mtls;

pub use h1::{
    read_http_request, write_chunk, write_chunk_end, write_response_bytes, write_response_head,
    ParsedRequest, MAX_LINE,
};
pub use mtls::{
    mail_body_tls_requires_client_cert, set_mail_body_tls_require_client_cert, MtlsMaterial,
};
