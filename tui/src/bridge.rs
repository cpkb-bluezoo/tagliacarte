//! Thin wrappers around `tagliacarte_app::frb_api` (blocking calls).

#![allow(dead_code)]

use serde::Deserialize;
use tagliacarte_app::frb_api::{
    self, FrbAccount, FrbBatchMailOperationResult, FrbComposeMessage, FrbConfig,
    FrbFolderMessageDetail, FrbNntpComposeMessage,
};

#[derive(Debug, Clone, Deserialize)]
pub struct MessageSummary {
    pub id: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub subject: String,
    #[serde(rename = "dateMs")]
    pub date_ms: Option<i64>,
    #[serde(rename = "isRead", default)]
    pub is_read: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListMessagesWindow {
    pub total: u64,
    #[serde(rename = "startIndex")]
    pub start_index: u64,
    pub messages: Vec<MessageSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MailFoldersJson {
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDetail {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    pub cc: Option<String>,
    #[serde(rename = "dateMs")]
    pub date_ms: Option<i64>,
    #[serde(rename = "messageId")]
    pub message_id: Option<String>,
    #[serde(rename = "bodyPlain")]
    pub body_plain: Option<String>,
    #[serde(rename = "bodyHtml")]
    pub body_html: Option<String>,
}

pub fn save_config(path: &str, cfg: &FrbConfig) -> Result<(), String> {
    frb_api::frb_save_config(path.to_string(), cfg.clone())
}

pub fn load_config(path: &str) -> FrbConfig {
    frb_api::frb_load_config(path.to_string())
}

pub fn list_folders(account_id: &str) -> Result<Vec<String>, String> {
    let v = frb_api::frb_list_mail_folders(account_id.to_string())?;
    Ok(v.folders)
}

pub fn list_messages_window(
    account_id: &str,
    folder: &str,
    start_index: i32,
    limit: i32,
    sort: &str,
) -> Result<ListMessagesWindow, String> {
    let r = frb_api::frb_list_folder_messages_window(
        account_id.to_string(),
        folder.to_string(),
        start_index,
        limit,
        sort.to_string(),
    )?;
    Ok(ListMessagesWindow {
        total: r.total,
        start_index: r.start_index,
        messages: r
            .messages
            .into_iter()
            .map(|m| MessageSummary {
                id: m.id,
                from: m.from,
                subject: m.subject,
                date_ms: m.date_ms,
                is_read: m.is_read,
            })
            .collect(),
    })
}

fn detail_vm(d: FrbFolderMessageDetail) -> MessageDetail {
    MessageDetail {
        subject: d.subject,
        from: d.from,
        to: d.to,
        cc: d.cc,
        date_ms: d.date_ms,
        message_id: d.message_id,
        body_plain: d.body_plain,
        body_html: d.body_html,
    }
}

pub fn get_message(account_id: &str, folder: &str, message_id: &str) -> Result<MessageDetail, String> {
    let d = frb_api::frb_get_folder_message(
        account_id.to_string(),
        folder.to_string(),
        message_id.to_string(),
    )?;
    Ok(detail_vm(d))
}

pub fn mark_read(account_id: &str, folder: &str, message_id: &str) -> Result<(), String> {
    frb_api::frb_mark_folder_message_read(
        account_id.to_string(),
        folder.to_string(),
        message_id.to_string(),
    )
}

pub fn transfer_messages(
    source_account_id: &str,
    source_folder: &str,
    dest_account_id: &str,
    dest_folder: &str,
    message_ids: Vec<String>,
    is_move: bool,
) -> Result<FrbBatchMailOperationResult, String> {
    frb_api::frb_transfer_mail_messages(
        source_account_id.to_string(),
        source_folder.to_string(),
        dest_account_id.to_string(),
        dest_folder.to_string(),
        message_ids,
        is_move,
    )
}

pub fn send_smtp(transport_id: &str, compose: FrbComposeMessage) -> Result<(), String> {
    frb_api::frb_send_smtp_message(transport_id.to_string(), compose)
}

pub fn send_nntp(account_id: &str, compose: FrbNntpComposeMessage) -> Result<(), String> {
    frb_api::frb_send_nntp_message(account_id.to_string(), compose)
}

pub fn transports_for_account(acc: &FrbAccount) -> Vec<String> {
    acc.lists
        .get("transportIds")
        .cloned()
        .unwrap_or_default()
}

pub fn default_transport_id(acc: &FrbAccount) -> Option<String> {
    transports_for_account(acc).into_iter().next()
}

pub fn save_store_credential(account_id: &str, username: &str, password: &str) -> Result<(), String> {
    frb_api::frb_save_store_credential(
        account_id.to_string(),
        username.to_string(),
        password.to_string(),
    )
}

pub fn save_transport_credential(transport_id: &str, username: &str, password: &str) -> Result<(), String> {
    frb_api::frb_save_transport_credential(
        transport_id.to_string(),
        username.to_string(),
        password.to_string(),
    )
}

pub fn nostr_secret_key_to_hex(input: &str) -> Result<String, String> {
    frb_api::frb_nostr_secret_key_to_hex(input.to_string())
}

pub fn nostr_public_key_from_secret_hex(secret_hex: &str) -> Result<String, String> {
    frb_api::frb_nostr_get_public_key_from_secret(secret_hex.to_string())
}

pub fn nostr_hex_to_npub(pubkey_hex: &str) -> Result<String, String> {
    frb_api::frb_nostr_hex_to_npub(pubkey_hex.to_string())
}
