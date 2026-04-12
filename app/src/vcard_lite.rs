/*
 * vcard_lite.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
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

//! Minimal vCard 3.0/4.0 parsing for FN, EMAIL, KEY, optional CERT.

#[derive(Debug, Clone, Default)]
pub struct ParsedVCard {
    pub fn_: String,
    pub emails: Vec<String>,
    /// Raw OpenPGP / key material lines (may be base64 folded)
    pub key_raw: Option<String>,
    pub cert_raw: Option<String>,
}

fn unfold_vcard_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = lines.last_mut() {
                last.push_str(&line[1..]);
            }
        } else {
            lines.push(line.to_string());
        }
    }
    lines
}

fn strip_type_params(s: &str) -> &str {
    // EMAIL;TYPE=INTERNET:user@x -> after first colon at property level already handled
    s.trim()
}

/// Split one logical line into property name (uppercase) and value (after first unescaped colon at property level — simplified: last colon for EMAIL;TYPE=work:value).
fn split_property(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (head, rest) = line.split_once(':')?;
    let name = head.split(';').next()?;
    Some((name, rest))
}

pub fn parse_vcards_utf8(bytes: &[u8]) -> Vec<ParsedVCard> {
    let text = String::from_utf8_lossy(bytes);
    parse_vcards_str(&text)
}

pub fn parse_vcards_str(text: &str) -> Vec<ParsedVCard> {
    let lines = unfold_vcard_lines(text);
    let mut out = Vec::new();
    let mut cur: Option<ParsedVCard> = None;
    for line in lines {
        let u = line.to_ascii_uppercase();
        if u.starts_with("BEGIN:VCARD") {
            cur = Some(ParsedVCard::default());
            continue;
        }
        if u.starts_with("END:VCARD") {
            if let Some(c) = cur.take() {
                out.push(c);
            }
            continue;
        }
        let Some(c) = cur.as_mut() else {
            continue;
        };
        let Some((prop, val)) = split_property(&line) else {
            continue;
        };
        let prop = prop.to_ascii_uppercase();
        let val = strip_type_params(val);
        match prop.as_str() {
            "FN" => c.fn_ = val.to_string(),
            "EMAIL" => {
                let e = val.trim();
                if !e.is_empty() && !c.emails.iter().any(|x| x == e) {
                    c.emails.push(e.to_string());
                }
            }
            "KEY" => {
                c.key_raw = Some(val.to_string());
            }
            "CERT" => {
                c.cert_raw = Some(val.to_string());
            }
            _ => {}
        }
    }
    out
}

pub fn build_vcard(contact: &str, emails: &[(&str, &str)], notes: &str) -> String {
    let mut b = String::new();
    b.push_str("BEGIN:VCARD\r\n");
    b.push_str("VERSION:3.0\r\n");
    b.push_str("FN:");
    b.push_str(contact);
    b.push_str("\r\n");
    for (addr, label) in emails {
        if !label.is_empty() {
            b.push_str(&format!("EMAIL;TYPE={label}:{addr}\r\n"));
        } else {
            b.push_str(&format!("EMAIL:{addr}\r\n"));
        }
    }
    if !notes.is_empty() {
        b.push_str("NOTE:");
        b.push_str(&notes.replace('\n', "\\n"));
        b.push_str("\r\n");
    }
    b.push_str("END:VCARD\r\n");
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fn_email() {
        let s = "BEGIN:VCARD\nVERSION:3.0\nFN:Test User\nEMAIL;TYPE=INTERNET:test@example.com\nEND:VCARD\n";
        let v = parse_vcards_str(s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].fn_, "Test User");
        assert_eq!(v[0].emails, vec!["test@example.com"]);
    }
}
