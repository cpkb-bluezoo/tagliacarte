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
//! blocks. Simple fields are attributes; under `<store>` use `<transport ref="…"/>` and optional
//! `<last-mail folder="…" message-id="…"/>`. Legacy mail location on `<selected-store>` attrs is
//! read for migration only (not written).

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};

use crate::json::IndentConfig;
use crate::xml::XmlContentHandler;
use crate::xml::XmlParser;
use crate::xml::XmlWriter;

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
    /// Default RFC 5322 From when this transport is selected in compose.
    pub default_from: String,
    /// Comma-separated tokens: `never`, `failure`, `success`, `delay` (default when absent: `failure`).
    pub dsn_notify: String,
    /// Optional OAuth provider hint for XOAUTH2 on SMTP transports (`google`, `microsoft`, ...).
    pub oauth_provider: String,
}

/// One mail / identity store (`<store …>` with optional `<transport ref>`, `<relay url>`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreXml {
    pub id: String,
    pub store_type: String,
    pub display_name: String,
    /// Store-specific attributes from XML (`username`, `host`, `port`, `npub`, `nip05`, …).
    pub attrs: BTreeMap<String, String>,
    /// Ordered transport ids (first = default for send).
    pub transport_refs: Vec<String>,
    /// Nostr bootstrap / definitive relay URLs (`<relay url="…"/>`).
    pub relay_urls: Vec<String>,
    /// Legacy: when `id` was a full connection URI (`maildir:///…`), connection is this string.
    pub legacy_connection_uri: Option<String>,
    /// Opaque connection URI as attribute `connection-uri` when `id` is a stable `sN` but the URI is not structured in other fields.
    pub connection_uri_attr: Option<String>,
    /// Last viewed folder for this store (`<last-mail folder="…"/>`).
    pub last_mail_folder: Option<String>,
    /// Last viewed message id within [last_mail_folder] (`<last-mail message-id="…"/>`).
    pub last_mail_message_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedStoreXml {
    pub store_id: Option<String>,
    /// Legacy: read from `<selected-store folder="…" message-id="…"/>`; merged into matching store on load; not written.
    pub legacy_folder: Option<String>,
    pub legacy_message_id: Option<String>,
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
        let f = fs::File::open(path).map_err(|e| e.to_string())?;
        load_tagliacarte_config_from_reader(f)
    }

    /// Load via [`tokio::fs::File`] and [`XmlParser::parse_async_read_to_close`] (non-blocking disk I/O).
    pub async fn load_async(path: &Path) -> Result<Self, String> {
        load_tagliacarte_config_async(path).await
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let bytes = write_tagliacarte_config(self)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, bytes).map_err(|e| e.to_string())
    }

    pub async fn write_async(&self, path: &Path) -> Result<(), String> {
        let bytes = write_tagliacarte_config(self)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        tokio::fs::write(path, bytes)
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn load_tagliacarte_config(path: &Path) -> Result<TagliacarteConfigFile, String> {
    TagliacarteConfigFile::load(path)
}

/// Max bytes read while sniffing for `<tagliacarte` vs `<config` before failing.
const CONFIG_XML_ROOT_SNIFF_MAX: usize = 1024 * 1024;

enum ConfigXmlRootKind {
    Tagliacarte,
    LegacyConfig,
}

fn classify_config_xml_prefix(buf: &[u8]) -> Result<Option<ConfigXmlRootKind>, String> {
    let s = match std::str::from_utf8(buf) {
        Ok(s) => s,
        Err(e) => {
            if e.error_len().is_some() {
                return Err("config.xml: invalid UTF-8".to_string());
            }
            return Ok(None);
        }
    };
    let t = s.trim_start();
    if t.contains("<tagliacarte") {
        return Ok(Some(ConfigXmlRootKind::Tagliacarte));
    }
    if t.contains("<config") {
        return Ok(Some(ConfigXmlRootKind::LegacyConfig));
    }
    Ok(None)
}

fn classify_config_xml_final(buf: &[u8]) -> Result<ConfigXmlRootKind, String> {
    let s = std::str::from_utf8(buf).map_err(|_| "config.xml: invalid UTF-8".to_string())?;
    let t = s.trim_start();
    if t.contains("<tagliacarte") {
        return Ok(ConfigXmlRootKind::Tagliacarte);
    }
    if t.contains("<config") {
        return Ok(ConfigXmlRootKind::LegacyConfig);
    }
    Err("config.xml: expected root <tagliacarte> or <config>".to_owned())
}

/// Load `config.xml` by streaming bytes into [`XmlParser::receive`] (chunked reads + [`XmlParser::close`]).
pub fn load_tagliacarte_config_from_reader<R: Read>(mut reader: R) -> Result<TagliacarteConfigFile, String> {
    let mut prefix = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if prefix.len() > CONFIG_XML_ROOT_SNIFF_MAX {
            return Err("config.xml: expected root <tagliacarte> or <config>".to_string());
        }
        let n = reader.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            let root = classify_config_xml_final(&prefix)?;
            let mut cur = Cursor::new(prefix);
            return finish_tagliacarte_load(root, &mut cur);
        }
        prefix.extend_from_slice(&tmp[..n]);
        if let Some(root) = classify_config_xml_prefix(&prefix)? {
            let mut chained = Read::chain(Cursor::new(prefix), reader);
            return finish_tagliacarte_load(root, &mut chained);
        }
    }
}

