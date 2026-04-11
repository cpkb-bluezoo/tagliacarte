/*
 * contacts_crypto.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

//! Resolve PGP/S/MIME material paths from the contacts DB (by email).

use rusqlite::{params, OptionalExtension};
use rusqlite::Connection;
use std::collections::HashMap;
use tagliacarte_core::store::Address;

/// Normalized `local@domain` for contact lookup.
pub fn normalize_email_addr(a: &Address) -> String {
    let dom = a.domain.as_deref().unwrap_or("").trim();
    if dom.is_empty() {
        a.local_part.trim().to_lowercase()
    } else {
        format!("{}@{}", a.local_part.trim(), dom).to_lowercase()
    }
}

/// Public key / cert file paths from [`contacts_store`] for one address.
#[derive(Debug, Clone, Default)]
pub struct ContactCryptoPaths {
    pub pgp_key_path: Option<String>,
    pub smime_cert_path: Option<String>,
}

/// Look up crypto paths for each email (missing contacts are omitted from the map).
pub fn lookup_crypto_paths(
    conn: &Connection,
    emails: &[String],
) -> Result<HashMap<String, ContactCryptoPaths>, String> {
    let mut out = HashMap::new();
    for email in emails {
        let e = email.trim().to_lowercase();
        if e.is_empty() {
            continue;
        }
        let row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                r#"SELECT c.pgp_key_path, c.smime_cert_path
                   FROM contact_emails ce
                   JOIN contacts c ON ce.contact_id = c.id
                   WHERE LOWER(TRIM(ce.email)) = ?1"#,
                params![e],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((pgp, smime)) = row {
            out.insert(
                e,
                ContactCryptoPaths {
                    pgp_key_path: pgp.filter(|s| !s.trim().is_empty()),
                    smime_cert_path: smime.filter(|s| !s.trim().is_empty()),
                },
            );
        }
    }
    Ok(out)
}
