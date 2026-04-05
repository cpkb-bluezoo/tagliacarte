/*
 * config_xml.rs
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

//! UI config: `config.xml` under the application data directory with `<store>` entries.

use std::fs;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// One `<store>` from config.xml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigXmlStore {
    pub id: String,
    pub display_name: String,
    pub store_type: String,
}

/// Load `<store id= type= display-name= …/>` (or element with child nodes) from XML.
pub fn load_config_xml_stores(path: &Path) -> Result<Vec<ConfigXmlStore>, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    load_config_xml_stores_from_str(&raw)
}

pub(crate) fn load_config_xml_stores_from_str(
    content: &str,
) -> Result<Vec<ConfigXmlStore>, String> {
    let mut reader = Reader::from_str(content.trim_start());
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    // Scratch for `read_to_end_into` so we do not double-borrow `buf`.
    let mut tail = Vec::new();
    let mut out = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("config.xml parse error: {}", e)),
            Ok(Event::Eof) => break,
            Ok(Event::Empty(ref e)) => {
                if e.name().as_ref() == b"store" {
                    if let Some(entry) = store_from_start(e) {
                        out.push(entry);
                    }
                }
            }
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"store" {
                    if let Some(entry) = store_from_start(e) {
                        out.push(entry);
                    }
                    reader
                        .read_to_end_into(e.name(), &mut tail)
                        .map_err(|e| e.to_string())?;
                    tail.clear();
                }
            }
            Ok(_) => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn store_from_start(e: &quick_xml::events::BytesStart<'_>) -> Option<ConfigXmlStore> {
    let id = match attr_value(e, b"id") {
        Some(s) if !s.is_empty() => s,
        _ => return None,
    };
    let store_type = attr_value(e, b"type").unwrap_or_else(|| "unknown".to_owned());
    let display_name = attr_value(e, b"display-name")
        .or_else(|| attr_value(e, b"displayName"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.clone());

    Some(ConfigXmlStore {
        id,
        display_name,
        store_type,
    })
}

fn attr_value(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    for a in e.attributes().filter_map(Result::ok) {
        if a.key.as_ref() == key {
            let v = a.unescape_value().ok()?;
            return Some(v.into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_store_with_display_name() {
        let xml = r##"
<?xml version="1.0"?>
<config>
  <store id="maildir:///Users/x/mail" type="maildir" display-name="maildir">
    <param key="path" value="/Users/x/mail"/>
  </store>
</config>
"##;
        let stores = load_config_xml_stores_from_str(xml).unwrap();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].id, "maildir:///Users/x/mail");
        assert_eq!(stores[0].store_type, "maildir");
        assert_eq!(stores[0].display_name, "maildir");
    }

    #[test]
    fn empty_store_id_skipped() {
        let xml = r#"<config><store type="maildir" display-name="x"/></config>"#;
        let stores = load_config_xml_stores_from_str(xml).unwrap();
        assert!(stores.is_empty());
    }
}
