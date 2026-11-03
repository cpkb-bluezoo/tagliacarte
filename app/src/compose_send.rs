/*
 * compose_send.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Sending a composed message without holding it in memory.
 *
 * The functions in `frb_api` take attachments as base64 inside the message,
 * which is how the JSON API passes them. This takes them as files: the
 * message is written to a private temp file as it is built (each attachment
 * read and encoded in bounded pieces) and streamed to the server from there.
 */

use std::path::PathBuf;

use crate::frb_api::{FrbComposeMessage, StreamedAttachment, resolve_transport_in_primary_config, send_smtp_message};

/// An attachment that stays on disk until it is written into the message.
#[derive(Debug, Clone)]
pub struct OutgoingFile {
    pub filename: Option<String>,
    pub mime_type: String,
    pub path: PathBuf,
}

/// Sends `compose` through the SMTP transport `transport_id`, with `files` as
/// its attachments (any in `compose.attachments` are sent too). Signing or
/// encrypting brings the message back into memory for that step only.
pub fn send_smtp_streaming(
    transport_id: &str,
    compose: &FrbComposeMessage,
    files: &[OutgoingFile],
) -> Result<(), String> {
    let (transport, use_keychain) = resolve_transport_in_primary_config(transport_id.trim())?;
    let files: Vec<StreamedAttachment> = files
        .iter()
        .map(|f| StreamedAttachment {
            filename: f.filename.clone(),
            mime_type: f.mime_type.clone(),
            path: f.path.clone(),
        })
        .collect();
    send_smtp_message(&transport, use_keychain, compose, &files)
}
