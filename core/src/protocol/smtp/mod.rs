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

//! SMTP client (Transport). Uses persistent connection with idle timeout and reconnect.

mod build_mime;
mod client;
pub mod dot_stuffer;

pub use client::{connect_smtp_async, send_message_async, SmtpConnection, SmtpClientError};

use crate::store::{Attachment, Envelope, SendPayload, SendSession, StoreError, Transport, TransportKind};
use crate::sasl::SaslMechanism;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300;

/// Request for the send worker: payload and oneshot to reply when done.
type SendRequest = (SendPayload, tokio::sync::oneshot::Sender<Result<(), StoreError>>);

/// SMTP transport (submission). Holds a persistent client: connection reuse, idle timeout, reconnect on error/timeout.
/// Supports implicit TLS (465), STARTTLS (587), and optional auth.
/// Optional send channel for non-blocking streaming send; start_send_worker must be called (e.g. after wrapping in Arc).
pub struct SmtpTransport {
    host: String,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: RwLock<Option<(String, String, SaslMechanism)>>,
    ehlo_hostname: String,
    idle_timeout_secs: u64,
    /// Handle to the shared tokio runtime (set by FFI layer at creation).
    runtime_handle: tokio::runtime::Handle,
    connection_state: Arc<Mutex<(Option<client::SmtpConnection>, Instant)>>,
    /// Channel to send worker; cloned for each streaming session.
    send_tx: mpsc::Sender<SendRequest>,
    /// Taken by start_send_worker to run the worker thread.
    send_rx: Mutex<Option<mpsc::Receiver<SendRequest>>>,
}