fn finish_tagliacarte_load<R: Read>(
    root: ConfigXmlRootKind,
    reader: &mut R,
) -> Result<TagliacarteConfigFile, String> {
    match root {
        ConfigXmlRootKind::Tagliacarte => {
            let mut h = TagliacarteLoader {
                stack: Vec::new(),
                out: TagliacarteConfigFile::default(),
            };
            let mut p = XmlParser::new(false);
            p.parse_reader_to_close(reader, &mut h)
                .map_err(|e| e.to_string())?;
            Ok(h.out)
        }
        ConfigXmlRootKind::LegacyConfig => {
            let stores = crate::config_xml::load_config_xml_stores_from_reader(reader)?;
            let mut out = TagliacarteConfigFile::default();
            for s in stores {
                out.stores.push(StoreXml {
                    id: s.id.clone(),
                    display_name: s.display_name,
                    store_type: s.store_type,
                    attrs: BTreeMap::new(),
                    transport_refs: Vec::new(),
                    relay_urls: Vec::new(),
                    legacy_connection_uri: Some(s.id),
                    connection_uri_attr: None,
                    last_mail_folder: None,
                    last_mail_message_id: None,
                });
            }
            Ok(out)
        }
    }
}

/// Memory prefix then remainder from [`tokio::fs::File`] (same layout as sync `Cursor::chain`).
struct PrefixedTokioFile {
    prefix: Cursor<Vec<u8>>,
    rest: Option<tokio::fs::File>,
}

impl AsyncRead for PrefixedTokioFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.as_mut().get_mut();
        let pos = this.prefix.position() as usize;
        let v = this.prefix.get_ref();
        if pos < v.len() {
            let n = (v.len() - pos).min(buf.remaining());
            buf.put_slice(&v[pos..pos + n]);
            this.prefix.set_position((pos + n) as u64);
            return Poll::Ready(Ok(()));
        }
        match &mut this.rest {
            Some(f) => Pin::new(f).poll_read(cx, buf),
            None => Poll::Ready(Ok(())),
        }
    }
}

