/*
 * client_session.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 */

//! Protocol-agnostic SASL client: decoded challenge bytes in, raw client SASL payload bytes out.
//! Wire framing (IMAP `+` continuations, SMTP `334`, base64 lines) stays in each protocol client.

use super::plain::initial_response_plain;
use super::scram::{client_final_with_server_first_str, client_first, ScramSha256State};
use super::xoauth2::xoauth2_initial_response;
use super::{cram_md5_response_decoded, SaslError, SaslMechanism};

/// Optional key–value material to persist after a successful exchange.
pub type SaslCredentialArtifacts = Vec<(String, String)>;

/// Outcome of feeding a server challenge.
#[derive(Debug)]
pub enum SaslClientOutput {
    /// Send this payload; the protocol encodes it (e.g. base64) as required by the wire format.
    Message(Vec<u8>),
    /// Client must not send another line; read the protocol's success reply (e.g. IMAP tagged OK).
    AwaitingSuccess { store: SaslCredentialArtifacts },
}

enum LoginStep {
    AwaitingUsernamePrompt,
    AwaitingPasswordPrompt,
    Finished,
}

enum ScramStep {
    AwaitingServerFirst { state: ScramSha256State },
    AwaitingServerFinal,
    Done,
}

enum SessionInner {
    Plain {
        initial: Vec<u8>,
        finished: bool,
    },
    XOAuth2 {
        initial: Vec<u8>,
        finished: bool,
    },
    Login {
        step: LoginStep,
        authcid: String,
        password: String,
    },
    CramMd5 {
        responded: bool,
        authcid: String,
        password: String,
    },
    Scram {
        step: ScramStep,
        initial: Vec<u8>,
        password: String,
    },
}

/// Stateful SASL client for one mechanism; no IMAP/SMTP/POP3/NNTP framing.
pub struct SaslClientSession {
    mech: SaslMechanism,
    inner: SessionInner,
}

impl SaslClientSession {
    pub fn new(
        mechanism: SaslMechanism,
        authzid: &str,
        authcid: &str,
        password: &str,
    ) -> Result<Self, SaslError> {
        let inner = match mechanism {
            SaslMechanism::Plain => SessionInner::Plain {
                initial: initial_response_plain(authzid, authcid, password)?,
                finished: false,
            },
            SaslMechanism::XOAuth2 => SessionInner::XOAuth2 {
                initial: xoauth2_initial_response(authcid, password),
                finished: false,
            },
            SaslMechanism::Login => SessionInner::Login {
                step: LoginStep::AwaitingUsernamePrompt,
                authcid: authcid.to_string(),
                password: password.to_string(),
            },
            SaslMechanism::CramMd5 => SessionInner::CramMd5 {
                responded: false,
                authcid: authcid.to_string(),
                password: password.to_string(),
            },
            SaslMechanism::ScramSha256 => {
                let (initial, state) = client_first(authcid);
                SessionInner::Scram {
                    step: ScramStep::AwaitingServerFirst { state },
                    initial,
                    password: password.to_string(),
                }
            }
        };
        Ok(Self {
            mech: mechanism,
            inner,
        })
    }

    pub fn mechanism(&self) -> SaslMechanism {
        self.mech
    }

    /// First SASL message body (raw octets). Empty if the client sends nothing after the mechanism name.
    pub fn initial_message(&self) -> Vec<u8> {
        match &self.inner {
            SessionInner::Plain { initial, .. } => initial.clone(),
            SessionInner::XOAuth2 { initial, .. } => initial.clone(),
            SessionInner::Login { .. } | SessionInner::CramMd5 { .. } => Vec::new(),
            SessionInner::Scram { initial, .. } => initial.clone(),
        }
    }