impl SmtpTransport {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self::with_runtime_handle(host, port, tokio::runtime::Handle::current())
    }

    /// Create an SmtpTransport with an explicit tokio runtime handle (used by FFI with the shared runtime).
    pub fn with_runtime_handle(host: impl Into<String>, port: u16, handle: tokio::runtime::Handle) -> Self {
        let host = host.into();
        let use_implicit_tls = port == 465;
        let (send_tx, send_rx) = mpsc::channel();
        Self {
            host: host.clone(),
            port,
            use_implicit_tls,
            use_starttls: true,
            auth: RwLock::new(None),
            ehlo_hostname: "localhost".to_string(),
            idle_timeout_secs: DEFAULT_IDLE_TIMEOUT_SECS,
            runtime_handle: handle,
            connection_state: Arc::new(Mutex::new((None, Instant::now()))),
            send_tx,
            send_rx: Mutex::new(Some(send_rx)),
        }
    }

    /// Start the background worker for non-blocking send. Call once after wrapping the transport in `Arc` (e.g. in FFI).
    /// The worker receives send requests and runs send_blocking; completion is reported via the oneshot.
    /// Uses spawn_blocking on the shared runtime instead of spawning a dedicated thread.
    pub fn start_send_worker(self: Arc<Self>) {
        let rx = match self.send_rx.lock() {
            Ok(mut g) => g.take(),
            Err(_) => return,
        };
        let Some(rx) = rx else { return };
        let transport = self;
        let handle = transport.runtime_handle.clone();
        handle.spawn_blocking(move || {
            for (payload, reply) in rx {
                let r = transport.send_blocking(&payload);
                let _ = reply.send(r);
            }
        });
    }

    /// Use implicit TLS (e.g. 465). Default is true when port is 465.
    pub fn set_implicit_tls(&mut self, use_tls: bool) -> &mut Self {
        self.use_implicit_tls = use_tls;
        self
    }

    /// Use STARTTLS when the server advertises it on a plain connection. Default true; set false only for debugging.
    pub fn set_use_starttls(&mut self, use_starttls: bool) -> &mut Self {
        self.use_starttls = use_starttls;
        self
    }

    /// Set auth (username, password, mechanism). Mechanism should be PLAIN or SCRAM-SHA-256 for TLS.
    pub fn set_auth(&mut self, username: impl Into<String>, password: impl Into<String>, mechanism: SaslMechanism) -> &mut Self {
        *self.auth.write().unwrap() = Some((username.into(), password.into(), mechanism));
        self
    }

    /// Set OAuth2 access token for XOAUTH2 authentication (Gmail, Outlook).
    /// `email` is the user's email address; `access_token` is the OAuth2 bearer token.
    pub fn set_oauth_token(&mut self, email: impl Into<String>, access_token: impl Into<String>) -> &mut Self {
        *self.auth.write().unwrap() = Some((email.into(), access_token.into(), SaslMechanism::XOAuth2));
        self
    }

    /// Set EHLO hostname (default "localhost").
    pub fn set_ehlo_hostname(&mut self, name: impl Into<String>) -> &mut Self {
        self.ehlo_hostname = name.into();
        self
    }

    /// Set idle timeout in seconds; connection is dropped and re-established after this period of inactivity. Default 300.
    pub fn set_idle_timeout_secs(&mut self, secs: u64) -> &mut Self {
        self.idle_timeout_secs = secs;
        self
    }

    fn send_blocking(&self, payload: &SendPayload) -> Result<(), StoreError> {
        let (message, envelope) = build_mime::build_rfc822_from_payload(payload);
        let host = self.host.clone();
        let port = self.port;
        let use_implicit_tls = self.use_implicit_tls;
        let use_starttls = self.use_starttls;
        let auth = self.auth.read().unwrap().as_ref().map(|(u, p, m)| (u.clone(), p.clone(), *m));
        let ehlo_hostname = self.ehlo_hostname.clone();
        let state = Arc::clone(&self.connection_state);
        let idle_timeout = Duration::from_secs(self.idle_timeout_secs);

        self.runtime_handle.block_on(async move {
            let mut guard = state.lock().map_err(|e| StoreError::new(e.to_string()))?;
            let now = Instant::now();
            let expired = guard.0.as_ref().map_or(true, |_| guard.1.elapsed() > idle_timeout);
            if expired {
                guard.0 = None;
            }
            if guard.0.is_none() {
                let auth_ref = auth.as_ref().map(|(u, p, m)| (u.as_str(), p.as_str(), *m));
                let conn = connect_smtp_async(
                    &host,
                    port,
                    use_implicit_tls,
                    use_starttls,
                    auth_ref,
                    &ehlo_hostname,
                )
                .await
                .map_err(|e| StoreError::new(e.to_string()))?;
                guard.0 = Some(conn);
            }
            guard.1 = now;
            let conn = guard.0.as_mut().unwrap();
            conn.send_one(&envelope, &message)
                .await
                .map_err(|e| StoreError::new(e.to_string()))
        })
    }
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
        on_complete(self.send_blocking(payload));
    }

    fn start_send(&self) -> Result<Box<dyn SendSession>, StoreError> {
        Ok(Box::new(SmtpSendSession {
            send_tx: self.send_tx.clone(),
            envelope: None,
            subject: None,
            body_plain: Vec::new(),
            body_html: Vec::new(),
            attachments: Vec::new(),
            current_attachment: None,
        }))
    }

    fn set_oauth_credential(&self, email: &str, token: &str) {
        *self.auth.write().unwrap() = Some((
            email.to_string(),
            token.to_string(),
            SaslMechanism::XOAuth2,
        ));
        // Drop stale connection so next send reconnects with the new token.
        if let Ok(mut guard) = self.connection_state.lock() {
            guard.0 = None;
        }
    }
}

/// Buffered streaming send session for SMTP; builds a SendPayload and submits to the transport's send worker.
struct SmtpSendSession {
    send_tx: mpsc::Sender<SendRequest>,
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
    fn send_metadata(&mut self, envelope: &Envelope, subject: Option<&str>) -> Result<(), StoreError> {
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

    fn start_attachment(&mut self, filename: Option<&str>, mime_type: &str) -> Result<(), StoreError> {
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

    fn end_send(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<(), StoreError>> + Send>> {
        let mut session = *self;
        session.flush_current_attachment();
        let envelope = match session.envelope {
            Some(e) => e,
            None => {
                return Box::pin(std::future::ready(Err(StoreError::new(
                    "send_metadata was not called",
                ))));
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
            subject: session.subject,
            body_plain,
            body_html,
            attachments: session.attachments,
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        if session.send_tx.send((payload, tx)).is_err() {
            return Box::pin(std::future::ready(Err(StoreError::new(
                "send worker not running",
            ))));
        }
        Box::pin(async move {
            match rx.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(StoreError::new("send worker dropped")),
            }
        })
    }
}
