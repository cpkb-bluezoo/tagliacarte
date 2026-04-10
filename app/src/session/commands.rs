/*
 * session/commands.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * JSON commands from UI (`type` tag); fire-and-forget via `frb_session_command`.
 */

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppCommand {
    #[serde(rename_all = "camelCase")]
    MarkRead {
        account_id: String,
        folder: String,
        message_id: String,
        #[serde(default)]
        request_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    RefreshFolders { account_id: String },
    #[serde(rename_all = "camelCase")]
    TransferMessages {
        source_account_id: String,
        source_folder: String,
        dest_account_id: String,
        dest_folder: String,
        message_ids: Vec<String>,
        is_move: bool,
        #[serde(default)]
        request_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    SendChatMessage {
        account_id: String,
        folder: String,
        text: String,
        /// Optional HTML (Matrix rich text). Ignored for Nostr.
        #[serde(default)]
        body_html: Option<String>,
        #[serde(default)]
        request_id: Option<String>,
    },
    /// Windowed message list: returns immediately; results on the session stream (§2–3).
    #[serde(rename_all = "camelCase")]
    ListMessagesWindow {
        account_id: String,
        folder_name: String,
        #[serde(default)]
        start_index: u64,
        limit: u64,
        message_list_sort: String,
        request_id: String,
        #[serde(default)]
        list_ready: bool,
        /// Inclusive oldest-first rank of the first visible list row (viewport), if known.
        #[serde(default)]
        visible_first_rank: Option<u64>,
        /// Inclusive oldest-first rank of the last visible list row (viewport), if known.
        #[serde(default)]
        visible_last_rank: Option<u64>,
    },
}
