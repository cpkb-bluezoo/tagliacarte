/*
 * session/events.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * App-level events pushed to all frontends (Flutter stream, future ncurses, etc.).
 */

use std::collections::HashMap;

/// One row for the **Available** folder tab (see `MailFoldersSnapshot::subscription_pane`).
#[derive(Debug, Clone)]
pub struct SubscriptionAvailableRow {
    pub id: String,
    pub is_subscribed: bool,
    pub display_name: Option<String>,
    pub unread: Option<u32>,
    pub allow_unsubscribe: bool,
}

/// Message list row summary for list windows (same fields as mail `MessageSummaryJson`).
#[derive(Debug, Clone)]
pub struct MessageListRowSummary {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub date_ms: Option<i64>,
    pub is_read: bool,
    pub marked_for_deletion: bool,
    pub nostr_sender_pubkey_hex: Option<String>,
}

/// Session events for `frb_session_start` (typed over FRB; native adapters may still serialize for tests).
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// `connectionState`: `connecting` | `connected` | `disconnected` | `error`
    AccountConnectionChanged {
        account_id: String,
        /// `email` | `nostr` | `matrix` (UI picks list vs conversation chrome).
        store_kind: String,
        connection_state: String,
        message: Option<String>,
    },
    FolderListUpdated {
        account_id: String,
        folders: Vec<String>,
        hierarchy_delimiter: Option<String>,
        unread_by_folder: HashMap<String, u32>,
        /// Optional UI labels keyed by folder id (e.g. Matrix room id → room / peer display name).
        folder_display_names: HashMap<String, String>,
        /// IMAP / NNTP / Matrix: **Available** tab rows (Subscribed tab is `folders`).
        subscription_available: Option<Vec<SubscriptionAvailableRow>>,
        /// Matrix: `m.direct` room ids for **Direct messages** tab (tab 1); tab 0 is non-DM rooms.
        matrix_dm_folder_ids: Option<Vec<String>>,
    },
    /// One folder discovered during a refresh (§3.2 ARCHITECTURE.md). [FolderListUpdated] follows
    /// with the authoritative full list for reconcile.
    FolderFound {
        account_id: String,
        folder_name: String,
        unread: u32,
    },
    /// Folder list snapshot failed (e.g. timeout waiting for store completion). Does not clear
    /// folders already reported via [FolderFound]; UI should treat this as a failed completion only.
    FolderListFailed {
        account_id: String,
        message: String,
    },
    MessageFlagsChanged {
        account_id: String,
        folder: String,
        message_id: String,
        is_read: bool,
    },
    /// Header for one window fetch: allocate [total] slots before row events (matches §3.2 flow).
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
    MessageListRowFound {
        request_id: String,
        account_id: String,
        folder_name: String,
        message_list_sort: String,
        rank: u64,
        summary: MessageListRowSummary,
    },
    /// Success: `error` absent; failure: `error` set (no started / no rows).
    MessageListWindowComplete {
        request_id: String,
        account_id: String,
        folder_name: String,
        message_list_sort: String,
        error: Option<String>,
    },
    CommandResult {
        request_id: Option<String>,
        ok: bool,
        error: Option<String>,
    },
    /// Kind 0 metadata resolved for a peer pubkey (async follow-up to message/folder list).
    NostrProfileUpdated {
        account_id: String,
        /// Lowercase hex pubkey (folder id for DM conversations).
        pubkey_hex: String,
        /// npub (bech32) for display fallback.
        npub: String,
        display_name: Option<String>,
        nip05: Option<String>,
        picture: Option<String>,
    },
}