async fn finish_tagliacarte_load_async<R: AsyncRead + Unpin>(
    root: ConfigXmlRootKind,
    reader: &mut R,
) -> Result<TagliacarteConfigFile, String> {
    match root {
        ConfigXmlRootKind::Tagliacarte => {
            let mut h = TagliacarteLoader {
                stack: Vec::new(),
                out: TagliacarteConfigFile::default(),
            };
            let mut p = XmlParser::new(false);
            p.parse_async_read_to_close(reader, &mut h)
                .await
                .map_err(|e| e.to_string())?;
            Ok(h.out)
        }
        ConfigXmlRootKind::LegacyConfig => {
            let stores = crate::config_xml::load_config_xml_stores_from_async_reader(reader).await?;
            let mut out = TagliacarteConfigFile::default();
            for s in stores {
                out.stores.push(StoreXml {
                    id: s.id.clone(),
                    display_name: s.display_name,
                    store_type: s.store_type,
                    attrs: BTreeMap::new(),
                    transport_refs: Vec::new(),
                    relay_urls: Vec::new(),
                    legacy_connection_uri: Some(s.id),
                    connection_uri_attr: None,
                    last_mail_folder: None,
                    last_mail_message_id: None,
                });
            }
            Ok(out)
        }
    }
}

/// Load `config.xml` with [`tokio::fs::File`] and [`XmlParser::parse_async_read_to_close`].
pub async fn load_tagliacarte_config_async(path: &Path) -> Result<TagliacarteConfigFile, String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let mut prefix = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if prefix.len() > CONFIG_XML_ROOT_SNIFF_MAX {
            return Err("config.xml: expected root <tagliacarte> or <config>".to_string());
        }
        let n = file
            .read(&mut tmp)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            let root = classify_config_xml_final(&prefix)?;
            let mut r = PrefixedTokioFile {
                prefix: Cursor::new(prefix),
                rest: None,
            };
            return finish_tagliacarte_load_async(root, &mut r).await;
        }
        prefix.extend_from_slice(&tmp[..n]);
        if let Some(root) = classify_config_xml_prefix(&prefix)? {
            let mut r = PrefixedTokioFile {
                prefix: Cursor::new(prefix),
                rest: Some(file),
            };
            return finish_tagliacarte_load_async(root, &mut r).await;
        }
    }
}

pub fn write_tagliacarte_config(cfg: &TagliacarteConfigFile) -> Result<Vec<u8>, String> {
    let mut w = XmlWriter::with_indent(IndentConfig::spaces(2));
    w.write_xml_declaration();
    w.write_start_element(None, "tagliacarte");

    if let Some(ref id) = cfg.selected_store.store_id {
        w.write_start_element(None, "selected-store");
        w.write_attribute(None, "id", id);
        if let Some(ref f) = cfg.selected_store.legacy_folder {
            w.write_attribute(None, "folder", f);
        }
        if let Some(ref m) = cfg.selected_store.legacy_message_id {
            w.write_attribute(None, "message-id", m);
        }
        w.write_end_element();
    }

    write_pref_empty(&mut w, "security", &cfg.security.attrs)?;
    write_pref_empty(&mut w, "viewing", &cfg.viewing.attrs)?;
    write_pref_empty(&mut w, "composing", &cfg.composing.attrs)?;

    w.write_start_element(None, "transports");
    for t in &cfg.transports {
        write_transport_empty(&mut w, t)?;
    }
    w.write_end_element();

    w.write_start_element(None, "stores");
    for s in &cfg.stores {
        write_store_element(&mut w, s)?;
    }
    w.write_end_element();

    w.write_end_element();
    Ok(w.take_buffer().to_vec())
}

fn write_pref_empty(
    w: &mut XmlWriter,
    name: &str,
    attrs: &BTreeMap<String, String>,
) -> Result<(), String> {
    if attrs.is_empty() {
        return Ok(());
    }
    w.write_start_element(None, name);
    for (k, v) in attrs {
        w.write_attribute(None, k.as_str(), v);
    }
    w.write_end_element();
    Ok(())
}

