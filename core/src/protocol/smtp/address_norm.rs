/*
 * address_norm.rs
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

//! Normalize [`crate::store::Address`] values that were split naively from `phrase <addr-spec>`.

use crate::mime::format_mailbox;
use crate::store::Address;

/// Addr-spec inside the outermost `… <addr-spec>` or `<addr-spec>` (RFC 5322).
pub(crate) fn addrspec_from_angle_brackets(s: &str) -> Option<String> {
    let lt = s.find('<')?;
    let gt = s.rfind('>')?;
    if gt <= lt {
        return None;
    }
    let inner = s[lt + 1..gt].trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// Addr-spec only for SMTP paths (MAIL FROM, RCPT TO) and Message-ID rhs.
pub(crate) fn store_address_addrspec(addr: &Address) -> String {
    let lp = addr.local_part.trim();
    let dom = addr.domain.as_deref().map(str::trim).filter(|d| !d.is_empty());

    let concatenated = match dom {
        Some(d) => format!("{lp}@{d}"),
        None => lp.to_string(),
    };
    if let Some(spec) = addrspec_from_angle_brackets(&concatenated) {
        return spec;
    }

    let domain_str = dom.unwrap_or("");
    let formatted = format_mailbox(
        addr.display_name.as_deref().filter(|n| !n.trim().is_empty()),
        lp,
        domain_str,
    );
    addrspec_from_angle_brackets(&formatted).unwrap_or_else(|| {
        concatenated
            .trim_matches(|c| c == '<' || c == '>')
            .trim()
            .to_string()
    })
}

/// RFC 5322 mailbox for From/To/Cc headers when `Address` may be misparsed.
pub(crate) fn store_address_header_mailbox(addr: &Address) -> String {
    let lp = addr.local_part.trim();
    let dom = addr.domain.as_deref().map(str::trim).filter(|d| !d.is_empty());
    let concatenated = match dom {
        Some(d) => format!("{lp}@{d}"),
        None => lp.to_string(),
    };

    let spec = store_address_addrspec(addr);

    let display = addr
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            if let Some(lt) = concatenated.find('<') {
                let phrase = concatenated[..lt].trim();
                if !phrase.is_empty() {
                    return Some(phrase.to_string());
                }
            }
            None
        });

    match display {
        Some(dn) => format!("{} <{}>", dn, spec),
        None => format!("<{}>", spec),
    }
}

/// Domain part after `@` in the normalized addr-spec, for Message-ID generation.
pub(crate) fn store_address_domain_for_mid(addr: &Address) -> Option<String> {
    let spec = store_address_addrspec(addr);
    let at = spec.rfind('@')?;
    let tail = spec[at + 1..].trim();
    let tail = tail.trim_matches(|c| c == '<' || c == '>').trim();
    if tail.is_empty() {
        None
    } else {
        Some(tail.to_string())
    }
}
