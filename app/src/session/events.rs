/*
 * session/events.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * App-level events pushed to all frontends (Flutter stream, future ncurses, etc.).
 */

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

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
    /// One folder discovered during a refresh (§3.2 ARCHITECTURE.md). [FolderListUpdated] follows
    /// with the authoritative full list for reconcile.
    #[serde(rename_all = "camelCase")]
    FolderFound {
        account_id: String,
        folder_name: String,
        unread: u32,
    },
    #[serde(rename_all = "camelCase")]
    MessageFlagsChanged {
        account_id: String,
        folder: String,
        message_id: String,
        is_read: bool,
    },
    /// Header for one window fetch: allocate [total] slots before row events (matches §3.2 flow).
    #[serde(rename_all = "camelCase")]
    MessageListWindowStarted {
        request_id: String,
        account_id: String,
        folder_name: String,
        message_list_sort: String,
        total: u64,
        start_index: u64,
        list_strategy: String,
        row_count: u32,
        list_ready: bool,
    },
    #[serde(rename_all = "camelCase")]
    MessageListRowFound {
        request_id: String,
        account_id: String,
        folder_name: String,
        message_list_sort: String,
        rank: u64,
        summary: Value,
    },
    /// Success: `error` absent; failure: `error` set (no started / no rows).
    #[serde(rename_all = "camelCase")]
    MessageListWindowComplete {
        request_id: String,
        account_id: String,
        folder_name: String,
        message_list_sort: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
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