fn write_transport_empty(w: &mut XmlWriter, t: &TransportXml) -> Result<(), String> {
    w.write_start_element(None, "transport");
    w.write_attribute(None, "id", &t.id);
    w.write_attribute(None, "type", &t.transport_type);
    w.write_attribute(None, "display-name", &t.display_name);
    w.write_attribute(None, "host", &t.host);
    w.write_attribute(None, "port", &t.port.to_string());
    w.write_attribute(None, "security", &t.security);
    if !t.default_from.trim().is_empty() {
        w.write_attribute(None, "default-from", &t.default_from);
    }
    if !t.dsn_notify.trim().is_empty() && t.dsn_notify != "failure" {
        w.write_attribute(None, "dsn-notify", &t.dsn_notify);
    }
    if !t.oauth_provider.trim().is_empty() {
        w.write_attribute(None, "oauth-provider", &t.oauth_provider);
    }
    w.write_end_element();
    Ok(())
}

fn write_store_element(w: &mut XmlWriter, s: &StoreXml) -> Result<(), String> {
    let has_children = !s.transport_refs.is_empty()
        || !s.relay_urls.is_empty()
        || s.last_mail_folder.is_some()
        || s.last_mail_message_id.is_some();

    w.write_start_element(None, "store");
    w.write_attribute(None, "id", &s.id);
    w.write_attribute(None, "type", &s.store_type);
    w.write_attribute(None, "display-name", &s.display_name);
    for (k, v) in &s.attrs {
        w.write_attribute(None, k.as_str(), v);
    }

    if !has_children {
        w.write_end_element();
        return Ok(());
    }

    for r in &s.transport_refs {
        w.write_start_element(None, "transport");
        w.write_attribute(None, "ref", r);
        w.write_end_element();
    }
    for url in &s.relay_urls {
        w.write_start_element(None, "relay");
        w.write_attribute(None, "url", url);
        w.write_end_element();
    }
    if s.last_mail_folder.is_some() || s.last_mail_message_id.is_some() {
        w.write_start_element(None, "last-mail");
        if let Some(ref f) = s.last_mail_folder {
            w.write_attribute(None, "folder", f);
        }
        if let Some(ref m) = s.last_mail_message_id {
            w.write_attribute(None, "message-id", m);
        }
        w.write_end_element();
    }
    w.write_end_element();
    Ok(())
}

pub fn load_tagliacarte_config_from_str(content: &str) -> Result<TagliacarteConfigFile, String> {
    load_tagliacarte_config_from_reader(Cursor::new(content.as_bytes()))
}

struct Frame {
    name: String,
    attrs: BTreeMap<String, String>,
    transport_refs: Vec<String>,
    relay_urls: Vec<String>,
    last_mail_folder: Option<String>,
    last_mail_message_id: Option<String>,
}

struct TagliacarteLoader {
    stack: Vec<Frame>,
    out: TagliacarteConfigFile,
}

impl XmlContentHandler for TagliacarteLoader {
    fn start_element(&mut self, _ns: Option<&str>, local_name: &str) {
        self.stack.push(Frame {
            name: local_name.to_string(),
            attrs: BTreeMap::new(),
            transport_refs: Vec::new(),
            relay_urls: Vec::new(),
            last_mail_folder: None,
            last_mail_message_id: None,
        });
    }

    fn attribute(&mut self, _ns: Option<&str>, local_name: &str, value: &str) {
        if let Some(f) = self.stack.last_mut() {
            f.attrs.insert(local_name.to_string(), value.to_string());
        }
    }

