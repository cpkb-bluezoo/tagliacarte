/*
 * config_persist.rs
 * Copyright (C) 2026 Chris Burdess
 */

//! Build and merge `TagliacarteConfigFile` for `config.xml` (single on-disk source of truth).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use tagliacarte_core::tagliacarte_config_xml::{StoreXml, TagliacarteConfigFile, TransportXml};
use url::Url;

use super::{FrbAccount, FrbConfig, FrbTransport};

/// `config.xml` in the same directory as the primary config path (Flutter passes `…/config.xml`).
pub(super) fn config_xml_beside_primary(primary_config_path: &str) -> Option<PathBuf> {
    Path::new(primary_config_path)
        .parent()
        .map(|p| p.join("config.xml"))
}

pub(super) fn try_load_tagliacarte_xml(
    primary_config_path: &str,
    fallback_xml: Option<PathBuf>,
) -> Option<TagliacarteConfigFile> {
    if let Some(p) = config_xml_beside_primary(primary_config_path) {
        if p.is_file() {
            if let Ok(f) = tagliacarte_core::tagliacarte_config_xml::load_tagliacarte_config(&p) {
                if !f.stores.is_empty() {
                    return Some(f);
                }
            }
        }
    }
    if let Some(p) = fallback_xml {
        if p.is_file() {
            if let Ok(f) = tagliacarte_core::tagliacarte_config_xml::load_tagliacarte_config(&p) {
                if !f.stores.is_empty() {
                    return Some(f);
                }
            }
        }
    }
    None
}

pub(super) fn apply_tagliacarte_file(
    cfg: &mut FrbConfig,
    file: &TagliacarteConfigFile,
) -> Result<(), String> {
    let mut transports = Vec::with_capacity(file.transports.len());
    for t in &file.transports {
        transports.push(frb_transport_from_xml(t)?);
    }
    let mut accounts = Vec::with_capacity(file.stores.len());
    for s in &file.stores {
        accounts.push(frb_account_from_store(s, file)?);
    }
    cfg.transports = transports;
    cfg.accounts = accounts;
    cfg.selected_store_id = file.selected_store.store_id.clone();
    migrate_legacy_selected_store_mail_location(cfg, file);
    Ok(())
}

/// Older configs stored folder/message on `<selected-store>`; merge into the matching account when
/// the store has no `<last-mail>` data.
fn migrate_legacy_selected_store_mail_location(cfg: &mut FrbConfig, file: &TagliacarteConfigFile) {
    let Some(sid) = file.selected_store.store_id.as_deref() else {
        return;
    };
    if file.selected_store.legacy_folder.is_none() && file.selected_store.legacy_message_id.is_none()
    {
        return;
    }
    let Some(acc) = cfg.accounts.iter_mut().find(|a| a.id == sid) else {
        return;
    };
    if acc.last_folder.is_some() || acc.last_message_id.is_some() {
        return;
    }
    acc.last_folder = file.selected_store.legacy_folder.clone();
    acc.last_message_id = file.selected_store.legacy_message_id.clone();
}

/// Full [FrbConfig] from a parsed `config.xml` (stores, transports, UI prefs).
pub(super) fn frb_config_from_tagliacarte_file(file: &TagliacarteConfigFile) -> FrbConfig {
    let mut cfg = FrbConfig::default();
    let _ = apply_tagliacarte_file(&mut cfg, file);
    apply_prefs_from_tagliacarte_file(&mut cfg, file);
    cfg
}

