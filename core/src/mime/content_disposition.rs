/*
 * content_disposition.rs
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

//! Content-Disposition header (RFC 2183).

use super::content_type::parse_parameter_list;
use super::parameter::Parameter;
use super::utils::is_token;

#[derive(Debug, Clone)]
pub struct ContentDisposition {
    disposition_type: String,
    parameter_map: std::collections::HashMap<String, String>,
}

impl ContentDisposition {
    pub fn new(disposition_type: impl Into<String>, parameters: Option<Vec<Parameter>>) -> Self {
        let disposition_type = disposition_type.into();
        let parameter_map = parameters
            .map(|p| {
                p.into_iter()
                    .map(|param| (param.get_name().to_lowercase(), param.get_value().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            disposition_type,
            parameter_map,
        }
    }

    pub fn get_disposition_type(&self) -> &str {
        &self.disposition_type
    }

    pub fn is_disposition_type(&self, t: &str) -> bool {
        self.disposition_type.eq_ignore_ascii_case(t)
    }

    pub fn get_parameter(&self, name: &str) -> Option<&str> {
        self.parameter_map.get(&name.to_lowercase()).map(String::as_str)
    }

    pub fn has_parameter(&self, name: &str) -> bool {
        self.parameter_map.contains_key(&name.to_lowercase())
    }
}

pub fn parse_content_disposition(value: &str) -> Option<ContentDisposition> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (disp_part, params_part) = match value.find(';') {
        Some(i) if i >= 3 => {
            let (a, b) = value.split_at(i);
            (a.trim(), b[1..].trim())
        }
        _ => (value, ""),
    };
    if !is_token(disp_part) {
        return None;
    }
    let parameters = parse_parameter_list(params_part);
    Some(ContentDisposition::new(disp_part, parameters))
}
