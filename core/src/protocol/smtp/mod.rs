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

//! SMTP transport on hopf. One hopf session per message (`SmtpSend`), reported
//! through callbacks; nothing here blocks the caller.
//!
//! Connection reuse across sends (the pre-hopf transport kept one session
//! open for 300 s) is not carried over yet: a persistent submission session
//! needs a driver that parks between messages and is woken by a poke, which
//! hopf-smtp does not ship. One connection per message is what most
//! submission servers see from mail clients anyway.

mod address_norm;
mod build_mime;
mod hopf_client;

pub use build_mime::{
    build_rfc822_from_payload, generate_smtp_message_id_angle_from_from_field,
    normalize_smtp_message_id_angle,
};
pub use hopf_client::{
    probe, probe_smtp_ehlo_auth_methods_blocking, probe_smtp_tcp_connect_blocking,
    send_message_blocking, submit, verify, verify_smtp_blocking, SmtpClientError, SmtpTarget,
    VerifyResult,
};

use crate::sasl::SaslMechanism;
use crate::store::{
    Attachment, Envelope, SendPayload, SendSession, StoreError, Transport, TransportKind,
};
use std::sync::RwLock;

/// SMTP transport (submission): implicit TLS (465), STARTTLS (587), optional
/// auth. Sends run on hopf's reactor; `send` and `end_send` return at once.
pub struct SmtpTransport {
    host: String,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: RwLock<Option<(String, String, SaslMechanism)>>,
    ehlo_hostname: String,
}

impl SmtpTransport {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 465;
        Self {
            host,
            port,
            use_implicit_tls,
            use_starttls: true,
            auth: RwLock::new(None),
            ehlo_hostname: String::new(),
        }
    }

    /// Use implicit TLS (e.g. 465). Default is true when port is 465.
    pub fn set_implicit_tls(&mut self, use_tls: bool) -> &mut Self {
        self.use_implicit_tls = use_tls;
        self
    }

    /// Require STARTTLS on a plain connection. Default true; set false only for debugging.
    pub fn set_use_starttls(&mut self, use_starttls: bool) -> &mut Self {
        self.use_starttls = use_starttls;
        self
    }

    /// Set auth (username, password, mechanism).
    pub fn set_auth(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
        mechanism: SaslMechanism,
    ) -> &mut Self {
        *self.auth.write().unwrap() = Some((username.into(), password.into(), mechanism));
        self
    }

    /// Set OAuth2 access token for XOAUTH2 authentication (Gmail, Outlook).
    pub fn set_oauth_token(
        &mut self,
        email: impl Into<String>,
        access_token: impl Into<String>,
    ) -> &mut Self {
        *self.auth.write().unwrap() =
            Some((email.into(), access_token.into(), SaslMechanism::XOAuth2));
        self
    }

    /// Set EHLO/HELO argument. When empty, the client's local IP is sent in RFC 5321 address-literal form.
    pub fn set_ehlo_hostname(&mut self, name: impl Into<String>) -> &mut Self {
        self.ehlo_hostname = name.into();
        self
    }

    fn target(&self) -> SmtpTarget {
        let auth = self.auth.read().unwrap().clone();
        SmtpTarget {
            host: self.host.clone(),
            port: self.port,
            implicit_tls: self.use_implicit_tls,
            starttls: self.use_starttls,
            auth,
            ehlo_hostname: self.ehlo_hostname.trim().to_string(),
        }
    }
}

/// Build the wire message from a payload and hand it to hopf.
fn submit_payload(
    target: &SmtpTarget,
    payload: &SendPayload,
    on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
) {
    let (message, mut envelope) = build_mime::build_rfc822_from_payload(payload);
    // Bcc recipients get RCPT TO but no header; `submit` uses to + cc.
    envelope.cc.extend(payload.bcc.iter().cloned());
    submit(
        target,
        message,
        &envelope,
        payload.smtp_notify.as_deref(),
        Box::new(move |r| on_complete(r.map_err(|e| StoreError::new(e.to_string())))),
    );
}

impl Transport for SmtpTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Email
    }

    fn send(
        &self,
        payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        submit_payload(&self.target(), payload, on_complete);
    }

    fn start_send(&self) -> Result<Box<dyn SendSession>, StoreError> {
        Ok(Box::new(SmtpSendSession {
            target: self.target(),
            envelope: None,
            subject: None,
            body_plain: Vec::new(),
            body_html: Vec::new(),
            attachments: Vec::new(),
            current_attachment: None,
        }))
    }

    fn set_oauth_credential(&self, email: &str, token: &str) {
        *self.auth.write().unwrap() =
            Some((email.to_string(), token.to_string(), SaslMechanism::XOAuth2));
    }
}

/// Buffered streaming send session for SMTP; builds a SendPayload and submits it on `end_send`.
struct SmtpSendSession {
    target: SmtpTarget,
    envelope: Option<Envelope>,
    subject: Option<String>,
    body_plain: Vec<u8>,
    body_html: Vec<u8>,
    attachments: Vec<Attachment>,
    current_attachment: Option<(Option<String>, String, Vec<u8>)>,
}

impl SmtpSendSession {
    fn flush_current_attachment(&mut self) {
        if let Some((filename, mime_type, content)) = self.current_attachment.take() {
            self.attachments.push(Attachment {
                filename,
                mime_type,
                content,
            });
        }
    }
}

impl SendSession for SmtpSendSession {
    fn send_metadata(
        &mut self,
        envelope: &Envelope,
        subject: Option<&str>,
    ) -> Result<(), StoreError> {
        self.envelope = Some(envelope.clone());
        self.subject = subject.map(|s| s.to_string());
        Ok(())
    }

    fn send_body_plain_chunk(&mut self, data: &[u8]) -> Result<(), StoreError> {
        self.body_plain.extend_from_slice(data);
        Ok(())
    }

    fn send_body_html_chunk(&mut self, data: &[u8]) -> Result<(), StoreError> {
        self.body_html.extend_from_slice(data);
        Ok(())
    }

    fn start_attachment(
        &mut self,
        filename: Option<&str>,
        mime_type: &str,
    ) -> Result<(), StoreError> {
        self.flush_current_attachment();
        self.current_attachment = Some((
            filename.map(|s| s.to_string()),
            mime_type.to_string(),
            Vec::new(),
        ));
        Ok(())
    }

    fn send_attachment_chunk(&mut self, data: &[u8]) -> Result<(), StoreError> {
        if let Some((_, _, ref mut content)) = self.current_attachment {
            content.extend_from_slice(data);
        }
        Ok(())
    }

    fn end_attachment(&mut self) -> Result<(), StoreError> {
        self.flush_current_attachment();
        Ok(())
    }

    fn end_send(self: Box<Self>, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let mut session = *self;
        session.flush_current_attachment();
        let envelope = match session.envelope {
            Some(e) => e,
            None => {
                on_complete(Err(StoreError::new("send_metadata was not called")));
                return;
            }
        };
        let body_plain = if session.body_plain.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&session.body_plain).into_owned())
        };
        let body_html = if session.body_html.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&session.body_html).into_owned())
        };
        let payload = SendPayload {
            from: envelope.from,
            to: envelope.to,
            cc: envelope.cc,
            bcc: Vec::new(),
            subject: session.subject,
            body_plain,
            body_html,
            attachments: session.attachments,
            newsgroups: Vec::new(),
            nntp_in_reply_to: None,
            nntp_references: None,
            smtp_notify: None,
            smtp_in_reply_to: None,
            smtp_references: None,
            smtp_message_id: None,
        };
        submit_payload(&session.target, &payload, on_complete);
    }
}
