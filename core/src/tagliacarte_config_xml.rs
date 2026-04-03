/*
 * tagliacarte_config_xml.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

//! Full `config.xml` model: root `<tagliacarte>` with `<transports>`, `<stores>`, and preference
//! blocks. Simple fields are attributes; under `<store>` only `<transport ref="…"/>` children are
//! allowed (document order = outbound priority). Symbolic strings replace numeric enums (e.g.
//! `security="starttls"`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

/// One outbound transport (`<transport …/>` under `<transports>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportXml {
    pub id: String,
    pub transport_type: String,
    pub display_name: String,
    pub host: String,
    pub port: u16,
    /// Symbolic: `tls` | `starttls` | `plain` | `implicit_tls`.
    pub security: String,
}

impl TransportXml {
    /// SMTP/SMTPS URL for this transport (`smtp://…` or `smtps://…` per [crate::uri]).
    pub fn smtp_connection_uri(&self) -> String {
        crate::uri::smtp_transport_uri(self.host.as_str(), self.port)
    }
}

impl StoreXml {
    /// Connection URI for mail/native open (`maildir:///…`, `imaps://…`, …).
    /// When [Self::legacy_connection_uri] is set (legacy `id` was a full URI), returns it unchanged.
    pub fn connection_uri(&self) -> Result<String, String> {
        if let Some(ref u) = self.connection_uri_attr {
            return Ok(u.clone());
        }
        if let Some(ref u) = self.legacy_connection_uri {
            return Ok(u.clone());
        }
        let t = self.store_type.to_lowercase();
        match t.as_str() {
            "maildir" => {
                let p = self
                    .path
                    .as_deref()
                    .ok_or_else(|| "maildir store: missing path attribute".to_string())?;
                Ok(crate::uri::maildir_store_uri(p))
            }
            "mbox" => {
                let p = self
                    .path
                    .as_deref()
                    .ok_or_else(|| "mbox store: missing path attribute".to_string())?;
                Ok(crate::uri::mbox_store_uri(p))
            }
            "imap" | "imaps" => {
                let host = self
                    .host
                    .as_deref()
                    .ok_or_else(|| "IMAP store: missing host attribute".to_string())?;
                let user = self.username.as_deref().unwrap_or("");
                let sec = self.security.as_deref().map(|s| s.to_lowercase());
                let port = self.port.unwrap_or_else(|| match sec.as_deref() {
                    Some("starttls") | Some("plain") => 143,
                    _ => 993,
                });
                Ok(crate::uri::imap_store_uri(user, host, port))
            }
            "pop3" | "pop3s" => {
                let host = self
                    .host
                    .as_deref()
                    .ok_or_else(|| "POP3 store: missing host attribute".to_string())?;
                let user = self.username.as_deref().unwrap_or("");
                let sec = self.security.as_deref().map(|s| s.to_lowercase());
                let port = self.port.unwrap_or_else(|| match sec.as_deref() {
                    Some("starttls") | Some("plain") => 110,
                    _ => 995,
                });
                Ok(crate::uri::pop3_store_uri(user, host, port))
            }
            "nostr" => {
                let id = self
                    .path
                    .as_deref()
                    .or(self.username.as_deref())
                    .ok_or_else(|| "nostr store: missing path or username (id)".to_string())?;
                Ok(crate::uri::nostr_store_uri(id))
            }
            "matrix" => {
                let hs = self
                    .host
                    .as_deref()
                    .ok_or_else(|| "matrix store: missing host (homeserver)".to_string())?;
                let u = self
                    .username
                    .as_deref()
                    .ok_or_else(|| "matrix store: missing username".to_string())?;
                Ok(crate::uri::matrix_store_uri(hs, u))
            }
            "graph" => {
                let email = self
                    .username
                    .as_deref()
                    .ok_or_else(|| "graph store: missing username (email)".to_string())?;
                Ok(crate::uri::graph_store_uri(email))
            }
            "gmail" => {
                let email = self
                    .username
                    .as_deref()
                    .ok_or_else(|| "gmail store: missing username (email)".to_string())?;
                Ok(crate::uri::gmail_store_uri(email))
            }
            _ => Err(format!(
                "cannot derive connection URI for store type {:?}",
                self.store_type
            )),
        }
    }
}

/// One mail / identity store (`<store …>` with optional `<transport ref>` children).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreXml {
    pub id: String,
    pub store_type: String,
    pub display_name: String,
    pub username: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub security: Option<String>,
    pub path: Option<String>,
    /// Ordered transport ids (first = default for send).
    pub transport_refs: Vec<String>,
    /// Legacy: when `id` was a full connection URI (`maildir:///…`), connection is this string.
    pub legacy_connection_uri: Option<String>,
    /// Opaque connection URI as attribute `connection-uri` when `id` is a stable `sN` but the URI is not structured in other fields.
    pub connection_uri_attr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedStoreXml {
    pub store_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecurityXml {
    pub attrs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewingXml {
    pub attrs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComposingXml {
    pub attrs: BTreeMap<String, String>,
}

/// Parsed `config.xml` / `tagliacarte` document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TagliacarteConfigFile {
    pub transports: Vec<TransportXml>,
    pub stores: Vec<StoreXml>,
    pub selected_store: SelectedStoreXml,
    pub security: SecurityXml,
    pub viewing: ViewingXml,
    pub composing: ComposingXml,
}

impl TagliacarteConfigFile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        load_tagliacarte_config_from_str(&raw)
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let bytes = write_tagliacarte_config(self)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, bytes).map_err(|e| e.to_string())
    }
}

pub fn load_tagliacarte_config(path: &Path) -> Result<TagliacarteConfigFile, String> {
    TagliacarteConfigFile::load(path)
}

pub fn write_tagliacarte_config(cfg: &TagliacarteConfigFile) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);
    w.write_event(Event::Decl(quick_xml::events::BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        None,
    )))
    .map_err(|e| e.to_string())?;

    let root = BytesStart::new("tagliacarte");
    w.write_event(Event::Start(root.clone()))
        .map_err(|e| e.to_string())?;

    if let Some(ref id) = cfg.selected_store.store_id {
        let mut el = BytesStart::new("selected-store");
        el.push_attribute(("id", id.as_str()));
        w.write_event(Event::Empty(el)).map_err(|e| e.to_string())?;
    }

    write_attrs_element(&mut w, "security", &cfg.security.attrs)?;
    write_attrs_element(&mut w, "viewing", &cfg.viewing.attrs)?;
    write_attrs_element(&mut w, "composing", &cfg.composing.attrs)?;

    w.write_event(Event::Start(BytesStart::new("transports")))
        .map_err(|e| e.to_string())?;
    for t in &cfg.transports {
        write_transport_empty(&mut w, t)?;
    }
    w.write_event(Event::End(BytesEnd::new("transports")))
        .map_err(|e| e.to_string())?;

    w.write_event(Event::Start(BytesStart::new("stores")))
        .map_err(|e| e.to_string())?;
    for s in &cfg.stores {
        write_store_element(&mut w, s)?;
    }
    w.write_event(Event::End(BytesEnd::new("stores")))
        .map_err(|e| e.to_string())?;

    w.write_event(Event::End(BytesEnd::new("tagliacarte")))
        .map_err(|e| e.to_string())?;

    Ok(buf)
}

fn write_attrs_element(
    w: &mut Writer<&mut Vec<u8>>,
    name: &str,
    attrs: &BTreeMap<String, String>,
) -> Result<(), String> {
    if attrs.is_empty() {
        return Ok(());
    }
    let mut el = BytesStart::new(name);
    for (k, v) in attrs {
        el.push_attribute((k.as_str(), v.as_str()));
    }
    w.write_event(Event::Empty(el)).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_transport_empty(w: &mut Writer<&mut Vec<u8>>, t: &TransportXml) -> Result<(), String> {
    let mut el = BytesStart::new("transport");
    el.push_attribute(("id", t.id.as_str()));
    el.push_attribute(("type", t.transport_type.as_str()));
    el.push_attribute(("display-name", t.display_name.as_str()));
    el.push_attribute(("host", t.host.as_str()));
    el.push_attribute(("port", t.port.to_string().as_str()));
    el.push_attribute(("security", t.security.as_str()));
    w.write_event(Event::Empty(el)).map_err(|e| e.to_string())
}

fn write_store_element(w: &mut Writer<&mut Vec<u8>>, s: &StoreXml) -> Result<(), String> {
    let mut start = BytesStart::new("store");
    start.push_attribute(("id", s.id.as_str()));
    start.push_attribute(("type", s.store_type.as_str()));
    start.push_attribute(("display-name", s.display_name.as_str()));
    if let Some(ref u) = s.username {
        start.push_attribute(("username", u.as_str()));
    }
    if let Some(ref h) = s.host {
        start.push_attribute(("host", h.as_str()));
    }
    if let Some(p) = s.port {
        start.push_attribute(("port", p.to_string().as_str()));
    }
    if let Some(ref sec) = s.security {
        start.push_attribute(("security", sec.as_str()));
    }
    if let Some(ref p) = s.path {
        start.push_attribute(("path", p.as_str()));
    }
    if let Some(ref c) = s.connection_uri_attr {
        start.push_attribute(("connection-uri", c.as_str()));
    }

    if s.transport_refs.is_empty() {
        w.write_event(Event::Empty(start))
            .map_err(|e| e.to_string())?;
    } else {
        w.write_event(Event::Start(start.clone()))
            .map_err(|e| e.to_string())?;
        for r in &s.transport_refs {
            let mut tr = BytesStart::new("transport");
            tr.push_attribute(("ref", r.as_str()));
            w.write_event(Event::Empty(tr)).map_err(|e| e.to_string())?;
        }
        w.write_event(Event::End(BytesEnd::new("store")))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn load_tagliacarte_config_from_str(content: &str) -> Result<TagliacarteConfigFile, String> {
    let trimmed = content.trim_start();
    if trimmed.contains("<tagliacarte") {
        parse_tagliacarte(trimmed)
    } else if trimmed.contains("<config") {
        parse_legacy_config_root(trimmed)
    } else {
        Err("config.xml: expected root <tagliacarte> or <config>".to_owned())
    }
}

fn parse_legacy_config_root(content: &str) -> Result<TagliacarteConfigFile, String> {
    let stores = crate::config_xml::load_config_xml_stores_from_str(content)?;
    let mut out = TagliacarteConfigFile::default();
    for s in stores {
        out.stores.push(StoreXml {
            id: s.id.clone(),
            display_name: s.display_name,
            store_type: s.store_type,
            username: None,
            host: None,
            port: None,
            security: None,
            path: None,
            transport_refs: Vec::new(),
            legacy_connection_uri: Some(s.id),
            connection_uri_attr: None,
        });
    }
    Ok(out)
}

fn parse_tagliacarte(content: &str) -> Result<TagliacarteConfigFile, String> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = TagliacarteConfigFile::default();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("config.xml parse error: {}", e)),
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"tagliacarte" => {
                let mut tail = Vec::new();
                read_tagliacarte_body(&mut reader, &mut buf, &mut tail, &mut out)?;
            }
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let _ = e;
            }
            Ok(_) => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn read_tagliacarte_body(
    reader: &mut Reader<&[u8]>,
    buf: &mut Vec<u8>,
    tail: &mut Vec<u8>,
    out: &mut TagliacarteConfigFile,
) -> Result<(), String> {
    loop {
        match reader.read_event_into(buf) {
            Err(e) => return Err(e.to_string()),
            Ok(Event::Eof) => break,
            Ok(Event::End(ref e)) if e.name().as_ref() == b"tagliacarte" => break,
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"transports" => {
                read_transports_block(reader, buf, tail, out)?;
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"stores" => {
                read_stores_block(reader, buf, tail, out)?;
            }
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"selected-store" => {
                if let Some(id) = attr_value(e, b"id") {
                    out.selected_store.store_id = Some(id);
                }
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"selected-store" => {
                reader
                    .read_to_end_into(e.name(), tail)
                    .map_err(|e| e.to_string())?;
                tail.clear();
            }
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"security" => {
                out.security.attrs = all_attrs(e);
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"security" => {
                out.security.attrs = all_attrs(e);
                reader
                    .read_to_end_into(e.name(), tail)
                    .map_err(|e| e.to_string())?;
                tail.clear();
            }
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"viewing" => {
                out.viewing.attrs = all_attrs(e);
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"viewing" => {
                out.viewing.attrs = all_attrs(e);
                reader
                    .read_to_end_into(e.name(), tail)
                    .map_err(|e| e.to_string())?;
                tail.clear();
            }
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"composing" => {
                out.composing.attrs = all_attrs(e);
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"composing" => {
                out.composing.attrs = all_attrs(e);
                reader
                    .read_to_end_into(e.name(), tail)
                    .map_err(|e| e.to_string())?;
                tail.clear();
            }
            Ok(Event::Start(ref e)) => {
                reader
                    .read_to_end_into(e.name(), tail)
                    .map_err(|e| e.to_string())?;
                tail.clear();
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

fn read_transports_block(
    reader: &mut Reader<&[u8]>,
    buf: &mut Vec<u8>,
    tail: &mut Vec<u8>,
    out: &mut TagliacarteConfigFile,
) -> Result<(), String> {
    loop {
        match reader.read_event_into(buf) {
            Err(e) => return Err(e.to_string()),
            Ok(Event::End(ref e)) if e.name().as_ref() == b"transports" => break,
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"transport" => {
                if let Some(t) = transport_from_empty(e) {
                    out.transports.push(t);
                }
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"transport" => {
                reader
                    .read_to_end_into(e.name(), tail)
                    .map_err(|e| e.to_string())?;
                tail.clear();
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

fn read_stores_block(
    reader: &mut Reader<&[u8]>,
    buf: &mut Vec<u8>,
    tail: &mut Vec<u8>,
    out: &mut TagliacarteConfigFile,
) -> Result<(), String> {
    loop {
        match reader.read_event_into(buf) {
            Err(e) => return Err(e.to_string()),
            Ok(Event::End(ref e)) if e.name().as_ref() == b"stores" => break,
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"store" => {
                if let Some(s) = store_from_element(e) {
                    out.stores.push(s);
                }
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"store" => {
                let mut s = store_from_element(e).ok_or_else(|| "store missing id".to_string())?;
                read_store_children(reader, buf, tail, &mut s)?;
                out.stores.push(s);
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

fn read_store_children(
    reader: &mut Reader<&[u8]>,
    buf: &mut Vec<u8>,
    tail: &mut Vec<u8>,
    s: &mut StoreXml,
) -> Result<(), String> {
    loop {
        match reader.read_event_into(buf) {
            Err(e) => return Err(e.to_string()),
            Ok(Event::End(ref e)) if e.name().as_ref() == b"store" => break,
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"transport" => {
                if let Some(r) = attr_value(e, b"ref") {
                    s.transport_refs.push(r);
                }
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"transport" => {
                reader
                    .read_to_end_into(e.name(), tail)
                    .map_err(|e| e.to_string())?;
                tail.clear();
            }
            Ok(_) => {}
        }
    }
    Ok(())
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

fn all_attrs(e: &quick_xml::events::BytesStart<'_>) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for a in e.attributes().filter_map(Result::ok) {
        if let Ok(v) = a.unescape_value() {
            let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
            m.insert(key, v.into_owned());
        }
    }
    m
}

fn transport_from_empty(e: &quick_xml::events::BytesStart<'_>) -> Option<TransportXml> {
    let id = attr_value(e, b"id")?;
    if id.is_empty() {
        return None;
    }
    let transport_type = attr_value(e, b"type").unwrap_or_else(|| "smtp".to_owned());
    let display_name = attr_value(e, b"display-name")
        .or_else(|| attr_value(e, b"displayName"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.clone());
    let host = attr_value(e, b"host").unwrap_or_default();
    let port = attr_value(e, b"port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(587);
    let security = attr_value(e, b"security").unwrap_or_else(|| "starttls".to_owned());
    Some(TransportXml {
        id,
        transport_type,
        display_name,
        host,
        port,
        security,
    })
}

fn store_from_element(e: &quick_xml::events::BytesStart<'_>) -> Option<StoreXml> {
    let id = attr_value(e, b"id")?;
    if id.is_empty() {
        return None;
    }
    let store_type = attr_value(e, b"type").unwrap_or_else(|| "unknown".to_owned());
    let display_name = attr_value(e, b"display-name")
        .or_else(|| attr_value(e, b"displayName"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.clone());
    let username = attr_value(e, b"username");
    let host = attr_value(e, b"host");
    let port = attr_value(e, b"port").and_then(|s| s.parse().ok());
    let security = attr_value(e, b"security");
    let path = attr_value(e, b"path");
    let legacy = if id.contains("://") {
        Some(id.clone())
    } else {
        None
    };
    let connection_uri_attr = attr_value(e, b"connection-uri");
    Some(StoreXml {
        id,
        store_type,
        display_name,
        username,
        host,
        port,
        security,
        path,
        transport_refs: Vec::new(),
        legacy_connection_uri: legacy,
        connection_uri_attr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tagliacarte>
  <selected-store id="s1"/>
  <security tls-verify="strict" password-storage="keychain"/>
  <viewing date-format="iso"/>
  <composing wrap="plain"/>
  <transports>
    <transport id="t1" type="smtp" display-name="SMTP" host="smtp.example.com" port="587" security="starttls"/>
  </transports>
  <stores>
    <store id="s1" type="imap" display-name="Work" username="u" host="imap.example.com" port="993" security="tls">
      <transport ref="t1"/>
    </store>
    <store id="s2" type="maildir" display-name="Local" path="/tmp/mail"/>
  </stores>
</tagliacarte>
"#;

    #[test]
    fn round_trip_preserves_symbolic_attrs_and_order() {
        let parsed = load_tagliacarte_config_from_str(SAMPLE).unwrap();
        assert_eq!(parsed.selected_store.store_id.as_deref(), Some("s1"));
        assert_eq!(
            parsed.security.attrs.get("tls-verify").map(String::as_str),
            Some("strict")
        );
        assert_eq!(parsed.transports.len(), 1);
        assert_eq!(parsed.transports[0].security, "starttls");
        assert_eq!(parsed.stores.len(), 2);
        assert_eq!(parsed.stores[0].transport_refs, vec!["t1".to_string()]);
        let bytes = write_tagliacarte_config(&parsed).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(!s.contains("security=\"2\""));
        assert!(s.contains("security=\"starttls\""));
        let again = load_tagliacarte_config_from_str(&s).unwrap();
        assert_eq!(again, parsed);
    }

    #[test]
    fn empty_transport_list_on_store() {
        let xml = r#"<tagliacarte><stores><store id="s1" type="imap" display-name="x" host="h" port="143" security="starttls"/></stores></tagliacarte>"#;
        let c = load_tagliacarte_config_from_str(xml).unwrap();
        assert!(c.stores[0].transport_refs.is_empty());
    }

    #[test]
    fn legacy_config_maps_uri_id() {
        let xml =
            r#"<config><store id="maildir:///a/b" type="maildir" display-name="m"/></config>"#;
        let c = load_tagliacarte_config_from_str(xml).unwrap();
        assert_eq!(c.stores.len(), 1);
        assert_eq!(
            c.stores[0].legacy_connection_uri.as_deref(),
            Some("maildir:///a/b")
        );
    }

    #[test]
    fn store_connection_uri_imap_and_maildir() {
        let maildir = StoreXml {
            id: "s2".to_owned(),
            store_type: "maildir".to_owned(),
            display_name: "L".to_owned(),
            username: None,
            host: None,
            port: None,
            security: None,
            path: Some("/var/mail/me".to_owned()),
            transport_refs: vec![],
            legacy_connection_uri: None,
            connection_uri_attr: None,
        };
        assert_eq!(maildir.connection_uri().unwrap(), "maildir:///var/mail/me");

        let imap = StoreXml {
            id: "s1".to_owned(),
            store_type: "imap".to_owned(),
            display_name: "W".to_owned(),
            username: Some("u".to_owned()),
            host: Some("imap.example.com".to_owned()),
            port: Some(993),
            security: Some("tls".to_owned()),
            path: None,
            transport_refs: vec![],
            legacy_connection_uri: None,
            connection_uri_attr: None,
        };
        assert_eq!(
            imap.connection_uri().unwrap(),
            "imaps://u@imap.example.com:993"
        );
    }
}
