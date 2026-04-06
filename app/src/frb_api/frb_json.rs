/*
 * frb_json.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Parse/format FRB config JSON with tagliacarte_core::json (camelCase).
 *
 * - FrbAccount: id, label, backendType, avatarUrl?, lastFolder?, lastMessageId?,
 *   attrs: { string → string }, lists: { transportIds?, relayUrls?, … }.
 * - Legacy flat keys (username, host, transportIds array at top) are merged into attrs/lists.
 */

use std::collections::HashMap;

use tagliacarte_core::json::{
    JsonContentHandler, JsonError, JsonNumber, JsonWriter, parse_str_complete, writer_into_string,
};

use crate::legacy_store_uri::merge_legacy_store_uri_into_account;

use super::{FrbAccount, FrbConfig, FrbTransport};

// --- FrbConfig parse -----------------------------------------------------------------------------

/// Parser state for a single account object; public for flutter_rust_bridge codegen.
#[derive(Debug)]
pub enum AccountParseState {
    Top {
        acc: FrbAccount,
        key: Option<String>,
        in_legacy_transport_ids: bool,
    },
    InAttrs {
        acc: FrbAccount,
        key: Option<String>,
    },
    InLists {
        acc: FrbAccount,
        list_key: Option<String>,
        in_array: bool,
    },
}

pub enum CfgStack {
    Root {
        key: Option<String>,
    },
    InAccountsArray,
    InAccount(AccountParseState),
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
    pub json_legacy_root_folder: Option<String>,
    pub json_legacy_root_message_id: Option<String>,
}

impl FrbConfigParse {
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

fn merge_legacy_top_level_attr(acc: &mut FrbAccount, k: &str, v: String) {
    match k {
        "username" | "host" | "security" | "path" | "email" => {
            acc.attrs.insert(k.to_string(), v);
        }
        "transportUri" => {
            acc.attrs.insert("transportUri".to_string(), v);
        }
        _ => {}
    }
}

impl JsonContentHandler for FrbConfigParse {
    fn start_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.stack.last_mut() {
            None => self.stack.push(CfgStack::Root { key: None }),
            Some(CfgStack::InAccountsArray) => self.stack.push(CfgStack::InAccount(
                AccountParseState::Top {
                    acc: FrbAccount::default(),
                    key: None,
                    in_legacy_transport_ids: false,
                },
            )),
            Some(CfgStack::InTransportsArray) => self.stack.push(CfgStack::InTransport {
                t: FrbTransport {
                    id: String::new(),
                    transport_type: String::new(),
                    display_name: String::new(),
                    host: String::new(),
                    port: 0,
                    security: String::new(),
                    default_from: String::new(),
                    dsn_notify: String::new(),
                },
                key: None,
            }),
            Some(CfgStack::InAccount(st)) => match st {
                AccountParseState::Top {
                    key,
                    acc,
                    in_legacy_transport_ids,
                } => {
                    let open = key.as_deref();
                    if open == Some("attrs") {
                        let a = std::mem::take(acc);
                        key.take();
                        *in_legacy_transport_ids = false;
                        *st = AccountParseState::InAttrs { acc: a, key: None };
                    } else if open == Some("lists") {
                        let a = std::mem::take(acc);
                        key.take();
                        *in_legacy_transport_ids = false;
                        *st = AccountParseState::InLists {
                            acc: a,
                            list_key: None,
                            in_array: false,
                        };
                    } else if open.is_some() {
                        self.set_err("unexpected nested object in FrbAccount (expected attrs or lists key)");
                    }
                    // open None: root `{` of account — no state change
                }
                _ => self.set_err("unexpected nested object in FrbAccount"),
            },
            _ => self.set_err("unexpected '{' in FrbConfig JSON"),
        }
    }

