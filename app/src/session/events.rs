/*
 * session/events.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * App-level events pushed to all frontends (Flutter stream, future ncurses, etc.).
 */

use std::collections::HashMap;

use serde::Serialize;

/// JSON events for `frb_session_start` stream (`type` tag matches Dart decode).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppEvent {
    /// `connectionState`: `connecting` | `connected` | `disconnected` | `error`
    #[serde(rename_all = "camelCase")]
    AccountConnectionChanged {
        account_id: String,
        /// `email` | `nostr` | `matrix` (UI picks list vs conversation chrome).
        store_kind: String,
        connection_state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    FolderListUpdated {
        account_id: String,
        folders: Vec<String>,
        hierarchy_delimiter: Option<String>,
        unread_by_folder: HashMap<String, u32>,
    },
    #[serde(rename_all = "camelCase")]
    MessageFlagsChanged {
        account_id: String,
        folder: String,
        message_id: String,
        is_read: bool,
    },
    #[serde(rename_all = "camelCase")]
    CommandResult {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}
