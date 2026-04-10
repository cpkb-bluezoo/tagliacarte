/*
 * auth_pick.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 */

//! Choose SASL mechanisms in **server advertisement order** (IMAP CAPABILITY, SMTP EHLO,
//! POP3 CAPA `SASL`, NNTP `SASL` capability line).

use super::SaslMechanism;

#[inline]
fn mechanism_uses_password_material(m: SaslMechanism) -> bool {
    !matches!(m, SaslMechanism::XOAuth2)
}

/// Uppercased tokens from NNTP-style capability lines (each line may list several words).
pub fn flatten_capability_lines_to_tokens(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .flat_map(|l| l.split_whitespace().map(|w| w.to_uppercase()))
        .collect()
}

/// Mechanisms listed after a `SASL` token (RFC 2449 POP3 CAPA, RFC 3977 NNTP when flattened).
pub fn sasl_mechanisms_after_sasl_caps_line(caps: &[String]) -> Vec<SaslMechanism> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < caps.len() {
        if caps[i] == "SASL" {
            i += 1;
            while i < caps.len() {
                if let Some(m) = SaslMechanism::from_name(&caps[i]) {
                    out.push(m);
                    i += 1;
                } else {
                    break;
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

/// First advertised mechanism the client can satisfy (password vs OAuth), preserving server order.
pub fn pick_first_sasl_for_credentials_in_server_order(
    mechanisms: &[SaslMechanism],
    have_oauth_token: bool,
    have_password: bool,
) -> Option<SaslMechanism> {
    for &m in mechanisms {
        let ok = if m == SaslMechanism::XOAuth2 {
            have_oauth_token
        } else {
            have_password && mechanism_uses_password_material(m)
        };
        if ok {
            return Some(m);
        }
    }
    None
}

/// IMAP `CAPABILITY` uses `AUTH=MECH` tokens. Iterates [caps] in order.
pub fn pick_first_imap_auth_for_credentials(
    caps: &[String],
    have_oauth_token: bool,
    have_password: bool,
) -> Option<SaslMechanism> {
    for cap in caps {
        let Some(rest) = cap.strip_prefix("AUTH=") else {
            continue;
        };
        let Some(m) = SaslMechanism::from_name(rest) else {
            continue;
        };
        let ok = if m == SaslMechanism::XOAuth2 {
            have_oauth_token
        } else {
            have_password && mechanism_uses_password_material(m)
        };
        if ok {
            return Some(m);
        }
    }
    None
}

/// SMTP EHLO lists mechanism names after `AUTH` (no `AUTH=` prefix). Iterates in server order.
pub fn pick_first_smtp_auth_for_credentials(
    auth_methods: &[String],
    have_oauth_token: bool,
    have_password: bool,
) -> Option<SaslMechanism> {
    for token in auth_methods {
        let Some(m) = SaslMechanism::from_name(token.as_str()) else {
            continue;
        };
        let ok = if m == SaslMechanism::XOAuth2 {
            have_oauth_token
        } else {
            have_password && mechanism_uses_password_material(m)
        };
        if ok {
            return Some(m);
        }
    }
    None
}

/// IMAP `CAPABILITY` uses `AUTH=PLAIN` style tokens.
pub fn imap_auth_advertised(caps: &[String], mechanism: SaslMechanism) -> bool {
    caps.iter()
        .any(|c| c == &format!("AUTH={}", mechanism.name()))
}

/// SMTP EHLO `AUTH` line lists mechanism names without an `AUTH=` prefix.
pub fn smtp_auth_advertised(methods: &[String], mechanism: SaslMechanism) -> bool {
    methods.iter().any(|m| m == mechanism.name())
}
