/*
 * config_persist.rs
 * Copyright (C) 2026 Chris Burdess
 */

//! Build and merge `TagliacarteConfigFile` for `config.xml` (single on-disk source of truth).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use tagliacarte_core::tagliacarte_config_xml::{StoreXml, TagliacarteConfigFile, TransportXml};

use crate::legacy_store_uri::migrate_store_xml_legacy_uri;
use crate::mail_kind::{normalize_store_type, normalize_transport_type};

use super::{FrbAccount, FrbConfig, FrbTransport};

/// `config.xml` in the same directory as the primary config path (Flutter passes `…/config.xml`).
pub(super) fn config_xml_beside_primary(primary_config_path: &str) -> Option<PathBuf> {
    Path::new(primary_config_path)
        .parent()
        .map(|p| p.join("config.xml"))
}

pub(super) async fn try_load_tagliacarte_xml(
    primary_config_path: &str,
    fallback_xml: Option<PathBuf>,
) -> Option<TagliacarteConfigFile> {
    if let Some(p) = config_xml_beside_primary(primary_config_path) {
        if tokio::fs::metadata(&p)
            .await
            .ok()
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            if let Ok(f) =
                tagliacarte_core::tagliacarte_config_xml::load_tagliacarte_config_async(&p).await
            {
                if !f.stores.is_empty() {
                    return Some(f);
                }
            }
        }
    }
    if let Some(p) = fallback_xml {
        if tokio::fs::metadata(&p)
            .await
            .ok()
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            if let Ok(f) =
                tagliacarte_core::tagliacarte_config_xml::load_tagliacarte_config_async(&p).await
            {
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
        let mut sc = s.clone();
        migrate_store_xml_legacy_uri(&mut sc)?;
        accounts.push(frb_account_from_store(&sc, file)?);
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
    if let Err(e) = apply_tagliacarte_file(&mut cfg, file) {
        eprintln!("tagliacarte: apply_tagliacarte_file failed (stores/transports not loaded): {e}");
    }
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
    if let Some(v) = file.composing.attrs.get("reply-header-template") {
        cfg.reply_header_template = v.clone();
    }
    if let Some(v) = file.composing.attrs.get("reply-date-format") {
        cfg.reply_date_format = v.clone();
    }
    if let Some(v) = file.composing.attrs.get("reply-time-format") {
        cfg.reply_time_format = v.clone();
    }
    if let Some(v) = file.composing.attrs.get("reply-line-prefix") {
        cfg.reply_line_prefix = v.clone();
    }
    if let Some(v) = file.composing.attrs.get("reply-quote-mode") {
        cfg.reply_quote_mode = v.clone();
    }
    if let Some(v) = file.composing.attrs.get("reply-plain-position") {
        if !v.is_empty() {
            cfg.reply_plain_position = v.clone();
        }
    }
    if let Some(v) = file.composing.attrs.get("compose-use-rich-text") {
        cfg.compose_use_rich_text = parse_bool_attr(v);
    }
    if let Some(v) = file.composing.attrs.get("matrix-chat-use-rich-text") {
        cfg.matrix_chat_use_rich_text = parse_bool_attr(v);
    }
    let legacy_delete = file
        .composing
        .attrs
        .get("delete-mode")
        .map(|s| s.as_str());
    let legacy_trash = file
        .composing
        .attrs
        .get("trash-folder-name")
        .map(|s| s.as_str());
    migrate_legacy_global_delete_prefs_to_imap_accounts(cfg, legacy_delete, legacy_trash);
}

/// One-time migration: old global `<composing delete-mode="…" trash-folder-name="…">` → each
/// IMAP/Gmail account’s `imapDeleteMode` / `imapTrashFolderName` when not already set.
pub(super) fn migrate_legacy_global_delete_prefs_to_imap_accounts(
    cfg: &mut FrbConfig,
    legacy_mode: Option<&str>,
    legacy_trash: Option<&str>,
) {
    let mode = legacy_mode.map(str::trim).filter(|s| !s.is_empty());
    let trash = legacy_trash.map(str::trim).filter(|s| !s.is_empty());
    if mode.is_none() && trash.is_none() {
        return;
    }
    for acc in &mut cfg.accounts {
        if !crate::mail_kind::is_imap_like_store(&acc.backend_type) {
            continue;
        }
        if let Some(m) = mode {
            if !acc.attrs.contains_key("imapDeleteMode") {
                acc.attrs.insert("imapDeleteMode".to_string(), m.to_string());
            }
        }
        if let Some(t) = trash {
            if !acc.attrs.contains_key("imapTrashFolderName") {
                acc.attrs.insert("imapTrashFolderName".to_string(), t.to_string());
            }
        }
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
    file.composing.attrs.insert(
        "reply-header-template".to_owned(),
        cfg.reply_header_template.clone(),
    );
    file.composing.attrs.insert(
        "reply-date-format".to_owned(),
        cfg.reply_date_format.clone(),
    );
    file.composing.attrs.insert(
        "reply-time-format".to_owned(),
        cfg.reply_time_format.clone(),
    );
    file.composing
        .attrs
        .insert("reply-line-prefix".to_owned(), cfg.reply_line_prefix.clone());
    file.composing
        .attrs
        .insert("reply-quote-mode".to_owned(), cfg.reply_quote_mode.clone());
    file.composing.attrs.insert(
        "reply-plain-position".to_owned(),
        cfg.reply_plain_position.clone(),
    );
    file.composing.attrs.insert(
        "compose-use-rich-text".to_owned(),
        bool_attr(cfg.compose_use_rich_text),
    );
    file.composing.attrs.insert(
        "matrix-chat-use-rich-text".to_owned(),
        bool_attr(cfg.matrix_chat_use_rich_text),
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

/// XML attribute name on `<store>` → JSON / FrbAccount [FrbAccount::attrs] key (camelCase where needed).
fn xml_store_attr_to_frb_key(xml_key: &str) -> String {
    match xml_key {
        "imap-idle-min-idle-seconds" => "imapIdleMinIdleSeconds".to_owned(),
        "imap-delete-mode" => "imapDeleteMode".to_owned(),
        "imap-trash-folder-name" => "imapTrashFolderName".to_owned(),
        "imap-junk-folder-name" => "imapJunkFolderName".to_owned(),
        "imap-sent-folder-name" => "imapSentFolderName".to_owned(),
        "imap-drafts-folder-name" => "imapDraftsFolderName".to_owned(),
        "imap-mirror-sent-if-missing" => "imapMirrorSentIfMissing".to_owned(),
        "draft-autosave-seconds" => "draftAutosaveSeconds".to_owned(),
        "gmail-trash-label-id" => "gmailTrashLabelId".to_owned(),
        "gmail-spam-label-id" => "gmailSpamLabelId".to_owned(),
        "gmail-sent-label-id" => "gmailSentLabelId".to_owned(),
        "gmail-draft-label-id" => "gmailDraftLabelId".to_owned(),
        "gmail-inbox-label-id" => "gmailInboxLabelId".to_owned(),
        "maildir-delete-mode" => "maildirDeleteMode".to_owned(),
        "maildir-trash-folder-name" => "maildirTrashFolderName".to_owned(),
        "maildir-junk-folder-name" => "maildirJunkFolderName".to_owned(),
        k => k.to_owned(),
    }
}

/// FrbAccount attrs key → XML `<store>` attribute name.
fn frb_attr_to_xml_store_key(frb_key: &str) -> String {
    match frb_key {
        "imapIdleMinIdleSeconds" => "imap-idle-min-idle-seconds".to_owned(),
        "imapDeleteMode" => "imap-delete-mode".to_owned(),
        "imapTrashFolderName" => "imap-trash-folder-name".to_owned(),
        "imapJunkFolderName" => "imap-junk-folder-name".to_owned(),
        "imapSentFolderName" => "imap-sent-folder-name".to_owned(),
        "imapDraftsFolderName" => "imap-drafts-folder-name".to_owned(),
        "imapMirrorSentIfMissing" => "imap-mirror-sent-if-missing".to_owned(),
        "draftAutosaveSeconds" => "draft-autosave-seconds".to_owned(),
        "gmailTrashLabelId" => "gmail-trash-label-id".to_owned(),
        "gmailSpamLabelId" => "gmail-spam-label-id".to_owned(),
        "gmailSentLabelId" => "gmail-sent-label-id".to_owned(),
        "gmailDraftLabelId" => "gmail-draft-label-id".to_owned(),
        "gmailInboxLabelId" => "gmail-inbox-label-id".to_owned(),
        "maildirDeleteMode" => "maildir-delete-mode".to_owned(),
        "maildirTrashFolderName" => "maildir-trash-folder-name".to_owned(),
        "maildirJunkFolderName" => "maildir-junk-folder-name".to_owned(),
        "transportUri" => return String::new(), // not persisted on store element
        k => k.to_owned(),
    }
}

fn frb_transport_from_xml(t: &TransportXml) -> Result<FrbTransport, String> {
    Ok(FrbTransport {
        id: t.id.clone(),
        transport_type: normalize_transport_type(&t.transport_type),
        display_name: t.display_name.clone(),
        host: t.host.clone(),
        port: t.port,
        security: t.security.clone(),
        default_from: t.default_from.clone(),
        dsn_notify: t.dsn_notify.clone(),
        oauth_provider: t.oauth_provider.clone(),
    })
}

fn frb_account_from_store(
    s: &StoreXml,
    _file: &TagliacarteConfigFile,
) -> Result<FrbAccount, String> {
    let mut attrs = HashMap::new();
    for (k, v) in &s.attrs {
        attrs.insert(xml_store_attr_to_frb_key(k), v.clone());
    }
    let t = normalize_store_type(&s.store_type);
    if (t == "gmail" || t == "graph" || t == "exchange") && !attrs.contains_key("email") {
        if let Some(u) = attrs.get("username").cloned() {
            attrs.insert("email".to_owned(), u);
        }
    }
    if t == "gmail" {
        migrate_gmail_rest_attrs(&mut attrs);
    }
    let mut lists = HashMap::new();
    if !s.transport_refs.is_empty() {
        lists.insert("transportIds".to_owned(), s.transport_refs.clone());
    }
    if !s.relay_urls.is_empty() {
        lists.insert("relayUrls".to_owned(), s.relay_urls.clone());
    }
    Ok(FrbAccount {
        id: s.id.clone(),
        label: s.display_name.clone(),
        backend_type: t,
        avatar_url: None,
        last_folder: s.last_mail_folder.clone(),
        last_message_id: s.last_mail_message_id.clone(),
        attrs,
        lists,
    })
}

fn migrate_gmail_rest_attrs(attrs: &mut HashMap<String, String>) {
    // Gmail REST no longer uses IMAP host/port/security fields.
    attrs.remove("host");
    attrs.remove("port");
    attrs.remove("security");
    attrs.remove("imapDeleteMode");

    // Backfill canonical Gmail system label IDs for stable behavior.
    if !attrs.contains_key("gmailTrashLabelId") {
        attrs.insert("gmailTrashLabelId".to_owned(), "TRASH".to_owned());
    }
    if !attrs.contains_key("gmailSpamLabelId") {
        attrs.insert("gmailSpamLabelId".to_owned(), "SPAM".to_owned());
    }
    if !attrs.contains_key("gmailSentLabelId") {
        attrs.insert("gmailSentLabelId".to_owned(), "SENT".to_owned());
    }
    if !attrs.contains_key("gmailDraftLabelId") {
        attrs.insert("gmailDraftLabelId".to_owned(), "DRAFT".to_owned());
    }
    if !attrs.contains_key("gmailInboxLabelId") {
        attrs.insert("gmailInboxLabelId".to_owned(), "INBOX".to_owned());
    }

    // Best-effort migration from legacy folder-name keys.
    if !attrs.contains_key("gmailTrashLabelId") {
        let t = attrs
            .get("imapTrashFolderName")
            .map(|s| s.trim().to_ascii_uppercase())
            .unwrap_or_default();
        if t == "TRASH" {
            attrs.insert("gmailTrashLabelId".to_owned(), "TRASH".to_owned());
        }
    }
    if !attrs.contains_key("gmailSpamLabelId") {
        let j = attrs
            .get("imapJunkFolderName")
            .map(|s| s.trim().to_ascii_uppercase())
            .unwrap_or_default();
        if j == "SPAM" || j == "JUNK" {
            attrs.insert("gmailSpamLabelId".to_owned(), "SPAM".to_owned());
        }
    }
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
        default_from: t.default_from.clone(),
        dsn_notify: if t.dsn_notify.trim().is_empty() {
            "failure".to_owned()
        } else {
            t.dsn_notify.clone()
        },
        oauth_provider: t.oauth_provider.trim().to_ascii_lowercase(),
    })
}

fn account_to_store_xml(a: &FrbAccount) -> Result<StoreXml, String> {
    let has_structured = !a.attrs.is_empty()
        || a.lists
            .get("transportIds")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
        || a.lists
            .get("relayUrls")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    if !has_structured {
        return Err(format!(
            "account {:?} has no attrs or transport/relay lists to persist",
            a.id
        ));
    }
    let mut xml_attrs = BTreeMap::new();
    for (k, v) in &a.attrs {
        if k == "transportUri" {
            continue;
        }
        let xk = frb_attr_to_xml_store_key(k);
        if !xk.is_empty() {
            xml_attrs.insert(xk, v.clone());
        }
    }
    let transport_refs = if normalize_store_type(&a.backend_type) == "gmail"
        || normalize_store_type(&a.backend_type) == "graph"
        || normalize_store_type(&a.backend_type) == "exchange"
    {
        Vec::new()
    } else {
        a.lists
            .get("transportIds")
            .cloned()
            .unwrap_or_default()
    };
    let relay_urls = a.lists.get("relayUrls").cloned().unwrap_or_default();
    Ok(StoreXml {
        id: a.id.clone(),
        store_type: normalize_store_type(&a.backend_type),
        display_name: a.label.clone(),
        attrs: xml_attrs,
        transport_refs,
        relay_urls,
        legacy_connection_uri: None,
        connection_uri_attr: None,
        last_mail_folder: a.last_folder.clone(),
        last_mail_message_id: a.last_message_id.clone(),
    })
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
        let file = tagliacarte_file_from_frb(&cfg).unwrap();
        let out = frb_config_from_tagliacarte_file(&file);
        assert!(!out.use_keychain);
        assert_eq!(out.date_format, "yyyy/MM/dd");
        assert_eq!(out.message_list_sort, "subject_asc");
    }

    #[test]
    fn frb_mail_location_round_trip_via_store_last_mail_child() {
        let mut cfg = FrbConfig::default();
        cfg.selected_store_id = Some("s1".to_owned());
        let attrs = HashMap::from([
            ("username".to_owned(), "u".to_owned()),
            ("host".to_owned(), "imap.example.com".to_owned()),
            ("port".to_owned(), "993".to_owned()),
            ("security".to_owned(), "tls".to_owned()),
            ("email".to_owned(), "u".to_owned()),
        ]);
        cfg.accounts.push(FrbAccount {
            id: "s1".to_owned(),
            label: "A".to_owned(),
            backend_type: "imap".to_owned(),
            avatar_url: None,
            last_folder: Some("INBOX".to_owned()),
            last_message_id: Some("uid-42".to_owned()),
            attrs,
            lists: HashMap::new(),
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

    #[test]
    fn gmail_store_migrates_to_rest_label_attrs_and_embedded_transport() {
        let mut s = StoreXml {
            id: "g1".to_owned(),
            store_type: "gmail".to_owned(),
            display_name: "Gmail".to_owned(),
            attrs: BTreeMap::new(),
            transport_refs: vec![],
            relay_urls: vec![],
            legacy_connection_uri: None,
            connection_uri_attr: None,
            last_mail_folder: None,
            last_mail_message_id: None,
        };
        s.attrs.insert("host".to_owned(), "imap.gmail.com".to_owned());
        s.attrs.insert("port".to_owned(), "993".to_owned());
        s.attrs.insert("security".to_owned(), "tls".to_owned());
        s.attrs
            .insert("imap-delete-mode".to_owned(), "Move to Trash".to_owned());
        s.attrs
            .insert("imap-trash-folder-name".to_owned(), "Trash".to_owned());
        s.attrs
            .insert("imap-junk-folder-name".to_owned(), "Spam".to_owned());
        s.transport_refs = vec!["t1".to_owned()];
        let file = TagliacarteConfigFile {
            stores: vec![s],
            ..Default::default()
        };

        let acc = frb_account_from_store(&file.stores[0], &file).unwrap();
        assert_eq!(acc.backend_type, "gmail");
        assert_eq!(
            acc.attrs.get("gmailTrashLabelId").map(String::as_str),
            Some("TRASH")
        );
        assert_eq!(
            acc.attrs.get("gmailSpamLabelId").map(String::as_str),
            Some("SPAM")
        );
        assert!(acc.attrs.get("host").is_none());
        assert!(acc.attrs.get("port").is_none());
        assert!(acc.attrs.get("security").is_none());

        let out_store = account_to_store_xml(&acc).unwrap();
        assert!(out_store.transport_refs.is_empty());
        assert_eq!(
            out_store
                .attrs
                .get("gmail-trash-label-id")
                .map(String::as_str),
            Some("TRASH")
        );
    }
}
