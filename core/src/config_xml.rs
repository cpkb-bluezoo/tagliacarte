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
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

//! UI config: `config.xml` under the application data directory with `<store>` entries.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use tokio::io::AsyncRead;

use crate::xml::XmlContentHandler;
use crate::xml::XmlParser;

/// One `<store>` from config.xml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigXmlStore {
    pub id: String,
    pub display_name: String,
    pub store_type: String,
}

/// Load `<store id= type= display-name= …/>` (or element with child nodes) from XML.
pub fn load_config_xml_stores(path: &Path) -> Result<Vec<ConfigXmlStore>, String> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    load_config_xml_stores_from_reader(f)
}

/// Stream bytes into the XML parser (chunked [`XmlParser::receive`], then [`XmlParser::close`]).
pub(crate) fn load_config_xml_stores_from_reader<R: Read>(
    mut reader: R,
) -> Result<Vec<ConfigXmlStore>, String> {
    let mut h = StoreScanHandler {
        stack: Vec::new(),
        stores: Vec::new(),
    };
    let mut p = XmlParser::new(false);
    p.parse_reader_to_close(&mut reader, &mut h)
        .map_err(|e| e.to_string())?;
    Ok(h.stores)
}

/// Same as [`load_config_xml_stores_from_reader`] using [`XmlParser::parse_async_read_to_close`].
pub(crate) async fn load_config_xml_stores_from_async_reader<R>(
    mut reader: R,
) -> Result<Vec<ConfigXmlStore>, String>
where
    R: AsyncRead + Unpin,
{
    let mut h = StoreScanHandler {
        stack: Vec::new(),
        stores: Vec::new(),
    };
    let mut p = XmlParser::new(false);
    p.parse_async_read_to_close(&mut reader, &mut h)
        .await
        .map_err(|e| e.to_string())?;
    Ok(h.stores)
}

struct Frame {
    name: String,
    attrs: BTreeMap<String, String>,
}

struct StoreScanHandler {
    stack: Vec<Frame>,
    stores: Vec<ConfigXmlStore>,
}

impl XmlContentHandler for StoreScanHandler {
    fn start_element(&mut self, _ns: Option<&str>, local_name: &str) {
        self.stack.push(Frame {
            name: local_name.to_string(),
            attrs: BTreeMap::new(),
        });
    }

    fn attribute(&mut self, _ns: Option<&str>, local_name: &str, value: &str) {
        if let Some(f) = self.stack.last_mut() {
            f.attrs.insert(local_name.to_string(), value.to_string());
        }
    }

    fn characters(&mut self, _text: &str) {}

    fn end_element(&mut self, _ns: Option<&str>, local_name: &str) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        if frame.name != local_name {
            return;
        }
        if frame.name == "store" {
            if let Some(entry) = store_from_attrs(&frame.attrs) {
                self.stores.push(entry);
            }
        }
    }
}

fn store_from_attrs(attrs: &BTreeMap<String, String>) -> Option<ConfigXmlStore> {
    let id = attrs.get("id").filter(|s| !s.is_empty())?.clone();
    let store_type = attrs
        .get("type")
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let display_name = attrs
        .get("display-name")
        .or_else(|| attrs.get("displayName"))
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| id.clone());

    Some(ConfigXmlStore {
        id,
        display_name,
        store_type,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

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
        let stores = load_config_xml_stores_from_reader(Cursor::new(xml.as_bytes())).unwrap();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].id, "maildir:///Users/x/mail");
        assert_eq!(stores[0].store_type, "maildir");
        assert_eq!(stores[0].display_name, "maildir");
    }

    #[test]
    fn empty_store_id_skipped() {
        let xml = r#"<config><store type="maildir" display-name="x"/></config>"#;
        let stores = load_config_xml_stores_from_reader(Cursor::new(xml.as_bytes())).unwrap();
        assert!(stores.is_empty());
    }
}
