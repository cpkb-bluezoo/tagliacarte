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
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! vCard 3.0/4.0 parsing and emission: FN, NICKNAME, EMAIL, TEL, URL, ADR, PHOTO, NOTE, KEY, CERT.

use base64::Engine;

#[derive(Debug, Clone, Default)]
pub struct ParsedAdr {
    pub label: String,
    pub po_box: String,
    pub extended: String,
    pub street: String,
    pub locality: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
}

#[derive(Debug, Clone)]
pub struct ParsedPhoto {
    pub mime_type: String,
    pub data: Option<Vec<u8>>,
    pub source_uri: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedVCard {
    pub fn_: String,
    pub nickname: String,
    pub organization: String,
    pub title: String,
    /// ISO date string when present (BDAY)
    pub birthday: String,
    /// (normalized email, TYPE label e.g. `INTERNET,work`)
    pub emails: Vec<(String, String)>,
    pub tels: Vec<(String, String)>,
    pub urls: Vec<(String, String)>,
    pub adrs: Vec<ParsedAdr>,
    pub photos: Vec<ParsedPhoto>,
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

/// Property name (before first `;`) from the full property line head.
fn prop_base_name(head: &str) -> &str {
    head.split(';').next().unwrap_or(head).trim()
}

/// Collect `TYPE=a,b` parameters from the property head as a single comma-separated label.
fn type_label_from_head(head: &str) -> String {
    let mut labels: Vec<String> = Vec::new();
    for part in head.split(';').skip(1) {
        let p = part.trim();
        let Some(rest) = p
            .strip_prefix("TYPE=")
            .or_else(|| p.strip_prefix("type="))
        else {
            continue;
        };
        for piece in rest.split(',') {
            let t = piece.trim();
            if !t.is_empty() && !labels.iter().any(|x: &String| x == t) {
                labels.push(t.to_string());
            }
        }
    }
    labels.join(",")
}

/// `TYPE=JPEG` / `TYPE=PNG` → `image/jpeg` / `image/png`
fn mime_from_head_type(head_upper: &str) -> String {
    if head_upper.contains("TYPE=PNG") {
        return "image/png".to_string();
    }
    if head_upper.contains("TYPE=GIF") {
        return "image/gif".to_string();
    }
    if head_upper.contains("TYPE=JPEG") || head_upper.contains("TYPE=JPG") {
        return "image/jpeg".to_string();
    }
    String::new()
}

fn split_once_colon(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    line.split_once(':')
}

fn parse_adr_value(val: &str) -> ParsedAdr {
    let parts: Vec<&str> = val.split(';').collect();
    let g = |i: usize| parts.get(i).map(|s| s.trim()).unwrap_or("").to_string();
    ParsedAdr {
        label: String::new(),
        po_box: g(0),
        extended: g(1),
        street: g(2),
        locality: g(3),
        region: g(4),
        postal_code: g(5),
        country: g(6),
    }
}

fn normalize_tel(val: &str) -> String {
    let v = val.trim();
    let v = v.strip_prefix("tel:").unwrap_or(v);
    v.strip_prefix("TEL:").unwrap_or(v).trim().to_string()
}

fn parse_photo(head: &str, val: &str) -> Option<ParsedPhoto> {
    let head_u = head.to_ascii_uppercase();
    let v = val.trim();
    if head_u.contains("VALUE=URI")
        || v.to_ascii_uppercase().starts_with("HTTP://")
        || v.to_ascii_uppercase().starts_with("HTTPS://")
    {
        return Some(ParsedPhoto {
            mime_type: String::new(),
            data: None,
            source_uri: Some(v.to_string()),
        });
    }
    let b64: String = v.chars().filter(|c| !c.is_whitespace()).collect();
    if b64.is_empty() {
        return None;
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .ok()?;
    let mime = mime_from_head_type(&head_u);
    Some(ParsedPhoto {
        mime_type: mime,
        data: Some(data),
        source_uri: None,
    })
}

fn escape_vcard_component(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

fn append_email_line(b: &mut String, addr: &str, label: &str) {
    let addr = addr.trim();
    if addr.is_empty() {
        return;
    }
    let label = label.trim();
    if label.is_empty() {
        b.push_str(&format!("EMAIL:{addr}\r\n"));
        return;
    }
    b.push_str("EMAIL");
    for t in label.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
        b.push_str(";TYPE=");
        b.push_str(&escape_vcard_component(t));
    }
    b.push(':');
    b.push_str(addr);
    b.push_str("\r\n");
}

fn append_typed_line(b: &mut String, prop: &str, value: &str, label: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let esc_val = escape_vcard_component(value);
    let label = label.trim();
    if label.is_empty() {
        b.push_str(prop);
        b.push(':');
        b.push_str(&esc_val);
        b.push_str("\r\n");
        return;
    }
    b.push_str(prop);
    for t in label.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
        b.push_str(";TYPE=");
        b.push_str(&escape_vcard_component(t));
    }
    b.push(':');
    b.push_str(&esc_val);
    b.push_str("\r\n");
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
        let Some((head, val)) = split_once_colon(&line) else {
            continue;
        };
        let base = prop_base_name(head).to_ascii_uppercase();
        let val = val.trim();
        let type_l = type_label_from_head(head);
        match base.as_str() {
            "FN" => c.fn_ = val.to_string(),
            "NICKNAME" => c.nickname = val.to_string(),
            "ORG" => c.organization = val.to_string(),
            "TITLE" => c.title = val.to_string(),
            "BDAY" => c.birthday = val.to_string(),
            "EMAIL" => {
                let e = normalize_email_addr(val);
                if !e.is_empty()
                    && !c
                        .emails
                        .iter()
                        .any(|(addr, _)| addr.as_str() == e.as_str())
                {
                    c.emails.push((e, type_l));
                }
            }
            "TEL" => {
                let n = normalize_tel(val);
                if !n.is_empty() {
                    c.tels.push((n, type_l));
                }
            }
            "URL" => {
                if !val.is_empty() {
                    c.urls.push((val.to_string(), type_l));
                }
            }
            "ADR" => {
                let mut a = parse_adr_value(val);
                a.label = type_l;
                c.adrs.push(a);
            }
            "PHOTO" => {
                if let Some(p) = parse_photo(head, val) {
                    c.photos.push(p);
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

fn normalize_email_addr(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Emit VERSION:3.0 vCard from structured parts (CardDAV PUT / export).
#[allow(clippy::too_many_arguments)]
pub fn build_vcard_full(
    display_name: &str,
    nickname: &str,
    organization: &str,
    title: &str,
    birthday: Option<&str>,
    notes: &str,
    emails: &[(&str, &str)],
    tels: &[(&str, &str)],
    urls: &[(&str, &str)],
    adrs: &[ParsedAdr],
    photos: &[ParsedPhoto],
) -> String {
    let mut b = String::new();
    b.push_str("BEGIN:VCARD\r\n");
    b.push_str("VERSION:3.0\r\n");
    b.push_str("FN:");
    b.push_str(&escape_vcard_component(display_name));
    b.push_str("\r\n");
    if !nickname.trim().is_empty() {
        b.push_str("NICKNAME:");
        b.push_str(&escape_vcard_component(nickname.trim()));
        b.push_str("\r\n");
    }
    if !organization.trim().is_empty() {
        b.push_str("ORG:");
        b.push_str(&escape_vcard_component(organization.trim()));
        b.push_str("\r\n");
    }
    if !title.trim().is_empty() {
        b.push_str("TITLE:");
        b.push_str(&escape_vcard_component(title.trim()));
        b.push_str("\r\n");
    }
    if let Some(bd) = birthday.map(str::trim).filter(|s| !s.is_empty()) {
        b.push_str("BDAY:");
        b.push_str(bd);
        b.push_str("\r\n");
    }
    for (addr, label) in emails {
        append_email_line(&mut b, addr, label);
    }
    for (num, label) in tels {
        append_typed_line(&mut b, "TEL", num, label);
    }
    for (url, label) in urls {
        append_typed_line(&mut b, "URL", url, label);
    }
    for a in adrs {
        let line = format!(
            "{};{};{};{};{};{};{}",
            escape_vcard_component(&a.po_box),
            escape_vcard_component(&a.extended),
            escape_vcard_component(&a.street),
            escape_vcard_component(&a.locality),
            escape_vcard_component(&a.region),
            escape_vcard_component(&a.postal_code),
            escape_vcard_component(&a.country),
        );
        if !a.label.trim().is_empty() {
            b.push_str(&format!(
                "ADR;TYPE={}:{}\r\n",
                escape_vcard_component(a.label.trim()),
                line
            ));
        } else {
            b.push_str(&format!("ADR:{line}\r\n"));
        }
    }
    for p in photos {
        if let Some(ref uri) = p.source_uri {
            if !uri.is_empty() {
                b.push_str(&format!(
                    "PHOTO;VALUE=URI:{}\r\n",
                    escape_vcard_component(uri)
                ));
            }
        } else if let Some(ref data) = p.data {
            let mime = if p.mime_type.is_empty() {
                "JPEG"
            } else if p.mime_type == "image/png" {
                "PNG"
            } else if p.mime_type == "image/gif" {
                "GIF"
            } else {
                "JPEG"
            };
            let b64 = base64::engine::general_purpose::STANDARD.encode(data);
            b.push_str(&format!(
                "PHOTO;ENCODING=b;TYPE={mime}:{b64}\r\n"
            ));
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
        assert_eq!(v[0].emails.len(), 1);
        assert_eq!(v[0].emails[0].0, "test@example.com");
    }

    #[test]
    fn parses_tel_url_adr() {
        let s = r#"BEGIN:VCARD
VERSION:3.0
FN:Jane
TEL;TYPE=CELL:+1 555 0100
URL;TYPE=work:https://example.com/~j
ADR;TYPE=HOME:;;123 Main St;Somewhere;CA;90210;US
END:VCARD
"#;
        let v = parse_vcards_str(s);
        assert_eq!(v[0].tels.len(), 1);
        assert_eq!(v[0].tels[0].0, "+1 555 0100");
        assert!(v[0].tels[0].1.contains("CELL"));
        assert_eq!(v[0].urls[0].0, "https://example.com/~j");
        assert_eq!(v[0].adrs[0].locality, "Somewhere");
        assert_eq!(v[0].adrs[0].postal_code, "90210");
    }

    #[test]
    fn roundtrip_build_full() {
        let adr = ParsedAdr {
            label: "home".into(),
            po_box: "".into(),
            extended: "".into(),
            street: "1 St".into(),
            locality: "X".into(),
            region: "".into(),
            postal_code: "".into(),
            country: "".into(),
        };
        let photo = ParsedPhoto {
            mime_type: "image/jpeg".into(),
            data: Some(vec![1, 2, 3]),
            source_uri: None,
        };
        let s = build_vcard_full(
            "P",
            "",
            "Org",
            "Title",
            Some("1990-01-01"),
            "n",
            &[("a@b.co", "work")],
            &[("+99", "cell")],
            &[("https://u", "")],
            &[adr],
            &[photo],
        );
        assert!(s.contains("TEL"));
        assert!(s.contains("URL"));
        assert!(s.contains("ADR"));
        assert!(s.contains("PHOTO;ENCODING=b"));
        let back = parse_vcards_str(&s);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].emails.len(), 1);
        assert_eq!(back[0].tels.len(), 1);
    }
}