    fn characters(&mut self, text: &str) {
        if text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
            return;
        }
        // Non-whitespace text in config is unexpected; ignore (legacy pretty-print only).
        let _ = text;
    }

    fn end_element(&mut self, _ns: Option<&str>, local_name: &str) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        if frame.name != local_name {
            return;
        }
        match local_name {
            "tagliacarte" => {}
            "transports" | "stores" => {}
            "transport" => {
                if let Some(parent) = self.stack.last_mut() {
                    if parent.name == "store" {
                        if let Some(r) = frame.attrs.get("ref") {
                            parent.transport_refs.push(r.clone());
                        }
                    } else if parent.name == "transports" {
                        if let Some(t) = transport_from_map(&frame.attrs) {
                            self.out.transports.push(t);
                        }
                    }
                }
            }
            "relay" => {
                if let Some(u) = frame.attrs.get("url") {
                    if !u.is_empty() {
                        if let Some(parent) = self.stack.last_mut() {
                            if parent.name == "store" {
                                parent.relay_urls.push(u.clone());
                            }
                        }
                    }
                }
            }
            "last-mail" => {
                if let Some(parent) = self.stack.last_mut() {
                    if parent.name == "store" {
                        parent.last_mail_folder = frame.attrs.get("folder").cloned();
                        parent.last_mail_message_id = frame.attrs.get("message-id").cloned();
                    }
                }
            }
            "store" => {
                if let Some(s) = store_from_maps(
                    &frame.attrs,
                    frame.transport_refs,
                    frame.relay_urls,
                    frame.last_mail_folder,
                    frame.last_mail_message_id,
                ) {
                    self.out.stores.push(s);
                }
            }
            "selected-store" => {
                if let Some(id) = frame.attrs.get("id") {
                    self.out.selected_store.store_id = Some(id.clone());
                }
                if let Some(f) = frame.attrs.get("folder") {
                    self.out.selected_store.legacy_folder = Some(f.clone());
                }
                if let Some(m) = frame.attrs.get("message-id") {
                    self.out.selected_store.legacy_message_id = Some(m.clone());
                }
            }
            "security" => {
                self.out.security.attrs = frame.attrs;
            }
            "viewing" => {
                self.out.viewing.attrs = frame.attrs;
            }
            "composing" => {
                self.out.composing.attrs = frame.attrs;
            }
            _ => {}
        }
    }
}

fn store_from_maps(
    attrs: &BTreeMap<String, String>,
    transport_refs: Vec<String>,
    relay_urls: Vec<String>,
    last_mail_folder: Option<String>,
    last_mail_message_id: Option<String>,
) -> Option<StoreXml> {
    let id = attrs.get("id")?.clone();
    if id.is_empty() {
        return None;
    }
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
    let legacy = if id.contains("://") {
        Some(id.clone())
    } else {
        None
    };
    let connection_uri_attr = attrs.get("connection-uri").cloned();
    let mut rest = BTreeMap::new();
    for (k, v) in attrs {
        if matches!(
            k.as_str(),
            "id" | "type" | "display-name" | "displayName" | "connection-uri"
        ) {
            continue;
        }
        rest.insert(k.clone(), v.clone());
    }
    Some(StoreXml {
        id,
        store_type,
        display_name,
        attrs: rest,
        transport_refs,
        relay_urls,
        legacy_connection_uri: legacy,
        connection_uri_attr,
        last_mail_folder,
        last_mail_message_id,
    })
}

