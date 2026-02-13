/*
 * content_type.rs
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

//! Content-Type header (RFC 2045). Parameter values: RFC 2047 (encoded-words) and RFC 2231 (charset''value, continuations).

use std::collections::HashMap;

use super::parameter::Parameter;
use super::rfc2047::decode_encoded_words;
use super::utils::is_token;

#[derive(Debug, Clone)]
pub struct ContentType {
    primary_type: String,
    sub_type: String,
    parameter_map: HashMap<String, String>,
}

impl ContentType {
    pub fn new(
        primary_type: impl Into<String>,
        sub_type: impl Into<String>,
        parameters: Option<Vec<Parameter>>,
    ) -> Self {
        let primary_type = primary_type.into();
        let sub_type = sub_type.into();
        let parameter_map = parameters
            .map(|p| {
                p.into_iter()
                    .map(|param| (param.get_name().to_lowercase(), param.get_value().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            primary_type,
            sub_type,
            parameter_map,
        }
    }

    pub fn get_primary_type(&self) -> &str {
        &self.primary_type
    }

    pub fn get_sub_type(&self) -> &str {
        &self.sub_type
    }

    pub fn is_primary_type(&self, t: &str) -> bool {
        self.primary_type.eq_ignore_ascii_case(t)
    }

    pub fn is_sub_type(&self, t: &str) -> bool {
        self.sub_type.eq_ignore_ascii_case(t)
    }

    pub fn is_mime_type(&self, primary: &str, sub: &str) -> bool {
        self.is_primary_type(primary) && self.is_sub_type(sub)
    }

    pub fn get_parameter(&self, name: &str) -> Option<&str> {
        self.parameter_map.get(&name.to_lowercase()).map(String::as_str)
    }

    pub fn has_parameter(&self, name: &str) -> bool {
        self.parameter_map.contains_key(&name.to_lowercase())
    }
}

/// Parse Content-Type header value.
pub fn parse_content_type(value: &str) -> Option<ContentType> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (type_part, params_part) = match value.find(';') {
        Some(i) if i >= 3 => {
            let (a, b) = value.split_at(i);
            (a.trim(), b[1..].trim())
        }
        _ => (value, ""),
    };
    let slash = type_part.find('/')?;
    let primary = type_part[..slash].trim();
    let sub = type_part[slash + 1..].trim();
    if !is_token(primary) || !is_token(sub) {
        return None;
    }
    let parameters = parse_parameter_list(params_part);
    Some(ContentType::new(primary, sub, parameters))
}

/// Raw parameter as parsed (name, value). Value is canonical (no surrounding quotes).
fn parse_one_param(bytes: &[u8], len: &mut usize) -> Option<(String, String)> {
    let mut pos = *len;
    while pos < bytes.len() && (bytes[pos] == b';' || bytes[pos].is_ascii_whitespace()) {
        pos += 1;
    }
    if pos >= bytes.len() {
        return None;
    }
    let eq = bytes[pos..].iter().position(|&b| b == b'=')?;
    let eq_abs = pos + eq;
    if eq_abs < 1 {
        return None;
    }
    let name = std::str::from_utf8(&bytes[pos..eq_abs]).ok()?.trim();
    if !is_token(name) {
        *len = bytes[pos..]
            .iter()
            .position(|&b| b == b';')
            .map(|i| pos + i + 1)
            .unwrap_or(bytes.len());
        return None;
    }
    pos = eq_abs + 1;
    let value = if pos < bytes.len() && bytes[pos] == b'"' {
        pos += 1;
        let mut v = String::new();
        while pos < bytes.len() {
            let c = bytes[pos];
            if c == b'\\' && pos + 1 < bytes.len() {
                v.push(bytes[pos + 1] as char);
                pos += 2;
            } else if c == b'"' {
                pos += 1;
                break;
            } else {
                v.push(c as char);
                pos += 1;
            }
        }
        v
    } else {
        let end = bytes[pos..]
            .iter()
            .position(|&b| b == b';')
            .map(|i| pos + i)
            .unwrap_or(bytes.len());
        let v = std::str::from_utf8(&bytes[pos..end]).ok()?.trim();
        pos = end;
        if !is_token(v) {
            *len = pos;
            return None;
        }
        v.to_string()
    };
    *len = pos;
    Some((name.to_string(), value))
}

/// RFC 2231: decode charset''percent-encoded or merged name*0/name*1 segments to string.
fn decode_rfc2231_value(charset: &str, raw: &str) -> String {
    let decoded = percent_decode(raw);
    let charset_lower = to_ascii_lowercase(charset);
    match charset_lower.as_str() {
        "utf-8" | "utf8" => String::from_utf8_lossy(&decoded).into_owned(),
        "iso-8859-1" | "latin1" | "iso_8859-1" => decoded.iter().map(|&b| b as char).collect(),
        _ => String::from_utf8_lossy(&decoded).into_owned(),
    }
}

fn to_ascii_lowercase(s: &str) -> String {
    s.chars()
        .map(|c| if c >= 'A' && c <= 'Z' { ((c as u8) + (b'a' - b'A')) as char } else { c })
        .collect()
}

fn percent_decode(input: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 <= bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if hi >= 0 && lo >= 0 {
                out.push((hi << 4 | lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn hex_val(b: u8) -> i32 {
    match b {
        b'0'..=b'9' => (b - b'0') as i32,
        b'A'..=b'F' => (b - b'A' + 10) as i32,
        b'a'..=b'f' => (b - b'a' + 10) as i32,
        _ => -1,
    }
}

/// Base name for RFC 2231: "filename*" -> "filename", "filename*0" -> "filename".
fn rfc2231_base_name(name: &str) -> Option<&str> {
    if name.ends_with('*') {
        return Some(name.trim_end_matches('*'));
    }
    if let Some(star) = name.rfind('*') {
        let after = &name[star + 1..];
        if after.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return Some(name[..star].trim_end_matches('*'));
        }
    }
    None
}

/// Parse semicolon-separated parameter list. Applies RFC 2047 and RFC 2231 decoding to parameter values.
/// Canonical form: no surrounding quotes; encoded-words and RFC 2231 decoded.
pub fn parse_parameter_list(params_part: &str) -> Option<Vec<Parameter>> {
    let params_part = params_part.trim();
    if params_part.is_empty() {
        return None;
    }
    let bytes = params_part.as_bytes();
    let mut pos = 0;

    let mut regular: HashMap<String, String> = HashMap::new();
    let mut rfc2231_star: HashMap<String, String> = HashMap::new();
    let mut rfc2231_continuations: HashMap<String, Vec<(u32, String)>> = HashMap::new();

    while let Some((name, value)) = parse_one_param(bytes, &mut pos) {
        if name.ends_with('*') && !name.as_str().trim_end_matches('*').ends_with('*') {
            let base = name.trim_end_matches('*').to_string();
            if let Some(apos) = value.find("''") {
                let charset = value[..apos].trim();
                let encoded = value[apos + 2..].trim();
                let decoded = decode_rfc2231_value(charset, encoded);
                rfc2231_star.insert(base.to_lowercase(), decoded);
            }
        } else if let Some(base) = rfc2231_base_name(&name) {
            let base = base.to_string();
            let idx_str = name
                .rfind('*')
                .and_then(|i| name.get(i + 1..))
                .unwrap_or("");
            if let Ok(idx) = idx_str.parse::<u32>() {
                rfc2231_continuations
                    .entry(base.to_lowercase())
                    .or_default()
                    .push((idx, value));
            }
        } else if is_token(&name) {
            let canonical = decode_encoded_words(&value);
            regular.insert(name.to_lowercase(), canonical);
        }
    }

    let mut rfc2231_merged: HashMap<String, String> = rfc2231_star;
    for (base, mut segs) in rfc2231_continuations {
        segs.sort_by_key(|(i, _)| *i);
        let merged: String = segs.iter().map(|(_, v)| v.as_str()).collect();
        let decoded = decode_rfc2231_value("utf-8", &merged);
        rfc2231_merged.entry(base).or_insert(decoded);
    }

    let mut out: Vec<Parameter> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for (name, value) in &regular {
        let final_val = rfc2231_merged
            .remove(name)
            .unwrap_or_else(|| value.clone());
        if seen.insert(name.clone()) {
            out.push(Parameter::new(name, final_val));
        }
    }
    for (name, value) in rfc2231_merged {
        if seen.insert(name.clone()) {
            out.push(Parameter::new(name, value));
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
