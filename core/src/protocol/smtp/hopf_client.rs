/*
 * hopf_client.rs
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

//! SMTP submission on hopf (`hopf-smtp`): one session per operation, driven by
//! hopf's `SmtpSend` (deliver one message) and `SmtpVerify` (prove settings
//! and credentials, or probe capabilities) auto-pilots.
//!
//! Everything here is callback-first: [`submit`], [`verify`] and [`probe`]
//! return as soon as the dial is started and report through a `FnOnce`
//! that hopf invokes on its reactor thread. The `*_async` functions wrap
//! those for synchronous callers that block on a channel.
//!
//! TLS policy, as before: port 465 or `use_implicit_tls` means TLS from the
//! first byte; otherwise `use_starttls` makes STARTTLS mandatory (a server
//! that does not offer it is an error, and credentials never go over
//! plaintext); `use_starttls == false` is the debugging-only plaintext path.

use std::io;
use std::sync::{Arc, Mutex};

use hopf_smtp::{
    DsnNotify, DsnRecipientParams, SmtpClient, SmtpClientTimeouts, SmtpSend, SmtpSendOutcome,
    SmtpVerify, SmtpVerifyOutcome,
};

use super::address_norm::store_address_addrspec;
use crate::net::{hopf_runtime, hopf_tls_connector};
use crate::sasl::SaslMechanism;
use crate::store::Envelope;

/// SMTP client error (network, protocol, auth).
#[derive(Debug, Clone)]
pub struct SmtpClientError {
    pub message: String,
}

impl SmtpClientError {
    pub(crate) fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

impl std::fmt::Display for SmtpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SmtpClientError {}

impl From<io::Error> for SmtpClientError {
    fn from(e: io::Error) -> Self {
        Self::new(e.to_string())
    }
}

/// Where and how to reach the submission server.
#[derive(Debug, Clone)]
pub struct SmtpTarget {
    pub host: String,
    pub port: u16,
    /// TLS from the first byte (port 465).
    pub implicit_tls: bool,
    /// On a plain connection, STARTTLS is mandatory. Ignored when
    /// `implicit_tls` is set.
    pub starttls: bool,
    /// Username, secret, mechanism family. `XOAuth2` means the secret is an
    /// OAuth bearer token; anything else is a password and hopf picks the
    /// strongest mechanism the server offers.
    pub auth: Option<(String, String, SaslMechanism)>,
    /// EHLO argument; empty means our own address literal.
    pub ehlo_hostname: String,
}

impl SmtpTarget {
    pub fn new(host: &str, port: u16, implicit_tls: bool, starttls: bool) -> Self {
        Self {
            host: host.to_string(),
            port,
            implicit_tls: implicit_tls || port == 465,
            starttls,
            auth: None,
            ehlo_hostname: String::new(),
        }
    }

    pub fn with_auth(mut self, auth: Option<(&str, &str, SaslMechanism)>) -> Self {
        self.auth = auth.map(|(u, p, m)| (u.to_string(), p.to_string(), m));
        self
    }

    pub fn with_ehlo_hostname(mut self, name: Option<&str>) -> Self {
        self.ehlo_hostname = name.map(str::trim).unwrap_or("").to_string();
        self
    }

    fn client(&self) -> SmtpClient {
        let client = SmtpClient::new(self.host.as_str(), self.port)
            .timeouts(SmtpClientTimeouts::default())
            .resolver(crate::net::hopf_dns_resolver());
        if self.implicit_tls {
            client.implicit_tls(hopf_tls_connector(), self.host.as_str())
        } else if self.starttls {
            client.starttls(hopf_tls_connector(), self.host.as_str())
        } else {
            client
        }
    }

    fn require_starttls(&self) -> bool {
        !self.implicit_tls && self.starttls
    }

    fn uses_bearer(&self) -> bool {
        matches!(self.auth, Some((_, _, SaslMechanism::XOAuth2)))
    }
}

/// RFC 3461 `NOTIFY=` for every recipient, from the comma-separated form the
/// compose UI produces (`FAILURE`, `FAILURE,SUCCESS`, `NEVER`, …). Sent only
/// to servers that advertise DSN; hopf drops it otherwise.
fn dsn_params(notify: Option<&str>) -> DsnRecipientParams {
    let mut n = DsnNotify::default();
    for tok in notify.unwrap_or("").split(',') {
        match tok.trim().to_ascii_uppercase().as_str() {
            "NEVER" => n.never = true,
            "SUCCESS" => n.success = true,
            "FAILURE" => n.failure = true,
            "DELAY" => n.delay = true,
            _ => {}
        }
    }
    DsnRecipientParams { notify: n, ..Default::default() }
}

/// A completion callback that both the hopf pipeline and a refused dial can
/// reach, whichever reports first; it fires at most once.
type Reporter<T> = Arc<Mutex<Option<Box<dyn FnOnce(T) + Send>>>>;

fn report<T>(reporter: &Reporter<T>, value: T) {
    let cb = reporter.lock().unwrap().take();
    if let Some(cb) = cb {
        cb(value);
    }
}

/// Submit one already-built RFC 5322 message. `RCPT TO` is every address in
/// `envelope.to` and `envelope.cc` (callers fold Bcc into `cc`). Returns once
/// the dial is started; `on_result` fires exactly once, on hopf's reactor
/// (or on this thread if the dial is refused outright).
pub fn submit(
    target: &SmtpTarget,
    message: Vec<u8>,
    envelope: &Envelope,
    smtp_notify: Option<&str>,
    on_result: Box<dyn FnOnce(Result<(), SmtpClientError>) + Send>,
) {
    crate::trace_log!(
        "smtp",
        "submit host={} port={} implicit_tls={} starttls={} auth={} ehlo={:?} message_bytes={}",
        target.host,
        target.port,
        target.implicit_tls,
        target.starttls,
        target.auth.is_some(),
        target.ehlo_hostname,
        message.len()
    );
    let sender = envelope.from.first().map(store_address_addrspec).unwrap_or_default();
    let recipients: Vec<(String, DsnRecipientParams)> = envelope
        .to
        .iter()
        .chain(envelope.cc.iter())
        .map(|a| (store_address_addrspec(a), dsn_params(smtp_notify)))
        .collect();
    if recipients.is_empty() {
        on_result(Err(SmtpClientError::new("no recipients")));
        return;
    }
    let reporter: Reporter<Result<(), SmtpClientError>> = Arc::new(Mutex::new(Some(on_result)));
    let from_pipeline = Arc::clone(&reporter);
    let mut body = Some(message);
    let mut send = SmtpSend::new(target.ehlo_hostname.as_str())
        .mail_from(sender)
        .recipients_with(recipients)
        .message_with(move || body.take())
        .require_starttls(target.require_starttls())
        .on_result(Box::new(move |outcome| {
            crate::trace_log!("smtp", "submit outcome: {outcome:?}");
            report(
                &from_pipeline,
                match outcome {
                    SmtpSendOutcome::Delivered => Ok(()),
                    SmtpSendOutcome::Rejected { code, message } => {
                        Err(SmtpClientError::new(format!("{code} {message}")))
                    }
                    SmtpSendOutcome::Failed(m) => Err(SmtpClientError::new(m)),
                },
            )
        }));
    if let Some((user, secret, _)) = &target.auth {
        send = if target.uses_bearer() {
            send.auth_bearer(user.as_str(), secret.as_str())
        } else {
            send.auth_plain(user.as_str(), secret.as_str())
        };
    }
    if let Err(e) = target.client().connect(&hopf_runtime(), Arc::new(send)) {
        // hopf refused to even start the dial (bad input); DNS and dial
        // failures that happen later arrive through `connect_failed`, which
        // `SmtpSend` routes to the same `on_result`.
        crate::trace_log!("smtp", "submit: dial refused: {e}");
        report(&reporter, Err(e.into()));
    }
}

/// What a [`verify`] or [`probe`] learned.
pub type VerifyResult = Result<hopf_smtp::SmtpCapabilities, SmtpClientError>;

fn run_verify(target: &SmtpTarget, with_auth: bool, on_result: Box<dyn FnOnce(VerifyResult) + Send>) {
    let reporter: Reporter<VerifyResult> = Arc::new(Mutex::new(Some(on_result)));
    let from_pipeline = Arc::clone(&reporter);
    let mut verify = SmtpVerify::new(target.ehlo_hostname.as_str())
        .require_starttls(target.require_starttls())
        .on_result(Box::new(move |outcome| {
            report(
                &from_pipeline,
                match outcome {
                    SmtpVerifyOutcome::Verified(caps) => Ok(caps),
                    SmtpVerifyOutcome::AuthFailed { code } => {
                        Err(SmtpClientError::new(format!("{code} authentication failed")))
                    }
                    SmtpVerifyOutcome::Failed(m) => Err(SmtpClientError::new(m)),
                },
            )
        }));
    if with_auth {
        if let Some((user, secret, _)) = &target.auth {
            verify = if target.uses_bearer() {
                verify.credentials_bearer(user.as_str(), secret.as_str())
            } else {
                verify.credentials_password(user.as_str(), secret.as_str())
            };
        }
    }
    if let Err(e) = target.client().connect(&hopf_runtime(), Arc::new(verify)) {
        crate::trace_log!("smtp", "verify: dial refused: {e}");
        report(&reporter, Err(e.into()));
    }
}

/// Connect, EHLO, STARTTLS per policy, authenticate with `target.auth`, QUIT.
/// Proves the settings without sending mail.
pub fn verify(target: &SmtpTarget, on_result: Box<dyn FnOnce(VerifyResult) + Send>) {
    crate::trace_log!("smtp", "verify host={} port={}", target.host, target.port);
    run_verify(target, true, on_result);
}

/// Connect, EHLO, STARTTLS per policy, QUIT: the server's capabilities as an
/// encrypted client sees them, credentials untouched.
pub fn probe(target: &SmtpTarget, on_result: Box<dyn FnOnce(VerifyResult) + Send>) {
    crate::trace_log!("smtp", "probe host={} port={}", target.host, target.port);
    run_verify(target, false, on_result);
}

// ── blocking wrappers for synchronous callers (FRB entry points, tests) ──

/// Start an operation that reports through a callback and wait for it on
/// this thread. The session's own timers bound the wait: when the handler
/// is dropped without reporting, the channel closes and this returns an
/// error rather than hanging.
fn wait_for<T: Send + 'static>(start: impl FnOnce(Box<dyn FnOnce(T) + Send>)) -> Result<T, SmtpClientError> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<T>(1);
    start(Box::new(move |v| {
        let _ = tx.send(v);
    }));
    rx.recv().map_err(|_| SmtpClientError::new("SMTP session ended without a result"))
}

/// Run an SMTP session to completion and wait for it: connect (plain or
/// implicit TLS), EHLO, optional STARTTLS+AUTH, send message, QUIT.
// The signature predates the hopf port and is what the app crate calls;
// `submit` + `SmtpTarget` is the structured, non-blocking form.
#[allow(clippy::too_many_arguments)]
pub fn send_message_blocking(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname_override: Option<&str>,
    message: &[u8],
    envelope: &Envelope,
    smtp_notify: Option<&str>,
) -> Result<(), SmtpClientError> {
    let target = SmtpTarget::new(host, port, use_implicit_tls, use_starttls)
        .with_auth(auth)
        .with_ehlo_hostname(ehlo_hostname_override);
    wait_for(|cb| submit(&target, message.to_vec(), envelope, smtp_notify, cb))?
}

/// Connect, authenticate if credentials are provided, then QUIT — verify
/// settings without sending mail. Blocks the calling thread.
pub fn verify_smtp_blocking(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    auth: Option<(&str, &str, SaslMechanism)>,
    ehlo_hostname_override: Option<&str>,
) -> Result<(), SmtpClientError> {
    let target = SmtpTarget::new(host, port, use_implicit_tls, use_starttls)
        .with_auth(auth)
        .with_ehlo_hostname(ehlo_hostname_override);
    wait_for(|cb| verify(&target, cb))?.map(|_| ())
}

/// Connect (with the same TLS rules as [send_message_blocking]), run EHLO on
/// the encrypted/plain session, and return the `AUTH` mechanism tokens from
/// the final EHLO (post-STARTTLS when used). Blocks the calling thread.
pub fn probe_smtp_ehlo_auth_methods_blocking(
    host: &str,
    port: u16,
    use_implicit_tls: bool,
    use_starttls: bool,
    ehlo_hostname_override: Option<&str>,
) -> Result<Vec<String>, SmtpClientError> {
    let target =
        SmtpTarget::new(host, port, use_implicit_tls, use_starttls).with_ehlo_hostname(ehlo_hostname_override);
    wait_for(|cb| probe(&target, cb))?.map(|caps| caps.auth_methods)
}

/// Reachability check: connect (TLS handshake included when implicit), read
/// the greeting, EHLO, QUIT. Use before prompting for SMTP credentials so DNS
/// and network errors surface first. Blocks the calling thread.
pub fn probe_smtp_tcp_connect_blocking(host: &str, port: u16, use_implicit_tls: bool) -> Result<(), SmtpClientError> {
    let target = SmtpTarget::new(host, port, use_implicit_tls, false);
    wait_for(|cb| probe(&target, cb))?.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_tokens_map_onto_dsn_flags() {
        let p = dsn_params(Some("FAILURE,SUCCESS"));
        assert!(p.notify.failure && p.notify.success && !p.notify.delay && !p.notify.never);
        assert!(dsn_params(Some(" never ")).notify.never);
        assert!(!dsn_params(None).notify.is_explicit());
        assert!(!dsn_params(Some("")).notify.is_explicit());
    }

    #[test]
    fn target_tls_policy() {
        let t = SmtpTarget::new("smtp.example", 465, false, true);
        assert!(t.implicit_tls && !t.require_starttls());
        let t = SmtpTarget::new("smtp.example", 587, false, true);
        assert!(!t.implicit_tls && t.require_starttls());
        let t = SmtpTarget::new("smtp.example", 25, false, false);
        assert!(!t.require_starttls());
        let t = t.with_auth(Some(("u", "tok", SaslMechanism::XOAuth2)));
        assert!(t.uses_bearer());
    }
}
