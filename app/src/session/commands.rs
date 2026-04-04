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
}