fn transport_from_map(attrs: &BTreeMap<String, String>) -> Option<TransportXml> {
    let id = attrs.get("id")?.clone();
    if id.is_empty() {
        return None;
    }
    let transport_type = attrs
        .get("type")
        .cloned()
        .unwrap_or_else(|| "smtp".to_owned());
    let display_name = attrs
        .get("display-name")
        .or_else(|| attrs.get("displayName"))
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| id.clone());
    let host = attrs.get("host").cloned().unwrap_or_default();
    let port = attrs
        .get("port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(587);
    let security = attrs
        .get("security")
        .cloned()
        .unwrap_or_else(|| "starttls".to_owned());
    let default_from = attrs
        .get("default-from")
        .or_else(|| attrs.get("defaultFrom"))
        .cloned()
        .unwrap_or_default();
    let dsn_notify = attrs
        .get("dsn-notify")
        .or_else(|| attrs.get("dsnNotify"))
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "failure".to_owned());
    let oauth_provider = attrs
        .get("oauth-provider")
        .or_else(|| attrs.get("oauthProvider"))
        .cloned()
        .unwrap_or_default();
    Some(TransportXml {
        id,
        transport_type,
        display_name,
        host,
        port,
        security,
        default_from,
        dsn_notify,
        oauth_provider,
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
    fn transports_paired_open_close_tags_parse() {
        let xml = r#"<tagliacarte>
  <transports>
    <transport id="t1" type="smtp" display-name="SMTP" host="smtp.example.com" port="587" security="starttls">
    </transport>
  </transports>
  <stores>
    <store id="s1" type="imap" display-name="A" host="h" port="993" security="tls">
      <transport ref="t1"/>
    </store>
  </stores>
</tagliacarte>"#;
        let c = load_tagliacarte_config_from_str(xml).unwrap();
        assert_eq!(c.transports.len(), 1);
        assert_eq!(c.transports[0].id, "t1");
        assert_eq!(c.stores[0].transport_refs, vec!["t1".to_string()]);
    }

    #[test]
    fn last_mail_child_round_trips_under_store() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tagliacarte>
  <stores>
    <store id="s1" type="imap" display-name="A" host="h" port="993" security="tls">
      <last-mail folder="INBOX" message-id="uid-9"/>
    </store>
  </stores>
</tagliacarte>"#;
        let c = load_tagliacarte_config_from_str(xml).unwrap();
        assert_eq!(c.stores[0].last_mail_folder.as_deref(), Some("INBOX"));
        assert_eq!(c.stores[0].last_mail_message_id.as_deref(), Some("uid-9"));
        let again = load_tagliacarte_config_from_str(
            &String::from_utf8(write_tagliacarte_config(&c).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(again.stores[0].last_mail_folder, c.stores[0].last_mail_folder);
        assert_eq!(
            again.stores[0].last_mail_message_id,
            c.stores[0].last_mail_message_id
        );
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
    fn store_xml_maildir_and_imap_attrs() {
        let mut maildir_attrs = BTreeMap::new();
        maildir_attrs.insert("path".to_owned(), "/var/mail/me".to_owned());
        let maildir = StoreXml {
            id: "s2".to_owned(),
            store_type: "maildir".to_owned(),
            display_name: "L".to_owned(),
            attrs: maildir_attrs,
            transport_refs: vec![],
            relay_urls: vec![],
            legacy_connection_uri: None,
            connection_uri_attr: None,
            last_mail_folder: None,
            last_mail_message_id: None,
        };
        assert_eq!(
            maildir.attrs.get("path").map(|s| s.as_str()),
            Some("/var/mail/me")
        );

        let mut imap_attrs = BTreeMap::new();
        imap_attrs.insert("username".to_owned(), "u".to_owned());
        imap_attrs.insert("host".to_owned(), "imap.example.com".to_owned());
        imap_attrs.insert("port".to_owned(), "993".to_owned());
        imap_attrs.insert("security".to_owned(), "tls".to_owned());
        let imap = StoreXml {
            id: "s1".to_owned(),
            store_type: "imap".to_owned(),
            display_name: "W".to_owned(),
            attrs: imap_attrs,
            transport_refs: vec![],
            relay_urls: vec![],
            legacy_connection_uri: None,
            connection_uri_attr: None,
            last_mail_folder: None,
            last_mail_message_id: None,
        };
        assert_eq!(
            imap.attrs.get("host").map(|s| s.as_str()),
            Some("imap.example.com")
        );
    }

    #[test]
    fn nostr_store_xml_keeps_npub_attr() {
        let mut a = BTreeMap::new();
        a.insert(
            "npub".to_owned(),
            "npub180cvv07tjdrrgpa0j7z7g0ng0v7yqkm0re8zt".to_owned(),
        );
        let s = StoreXml {
            id: "s3".to_owned(),
            store_type: "nostr".to_owned(),
            display_name: "N".to_owned(),
            attrs: a,
            transport_refs: vec![],
            relay_urls: vec!["wss://relay.damus.io".to_owned()],
            legacy_connection_uri: None,
            connection_uri_attr: None,
            last_mail_folder: None,
            last_mail_message_id: None,
        };
        assert!(s.attrs.contains_key("npub"));
    }
}
