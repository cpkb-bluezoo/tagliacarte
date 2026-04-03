/*
 * types.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct AppConfig {
    pub accounts: Vec<AccountConfig>,
    pub date_format: String,
    pub resource_policy: String,
}

#[derive(Clone, Debug, Default)]
pub struct AccountConfig {
    pub id: String,
    pub label: String,
    pub store_uri: String,
    pub transport_uri: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayAttachment {
    pub filename: Option<String>,
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayMessage {
    pub subject: String,
    pub from: String,
    pub to: String,
    pub date: String,
    pub body_html: Option<String>,
    pub body_plain: Option<String>,
    pub attachments: Vec<DisplayAttachment>,
    pub inline_images: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
pub struct ComposeData {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_plain: String,
    pub body_html: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub sender: String,
    pub sender_display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub body: String,
    pub timestamp: i64,
    pub is_own: bool,
}

#[derive(Clone, Debug)]
pub enum FolderEvent {
    Found {
        name: String,
        delimiter: Option<char>,
        attributes: String,
    },
    Removed {
        name: String,
    },
    Complete,
    Error {
        message: String,
    },
    NeedsCredential {
        username: String,
        is_plaintext: bool,
    },
}

#[derive(Clone, Debug)]
pub enum MessageEvent {
    Metadata {
        subject: String,
        from: String,
        to: String,
        date: String,
    },
    BodyReady {
        html: Option<String>,
        plain: Option<String>,
    },
    InlineImage {
        cid: String,
        data: Vec<u8>,
        mime_type: String,
    },
    Attachment {
        filename: Option<String>,
        mime_type: String,
        data: Vec<u8>,
    },
    Complete,
    Error {
        message: String,
    },
}

#[derive(Clone, Debug)]
pub enum SendEvent {
    Progress { status: String },
    Complete,
    Error { message: String },
}

#[derive(Clone, Debug)]
pub enum OAuthEvent {
    AuthorizationUrl { url: String },
    Complete,
    Error { message: String },
}