    fn end_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.stack.pop() {
            Some(CfgStack::InAccount(AccountParseState::Top { acc, .. })) => {
                self.config.accounts.push(acc);
            }
            Some(CfgStack::InAccount(AccountParseState::InAttrs { acc, .. })) => {
                self.stack.push(CfgStack::InAccount(AccountParseState::Top {
                    acc,
                    key: None,
                    in_legacy_transport_ids: false,
                }));
            }
            Some(CfgStack::InAccount(AccountParseState::InLists { acc, .. })) => {
                self.stack.push(CfgStack::InAccount(AccountParseState::Top {
                    acc,
                    key: None,
                    in_legacy_transport_ids: false,
                }));
            }
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
            Some(CfgStack::InAccount(st)) => match st {
                AccountParseState::Top {
                    key,
                    in_legacy_transport_ids,
                    ..
                } => {
                    if key.as_deref() == Some("transportIds") {
                        *key = None;
                        *in_legacy_transport_ids = true;
                    } else {
                        self.set_err("unexpected '[' in FrbAccount top object");
                    }
                }
                AccountParseState::InLists {
                    list_key,
                    in_array,
                    ..
                } => {
                    if list_key.is_some() {
                        *in_array = true;
                    } else {
                        self.set_err("unexpected '[' in lists object");
                    }
                }
                _ => self.set_err("unexpected '[' in FrbAccount"),
            },
            _ => self.set_err("unexpected '[' in FrbConfig JSON"),
        }
    }

    fn end_array(&mut self) {
        if self.err.is_some() {
            return;
        }
        if let Some(CfgStack::InAccount(st)) = self.stack.last_mut() {
            match st {
                AccountParseState::Top {
                    in_legacy_transport_ids,
                    ..
                } => {
                    if *in_legacy_transport_ids {
                        *in_legacy_transport_ids = false;
                        return;
                    }
                }
                AccountParseState::InLists {
                    in_array,
                    list_key,
                    ..
                } => {
                    if *in_array {
                        *in_array = false;
                        *list_key = None;
                        return;
                    }
                }
                _ => {}
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
            Some(CfgStack::InTransport { key: slot, .. }) => *slot = Some(k),
            Some(CfgStack::InAccount(st)) => match st {
                AccountParseState::Top { key: slot, .. } => *slot = Some(k),
                AccountParseState::InAttrs { key: slot, .. } => *slot = Some(k),
                AccountParseState::InLists { list_key, .. } => *list_key = Some(k),
            },
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
                    "replyHeaderTemplate" => self.config.reply_header_template = v,
                    "replyDateFormat" => self.config.reply_date_format = v,
                    "replyTimeFormat" => self.config.reply_time_format = v,
                    "replyLinePrefix" => self.config.reply_line_prefix = v,
                    "replyQuoteMode" => self.config.reply_quote_mode = v,
                    _ => {}
                }
            }
            Some(CfgStack::InAccount(st)) => match st {
                AccountParseState::Top {
                    acc,
                    key,
                    in_legacy_transport_ids,
                } => {
                    if *in_legacy_transport_ids {
                        acc.lists
                            .entry("transportIds".to_string())
                            .or_default()
                            .push(v);
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
                        "storeUri" => {
                            if let Err(e) = merge_legacy_store_uri_into_account(acc, &v) {
                                self.set_err(e);
                            }
                        }
                        "avatarUrl" => acc.avatar_url = Some(v),
                        "lastFolder" => acc.last_folder = Some(v),
                        "lastMessageId" => acc.last_message_id = Some(v),
                        _ => merge_legacy_top_level_attr(acc, &k, v),
                    }
                }
                AccountParseState::InAttrs { acc, key } => {
                    let Some(k) = key.take() else {
                        self.set_err("orphan string in attrs");
                        return;
                    };
                    acc.attrs.insert(k, v);
                }
                AccountParseState::InLists {
                    acc,
                    list_key,
                    in_array,
                } => {
                    if !*in_array {
                        self.set_err("string value in lists object outside array");
                        return;
                    }
                    let Some(lk) = list_key.clone() else {
                        self.set_err("orphan string in lists array");
                        return;
                    };
                    acc.lists.entry(lk).or_default().push(v);
                }
            },
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
                    "transportUri" => {}
                    "defaultFrom" => t.default_from = v,
                    "dsnNotify" => t.dsn_notify = v,
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
            Some(CfgStack::InAccount(st)) => match st {
                AccountParseState::Top { acc, key, .. } => {
                    let Some(k) = key.take() else {
                        self.set_err("orphan number in FrbAccount");
                        return;
                    };
                    if k == "port" {
                        acc.attrs.insert("port".to_string(), n_u16.to_string());
                    } else if k == "imapIdleMinIdleSeconds" {
                        let clamped = n.clamp(1, 864_000);
                        acc.attrs.insert(
                            "imapIdleMinIdleSeconds".to_string(),
                            clamped.to_string(),
                        );
                    }
                }
                AccountParseState::InAttrs { acc, key } => {
                    let Some(k) = key.take() else {
                        self.set_err("orphan number in attrs");
                        return;
                    };
                    acc.attrs.insert(k, n.to_string());
                }
                _ => {}
            },
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
            Some(CfgStack::InAccount(st)) => match st {
                AccountParseState::Top { acc, key, .. } => {
                    let Some(k) = key.take() else {
                        return;
                    };
                    match k.as_str() {
                        "avatarUrl" => acc.avatar_url = None,
                        "lastFolder" => acc.last_folder = None,
                        "lastMessageId" => acc.last_message_id = None,
                        "username" => {
                            acc.attrs.remove("username");
                        }
                        "host" => {
                            acc.attrs.remove("host");
                        }
                        "port" => {
                            acc.attrs.remove("port");
                        }
                        "security" => {
                            acc.attrs.remove("security");
                        }
                        "path" => {
                            acc.attrs.remove("path");
                        }
                        "email" => {
                            acc.attrs.remove("email");
                        }
                        "transportUri" => {
                            acc.attrs.remove("transportUri");
                        }
                        "imapIdleMinIdleSeconds" => {
                            acc.attrs.remove("imapIdleMinIdleSeconds");
                        }
                        _ => {}
                    }
                }
                AccountParseState::InAttrs { acc, key } => {
                    if let Some(k) = key.take() {
                        acc.attrs.remove(&k);
                    }
                }
                _ => {}
            },
            Some(CfgStack::InTransport { t, key }) => {
                if let Some(k) = key.take() {
                    if k == "port" {
                        t.port = 0;
                    }
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

// --- FrbAccount parse (single object) -----------------------------------------------------------

enum SingleAccState {
    Top {
        acc: FrbAccount,
        key: Option<String>,
        in_legacy_transport_ids: bool,
    },
    InAttrs {
        acc: FrbAccount,
        key: Option<String>,
    },
    InLists {
        acc: FrbAccount,
        list_key: Option<String>,
        in_array: bool,
    },
}

struct FrbAccountParse {
    stack: Vec<SingleAccState>,
    err: Option<String>,
}

impl FrbAccountParse {
    fn new() -> Self {
        Self {
            stack: vec![SingleAccState::Top {
                acc: FrbAccount::default(),
                key: None,
                in_legacy_transport_ids: false,
            }],
            err: None,
        }
    }

    fn set_err(&mut self, msg: impl Into<String>) {
        if self.err.is_none() {
            self.err = Some(msg.into());
        }
    }

    fn top_mut(&mut self) -> Option<&mut SingleAccState> {
        self.stack.last_mut()
    }

    fn into_result(mut self) -> Result<FrbAccount, String> {
        if let Some(e) = self.err {
            return Err(e);
        }
        match self.stack.pop() {
            Some(SingleAccState::Top { acc, .. }) => Ok(acc),
            _ => Err("incomplete FrbAccount object".to_string()),
        }
    }
}

impl JsonContentHandler for FrbAccountParse {
    fn start_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.top_mut() {
            Some(SingleAccState::Top { key, acc, .. }) => {
                let open = key.as_deref();
                if open == Some("attrs") {
                    key.take();
                    let a = std::mem::take(acc);
                    self.stack.push(SingleAccState::InAttrs { acc: a, key: None });
                } else if open == Some("lists") {
                    key.take();
                    let a = std::mem::take(acc);
                    self.stack.push(SingleAccState::InLists {
                        acc: a,
                        list_key: None,
                        in_array: false,
                    });
                } else if open.is_some() {
                    self.set_err("unexpected nested object in FrbAccount JSON");
                }
            }
            _ => self.set_err("unexpected '{' in FrbAccount JSON"),
        }
    }

    fn end_object(&mut self) {
        if self.err.is_some() {
            return;
        }
        if self.stack.len() >= 2 {
            match self.stack.pop() {
                Some(SingleAccState::InAttrs { acc, .. }) | Some(SingleAccState::InLists { acc, .. }) => {
                    if let Some(SingleAccState::Top { acc: parent, .. }) = self.stack.last_mut() {
                        *parent = acc;
                    }
                }
                _ => self.set_err("unexpected '}' in FrbAccount JSON"),
            }
        } else if let Some(SingleAccState::Top { .. }) = self.stack.last() {
            // root account object closed
        } else {
            self.set_err("unexpected '}' in FrbAccount JSON");
        }
    }

    fn start_array(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.top_mut() {
            Some(SingleAccState::Top {
                key,
                in_legacy_transport_ids,
                ..
            }) => {
                if key.as_deref() == Some("transportIds") {
                    *key = None;
                    *in_legacy_transport_ids = true;
                } else {
                    self.set_err("unexpected '[' in FrbAccount JSON");
                }
            }
            Some(SingleAccState::InLists {
                list_key,
                in_array,
                ..
            }) => {
                if list_key.is_some() {
                    *in_array = true;
                } else {
                    self.set_err("unexpected '[' in lists");
                }
            }
            _ => self.set_err("unexpected '[' in FrbAccount JSON"),
        }
    }

    fn end_array(&mut self) {
        if self.err.is_some() {
            return;
        }
        match self.top_mut() {
            Some(SingleAccState::Top {
                in_legacy_transport_ids,
                ..
            }) => {
                if *in_legacy_transport_ids {
                    *in_legacy_transport_ids = false;
                    return;
                }
            }
            Some(SingleAccState::InLists {
                in_array,
                list_key,
                ..
            }) => {
                if *in_array {
                    *in_array = false;
                    *list_key = None;
                    return;
                }
            }
            _ => {}
        }
        self.set_err("unexpected ']' in FrbAccount JSON");
    }

    fn key(&mut self, key: &str) {
        if self.err.is_some() {
            return;
        }
        let k = key.to_string();
        match self.top_mut() {
            Some(SingleAccState::Top { key: slot, .. }) => *slot = Some(k),
            Some(SingleAccState::InAttrs { key: slot, .. }) => *slot = Some(k),
            Some(SingleAccState::InLists { list_key, .. }) => *list_key = Some(k),
            None => self.set_err("unexpected key"),
        }
    }

    fn string_value(&mut self, value: &str) {
        if self.err.is_some() {
            return;
        }
        let v = value.to_string();
        match self.top_mut() {
            Some(SingleAccState::Top {
                acc,
                key,
                in_legacy_transport_ids,
            }) => {
                if *in_legacy_transport_ids {
                    acc.lists
                        .entry("transportIds".to_string())
                        .or_default()
                        .push(v);
                    return;
                }
                let Some(k) = key.take() else {
                    self.set_err("orphan string in FrbAccount JSON");
                    return;
                };
                match k.as_str() {
                    "id" => acc.id = v,
                    "label" => acc.label = v,
                    "backendType" => acc.backend_type = v,
                    "storeUri" => {
                        if let Err(e) = merge_legacy_store_uri_into_account(acc, &v) {
                            self.set_err(e);
                        }
                    }
                    "avatarUrl" => acc.avatar_url = Some(v),
                    "lastFolder" => acc.last_folder = Some(v),
                    "lastMessageId" => acc.last_message_id = Some(v),
                    _ => merge_legacy_top_level_attr(acc, &k, v),
                }
            }
            Some(SingleAccState::InAttrs { acc, key }) => {
                let Some(k) = key.take() else {
                    self.set_err("orphan string in attrs");
                    return;
                };
                acc.attrs.insert(k, v);
            }
            Some(SingleAccState::InLists {
                acc,
                list_key,
                in_array,
            }) => {
                if !*in_array {
                    self.set_err("string in lists outside array");
                    return;
                }
                let Some(lk) = list_key.clone() else {
                    return;
                };
                acc.lists.entry(lk).or_default().push(v);
            }
            None => self.set_err("orphan string"),
        }
    }

    fn number_value(&mut self, number: JsonNumber) {
        if self.err.is_some() {
            return;
        }
        let n = number.as_i64().unwrap_or_else(|| number.as_f64() as i64);
        let n_u16 = u16::try_from(n).unwrap_or(0);
        match self.top_mut() {
            Some(SingleAccState::Top { acc, key, .. }) => {
                let Some(k) = key.take() else {
                    self.set_err("orphan number");
                    return;
                };
                if k == "port" {
                    acc.attrs.insert("port".to_string(), n_u16.to_string());
                } else if k == "imapIdleMinIdleSeconds" {
                    let clamped = n.clamp(1, 864_000);
                    acc.attrs.insert("imapIdleMinIdleSeconds".to_string(), clamped.to_string());
                }
            }
            Some(SingleAccState::InAttrs { acc, key }) => {
                let Some(k) = key.take() else {
                    return;
                };
                acc.attrs.insert(k, n.to_string());
            }
            _ => {}
        }
    }

    fn boolean_value(&mut self, _value: bool) {}

    fn null_value(&mut self) {
        if self.err.is_some() {
            return;
        }
        if let Some(SingleAccState::Top { acc, key, .. }) = self.top_mut() {
            let Some(k) = key.take() else {
                return;
            };
            match k.as_str() {
                "avatarUrl" => acc.avatar_url = None,
                "lastFolder" => acc.last_folder = None,
                "lastMessageId" => acc.last_message_id = None,
                "username" => {
                    acc.attrs.remove("username");
                }
                "host" => {
                    acc.attrs.remove("host");
                }
                "port" => {
                    acc.attrs.remove("port");
                }
                "security" => {
                    acc.attrs.remove("security");
                }
                "path" => {
                    acc.attrs.remove("path");
                }
                "email" => {
                    acc.attrs.remove("email");
                }
                "transportUri" => {
                    acc.attrs.remove("transportUri");
                }
                "imapIdleMinIdleSeconds" => {
                    acc.attrs.remove("imapIdleMinIdleSeconds");
                }
                _ => {}
            }
        }
    }
}

pub fn parse_frb_account_json(input: &str) -> Result<FrbAccount, String> {
    let mut h = FrbAccountParse::new();
    parse_str_complete(input, &mut h).map_err(|e: JsonError| e.to_string())?;
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
    w.write_key("defaultFrom");
    w.write_string(&t.default_from);
    w.write_key("dsnNotify");
    w.write_string(&t.dsn_notify);
    w.write_end_object();
}

fn write_string_map(w: &mut JsonWriter, m: &HashMap<String, String>) {
    w.write_start_object();
    let mut keys: Vec<&String> = m.keys().collect();
    keys.sort();
    for k in keys {
        w.write_key(k);
        w.write_string(&m[k]);
    }
    w.write_end_object();
}

fn write_lists_map(w: &mut JsonWriter, m: &HashMap<String, Vec<String>>) {
    w.write_start_object();
    let mut keys: Vec<&String> = m.keys().collect();
    keys.sort();
    for k in keys {
        w.write_key(k);
        w.write_start_array();
        for s in &m[k] {
            w.write_string(s);
        }
        w.write_end_array();
    }
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
    write_optional_string(w, "avatarUrl", &a.avatar_url);
    write_optional_string(w, "lastFolder", &a.last_folder);
    write_optional_string(w, "lastMessageId", &a.last_message_id);
    if !a.attrs.is_empty() {
        w.write_key("attrs");
        write_string_map(w, &a.attrs);
    }
    if !a.lists.is_empty() {
        w.write_key("lists");
        write_lists_map(w, &a.lists);
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
    w.write_key("replyHeaderTemplate");
    w.write_string(&cfg.reply_header_template);
    w.write_key("replyDateFormat");
    w.write_string(&cfg.reply_date_format);
    w.write_key("replyTimeFormat");
    w.write_string(&cfg.reply_time_format);
    w.write_key("replyLinePrefix");
    w.write_string(&cfg.reply_line_prefix);
    w.write_key("replyQuoteMode");
    w.write_string(&cfg.reply_quote_mode);
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
