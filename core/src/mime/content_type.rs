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

//! Content-Type header (RFC 2045).

use std::collections::HashMap;

use super::parameter::Parameter;
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

/// Parse semicolon-separated parameter list (name=value; name="value").
pub fn parse_parameter_list(params_part: &str) -> Option<Vec<Parameter>> {
    let params_part = params_part.trim();
    if params_part.is_empty() {
        return None;
    }
    let mut parameters = Vec::new();
    let mut pos = 0;
    let bytes = params_part.as_bytes();
    let len = bytes.len();

    while pos < len {
        while pos < len && (bytes[pos] == b';' || bytes[pos].is_ascii_whitespace()) {
            pos += 1;
        }
        if pos >= len {
            break;
        }
        let eq = bytes[pos..].iter().position(|&b| b == b'=')?;
        let eq_abs = pos + eq;
        if eq_abs < 1 {
            break;
        }
        let name = std::str::from_utf8(&bytes[pos..eq_abs]).ok()?.trim();
        if !is_token(name) {
            if let Some(semi) = bytes[pos..].iter().position(|&b| b == b';') {
                pos += semi + 1;
                continue;
            }
            break;
        }
        pos = eq_abs + 1;
        let value = if pos < len && bytes[pos] == b'"' {
            pos += 1;
            let mut v = String::new();
            while pos < len {
                let c = bytes[pos];
                if c == b'\\' && pos + 1 < len {
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
            let end = bytes[pos..].iter().position(|&b| b == b';').map(|i| pos + i).unwrap_or(len);
            let v = std::str::from_utf8(&bytes[pos..end]).ok()?.trim();
            pos = end;
            if !is_token(v) {
                continue;
            }
            v.to_string()
        };
        parameters.push(Parameter::new(name, value));
    }
    if parameters.is_empty() {
        None
    } else {
        Some(parameters)
    }
}
