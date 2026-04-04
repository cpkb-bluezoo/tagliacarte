/*
 * frb_json.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Parse/format FRB config JSON with tagliacarte_core::json (camelCase, same defaults as former serde).
 *
 * Key checklist (acceptance / parity with former #[serde(rename_all = "camelCase")] + defaults):
 * - FrbConfig root: accounts, transports, selectedStoreId?, dateFormat, resourcePolicy, useKeychain,
 *   loadRemoteImages, threadedView, quoteOriginal, deleteMode, trashFolderName, messageListSort,
 *   notifyNewMessages
 *   (default date_desc). Root `selectedFolder` / `selectedMessageId` are legacy and merged into the
 *   account matching `selectedStoreId` when that account has no `lastFolder` / `lastMessageId`.
 * - FrbAccount: …, lastFolder?, lastMessageId? (per-store last mail UI location).
 * - FrbTransport: id, transportType, displayName, host, port, security, transportUri
 */

use tagliacarte_core::json::{
    JsonContentHandler, JsonError, JsonNumber, JsonWriter, parse_str_complete, writer_into_string,
};

use super::{FrbAccount, FrbConfig, FrbTransport};

// --- FrbConfig parse -----------------------------------------------------------------------------

pub enum CfgStack {
    Root {
        key: Option<String>,
    },
    InAccountsArray,
    InAccount {
        acc: FrbAccount,
        key: Option<String>,
        in_transport_ids: bool,
    },
    InTransportsArray,
    InTransport {
        t: FrbTransport,
        key: Option<String>,
    },
}

pub struct FrbConfigParse {
    pub config: FrbConfig,
    pub stack: Vec<CfgStack>,
    pub err: Option<String>,
    /// Exposed for flutter_rust_bridge generated codecs.
    pub json_legacy_root_folder: Option<String>,
    pub json_legacy_root_message_id: Option<String>,
}

impl FrbConfigParse {
    /// Used by flutter_rust_bridge generated `SseDecode` / `CstDecode`; JSON parse uses [Self::new] and fills legacy fields during streaming.
    pub fn from_bridge_fields(
        config: FrbConfig,
        stack: Vec<CfgStack>,
        err: Option<String>,
    ) -> Self {
        Self {
            config,
            stack,
            err,
            json_legacy_root_folder: None,
            json_legacy_root_message_id: None,
        }
    }

    pub fn new() -> Self {
        Self {
            config: FrbConfig::default(),
            stack: Vec::new(),
            err: None,
            json_legacy_root_folder: None,
            json_legacy_root_message_id: None,
        }
    }

    fn set_err(&mut self, msg: impl Into<String>) {
        if self.err.is_none() {
            self.err = Some(msg.into());
        }
    }

    pub fn into_result(mut self) -> Result<FrbConfig, String> {
        if let Some(e) = self.err {
            return Err(e);
        }
        if let Some(sid) = self.config.selected_store_id.clone() {
            if let Some(acc) = self.config.accounts.iter_mut().find(|a| a.id == sid) {
                if acc.last_folder.is_none() {
                    acc.last_folder = self.json_legacy_root_folder.take();
                }
                if acc.last_message_id.is_none() {
                    acc.last_message_id = self.json_legacy_root_message_id.take();
                }
            }
        }
        Ok(self.config)
    }
}

impl Default for FrbConfigParse {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonContentHandler for FrbConfigParse {
    fn start_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.stack.last() {
            None => self.stack.push(CfgStack::Root { key: None }),
            Some(CfgStack::InAccountsArray) => self.stack.push(CfgStack::InAccount {
                acc: FrbAccount::default(),
                key: None,
                in_transport_ids: false,
            }),
            Some(CfgStack::InTransportsArray) => self.stack.push(CfgStack::InTransport {
                t: FrbTransport {
                    id: String::new(),
                    transport_type: String::new(),
                    display_name: String::new(),
                    host: String::new(),
                    port: 0,
                    security: String::new(),
                    transport_uri: String::new(),
                },
                key: None,
            }),
            _ => self.set_err("unexpected '{' in FrbConfig JSON"),
        }
    }

