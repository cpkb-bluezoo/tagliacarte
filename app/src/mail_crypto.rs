/*
 * mail_crypto.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

//! Outgoing and incoming MIME transforms using **in-process** OpenPGP (`pgp` / rPGP) and
//! S/MIME (PKCS#7 via OpenSSL). No external `gpg` or `openssl` executables.

use std::io::{BufReader, Cursor};
use std::path::Path;

use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkcs7::{Pkcs7, Pkcs7Flags};
use openssl::pkey::PKey;
use openssl::stack::Stack;
use openssl::symm::Cipher;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;
use pgp::composed::{
    ArmorOptions, Deserializable, DetachedSignature, Message, MessageBuilder, SignedPublicKey,
    SignedSecretKey,
};
use pgp::crypto::hash::HashAlgorithm;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::types::Password;
use rand::thread_rng;
use rusqlite::Connection;
use tagliacarte_core::mime::emit_message_parts;

use crate::contacts_crypto::{lookup_crypto_paths, normalize_email_addr};
use crate::frb_api::FrbConfig;

/// Optional contacts + recipient list for **encrypt** modes.
pub struct OutgoingCryptoCtx<'a> {
    pub contacts: Option<&'a Connection>,
    pub recipient_emails: &'a [String],
    /// Normalized `From` address (for encrypt-to-self Sent copy).
    pub from_email: Option<&'a str>,
}

impl<'a> Default for OutgoingCryptoCtx<'a> {
    fn default() -> Self {
        Self {
            contacts: None,
            recipient_emails: &[],
            from_email: None,
        }
    }
}

/// Apply signing/encryption to a complete RFC 822 message.
///
/// Returns `(wire_bytes, sent_folder_bytes)` where `sent_folder_bytes` is `Some` when the
/// recipient-bound wire message should differ from what is stored in Sent (encrypt-to-self).
///
/// `crypto_mode` wire values: `none` | `sign` | `encrypt` | `sign_encrypt`, plus legacy
/// `smime_sign`, `pgp_sign`, `smime_sign_encrypt`, `pgp_sign_encrypt` (mapped to the first four).
/// OpenPGP vs S/MIME follows [`FrbConfig::mail_crypto_stack`] (`openpgp` | `smime`) in Security settings.
/// Whether a compose mode asks for signing or encryption. Both need the whole
/// message in hand, so a message without either can be streamed.
pub fn crypto_requested(crypto_mode: Option<&str>) -> bool {
    let mode = crypto_mode.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("none");
    normalize_outgoing_crypto_mode(mode) != "none"
}

pub fn apply_outgoing_mime_crypto(
    raw_rfc822: &[u8],
    crypto_mode: Option<&str>,
    cfg: &FrbConfig,
    ctx: &OutgoingCryptoCtx<'_>,
) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let mode = crypto_mode
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("none");
    let mode = normalize_outgoing_crypto_mode(mode);
    match mode {
        "none" => Ok((raw_rfc822.to_vec(), None)),
        "sign" => {
            let b = crypto_stack_from_config(cfg)?;
            ensure_signing_material(cfg, b)?;
            match b {
                CryptoBackend::Pgp => {
                    pgp_sign_detached_multipart(raw_rfc822, cfg).map(|v| (v, None))
                }
                CryptoBackend::Smime => smime_sign(raw_rfc822, cfg).map(|v| (v, None)),
            }
        }
        "encrypt" => match crypto_stack_from_config(cfg)? {
            CryptoBackend::Pgp => pgp_encrypt_only(raw_rfc822, cfg, ctx),
            CryptoBackend::Smime => smime_encrypt_only(raw_rfc822, cfg, ctx),
        },
        "sign_encrypt" => {
            let b = crypto_stack_from_config(cfg)?;
            ensure_signing_material(cfg, b)?;
            match b {
                CryptoBackend::Pgp => pgp_sign_encrypt(raw_rfc822, cfg, ctx),
                CryptoBackend::Smime => smime_sign_encrypt(raw_rfc822, cfg, ctx),
            }
        }
        _ => Ok((raw_rfc822.to_vec(), None)),
    }
}

fn normalize_outgoing_crypto_mode(mode: &str) -> &str {
    match mode {
        "smime_sign" | "pgp_sign" => "sign",
        "smime_sign_encrypt" | "pgp_sign_encrypt" => "sign_encrypt",
        _ => mode,
    }
}

#[derive(Clone, Copy, Debug)]
enum CryptoBackend {
    Pgp,
    Smime,
}

fn crypto_stack_from_config(cfg: &FrbConfig) -> Result<CryptoBackend, String> {
    let s = cfg.mail_crypto_stack.trim().to_ascii_lowercase();
    if s.is_empty() || s == "openpgp" || s == "pgp" {
        Ok(CryptoBackend::Pgp)
    } else if s == "smime" || s == "s/mime" {
        Ok(CryptoBackend::Smime)
    } else {
        Err(format!(
            "Unknown mail crypto stack {:?} (set to openpgp or smime in Settings → Security)",
            cfg.mail_crypto_stack
        ))
    }
}

fn ensure_signing_material(cfg: &FrbConfig, b: CryptoBackend) -> Result<(), String> {
    match b {
        CryptoBackend::Pgp => {
            if cfg.mail_crypto_pgp_secret_key_path.trim().is_empty() {
                Err(
                    "OpenPGP signing: set path to exported secret key in Settings → Security"
                        .to_string(),
                )
            } else {
                Ok(())
            }
        }
        CryptoBackend::Smime => {
            if cfg.mail_crypto_smime_cert_path.trim().is_empty()
                || cfg.mail_crypto_smime_key_path.trim().is_empty()
            {
                Err(
                    "S/MIME signing: set certificate and private key paths in Settings → Security"
                        .to_string(),
                )
            } else {
                Ok(())
            }
        }
    }
}

fn smime_sign(raw: &[u8], cfg: &FrbConfig) -> Result<Vec<u8>, String> {
    let cert_path = cfg.mail_crypto_smime_cert_path.trim();
    let key_path = cfg.mail_crypto_smime_key_path.trim();
    if cert_path.is_empty() || key_path.is_empty() {
        return Err(
            "S/MIME signing: set S/MIME certificate and private key paths in Settings → Security"
                .to_string(),
        );
    }
    let cert_pem = std::fs::read(Path::new(cert_path)).map_err(|e| format!("S/MIME cert: {e}"))?;
    let key_pem = std::fs::read(Path::new(key_path)).map_err(|e| format!("S/MIME key: {e}"))?;
    let cert = X509::from_pem(&cert_pem).map_err(|e| format!("S/MIME cert PEM: {e}"))?;
    let key = PKey::private_key_from_pem(&key_pem).map_err(|e| format!("S/MIME key PEM: {e}"))?;
    let chain = Stack::new().map_err(|e| e.to_string())?;
    let flags = Pkcs7Flags::DETACHED | Pkcs7Flags::TEXT;
    let pkcs7 = Pkcs7::sign(cert.as_ref(), key.as_ref(), chain.as_ref(), raw, flags)
        .map_err(|e| format!("PKCS#7 sign: {e}"))?;
    pkcs7
        .to_smime(raw, flags)
        .map_err(|e| format!("S/MIME encode: {e}"))
}

fn load_pgp_secret(cfg: &FrbConfig) -> Result<SignedSecretKey, String> {
    let path = cfg.mail_crypto_pgp_secret_key_path.trim();
    if path.is_empty() {
        return Err(
            "OpenPGP: set path to exported secret key in Settings → Security".to_string(),
        );
    }
    let buf = std::fs::read(Path::new(path)).map_err(|e| format!("OpenPGP secret key file: {e}"))?;
    SignedSecretKey::from_armor_single(Cursor::new(&buf))
        .map(|(ssk, _)| ssk)
        .or_else(|_| SignedSecretKey::from_bytes(BufReader::new(Cursor::new(&buf))))
        .map_err(|e| format!("OpenPGP secret key: {e}"))
}

fn pgp_password(cfg: &FrbConfig) -> Password {
    cfg.mail_crypto_pgp_passphrase.trim().into()
}

fn pgp_sign_detached_multipart(raw: &[u8], cfg: &FrbConfig) -> Result<Vec<u8>, String> {
    let ssk = load_pgp_secret(cfg)?;
    let pw = pgp_password(cfg);
    let mut rng = thread_rng();
    let sig = DetachedSignature::sign_binary_data(&mut rng, &*ssk, &pw, HashAlgorithm::Sha256, raw)
        .map_err(|e| format!("OpenPGP sign: {e}"))?;
    let armored = sig
        .to_armored_string(ArmorOptions::default())
        .map_err(|e| format!("OpenPGP armor: {e}"))?;

    let pid = std::process::id();
    let sep = b"\r\n\r\n";
    let hdr_sep = raw
        .windows(sep.len())
        .position(|w| w == sep)
        .ok_or_else(|| "invalid RFC 822 message (missing header/body separator)".to_string())?;
    let header_block = std::str::from_utf8(&raw[..hdr_sep])
        .map_err(|_| "message headers are not valid UTF-8".to_string())?;
    let mut outer_headers = String::new();
    for line in header_block.split("\r\n") {
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-type:")
            || lower.starts_with("content-transfer-encoding:")
            || lower.starts_with("mime-version:")
        {
            continue;
        }
        outer_headers.push_str(line);
        outer_headers.push_str("\r\n");
    }
    outer_headers.push_str("MIME-Version: 1.0\r\n");
    let boundary = format!("tc_pgp_{pid}");
    outer_headers.push_str(&format!(
        "Content-Type: multipart/signed; boundary=\"{boundary}\"; protocol=\"application/pgp-signature\"; micalg=pgp-sha256\r\n"
    ));
    outer_headers.push_str("\r\n");

    let mut out = outer_headers.into_bytes();
    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(b"Content-Type: application/octet-stream; name=\"signed_payload.eml\"\r\n");
    out.extend_from_slice(b"Content-Transfer-Encoding: binary\r\n\r\n");
    out.extend_from_slice(raw);
    if !out.ends_with(b"\r\n") {
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(
        b"Content-Type: application/pgp-signature; name=\"OpenPGP_signature.asc\"\r\n",
    );
    out.extend_from_slice(b"Content-Description: OpenPGP digital signature\r\n\r\n");
    out.extend_from_slice(armored.trim().as_bytes());
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(b"--\r\n");
    Ok(out)
}

fn load_pgp_public_key_file(path: &str) -> Result<SignedPublicKey, String> {
    let buf = std::fs::read(Path::new(path)).map_err(|e| format!("read {path}: {e}"))?;
    let (pk, _) = SignedPublicKey::from_armor_single(Cursor::new(&buf))
        .map_err(|e| format!("OpenPGP public key: {e}"))?;
    Ok(pk)
}

fn pgp_encrypt_literal_to_recipients(
    plaintext: &[u8],
    recipient_paths: &[String],
) -> Result<Vec<u8>, String> {
    if recipient_paths.is_empty() {
        return Err("OpenPGP encrypt: no recipient public keys (contacts)".to_string());
    }
    let mut rng = thread_rng();
    let mut builder = MessageBuilder::from_bytes("message.eml", plaintext.to_vec())
        .seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES256);
    for p in recipient_paths {
        let pk = load_pgp_public_key_file(p.trim())?;
        builder
            .encrypt_to_key(&mut rng, &pk)
            .map_err(|e| format!("OpenPGP encrypt to recipient: {e}"))?;
    }
    builder
        .to_vec(&mut rng)
        .map_err(|e| format!("OpenPGP message finalize: {e}"))
}

fn wrap_pgp_mime_encrypted(ciphertext: &[u8]) -> Vec<u8> {
    let b = format!("tc_pgpenc_{}", std::process::id());
    let mut out = Vec::new();
    out.extend_from_slice(b"MIME-Version: 1.0\r\n");
    out.extend_from_slice(
        format!("Content-Type: multipart/encrypted; boundary=\"{b}\"; protocol=\"application/pgp-encrypted\"\r\n\r\n").as_bytes(),
    );
    out.extend_from_slice(format!("--{b}\r\n").as_bytes());
    out.extend_from_slice(b"Content-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n\r\n");
    out.extend_from_slice(format!("--{b}\r\n").as_bytes());
    out.extend_from_slice(b"Content-Type: application/octet-stream; name=\"encrypted.asc\"\r\n\r\n");
    out.extend_from_slice(ciphertext);
    if !out.ends_with(b"\r\n") {
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{b}--\r\n").as_bytes());
    out
}

fn collect_paths_for_mode(
    ctx: &OutgoingCryptoCtx<'_>,
    mode: CryptoNeed,
) -> Result<Vec<String>, String> {
    let conn = ctx
        .contacts
        .ok_or_else(|| "Mail encryption requires the contacts database".to_string())?;
    let map = lookup_crypto_paths(conn, ctx.recipient_emails)?;
    let mut paths = Vec::new();
    for email in ctx.recipient_emails {
        let key = email.trim().to_lowercase();
        let entry = map.get(&key).ok_or_else(|| {
            format!("No contact with crypto keys for recipient: {email}")
        })?;
        let p = match mode {
            CryptoNeed::Pgp => entry.pgp_key_path.as_ref(),
            CryptoNeed::Smime => entry.smime_cert_path.as_ref(),
        };
        let Some(path) = p.filter(|s| !s.trim().is_empty()) else {
            return Err(format!("Missing {:?} key material for {email}", mode));
        };
        paths.push(path.clone());
    }
    Ok(paths)
}

#[derive(Clone, Copy, Debug)]
enum CryptoNeed {
    Pgp,
    Smime,
}

fn smime_encrypt_bytes(data: &[u8], cert_paths: &[String]) -> Result<Vec<u8>, String> {
    if cert_paths.is_empty() {
        return Err("S/MIME encrypt: no recipient certificates".to_string());
    }
    let mut stack = Stack::new().map_err(|e| e.to_string())?;
    for p in cert_paths {
        let pem = std::fs::read(Path::new(p)).map_err(|e| format!("recipient cert {p}: {e}"))?;
        let cert = X509::from_pem(&pem).map_err(|e| format!("X.509 {p}: {e}"))?;
        stack.push(cert).map_err(|e| e.to_string())?;
    }
    let flags = Pkcs7Flags::empty();
    let pkcs7 = Pkcs7::encrypt(
        stack.as_ref(),
        data,
        Cipher::aes_256_cbc(),
        flags,
    )
    .map_err(|e| format!("PKCS#7 encrypt: {e}"))?;
    pkcs7
        .to_smime(data, flags)
        .map_err(|e| format!("S/MIME encrypt encode: {e}"))
}

fn smime_encrypt_only(
    raw: &[u8],
    cfg: &FrbConfig,
    ctx: &OutgoingCryptoCtx<'_>,
) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let wire_paths = collect_paths_for_mode(ctx, CryptoNeed::Smime)?;
    let wire = smime_encrypt_bytes(raw, &wire_paths)?;
    let sent = build_smime_sent_copy(raw, cfg, ctx)?;
    Ok((wire, Some(sent)))
}

fn pgp_encrypt_only(
    raw: &[u8],
    cfg: &FrbConfig,
    ctx: &OutgoingCryptoCtx<'_>,
) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let paths = collect_paths_for_mode(ctx, CryptoNeed::Pgp)?;
    let ct = pgp_encrypt_literal_to_recipients(raw, &paths)?;
    let wire = wrap_pgp_mime_encrypted(&ct);
    let sent = build_pgp_sent_copy(raw, cfg, ctx)?;
    Ok((wire, Some(sent)))
}

fn smime_sign_encrypt(
    raw: &[u8],
    cfg: &FrbConfig,
    ctx: &OutgoingCryptoCtx<'_>,
) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let signed = smime_sign(raw, cfg)?;
    let wire_paths = collect_paths_for_mode(ctx, CryptoNeed::Smime)?;
    let wire = smime_encrypt_bytes(signed.as_slice(), &wire_paths)?;
    let sent = build_smime_sent_copy(raw, cfg, ctx)?;
    Ok((wire, Some(sent)))
}

fn build_smime_sent_copy(
    inner: &[u8],
    _cfg: &FrbConfig,
    ctx: &OutgoingCryptoCtx<'_>,
) -> Result<Vec<u8>, String> {
    let from = ctx
        .from_email
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "S/MIME encrypt-to-self: missing From address".to_string())?;
    let conn = ctx
        .contacts
        .ok_or_else(|| "contacts required for Sent copy".to_string())?;
    let map = lookup_crypto_paths(conn, &[from.to_lowercase()])?;
    let paths = map
        .get(&from.to_lowercase())
        .and_then(|c| c.smime_cert_path.clone())
        .into_iter()
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err(
            "S/MIME: add your address to contacts with an S/MIME certificate for Sent copy"
                .to_string(),
        );
    }
    smime_encrypt_bytes(inner, &paths)
}

fn pgp_sign_encrypt(
    raw: &[u8],
    cfg: &FrbConfig,
    ctx: &OutgoingCryptoCtx<'_>,
) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let signed = pgp_sign_detached_multipart(raw, cfg)?;
    let paths = collect_paths_for_mode(ctx, CryptoNeed::Pgp)?;
    let ct = pgp_encrypt_literal_to_recipients(signed.as_slice(), &paths)?;
    let wire = wrap_pgp_mime_encrypted(&ct);
    let sent = build_pgp_sent_copy(raw, cfg, ctx)?;
    Ok((wire, Some(sent)))
}

fn build_pgp_sent_copy(
    inner: &[u8],
    _cfg: &FrbConfig,
    ctx: &OutgoingCryptoCtx<'_>,
) -> Result<Vec<u8>, String> {
    let from = ctx
        .from_email
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "OpenPGP encrypt-to-self: missing From address".to_string())?;
    let conn = ctx.contacts.ok_or_else(|| "contacts required".to_string())?;
    let map = lookup_crypto_paths(conn, &[from.to_lowercase()])?;
    let entry = map.get(&from.to_lowercase()).ok_or_else(|| {
        "OpenPGP: add your address to contacts with a PGP public key for Sent copy".to_string()
    })?;
    let p = entry
        .pgp_key_path
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            "OpenPGP: contact row has no pgp_key_path for your address".to_string()
        })?;
    let ct = pgp_encrypt_literal_to_recipients(inner, &[p.clone()])?;
    Ok(wrap_pgp_mime_encrypted(&ct))
}

// ── Inbound: decrypt / unwrap for display ─────────────────────────────

/// If the message is PGP or S/MIME protected, peel one layer so core MIME extraction can run.
pub fn prepare_incoming_rfc822_for_display(raw: &[u8], cfg: &FrbConfig) -> Result<Vec<u8>, String> {
    let mut work = raw.to_vec();
    // PGP multipart/encrypted
    if body_content_type_includes(&work, "multipart/encrypted")
        && body_content_type_includes(&work, "application/pgp-encrypted")
    {
        if let Some(ct) = extract_application_octet_stream_part(&work) {
            work = decrypt_pgp_ciphertext(&ct, cfg)?;
        }
    }
    // S/MIME enveloped (opaque)
    if body_content_type_includes(&work, "application/pkcs7-mime")
        || body_content_type_includes(&work, "application/x-pkcs7-mime")
    {
        if let Some(inner) = decrypt_smime_enveloped(&work, cfg)? {
            work = inner;
        }
    }
    // multipart/signed (OpenPGP): use signed payload part as nested message
    if body_content_type_includes(&work, "multipart/signed")
        && ascii_lower_contains(&work, "protocol=\"application/pgp-signature\"")
    {
        if let Some(inner) = extract_first_multipart_part(&work) {
            work = inner;
        }
    }
    Ok(work)
}

/// The verdict on a message that has arrived (`valid`, `invalid`, `unknown`), checked
/// against the key the address book holds for its sender; `None` when it is not signed,
/// has no sender, or the sender is not in the address book.
pub(crate) fn incoming_signature_status(raw: &[u8], sender_email: &str) -> Option<String> {
    if sender_email.trim().is_empty() {
        return None;
    }
    let dir = tagliacarte_core::config::tagliacarte_data_dir()?;
    let conn = crate::contacts_store::open_contacts_db(&dir).ok()?;
    incoming_signature_verification(raw, &conn, sender_email)
}

/// Returns `None` if there is no PGP or S/MIME signed structure we handle, otherwise
/// `Some("valid" | "invalid" | "unknown")` for UI (contacts-based verification).
pub fn incoming_signature_verification(
    raw_message: &[u8],
    contacts: &Connection,
    sender_email: &str,
) -> Option<String> {
    let key = sender_email.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    let map = lookup_crypto_paths(contacts, &[key.clone()]).ok()?;
    let paths = map.get(&key)?;

    if looks_like_pgp_signed(raw_message) {
        return verify_pgp_multipart_signed(raw_message, paths);
    }
    if looks_like_smime_signed(raw_message) {
        return verify_smime_signed(raw_message, paths);
    }
    None
}

fn looks_like_pgp_signed(raw: &[u8]) -> bool {
    body_content_type_includes(raw, "multipart/signed")
        && ascii_lower_contains(raw, "application/pgp-signature")
}

fn looks_like_smime_signed(raw: &[u8]) -> bool {
    body_content_type_includes(raw, "multipart/signed")
        && (ascii_lower_contains(raw, "pkcs7-signature")
            || ascii_lower_contains(raw, "x-pkcs7-signature"))
}

fn collect_leaf_parts(raw: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let _ = emit_message_parts(raw, |ct, body, _fname| {
        out.push((ct.to_string(), body.to_vec()));
    });
    out
}

/// The value of parameter `name` in the `Content-Type` of a header block (folded lines joined).
fn mime_param(header_block: &[u8], name: &str) -> Option<String> {
    let text = String::from_utf8_lossy(header_block).replace("\r\n", "\n").replace("\n ", " ").replace("\n\t", " ");
    let ct = text.lines().find(|l| l.to_ascii_lowercase().starts_with("content-type:"))?;
    let lower = ct.to_ascii_lowercase();
    let at = lower.find(&format!("{name}="))?;
    let rest = &ct[at + name.len() + 1..];
    let value = match rest.strip_prefix('"') {
        Some(q) => q.split('"').next()?,
        None => rest.split(|c: char| c == ';' || c.is_whitespace()).next()?,
    };
    (!value.is_empty()).then(|| value.to_owned())
}

/// Where the header block of `bytes` ends: the index of the blank line, and its length.
fn header_end(bytes: &[u8]) -> Option<(usize, usize)> {
    bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, 4))
        .or_else(|| bytes.windows(2).position(|w| w == b"\n\n").map(|i| (i, 2)))
}

/// The first part of a top-level multipart message exactly as it stands in the
/// message (its own headers included, not the line break before the next
/// boundary), and where its body begins. RFC 3156 signs this, byte for byte.
fn first_part_of_multipart(raw: &[u8]) -> Option<(&[u8], usize)> {
    let (head, gap) = header_end(raw)?;
    let boundary = mime_param(&raw[..head], "boundary")?;
    let delimiter = format!("--{boundary}");

    // The first two lines that are delimiters.
    let mut starts = Vec::new();
    let mut line = head + gap;
    while line < raw.len() {
        let end = raw[line..].iter().position(|&b| b == b'\n').map_or(raw.len(), |i| line + i + 1);
        if raw[line..end].starts_with(delimiter.as_bytes()) {
            starts.push((line, end));
            if starts.len() == 2 {
                break;
            }
        }
        line = end;
    }
    let [(_, first_end), (second_start, _)] = starts[..] else { return None };
    // The line break before a delimiter belongs to the delimiter.
    let mut part_end = second_start;
    if raw[..part_end].ends_with(b"\r\n") {
        part_end -= 2;
    } else if raw[..part_end].ends_with(b"\n") {
        part_end -= 1;
    }
    let part = raw.get(first_end..part_end)?;
    let body = header_end(part).map_or(0, |(i, g)| i + g);
    Some((part, body))
}

/// `bytes` with every line break a CRLF, which is what a signature is made over.
fn with_crlf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 16);
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' && (i == 0 || bytes[i - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(b);
    }
    out
}

fn verify_pgp_multipart_signed(
    raw: &[u8],
    paths: &crate::contacts_crypto::ContactCryptoPaths,
) -> Option<String> {
    let parts = collect_leaf_parts(raw);
    let sig_bytes = parts
        .iter()
        .find(|(ct, _)| ct.to_ascii_lowercase().contains("application/pgp-signature"))
        .map(|(_, b)| b.clone());
    let (Some(sig), Some((signed_part, body_at))) = (sig_bytes, first_part_of_multipart(raw)) else {
        return Some("unknown".to_string());
    };
    // What may have been signed: the first part as it stands (RFC 3156, which is what
    // other mail programs make), or its body alone, as this app has signed (with or
    // without the closing line break). Each as it is and with CRLF line breaks, since
    // a message kept with bare LF would otherwise never check out.
    let body = &signed_part[body_at..];
    let mut with_break = body.to_vec();
    with_break.extend_from_slice(b"\r\n");
    let mut payloads: Vec<Vec<u8>> = vec![signed_part.to_vec(), body.to_vec(), with_break];
    let normalised: Vec<Vec<u8>> = payloads.iter().map(|p| with_crlf(p)).filter(|n| !payloads.contains(n)).collect();
    payloads.extend(normalised);
    let pgp_path = match paths.pgp_key_path.as_ref().filter(|s| !s.trim().is_empty()) {
        Some(p) => p,
        None => return Some("unknown".to_string()),
    };
    let pk = match load_pgp_public_key_file(pgp_path) {
        Ok(k) => k,
        Err(_) => return Some("unknown".to_string()),
    };
    let ds = match DetachedSignature::from_armor_single(Cursor::new(sig.as_slice())) {
        Ok((d, _)) => d,
        Err(_) => match DetachedSignature::from_bytes(BufReader::new(Cursor::new(sig.as_slice()))) {
            Ok(d) => d,
            Err(_) => return Some("invalid".to_string()),
        },
    };
    if payloads.iter().any(|p| verify_detached_on_public_key(&ds, &pk, p)) {
        Some("valid".to_string())
    } else {
        Some("invalid".to_string())
    }
}

fn verify_detached_on_public_key(ds: &DetachedSignature, pk: &SignedPublicKey, content: &[u8]) -> bool {
    if ds.verify(pk, content).is_ok() {
        return true;
    }
    for sub in &pk.public_subkeys {
        if ds.verify(sub, content).is_ok() {
            return true;
        }
    }
    false
}

fn verify_smime_signed(
    raw: &[u8],
    paths: &crate::contacts_crypto::ContactCryptoPaths,
) -> Option<String> {
    let (pkcs7, indata) = match Pkcs7::from_smime(raw) {
        Ok(x) => x,
        Err(_) => return None,
    };
    if pkcs7
        .type_()
        .map(|t| t.nid())
        .is_none_or(|n| n != Nid::PKCS7_SIGNED)
    {
        return None;
    }
    let empty = match Stack::<X509>::new() {
        Ok(s) => s,
        Err(_) => return Some("unknown".to_string()),
    };
    let signers = match pkcs7.signers(empty.as_ref(), Pkcs7Flags::empty()) {
        Ok(s) => s,
        Err(_) => return Some("invalid".to_string()),
    };
    if signers.is_empty() {
        return Some("invalid".to_string());
    }
    let signer = &signers[0];
    let mut store_builder = match X509StoreBuilder::new() {
        Ok(b) => b,
        Err(_) => return Some("unknown".to_string()),
    };
    let signer_owned = signer.to_owned();
    if store_builder.add_cert(signer_owned).is_err() {
        return Some("unknown".to_string());
    }
    let store = store_builder.build();
    let certs = match Stack::<X509>::new() {
        Ok(s) => s,
        Err(_) => return Some("unknown".to_string()),
    };
    let mut out = Vec::new();
    let crypto_ok = pkcs7
        .verify(
            certs.as_ref(),
            store.as_ref(),
            indata.as_deref(),
            Some(&mut out),
            Pkcs7Flags::empty(),
        )
        .is_ok();
    if !crypto_ok {
        return Some("invalid".to_string());
    }
    let contact_pem = match paths.smime_cert_path.as_ref().filter(|s| !s.trim().is_empty()) {
        Some(p) => p,
        None => return Some("unknown".to_string()),
    };
    let contact_pem_data = match std::fs::read(Path::new(contact_pem)) {
        Ok(b) => b,
        Err(_) => return Some("unknown".to_string()),
    };
    let contact_cert = match X509::from_pem(&contact_pem_data) {
        Ok(c) => c,
        Err(_) => return Some("unknown".to_string()),
    };
    let digest_sig = signer.digest(MessageDigest::sha256()).ok().map(|d| d.to_vec());
    let digest_contact = contact_cert
        .digest(MessageDigest::sha256())
        .ok()
        .map(|d| d.to_vec());
    match (digest_sig, digest_contact) {
        (Some(a), Some(b)) if a == b => Some("valid".to_string()),
        _ => Some("invalid".to_string()),
    }
}

fn ascii_lower_contains(hay: &[u8], needle: &str) -> bool {
    let h = String::from_utf8_lossy(hay);
    h.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
}

fn body_content_type_includes(raw: &[u8], needle: &str) -> bool {
    let Ok(s) = std::str::from_utf8(raw) else {
        return false;
    };
    let lower = s.to_ascii_lowercase();
    lower.contains(&format!("content-type:")) && lower.contains(needle)
}

fn extract_application_octet_stream_part(raw: &[u8]) -> Option<Vec<u8>> {
    let mut found = None;
    let _ = emit_message_parts(raw, |ct, body, _fname| {
        if ct.to_ascii_lowercase().contains("application/octet-stream") {
            found = Some(body.to_vec());
        }
    });
    found
}

fn extract_first_multipart_part(raw: &[u8]) -> Option<Vec<u8>> {
    let mut first = None;
    let _ = emit_message_parts(raw, |ct, body, _fname| {
        if first.is_none() && !ct.to_ascii_lowercase().contains("application/pgp-signature") {
            first = Some(body.to_vec());
        }
    });
    first
}

fn decrypt_pgp_ciphertext(ct: &[u8], cfg: &FrbConfig) -> Result<Vec<u8>, String> {
    let ssk = load_pgp_secret(cfg)?;
    let pw = pgp_password(cfg);
    let msg = Message::from_bytes(ct).map_err(|e| format!("OpenPGP parse: {e}"))?;
    let mut dec = msg
        .decrypt(&pw, &ssk)
        .map_err(|e| format!("OpenPGP decrypt: {e}"))?;
    dec.as_data_vec().map_err(|e| format!("OpenPGP data: {e}"))
}

fn decrypt_smime_enveloped(raw: &[u8], cfg: &FrbConfig) -> Result<Option<Vec<u8>>, String> {
    let cert_path = cfg.mail_crypto_smime_cert_path.trim();
    let key_path = cfg.mail_crypto_smime_key_path.trim();
    if cert_path.is_empty() || key_path.is_empty() {
        return Ok(None);
    }
    let cert_pem = std::fs::read(Path::new(cert_path)).map_err(|e| format!("S/MIME cert: {e}"))?;
    let key_pem = std::fs::read(Path::new(key_path)).map_err(|e| format!("S/MIME key: {e}"))?;
    let cert = X509::from_pem(&cert_pem).map_err(|e| format!("S/MIME cert: {e}"))?;
    let key = PKey::private_key_from_pem(&key_pem).map_err(|e| format!("S/MIME key: {e}"))?;
    let (pkcs7, _maybe) = Pkcs7::from_smime(raw).map_err(|e| format!("S/MIME parse: {e}"))?;
    let out = pkcs7
        .decrypt(key.as_ref(), cert.as_ref(), Pkcs7Flags::empty())
        .map_err(|e| format!("S/MIME decrypt: {e}"))?;
    if out.is_empty() {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}

/// Collect unique normalized recipient emails from compose payload fields.
pub fn recipient_emails_from_addresses(addrs: &[tagliacarte_core::store::Address]) -> Vec<String> {
    let mut v: Vec<String> = addrs.iter().map(|a| normalize_email_addr(a)).collect();
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_passthrough() {
        let cfg = FrbConfig::default();
        let b = b"From: a@b\r\n\r\nHi";
        let (r, s) = apply_outgoing_mime_crypto(b, Some("none"), &cfg, &OutgoingCryptoCtx::default())
            .unwrap();
        assert_eq!(r.as_slice(), b);
        assert!(s.is_none());
    }

    #[test]
    fn sign_mode_requires_security_config() {
        let cfg = FrbConfig::default();
        let b = b"From: a@b\r\n\r\nHi";
        let ctx = OutgoingCryptoCtx::default();
        let err = apply_outgoing_mime_crypto(b, Some("sign"), &cfg, &ctx).unwrap_err();
        assert!(err.contains("Settings"));
    }

    #[test]
    fn legacy_pgp_sign_maps_to_sign() {
        let cfg = FrbConfig::default();
        let b = b"From: a@b\r\n\r\nHi";
        let ctx = OutgoingCryptoCtx::default();
        let err = apply_outgoing_mime_crypto(b, Some("pgp_sign"), &cfg, &ctx).unwrap_err();
        assert!(err.contains("Settings"));
    }

    // ── checking a signature on a message that arrives ──

    use std::path::{Path, PathBuf};

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
    }

    /// An address book with one contact, who has `key` (a file) if given.
    fn contacts_with(dir: &Path, email: &str, key: Option<&Path>) -> Connection {
        let conn = crate::contacts_store::open_contacts_db(dir).unwrap();
        conn.execute(
            "INSERT INTO contacts (id, display_name, pgp_key_path, created_at, updated_at) VALUES (1, 'Someone', ?1, 0, 0)",
            [key.map(|k| k.to_string_lossy().into_owned())],
        )
        .unwrap();
        conn.execute("INSERT INTO contact_emails (contact_id, email) VALUES (1, ?1)", [email]).unwrap();
        conn
    }

    fn signer_key() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/signer_public.asc")
    }

    #[test]
    fn a_standard_pgp_mime_signature_checks_out() {
        let dir = tempfile::tempdir().unwrap();
        let conn = contacts_with(dir.path(), "signer@example.org", Some(&signer_key()));
        // RFC 3156: the signature covers the first part as it stands, headers and all.
        let plain = incoming_signature_verification(&fixture("pgp_signed.eml"), &conn, "signer@example.org");
        assert_eq!(plain.as_deref(), Some("valid"));
        // Including a signed part that holds an attachment.
        let mixed = incoming_signature_verification(&fixture("pgp_signed_with_attachment.eml"), &conn, "signer@example.org");
        assert_eq!(mixed.as_deref(), Some("valid"));
    }

    #[test]
    fn a_changed_message_or_signature_does_not_check_out() {
        let dir = tempfile::tempdir().unwrap();
        let conn = contacts_with(dir.path(), "signer@example.org", Some(&signer_key()));
        let good = fixture("pgp_signed.eml");

        let mut tampered = good.clone();
        let at = tampered.windows(5).position(|w| w == b"Hello").unwrap();
        tampered[at] = b'J';
        assert_eq!(incoming_signature_verification(&tampered, &conn, "signer@example.org").as_deref(), Some("invalid"));

        // A header of the signed part is as much a part of it as the text.
        let mut retyped = good.clone();
        let at = retyped.windows(10).position(|w| w == b"text/plain").unwrap();
        retyped[at..at + 10].copy_from_slice(b"text/PLAIN");
        assert_eq!(incoming_signature_verification(&retyped, &conn, "signer@example.org").as_deref(), Some("invalid"));
    }

    #[test]
    fn without_a_key_for_the_sender_there_is_nothing_to_say_or_it_is_unknown() {
        let good = fixture("pgp_signed.eml");
        let dir = tempfile::tempdir().unwrap();
        // Not in the address book: no verdict at all.
        let nobody = contacts_with(dir.path(), "someone.else@example.org", Some(&signer_key()));
        assert_eq!(incoming_signature_verification(&good, &nobody, "signer@example.org"), None);
        // In it, with no key or a file that is not a key: it cannot be checked.
        let dir2 = tempfile::tempdir().unwrap();
        let no_key = contacts_with(dir2.path(), "signer@example.org", None);
        assert_eq!(incoming_signature_verification(&good, &no_key, "signer@example.org").as_deref(), Some("unknown"));
        let dir3 = tempfile::tempdir().unwrap();
        let junk = dir3.path().join("junk.asc");
        std::fs::write(&junk, "not a key").unwrap();
        let bad_key = contacts_with(dir3.path(), "signer@example.org", Some(&junk));
        assert_eq!(incoming_signature_verification(&good, &bad_key, "signer@example.org").as_deref(), Some("unknown"));
        // An ordinary message has no signature to check.
        let plain = b"From: signer@example.org\r\nSubject: hi\r\n\r\nno signature here\r\n";
        let dir4 = tempfile::tempdir().unwrap();
        let conn = contacts_with(dir4.path(), "signer@example.org", Some(&signer_key()));
        assert_eq!(incoming_signature_verification(plain, &conn, "signer@example.org"), None);
    }

    #[test]
    fn what_this_app_signs_still_checks_out() {
        use pgp::composed::{EncryptionCaps, KeyType, SecretKeyParamsBuilder, SignedPublicKey};
        let dir = tempfile::tempdir().unwrap();
        let mut p = SecretKeyParamsBuilder::default();
        p.key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(true)
            .can_encrypt(EncryptionCaps::None)
            .primary_user_id("Me <me@example.org>".into());
        let key = p.build().unwrap().generate(thread_rng()).unwrap();
        let secret = dir.path().join("secret.asc");
        let public = dir.path().join("public.asc");
        std::fs::write(&secret, key.to_armored_string(ArmorOptions::default()).unwrap()).unwrap();
        std::fs::write(&public, SignedPublicKey::from(key).to_armored_string(ArmorOptions::default()).unwrap()).unwrap();

        let mut cfg = FrbConfig::default();
        cfg.mail_crypto_stack = "openpgp".into();
        cfg.mail_crypto_pgp_secret_key_path = secret.to_string_lossy().into_owned();
        let raw = b"From: me@example.org\r\nTo: you@example.org\r\nSubject: mine\r\n\r\nSigned by this app.\r\n";
        let (signed, _) = apply_outgoing_mime_crypto(raw, Some("sign"), &cfg, &OutgoingCryptoCtx::default()).unwrap();

        let conn = contacts_with(dir.path(), "me@example.org", Some(&public));
        assert_eq!(incoming_signature_verification(&signed, &conn, "me@example.org").as_deref(), Some("valid"));
    }

    /// As `contacts_with`, but the contact's S/MIME certificate.
    fn contacts_with_cert(dir: &Path, email: &str, cert: &str) -> Connection {
        let conn = crate::contacts_store::open_contacts_db(dir).unwrap();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(cert);
        conn.execute(
            "INSERT INTO contacts (id, display_name, smime_cert_path, created_at, updated_at) VALUES (1, 'Someone', ?1, 0, 0)",
            [path.to_string_lossy().into_owned()],
        )
        .unwrap();
        conn.execute("INSERT INTO contact_emails (contact_id, email) VALUES (1, ?1)", [email]).unwrap();
        conn
    }

    #[test]
    fn a_standard_smime_signature_checks_out_only_for_the_contacts_own_certificate() {
        let good = fixture("smime_signed.eml");
        let dir = tempfile::tempdir().unwrap();
        let conn = contacts_with_cert(dir.path(), "signer@example.org", "smime_signer_cert.pem");
        assert_eq!(incoming_signature_verification(&good, &conn, "signer@example.org").as_deref(), Some("valid"));

        // Signed by someone other than the certificate the address book holds for that address.
        let dir2 = tempfile::tempdir().unwrap();
        let other = contacts_with_cert(dir2.path(), "signer@example.org", "smime_other_cert.pem");
        assert_eq!(incoming_signature_verification(&good, &other, "signer@example.org").as_deref(), Some("invalid"));

        // The text changed after it was signed.
        let mut tampered = good.clone();
        let at = tampered.windows(5).position(|w| w == b"Hello").unwrap();
        tampered[at] = b'J';
        assert_eq!(incoming_signature_verification(&tampered, &conn, "signer@example.org").as_deref(), Some("invalid"));
    }
}
