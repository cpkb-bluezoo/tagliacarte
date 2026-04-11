/*
 * href_collector.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Collect character data inside DAV-style `<href>` elements (any namespace prefix).

use std::io::Read;

use super::handler::XmlContentHandler;
use super::parser::XmlParser;

fn is_href_local_name(local: &str) -> bool {
    let lower = local.to_ascii_lowercase();
    lower == "href" || lower.ends_with(":href")
}

/// Extract trimmed `href` text from a PROPFIND (or similar) XML body. Namespace-agnostic.
/// Streams bytes through [`XmlParser::receive`] (no full-body [`String`]).
pub fn collect_href_texts_from_reader<R: Read>(mut reader: R) -> Result<Vec<String>, super::error::XmlError> {
    let mut p = XmlParser::new(false);
    let mut c = HrefCollector::default();
    p.parse_reader_to_close(&mut reader, &mut c)?;
    Ok(c.hrefs)
}

/// Convenience when the response is already a UTF-8 [`str`].
pub fn collect_href_texts(xml: &str) -> Result<Vec<String>, super::error::XmlError> {
    collect_href_texts_from_reader(std::io::Cursor::new(xml.as_bytes()))
}

#[derive(Default)]
struct HrefCollector {
    href_depth: usize,
    text_buf: String,
    hrefs: Vec<String>,
}

impl XmlContentHandler for HrefCollector {
    fn start_element(&mut self, _ns: Option<&str>, local_name: &str) {
        if is_href_local_name(local_name) {
            self.href_depth += 1;
            self.text_buf.clear();
        }
    }

    fn attribute(&mut self, _ns: Option<&str>, _local: &str, _value: &str) {}

    fn characters(&mut self, text: &str) {
        if self.href_depth > 0 {
            self.text_buf.push_str(text);
        }
    }

    fn end_element(&mut self, _ns: Option<&str>, local_name: &str) {
        if is_href_local_name(local_name) && self.href_depth > 0 {
            self.href_depth -= 1;
            let t = self.text_buf.trim();
            if !t.is_empty() {
                self.hrefs.push(t.to_string());
            }
            self.text_buf.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixed_href() {
        let xml = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/addr/1.vcf</d:href></d:response></d:multistatus>"#;
        let h = collect_href_texts(xml).unwrap();
        assert!(h.iter().any(|s| s.contains("1.vcf")));
    }
}