    /// Decoded server challenge body (octets inside the wire base64 / literal), not framing tokens.
    pub fn consume_server_challenge(
        &mut self,
        challenge: &[u8],
    ) -> Result<SaslClientOutput, SaslError> {
        match &mut self.inner {
            SessionInner::Plain { .. } | SessionInner::XOAuth2 { .. } => Err(SaslError::invalid(
                "unexpected server challenge for PLAIN/XOAUTH2",
            )),
            SessionInner::Login {
                step,
                authcid,
                password,
            } => match step {
                LoginStep::AwaitingUsernamePrompt => {
                    let s = String::from_utf8_lossy(challenge).to_lowercase();
                    if s.trim().is_empty()
                        || s.contains("username")
                        || s.trim() == "username:"
                    {
                        *step = LoginStep::AwaitingPasswordPrompt;
                        Ok(SaslClientOutput::Message(authcid.as_bytes().to_vec()))
                    } else {
                        Err(SaslError::invalid("unexpected LOGIN challenge (expected username)"))
                    }
                }
                LoginStep::AwaitingPasswordPrompt => {
                    let s = String::from_utf8_lossy(challenge).to_lowercase();
                    if s.trim().is_empty()
                        || s.contains("password")
                        || s.trim() == "password:"
                    {
                        *step = LoginStep::Finished;
                        Ok(SaslClientOutput::Message(password.as_bytes().to_vec()))
                    } else {
                        Err(SaslError::invalid("unexpected LOGIN challenge (expected password)"))
                    }
                }
                LoginStep::Finished => Err(SaslError::invalid("LOGIN session already finished")),
            },
            SessionInner::CramMd5 {
                responded,
                authcid,
                password,
            } => {
                if *responded {
                    return Err(SaslError::invalid("CRAM-MD5: duplicate challenge"));
                }
                *responded = true;
                let challenge_str = String::from_utf8(challenge.to_vec())
                    .map_err(|_| SaslError::invalid("CRAM-MD5 challenge not UTF-8"))?;
                let msg = cram_md5_response_decoded(authcid, password, challenge_str.as_str())?;
                Ok(SaslClientOutput::Message(msg))
            }
            SessionInner::Scram {
                step,
                password: pw,
                ..
            } => match std::mem::replace(step, ScramStep::AwaitingServerFinal) {
                ScramStep::AwaitingServerFirst { state } => {
                    let server_first = String::from_utf8(challenge.to_vec())
                        .map_err(|_| SaslError::invalid("SCRAM server-first not UTF-8"))?;
                    let out = client_final_with_server_first_str(
                        &state,
                        server_first.as_str(),
                        pw.as_str(),
                    )?;
                    *step = ScramStep::AwaitingServerFinal;
                    Ok(SaslClientOutput::Message(out))
                }
                ScramStep::AwaitingServerFinal => {
                    *step = ScramStep::Done;
                    let _server_final = String::from_utf8_lossy(challenge);
                    Ok(SaslClientOutput::AwaitingSuccess { store: vec![] })
                }
                ScramStep::Done => {
                    *step = ScramStep::Done;
                    Err(SaslError::invalid("SCRAM: duplicate challenge"))
                }
            },
        }
    }

    /// Call when the protocol reports authentication success with no further SASL payload from the client.
    pub fn consume_server_success(&mut self) -> Result<SaslCredentialArtifacts, SaslError> {
        match &mut self.inner {
            SessionInner::Plain { finished, .. } => {
                if *finished {
                    return Err(SaslError::invalid("PLAIN: duplicate success"));
                }
                *finished = true;
                Ok(vec![])
            }
            SessionInner::XOAuth2 { finished, .. } => {
                if *finished {
                    return Err(SaslError::invalid("XOAUTH2: duplicate success"));
                }
                *finished = true;
                Ok(vec![])
            }
            SessionInner::Login { step, .. } => {
                if !matches!(step, LoginStep::Finished) {
                    return Err(SaslError::invalid(
                        "LOGIN: success before password step completed",
                    ));
                }
                Ok(vec![])
            }
            SessionInner::CramMd5 { responded, .. } => {
                if !*responded {
                    return Err(SaslError::invalid("CRAM-MD5: success before challenge"));
                }
                Ok(vec![])
            }
            SessionInner::Scram { step, .. } => match step {
                ScramStep::AwaitingServerFinal => {
                    *step = ScramStep::Done;
                    Ok(vec![])
                }
                ScramStep::Done => Ok(vec![]),
                ScramStep::AwaitingServerFirst { .. } => Err(SaslError::invalid(
                    "SCRAM: success before server-first",
                )),
            },
        }
    }
}