fn parse_bool_attr(s: &str) -> bool {
    matches!(
        s.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn bool_attr(b: bool) -> String {
    if b {
        "true".to_owned()
    } else {
        "false".to_owned()
    }
}

fn apply_prefs_from_tagliacarte_file(cfg: &mut FrbConfig, file: &TagliacarteConfigFile) {
    if let Some(v) = file.security.attrs.get("use-keychain") {
        cfg.use_keychain = parse_bool_attr(v);
    } else if let Some(v) = file.security.attrs.get("password-storage") {
        cfg.use_keychain = v.eq_ignore_ascii_case("keychain");
    }

    if let Some(v) = file.viewing.attrs.get("date-format") {
        cfg.date_format = v.clone();
    }
    if let Some(v) = file.viewing.attrs.get("resource-policy") {
        cfg.resource_policy = v.clone();
    }
    if let Some(v) = file.viewing.attrs.get("load-remote-images") {
        cfg.load_remote_images = parse_bool_attr(v);
    }
    if let Some(v) = file.viewing.attrs.get("threaded-view") {
        cfg.threaded_view = parse_bool_attr(v);
    }
    if let Some(v) = file.viewing.attrs.get("message-list-sort") {
        if !v.is_empty() {
            cfg.message_list_sort = v.clone();
        }
    }
    if let Some(v) = file.viewing.attrs.get("notify-new-messages") {
        cfg.notify_new_messages = parse_bool_attr(v);
    }

    if let Some(v) = file.composing.attrs.get("quote-original") {
        cfg.quote_original = parse_bool_attr(v);
    }
    if let Some(v) = file.composing.attrs.get("delete-mode") {
        cfg.delete_mode = v.clone();
    }
    if let Some(v) = file.composing.attrs.get("trash-folder-name") {
        cfg.trash_folder_name = v.clone();
    }
}

fn push_frb_prefs_into_file(file: &mut TagliacarteConfigFile, cfg: &FrbConfig) {
    file.security.attrs.insert("use-keychain".to_owned(), bool_attr(cfg.use_keychain));

    file.viewing.attrs.insert("date-format".to_owned(), cfg.date_format.clone());
    file
        .viewing
        .attrs
        .insert("resource-policy".to_owned(), cfg.resource_policy.clone());
    file.viewing.attrs.insert(
        "load-remote-images".to_owned(),
        bool_attr(cfg.load_remote_images),
    );
    file.viewing
        .attrs
        .insert("threaded-view".to_owned(), bool_attr(cfg.threaded_view));
    file.viewing.attrs.insert(
        "message-list-sort".to_owned(),
        cfg.message_list_sort.clone(),
    );
    file.viewing.attrs.insert(
        "notify-new-messages".to_owned(),
        bool_attr(cfg.notify_new_messages),
    );

    file.composing.attrs.insert(
        "quote-original".to_owned(),
        bool_attr(cfg.quote_original),
    );
    file.composing
        .attrs
        .insert("delete-mode".to_owned(), cfg.delete_mode.clone());
    file.composing.attrs.insert(
        "trash-folder-name".to_owned(),
        cfg.trash_folder_name.clone(),
    );
}

pub(super) fn merge_pref_attr_maps(
    existing: BTreeMap<String, String>,
    from_frb: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut m = existing;
    m.extend(from_frb);
    m
}

fn frb_transport_from_xml(t: &TransportXml) -> Result<FrbTransport, String> {
    Ok(FrbTransport {
        id: t.id.clone(),
        transport_type: t.transport_type.clone(),
        display_name: t.display_name.clone(),
        host: t.host.clone(),
        port: t.port,
        security: t.security.clone(),
        transport_uri: t.smtp_connection_uri(),
    })
}

fn frb_account_from_store(
    s: &StoreXml,
    file: &TagliacarteConfigFile,
) -> Result<FrbAccount, String> {
    let store_uri = s.connection_uri()?;
    let transport_uri = s
        .transport_refs
        .first()
        .and_then(|tid| file.transports.iter().find(|x| x.id == *tid))
        .map(|x| x.smtp_connection_uri());
    Ok(FrbAccount {
        id: s.id.clone(),
        label: s.display_name.clone(),
        backend_type: s.store_type.clone(),
        store_uri,
        transport_ids: s.transport_refs.clone(),
        transport_uri,
        username: s.username.clone(),
        host: s.host.clone(),
        port: s.port,
        security: s.security.clone(),
        path: s.path.clone(),
        email: s.username.clone(),
        avatar_url: None,
        last_folder: s.last_mail_folder.clone(),
        last_message_id: s.last_mail_message_id.clone(),
        imap_idle_min_idle_seconds: s.imap_idle_min_idle_seconds,
    })
}

/// Build XML document from merged FRB config (UI prefs as attributes on `<security>` / `<viewing>` / `<composing>`).
pub(super) fn tagliacarte_file_from_frb(cfg: &FrbConfig) -> Result<TagliacarteConfigFile, String> {
    let mut transports = Vec::with_capacity(cfg.transports.len());
    for t in &cfg.transports {
        transports.push(transport_xml_from_frb(t)?);
    }
    let mut stores = Vec::with_capacity(cfg.accounts.len());
    for a in &cfg.accounts {
        stores.push(account_to_store_xml(a)?);
    }
    let mut file = TagliacarteConfigFile {
        transports,
        stores,
        selected_store: tagliacarte_core::tagliacarte_config_xml::SelectedStoreXml {
            store_id: cfg.selected_store_id.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    push_frb_prefs_into_file(&mut file, cfg);
    Ok(file)
}

fn transport_xml_from_frb(t: &FrbTransport) -> Result<TransportXml, String> {
    Ok(TransportXml {
        id: t.id.clone(),
        transport_type: t.transport_type.clone(),
        display_name: t.display_name.clone(),
        host: t.host.clone(),
        port: t.port,
        security: t.security.clone(),
    })
}

fn account_to_store_xml(a: &FrbAccount) -> Result<StoreXml, String> {
    if a.username.is_some()
        || a.host.is_some()
        || a.port.is_some()
        || a.path.is_some()
        || a.security.is_some()
    {
        return Ok(StoreXml {
            id: a.id.clone(),
            store_type: a.backend_type.clone(),
            display_name: a.label.clone(),
            username: a.username.clone(),
            host: a.host.clone(),
            port: a.port,
            security: a.security.clone(),
            path: a.path.clone(),
            transport_refs: a.transport_ids.clone(),
            legacy_connection_uri: None,
            connection_uri_attr: None,
            last_mail_folder: a.last_folder.clone(),
            last_mail_message_id: a.last_message_id.clone(),
            imap_idle_min_idle_seconds: a.imap_idle_min_idle_seconds,
        });
    }
    infer_store_xml_from_uri(a)
}

fn infer_store_xml_from_uri(a: &FrbAccount) -> Result<StoreXml, String> {
    let u = Url::parse(a.store_uri.trim()).map_err(|e| e.to_string())?;
    let scheme = u.scheme();
    match scheme {
        "maildir" | "mbox" => {
            let path = u.path();
            if path.is_empty() {
                return Err("maildir/mbox URL has no path".to_owned());
            }
            Ok(StoreXml {
                id: a.id.clone(),
                store_type: if scheme == "mbox" {
                    "mbox".to_owned()
                } else {
                    "maildir".to_owned()
                },
                display_name: a.label.clone(),
                username: None,
                host: None,
                port: None,
                security: None,
                path: Some(path.to_owned()),
                transport_refs: a.transport_ids.clone(),
                legacy_connection_uri: None,
                connection_uri_attr: None,
                last_mail_folder: a.last_folder.clone(),
                last_mail_message_id: a.last_message_id.clone(),
                imap_idle_min_idle_seconds: a.imap_idle_min_idle_seconds,
            })
        }
        "imap" | "imaps" => {
            let host = u
                .host_str()
                .ok_or_else(|| "IMAP URL missing host".to_owned())?
                .to_owned();
            let port = u
                .port()
                .unwrap_or(if scheme == "imaps" { 993 } else { 143 });
            let user = percent_decode_str(u.username())
                .decode_utf8_lossy()
                .into_owned();
            let security = if scheme == "imaps" || port == 993 {
                Some("tls".to_owned())
            } else {
                Some("starttls".to_owned())
            };
            Ok(StoreXml {
                id: a.id.clone(),
                store_type: "imap".to_owned(),
                display_name: a.label.clone(),
                username: if user.is_empty() { None } else { Some(user) },
                host: Some(host),
                port: Some(port),
                security,
                path: None,
                transport_refs: a.transport_ids.clone(),
                legacy_connection_uri: None,
                connection_uri_attr: None,
                last_mail_folder: a.last_folder.clone(),
                last_mail_message_id: a.last_message_id.clone(),
                imap_idle_min_idle_seconds: a.imap_idle_min_idle_seconds,
            })
        }
        _ => Ok(StoreXml {
            id: a.id.clone(),
            store_type: a.backend_type.clone(),
            display_name: a.label.clone(),
            username: None,
            host: None,
            port: None,
            security: None,
            path: None,
            transport_refs: a.transport_ids.clone(),
            legacy_connection_uri: None,
            connection_uri_attr: Some(a.store_uri.clone()),
            last_mail_folder: a.last_folder.clone(),
            last_mail_message_id: a.last_message_id.clone(),
            imap_idle_min_idle_seconds: a.imap_idle_min_idle_seconds,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frb_prefs_round_trip_via_xml_attrs() {
        let mut cfg = FrbConfig::default();
        cfg.use_keychain = false;
        cfg.date_format = "yyyy/MM/dd".to_owned();
        cfg.message_list_sort = "subject_asc".to_owned();
        cfg.delete_mode = "Hard delete".to_owned();
        let file = tagliacarte_file_from_frb(&cfg).unwrap();
        let out = frb_config_from_tagliacarte_file(&file);
        assert!(!out.use_keychain);
        assert_eq!(out.date_format, "yyyy/MM/dd");
        assert_eq!(out.message_list_sort, "subject_asc");
        assert_eq!(out.delete_mode, "Hard delete");
    }

    #[test]
    fn frb_mail_location_round_trip_via_store_last_mail_child() {
        let mut cfg = FrbConfig::default();
        cfg.selected_store_id = Some("s1".to_owned());
        cfg.accounts.push(FrbAccount {
            id: "s1".to_owned(),
            label: "A".to_owned(),
            backend_type: "imap".to_owned(),
            store_uri: "imaps://u@imap.example.com:993".to_owned(),
            transport_ids: vec![],
            transport_uri: None,
            username: Some("u".to_owned()),
            host: Some("imap.example.com".to_owned()),
            port: Some(993),
            security: Some("tls".to_owned()),
            path: None,
            email: Some("u".to_owned()),
            avatar_url: None,
            last_folder: Some("INBOX".to_owned()),
            last_message_id: Some("uid-42".to_owned()),
            imap_idle_min_idle_seconds: None,
        });
        let file = tagliacarte_file_from_frb(&cfg).unwrap();
        let s1 = file.stores.iter().find(|s| s.id == "s1").unwrap();
        assert_eq!(s1.last_mail_folder.as_deref(), Some("INBOX"));
        assert_eq!(s1.last_mail_message_id.as_deref(), Some("uid-42"));
        let out = frb_config_from_tagliacarte_file(&file);
        let acc = out.accounts.iter().find(|a| a.id == "s1").unwrap();
        assert_eq!(acc.last_folder.as_deref(), Some("INBOX"));
        assert_eq!(acc.last_message_id.as_deref(), Some("uid-42"));
    }
}