    fn end_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.stack.pop() {
            Some(CfgStack::InAccount { acc, .. }) => self.config.accounts.push(acc),
            Some(CfgStack::InTransport { t, .. }) => self.config.transports.push(t),
            Some(CfgStack::Root { .. }) => {}
            _ => self.set_err("unexpected '}' in FrbConfig JSON"),
        }
    }

    fn start_array(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.stack.last_mut() {
            Some(CfgStack::Root { key }) => {
                let k = key.take();
                match k.as_deref() {
                    Some("accounts") => self.stack.push(CfgStack::InAccountsArray),
                    Some("transports") => self.stack.push(CfgStack::InTransportsArray),
                    _ => self.set_err("unexpected '[' at FrbConfig root"),
                }
            }
            Some(CfgStack::InAccount {
                key,
                in_transport_ids,
                ..
            }) => {
                if key.as_deref() == Some("transportIds") {
                    *key = None;
                    *in_transport_ids = true;
                } else {
                    self.set_err("unexpected '[' in FrbAccount object");
                }
            }
            _ => self.set_err("unexpected '[' in FrbConfig JSON"),
        }
    }

    fn end_array(&mut self) {
        if self.err.is_some() {
            return;
        }
        if let Some(CfgStack::InAccount {
            in_transport_ids,
            ..
        }) = self.stack.last_mut()
        {
            if *in_transport_ids {
                *in_transport_ids = false;
                return;
            }
        }
        match self.stack.pop() {
            Some(CfgStack::InAccountsArray) | Some(CfgStack::InTransportsArray) => {}
            _ => self.set_err("unexpected ']' in FrbConfig JSON"),
        }
    }

    fn key(&mut self, key: &str) {
        if self.err.is_some() {
            return;
        }
        let k = key.to_string();
        match self.stack.last_mut() {
            Some(CfgStack::Root { key: slot }) => *slot = Some(k),
            Some(CfgStack::InAccount { key: slot, .. }) => *slot = Some(k),
            Some(CfgStack::InTransport { key: slot, .. }) => *slot = Some(k),
            _ => self.set_err("unexpected key in FrbConfig JSON"),
        }
    }

    fn string_value(&mut self, value: &str) {
        if self.err.is_some() {
            return;
        }
        let v = value.to_string();
        match self.stack.last_mut() {
            Some(CfgStack::Root { key }) => {
                let Some(k) = key.take() else {
                    self.set_err("orphan string at FrbConfig root");
                    return;
                };
                match k.as_str() {
                    "selectedStoreId" => self.config.selected_store_id = Some(v),
                    "selectedFolder" => self.json_legacy_root_folder = Some(v),
                    "selectedMessageId" => self.json_legacy_root_message_id = Some(v),
                    "dateFormat" => self.config.date_format = v,
                    "resourcePolicy" => self.config.resource_policy = v,
                    "deleteMode" => self.config.delete_mode = v,
                    "trashFolderName" => self.config.trash_folder_name = v,
                    "messageListSort" => self.config.message_list_sort = v,
                    _ => {}
                }
            }
            Some(CfgStack::InAccount {
                acc,
                key,
                in_transport_ids,
            }) => {
                if *in_transport_ids {
                    acc.transport_ids.push(v);
                    return;
                }
                let Some(k) = key.take() else {
                    self.set_err("orphan string in FrbAccount");
                    return;
                };
                match k.as_str() {
                    "id" => acc.id = v,
                    "label" => acc.label = v,
                    "backendType" => acc.backend_type = v,
                    "storeUri" => acc.store_uri = v,
                    "transportUri" => acc.transport_uri = Some(v),
                    "username" => acc.username = Some(v),
                    "host" => acc.host = Some(v),
                    "security" => acc.security = Some(v),
                    "path" => acc.path = Some(v),
                    "email" => acc.email = Some(v),
                    "avatarUrl" => acc.avatar_url = Some(v),
                    "lastFolder" => acc.last_folder = Some(v),
                    "lastMessageId" => acc.last_message_id = Some(v),
                    _ => {}
                }
            }
            Some(CfgStack::InTransport { t, key }) => {
                let Some(k) = key.take() else {
                    self.set_err("orphan string in FrbTransport");
                    return;
                };
                match k.as_str() {
                    "id" => t.id = v,
                    "transportType" => t.transport_type = v,
                    "displayName" => t.display_name = v,
                    "host" => t.host = v,
                    "security" => t.security = v,
                    "transportUri" => t.transport_uri = v,
                    _ => {}
                }
            }
            _ => self.set_err("unexpected string in FrbConfig JSON"),
        }
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.err.is_some() {
            return;
        }
        let n = number.as_i64().unwrap_or_else(|| number.as_f64() as i64);
        let n_u16 = u16::try_from(n).unwrap_or(0);
        match self.stack.last_mut() {
            Some(CfgStack::InAccount { acc, key, .. }) => {
                let Some(k) = key.take() else {
                    self.set_err("orphan number in FrbAccount");
                    return;
                };
                if k == "port" {
                    acc.port = Some(n_u16);
                } else if k == "imapIdleMinIdleSeconds" {
                    let clamped = n.clamp(1, 864_000);
                    acc.imap_idle_min_idle_seconds =
                        Some(u32::try_from(clamped).unwrap_or(120));
                }
            }
            Some(CfgStack::InTransport { t, key }) => {
                let Some(k) = key.take() else {
                    self.set_err("orphan number in FrbTransport");
                    return;
                };
                if k == "port" {
                    t.port = n_u16;
                }
            }
            _ => {}
        }
    }

    fn boolean_value(&mut self, value: bool) {
        if self.err.is_some() {
            return;
        }
        match self.stack.last_mut() {
            Some(CfgStack::Root { key }) => {
                let Some(k) = key.take() else {
                    self.set_err("orphan boolean at FrbConfig root");
                    return;
                };
                match k.as_str() {
                    "useKeychain" => self.config.use_keychain = value,
                    "loadRemoteImages" => self.config.load_remote_images = value,
                    "threadedView" => self.config.threaded_view = value,
                    "quoteOriginal" => self.config.quote_original = value,
                    "notifyNewMessages" => self.config.notify_new_messages = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn null_value(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.stack.last_mut() {
            Some(CfgStack::Root { key }) => {
                if let Some(k) = key.take() {
                    match k.as_str() {
                        "selectedStoreId" => self.config.selected_store_id = None,
                        "selectedFolder" => self.json_legacy_root_folder = None,
                        "selectedMessageId" => self.json_legacy_root_message_id = None,
                        _ => {}
                    }
                }
            }
            Some(CfgStack::InAccount { acc, key, .. }) => {
                let Some(k) = key.take() else {
                    return;
                };
                match k.as_str() {
                    "transportUri" => acc.transport_uri = None,
                    "username" => acc.username = None,
                    "host" => acc.host = None,
                    "port" => acc.port = None,
                    "security" => acc.security = None,
                    "path" => acc.path = None,
                    "email" => acc.email = None,
                    "avatarUrl" => acc.avatar_url = None,
                    "lastFolder" => acc.last_folder = None,
                    "lastMessageId" => acc.last_message_id = None,
                    "imapIdleMinIdleSeconds" => acc.imap_idle_min_idle_seconds = None,
                    _ => {}
                }
            }
            Some(CfgStack::InTransport { t, key }) => {
                let Some(k) = key.take() else {
                    return;
                };
                if k == "port" {
                    t.port = 0;
                }
            }
            _ => {}
        }
    }
}

pub fn parse_frb_config_json(input: &str) -> Result<FrbConfig, String> {
    let mut h = FrbConfigParse::new();
    parse_str_complete(input, &mut h).map_err(|e: JsonError| e.to_string())?;
    h.into_result()
}

// --- FrbAccount parse (single object document) ---------------------------------------------------

struct FrbAccountParse {
    acc: FrbAccount,
    key: Option<String>,
    in_transport_ids: bool,
    depth: u32,
    err: Option<String>,
}

impl FrbAccountParse {
    fn new() -> Self {
        Self {
            acc: FrbAccount::default(),
            key: None,
            in_transport_ids: false,
            depth: 0,
            err: None,
        }
    }

    fn set_err(&mut self, msg: impl Into<String>) {
        if self.err.is_none() {
            self.err = Some(msg.into());
        }
    }

    fn into_result(self) -> Result<FrbAccount, String> {
        if let Some(e) = self.err {
            return Err(e);
        }
        Ok(self.acc)
    }
}

impl JsonContentHandler for FrbAccountParse {
    fn start_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        self.depth += 1;
        if self.depth != 1 {
            self.set_err("unexpected nested object in FrbAccount JSON");
        }
    }

    fn end_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        if self.depth == 0 {
            self.set_err("unexpected '}' in FrbAccount JSON");
            return;
        }
        self.depth -= 1;
    }

    fn start_array(&mut self) {
        if self.err.is_some() {
            return;
        }
        if self.key.as_deref() == Some("transportIds") {
            self.key = None;
            self.in_transport_ids = true;
        } else {
            self.set_err("unexpected '[' in FrbAccount JSON");
        }
    }

    fn end_array(&mut self) {
        if self.err.is_some() {
            return;
        }
        if self.in_transport_ids {
            self.in_transport_ids = false;
        } else {
            self.set_err("unexpected ']' in FrbAccount JSON");
        }
    }

    fn key(&mut self, key: &str) {
        if self.err.is_some() {
            return;
        }
        self.key = Some(key.to_string());
    }

    fn string_value(&mut self, value: &str) {
        if self.err.is_some() {
            return;
        }
        let v = value.to_string();
        if self.in_transport_ids {
            self.acc.transport_ids.push(v);
            return;
        }
        let Some(k) = self.key.take() else {
            self.set_err("orphan string in FrbAccount JSON");
            return;
        };
        match k.as_str() {
            "id" => self.acc.id = v,
            "label" => self.acc.label = v,
            "backendType" => self.acc.backend_type = v,
            "storeUri" => self.acc.store_uri = v,
            "transportUri" => self.acc.transport_uri = Some(v),
            "username" => self.acc.username = Some(v),
            "host" => self.acc.host = Some(v),
            "security" => self.acc.security = Some(v),
            "path" => self.acc.path = Some(v),
            "email" => self.acc.email = Some(v),
            "avatarUrl" => self.acc.avatar_url = Some(v),
            "lastFolder" => self.acc.last_folder = Some(v),
            "lastMessageId" => self.acc.last_message_id = Some(v),
            _ => {}
        }
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.err.is_some() {
            return;
        }
        let n = number.as_i64().unwrap_or_else(|| number.as_f64() as i64);
        let Some(k) = self.key.take() else {
            self.set_err("orphan number in FrbAccount JSON");
            return;
        };
        if k == "port" {
            self.acc.port = Some(u16::try_from(n).unwrap_or(0));
        }
    }

    fn boolean_value(&mut self, _value: bool) {}

    fn null_value(&mut self) {
        if self.err.is_some() {
            return;
        }
        let Some(k) = self.key.take() else {
            return;
        };
        match k.as_str() {
            "transportUri" => self.acc.transport_uri = None,
            "username" => self.acc.username = None,
            "host" => self.acc.host = None,
            "port" => self.acc.port = None,
            "security" => self.acc.security = None,
            "path" => self.acc.path = None,
            "email" => self.acc.email = None,
            "avatarUrl" => self.acc.avatar_url = None,
            "lastFolder" => self.acc.last_folder = None,
            "lastMessageId" => self.acc.last_message_id = None,
            _ => {}
        }
    }
}

pub fn parse_frb_account_json(input: &str) -> Result<FrbAccount, String> {
    let mut h = FrbAccountParse::new();
    parse_str_complete(input, &mut h).map_err(|e: JsonError| e.to_string())?;
    if h.depth != 0 {
        return Err("incomplete FrbAccount object".to_string());
    }
    h.into_result()
}

// --- Write helpers -------------------------------------------------------------------------------

fn write_optional_string(w: &mut JsonWriter, json_key: &str, v: &Option<String>) {
    if let Some(s) = v {
        w.write_key(json_key);
        w.write_string(s);
    }
}

fn write_frb_transport(w: &mut JsonWriter, t: &FrbTransport) {
    w.write_start_object();
    w.write_key("id");
    w.write_string(&t.id);
    w.write_key("transportType");
    w.write_string(&t.transport_type);
    w.write_key("displayName");
    w.write_string(&t.display_name);
    w.write_key("host");
    w.write_string(&t.host);
    w.write_key("port");
    w.write_number(JsonNumber::I64(t.port as i64));
    w.write_key("security");
    w.write_string(&t.security);
    w.write_key("transportUri");
    w.write_string(&t.transport_uri);
    w.write_end_object();
}

fn write_frb_account(w: &mut JsonWriter, a: &FrbAccount) {
    w.write_start_object();
    w.write_key("id");
    w.write_string(&a.id);
    w.write_key("label");
    w.write_string(&a.label);
    w.write_key("backendType");
    w.write_string(&a.backend_type);
    w.write_key("storeUri");
    w.write_string(&a.store_uri);
    w.write_key("transportIds");
    w.write_start_array();
    for id in &a.transport_ids {
        w.write_string(id);
    }
    w.write_end_array();
    write_optional_string(w, "transportUri", &a.transport_uri);
    write_optional_string(w, "username", &a.username);
    write_optional_string(w, "host", &a.host);
    if let Some(p) = a.port {
        w.write_key("port");
        w.write_number(JsonNumber::I64(p as i64));
    }
    write_optional_string(w, "security", &a.security);
    write_optional_string(w, "path", &a.path);
    write_optional_string(w, "email", &a.email);
    write_optional_string(w, "avatarUrl", &a.avatar_url);
    write_optional_string(w, "lastFolder", &a.last_folder);
    write_optional_string(w, "lastMessageId", &a.last_message_id);
    if let Some(s) = a.imap_idle_min_idle_seconds {
        w.write_key("imapIdleMinIdleSeconds");
        w.write_number(JsonNumber::I64(s as i64));
    }
    w.write_end_object();
}

pub fn format_frb_config_json(cfg: &FrbConfig) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("accounts");
    w.write_start_array();
    for a in &cfg.accounts {
        write_frb_account(&mut w, a);
    }
    w.write_end_array();
    w.write_key("transports");
    w.write_start_array();
    for t in &cfg.transports {
        write_frb_transport(&mut w, t);
    }
    w.write_end_array();
    write_optional_string(&mut w, "selectedStoreId", &cfg.selected_store_id);
    w.write_key("dateFormat");
    w.write_string(&cfg.date_format);
    w.write_key("resourcePolicy");
    w.write_string(&cfg.resource_policy);
    w.write_key("useKeychain");
    w.write_bool(cfg.use_keychain);
    w.write_key("loadRemoteImages");
    w.write_bool(cfg.load_remote_images);
    w.write_key("threadedView");
    w.write_bool(cfg.threaded_view);
    w.write_key("quoteOriginal");
    w.write_bool(cfg.quote_original);
    w.write_key("deleteMode");
    w.write_string(&cfg.delete_mode);
    w.write_key("trashFolderName");
    w.write_string(&cfg.trash_folder_name);
    w.write_key("messageListSort");
    w.write_string(&cfg.message_list_sort);
    w.write_key("notifyNewMessages");
    w.write_bool(cfg.notify_new_messages);
    w.write_end_object();
    writer_into_string(w)
}
