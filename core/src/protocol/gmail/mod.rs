/*
 * mod.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 */

//! Gmail REST API protocol: GmailStore (Store) and GmailTransport (Transport).

mod parse;

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::config::default_credentials_path;
use crate::json::JsonWriter;
use crate::message_id::MessageId;
use crate::oauth::token_store::get_valid_access_token_for_store_credential;
use crate::oauth::GoogleOAuthProvider;
use crate::protocol::http::api_trace::{log_http_request, log_http_response};
use crate::protocol::http::client::HttpClient;
use crate::protocol::http::connection::HttpConnection;
use crate::protocol::http::{HttpVersion, Method, RequestBuilder, Response, ResponseHandler};
use crate::store::{
    Address, ConversationSummary, DateTime, Envelope, Flag, Folder, FolderInfo,
    MessageForDisplay, OpenFolderEvent, SendPayload, Store, StoreError, StoreKind, Transport,
    TransportKind,
};
use tokio::runtime::Handle;

const GMAIL_HOST: &str = "gmail.googleapis.com";
const GMAIL_PORT: u16 = 443;
const GMAIL_BASE: &str = "/gmail/v1/users/me";
/// Stored in [`MessageAttachmentRef::imap_section`] so [`Folder::fetch_message_part`] can call
/// `users.messages.attachments.get` on a **dedicated** TCP connection (see `raw_request`).
const GMAIL_ATTACHMENT_SECTION_PREFIX: &str = "gmail:";
/// `messages.list` page size (API max 500).
const GMAIL_LIST_PAGE: u32 = 500;
/// Max concurrent `messages.get` requests per batch on one HTTP/2 connection (Gmail rate limits).
const GMAIL_METADATA_H2_BATCH: usize = 6;
/// When the UI does not send a visible range, prefetch metadata for this many newest rows first.
const GMAIL_METADATA_HEURISTIC_VISIBLE: usize = 12;

fn qp(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn gmail_json_object_one_field(key: &str, value: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key(key);
    w.write_string(value);
    w.write_end_object();
    w.take_buffer().to_vec()
}

fn gmail_modify_labels_body(add: &[&str], remove: &[&str]) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("addLabelIds");
    w.write_start_array();
    for s in add {
        w.write_string(s);
    }
    w.write_end_array();
    w.write_key("removeLabelIds");
    w.write_start_array();
    for s in remove {
        w.write_string(s);
    }
    w.write_end_array();
    w.write_end_object();
    w.take_buffer().to_vec()
}

fn gmail_send_body_raw_base64(raw_b64: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("raw");
    w.write_string(raw_b64);
    w.write_end_object();
    w.take_buffer().to_vec()
}

/// Gmail API returns system label `name` like `INBOX`; map to UI strings similar to Gmail web/IMAP.
fn gmail_label_list_name(label_id: &str, api_name: &str) -> String {
    match label_id {
        "INBOX" => "Inbox".to_owned(),
        "SENT" => "Sent".to_owned(),
        "DRAFT" => "Drafts".to_owned(),
        "SPAM" => "Spam".to_owned(),
        "TRASH" => "Trash".to_owned(),
        "STARRED" => "Starred".to_owned(),
        "IMPORTANT" => "Important".to_owned(),
        "CHAT" => "Chat".to_owned(),
        "UNREAD" => "Unread".to_owned(),
        "MUTED" => "Muted".to_owned(),
        "SNOOZED" => "Snoozed".to_owned(),
        id if id.starts_with("CATEGORY_") => match id {
            "CATEGORY_PERSONAL" => "Primary".to_owned(),
            "CATEGORY_SOCIAL" => "Social".to_owned(),
            "CATEGORY_PROMOTIONS" => "Promotions".to_owned(),
            "CATEGORY_UPDATES" => "Updates".to_owned(),
            "CATEGORY_FORUMS" => "Forums".to_owned(),
            _ => title_case_underscores(id.strip_prefix("CATEGORY_").unwrap_or(id)),
        },
        _ => api_name.to_owned(),
    }
}

fn title_case_underscores(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str().to_lowercase().as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone)]
struct GmailLabel {
    id: String,
    name: String,
}

pub struct GmailStore {
    email: String,
    uri: String,
    credential_key: String,
    use_keychain: bool,
    client_id: String,
    client_secret: String,
    runtime_handle: Handle,
    credentials_path: PathBuf,
    labels: Arc<RwLock<Vec<GmailLabel>>>,
}

impl GmailStore {
    pub fn new(
        email: impl Into<String>,
        credential_key: impl Into<String>,
        use_keychain: bool,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        runtime_handle: Handle,
    ) -> Result<Self, StoreError> {
        let email = email.into();
        let uri = crate::uri::gmail_store_uri(&email);
        let credentials_path = default_credentials_path()
            .ok_or_else(|| StoreError::new("no credentials path available"))?;
        Ok(Self {
            email,
            uri,
            credential_key: credential_key.into(),
            use_keychain,
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            runtime_handle,
            credentials_path,
            labels: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn access_token(&self) -> Result<String, StoreError> {
        let provider = GoogleOAuthProvider::new(&self.client_id, &self.client_secret);
        get_valid_access_token_for_store_credential(
            &self.credentials_path,
            &provider,
            self.credential_key.as_str(),
            self.use_keychain,
            &self.runtime_handle,
        )
        .map_err(|_| StoreError::NeedsCredential {
            username: self.email.clone(),
            is_plaintext: false,
            advertised_capabilities: None,
        })
    }

    fn label_id_by_name(&self, name: &str) -> Option<String> {
        let n = name.trim();
        self.labels
            .read()
            .ok()?
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(n) || l.id.eq_ignore_ascii_case(n))
            .map(|l| l.id.clone())
    }
}

impl Store for GmailStore {
    fn store_kind(&self) -> StoreKind {
        StoreKind::Email
    }

    fn list_folders(
        &self,
        on_folder: Box<dyn Fn(FolderInfo) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let labels_ref = self.labels.clone();
        self.runtime_handle.spawn(async move {
            let path = format!("{}/labels", GMAIL_BASE);
            let res = json_request(Method::Get, &path, &token, None).await;
            match res {
                Ok(body) => match parse::parse_labels_list(&body) {
                    Ok(rows) => {
                        let mut labels = Vec::new();
                        for (id, api_name) in rows {
                            if id.is_empty() || api_name.is_empty() {
                                continue;
                            }
                            let list_name = gmail_label_list_name(&id, &api_name);
                            labels.push(GmailLabel {
                                id: id.clone(),
                                name: list_name.clone(),
                            });
                            on_folder(FolderInfo {
                                name: list_name,
                                delimiter: Some('/'),
                                attributes: vec![],
                            });
                        }
                        if let Ok(mut guard) = labels_ref.write() {
                            *guard = labels;
                        }
                        on_complete(Ok(()));
                    }
                    Err(e) => on_complete(Err(e)),
                },
                Err(e) => on_complete(Err(e)),
            }
        });
    }

    fn open_folder(
        &self,
        name: &str,
        _on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send>,
    ) {
        let Some(label_id) = self.label_id_by_name(name).or_else(|| Some(name.to_string())) else {
            return on_complete(Err(StoreError::new("folder not found")));
        };
        on_complete(Ok(Box::new(GmailFolder {
            credential_key: self.credential_key.clone(),
            use_keychain: self.use_keychain,
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            credentials_path: self.credentials_path.clone(),
            runtime_handle: self.runtime_handle.clone(),
            label_id,
            labels: self.labels.clone(),
        })));
    }

    fn hierarchy_delimiter(&self) -> Option<char> {
        Some('/')
    }

    fn default_folder(&self) -> Option<&str> {
        Some("Inbox")
    }

    fn create_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let body = gmail_json_object_one_field("name", name);
        self.runtime_handle.spawn(async move {
            let path = format!("{}/labels", GMAIL_BASE);
            on_complete(json_request(Method::Post, &path, &token, Some(body)).await.map(|_| ()));
        });
    }

    fn rename_folder(
        &self,
        old_name: &str,
        new_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let Some(label_id) = self.label_id_by_name(old_name) else {
            return on_complete(Err(StoreError::new("folder not found")));
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let body = gmail_json_object_one_field("name", new_name);
        self.runtime_handle.spawn(async move {
            let path = format!("{}/labels/{}", GMAIL_BASE, label_id);
            on_complete(json_request(Method::Patch, &path, &token, Some(body)).await.map(|_| ()));
        });
    }

    fn delete_folder(
        &self,
        name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let Some(label_id) = self.label_id_by_name(name) else {
            return on_complete(Err(StoreError::new("folder not found")));
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        self.runtime_handle.spawn(async move {
            let path = format!("{}/labels/{}", GMAIL_BASE, label_id);
            on_complete(raw_request(Method::Delete, &path, &token, None).await.map(|_| ()));
        });
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct GmailFolder {
    credential_key: String,
    use_keychain: bool,
    client_id: String,
    client_secret: String,
    credentials_path: PathBuf,
    runtime_handle: Handle,
    label_id: String,
    labels: Arc<RwLock<Vec<GmailLabel>>>,
}

impl GmailFolder {
    fn access_token(&self) -> Result<String, StoreError> {
        let provider = GoogleOAuthProvider::new(&self.client_id, &self.client_secret);
        get_valid_access_token_for_store_credential(
            &self.credentials_path,
            &provider,
            self.credential_key.as_str(),
            self.use_keychain,
            &self.runtime_handle,
        )
        .map_err(|e| StoreError::new(format!("gmail oauth: {e}")))
    }

    fn label_id_by_name(&self, name: &str) -> Option<String> {
        let n = name.trim();
        self.labels
            .read()
            .ok()?
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(n) || l.id.eq_ignore_ascii_case(n))
            .map(|l| l.id.clone())
    }
}

/// Half-open [`range`] uses **oldest-first** indices (same as IMAP / mail window): `0` = oldest.
/// Gmail `messages.list` is **newest-first**, so we skip `total - range.end` ids then take `len`.
fn gmail_skip_take_for_oldest_first(total: u64, range: &Range<u64>) -> (u64, u64) {
    let end = range.end.min(total);
    let start = range.start.min(end);
    let take = end.saturating_sub(start);
    let skip_newest = total.saturating_sub(end);
    (skip_newest, take)
}

async fn gmail_label_messages_total(label_id: &str, token: &str) -> Result<u64, StoreError> {
    let path = format!("{}/labels/{}", GMAIL_BASE, qp(label_id));
    let body = json_request(Method::Get, &path, token, None).await?;
    parse::parse_label_messages_total(&body)
}

async fn gmail_list_one_page(
    label_id: &str,
    token: &str,
    max_results: u32,
    page_token: Option<&str>,
) -> Result<(Vec<String>, Option<String>), StoreError> {
    let mut path = format!(
        "{}/messages?labelIds={}&maxResults={}",
        GMAIL_BASE,
        qp(label_id),
        max_results
    );
    if let Some(pt) = page_token {
        path.push_str("&pageToken=");
        path.push_str(&qp(pt));
    }
    let body = json_request(Method::Get, &path, token, None).await?;
    parse::parse_messages_list_page(&body)
}

async fn gmail_collect_ids_for_window(
    label_id: &str,
    token: &str,
    skip_newest: u64,
    take: u64,
) -> Result<Vec<String>, StoreError> {
    let mut out = Vec::with_capacity(take.min(512) as usize);
    if take == 0 {
        return Ok(out);
    }
    let mut skipped: u64 = 0;
    let mut page_token: Option<String> = None;
    loop {
        let (page_ids, next) =
            gmail_list_one_page(label_id, token, GMAIL_LIST_PAGE, page_token.as_deref()).await?;
        for id in page_ids {
            if skipped < skip_newest {
                skipped += 1;
                continue;
            }
            out.push(id);
            if out.len() as u64 >= take {
                return Ok(out);
            }
        }
        if out.len() as u64 >= take {
            return Ok(out);
        }
        match next {
            Some(t) => page_token = Some(t),
            None => return Ok(out),
        }
    }
}

fn gmail_metadata_get_path(message_id: &str) -> String {
    format!(
        "{}/messages/{}?format=metadata&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Cc&metadataHeaders=Subject&metadataHeaders=Date&metadataHeaders=Message-ID&metadataHeaders=In-Reply-To&metadataHeaders=References",
        GMAIL_BASE,
        qp(message_id)
    )
}

fn gmail_attachment_api_path(message_id: &str, attachment_id: &str) -> String {
    format!(
        "{}/messages/{}/attachments/{}",
        GMAIL_BASE,
        qp(message_id),
        qp(attachment_id)
    )
}

/// `users.messages.attachments.get` — always a fresh `HttpClient::connect` via [`raw_request`].
async fn gmail_fetch_attachment_body(
    message_id: &str,
    attachment_id: &str,
    token: &str,
) -> Result<Vec<u8>, StoreError> {
    let path = gmail_attachment_api_path(message_id, attachment_id);
    let (_code, body) = raw_request(Method::Get, &path, token, None).await?;
    let data = parse::parse_attachment_data(&body)?;
    if data.is_empty() {
        return Err(StoreError::new(
            "gmail attachment: empty data (size may exceed download limit for this API shape)",
        ));
    }
    Ok(base64url_decode(&data))
}

/// Map oldest-first global rank `r` into `ids` index (`ids[0]` = newest in window).
fn gmail_rank_to_id_index(r: u64, window_start: u64, take: u64) -> Option<usize> {
    if take == 0 || r < window_start {
        return None;
    }
    let last_rank = window_start.saturating_add(take.saturating_sub(1));
    if r > last_rank {
        return None;
    }
    let offset = r - window_start;
    let idx = take - 1 - offset;
    usize::try_from(idx).ok()
}

/// Indices into `ids` (newest-first) to fetch first: viewport intersection, or a small head heuristic.
fn gmail_visible_id_indices(
    window_start: u64,
    ids_len: usize,
    visible: Option<(u64, u64)>,
) -> Vec<usize> {
    let take = ids_len as u64;
    let mut out = Vec::new();
    match visible {
        Some((vf, vl)) => {
            let win_end = window_start.saturating_add(take);
            let lo = vf.max(window_start);
            let hi = vl.min(win_end.saturating_sub(1));
            if lo <= hi {
                for r in lo..=hi {
                    if let Some(ix) = gmail_rank_to_id_index(r, window_start, take) {
                        if ix < ids_len {
                            out.push(ix);
                        }
                    }
                }
            }
        }
        None => {
            for i in 0..ids_len.min(GMAIL_METADATA_HEURISTIC_VISIBLE) {
                out.push(i);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Ordered batches: visible rows first (newest among visible = smallest index), then the rest.
fn gmail_metadata_fetch_batches(
    ids: &[String],
    window_start: u64,
    visible: Option<(u64, u64)>,
) -> Vec<Vec<String>> {
    let n = ids.len();
    if n == 0 {
        return Vec::new();
    }
    let vis = gmail_visible_id_indices(window_start, n, visible);
    let vis_set: HashSet<usize> = vis.iter().copied().collect();
    let mut batches: Vec<Vec<String>> = Vec::new();
    for chunk in vis.chunks(GMAIL_METADATA_H2_BATCH) {
        batches.push(chunk.iter().map(|&i| ids[i].clone()).collect());
    }
    let rest: Vec<usize> = (0..n).filter(|i| !vis_set.contains(i)).collect();
    for chunk in rest.chunks(GMAIL_METADATA_H2_BATCH) {
        batches.push(chunk.iter().map(|&i| ids[i].clone()).collect());
    }
    batches
}

async fn gmail_fetch_metadata_batch_h2(
    conn: &mut HttpConnection,
    tok: &str,
    batch: &[String],
) -> Result<Vec<ConversationSummary>, StoreError> {
    let mut req_batch = Vec::with_capacity(batch.len());
    for (j, mid) in batch.iter().enumerate() {
        let w = 200u8.saturating_sub(j as u8).max(16);
        let path = gmail_metadata_get_path(mid);
        let mut req = conn.request(Method::Get, path);
        req.header("Authorization", format!("Bearer {}", tok));
        req.header("Accept", "application/json");
        req_batch.push((req, w));
    }
    let results = conn
        .send_http2_parallel_gets(req_batch)
        .await
        .map_err(|e| StoreError::new(format!("gmail metadata multiplex: {e}")))?;
    let mut out = Vec::with_capacity(batch.len());
    for (k, r) in results.into_iter().enumerate() {
        let mid = batch[k].as_str();
        match r {
            Ok((code, body)) => {
                if !(200..300).contains(&code) {
                    return Err(StoreError::new(format!(
                        "gmail metadata {}: {}",
                        code,
                        String::from_utf8_lossy(&body)
                    )));
                }
                let meta = parse::parse_message_metadata(&body)?;
                out.push(gmail_summary_from_parsed(mid, &meta));
            }
            Err(e) => return Err(StoreError::new(e.to_string())),
        }
    }
    Ok(out)
}

/// HTTP/1.1 fallback: sequential batches of parallel GETs (new TCP each); cap concurrency per batch.
async fn gmail_fetch_metadata_summaries_h1_fallback(
    ids: &[String],
    token: &str,
    window_start: u64,
    visible: Option<(u64, u64)>,
) -> Result<Vec<ConversationSummary>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let batches = gmail_metadata_fetch_batches(ids, window_start, visible);
    let token = token.to_string();
    let mut map: HashMap<String, ConversationSummary> = HashMap::with_capacity(ids.len());
    for batch in batches {
        let mut set = tokio::task::JoinSet::new();
        for mid in batch {
            let tok = token.clone();
            set.spawn(async move {
                let path = gmail_metadata_get_path(&mid);
                let body = json_request(Method::Get, &path, &tok, None).await?;
                let meta = parse::parse_message_metadata(&body)?;
                Ok::<ConversationSummary, StoreError>(gmail_summary_from_parsed(&mid, &meta))
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(s)) => {
                    map.insert(s.id.as_str().to_string(), s);
                }
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(StoreError::new(format!("gmail metadata task: {e}"))),
            }
        }
    }
    ids.iter()
        .map(|id| {
            map.remove(id).ok_or_else(|| {
                StoreError::new(format!("gmail metadata missing id after fetch: {id}"))
            })
        })
        .collect()
}

/// One TLS connection when ALPN is HTTP/2; metadata GETs run in small batches, one batch at a time.
async fn gmail_fetch_metadata_summaries(
    ids: &[String],
    token: &str,
    window_start: u64,
    visible: Option<(u64, u64)>,
) -> Result<Vec<ConversationSummary>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut conn = HttpClient::connect(GMAIL_HOST, GMAIL_PORT, true)
        .await
        .map_err(|e| StoreError::new(format!("gmail connect: {e}")))?;
    if conn.version() != HttpVersion::Http2 {
        return gmail_fetch_metadata_summaries_h1_fallback(ids, token, window_start, visible).await;
    }
    let batches = gmail_metadata_fetch_batches(ids, window_start, visible);
    let tok = token.to_string();
    let mut map: HashMap<String, ConversationSummary> = HashMap::with_capacity(ids.len());
    for batch in batches {
        if batch.is_empty() {
            continue;
        }
        let part = gmail_fetch_metadata_batch_h2(&mut conn, &tok, &batch).await?;
        for s in part {
            map.insert(s.id.as_str().to_string(), s);
        }
    }
    ids.iter()
        .map(|id| {
            map.remove(id).ok_or_else(|| {
                StoreError::new(format!("gmail metadata missing id after fetch: {id}"))
            })
        })
        .collect()
}

impl Folder for GmailFolder {
    fn list_conversations(
        &self,
        range: std::ops::Range<u64>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        self.list_conversations_with_visible_ranks(range, None, on_summary, on_complete);
    }

    fn list_conversations_with_visible_ranks(
        &self,
        range: std::ops::Range<u64>,
        visible_ranks_inclusive: Option<(u64, u64)>,
        on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let label_id = self.label_id.clone();
        self.runtime_handle.spawn(async move {
            let outcome: Result<(), StoreError> = async {
                let total = gmail_label_messages_total(&label_id, &token).await?;
                let (skip_newest, take) = gmail_skip_take_for_oldest_first(total, &range);
                if take == 0 {
                    return Ok(());
                }
                let ids =
                    gmail_collect_ids_for_window(&label_id, &token, skip_newest, take).await?;
                let summaries = gmail_fetch_metadata_summaries(
                    &ids,
                    &token,
                    range.start,
                    visible_ranks_inclusive,
                )
                .await?;
                for s in summaries {
                    on_summary(s);
                }
                Ok(())
            }
            .await;
            on_complete(outcome);
        });
    }

    fn message_count(&self, on_complete: Box<dyn FnOnce(Result<u64, StoreError>) + Send>) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let label_id = self.label_id.clone();
        self.runtime_handle.spawn(async move {
            match gmail_label_messages_total(&label_id, &token).await {
                Ok(n) => on_complete(Ok(n)),
                Err(e) => on_complete(Err(e)),
            }
        });
    }

    fn get_message(
        &self,
        id: &MessageId,
        on_metadata: Box<dyn Fn(Envelope) + Send + Sync>,
        on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync>,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let id = id.to_string();
        self.runtime_handle.spawn(async move {
            let metadata_path = format!(
                "{}/messages/{}?format=metadata&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Cc&metadataHeaders=Subject&metadataHeaders=Date&metadataHeaders=Message-ID&metadataHeaders=In-Reply-To&metadataHeaders=References",
                GMAIL_BASE, id
            );
            let raw_path = format!("{}/messages/{}?format=raw", GMAIL_BASE, id);
            let md = match json_request(Method::Get, &metadata_path, &token, None).await {
                Ok(b) => b,
                Err(e) => return on_complete(Err(e)),
            };
            let meta = match parse::parse_message_metadata(&md) {
                Ok(m) => m,
                Err(e) => return on_complete(Err(e)),
            };
            on_metadata(gmail_envelope_from_parsed(&meta));
            match json_request(Method::Get, &raw_path, &token, None).await {
                Ok(b) => {
                    let raw = match parse::parse_message_raw_field(&b) {
                        Ok(r) => r,
                        Err(e) => return on_complete(Err(e)),
                    };
                    let bytes = base64url_decode(&raw);
                    on_content_chunk(&bytes);
                    on_complete(Ok(()));
                }
                Err(e) => on_complete(Err(e)),
            }
        });
    }

    fn get_message_display(
        &self,
        id: &MessageId,
        on_done: Box<dyn FnOnce(Result<MessageForDisplay, StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_done(Err(e)),
        };
        let id = id.to_string();
        self.runtime_handle.spawn(async move {
            let path = format!("{}/messages/{}?format=full", GMAIL_BASE, id);
            match json_request(Method::Get, &path, &token, None).await {
                Ok(body) => match parse::parse_message_full(&body) {
                    Ok(full) => on_done(Ok(gmail_message_for_display_from_full(full))),
                    Err(e) => on_done(Err(e)),
                },
                Err(e) => on_done(Err(e)),
            }
        });
    }

    fn fetch_message_part(
        &self,
        id: &MessageId,
        imap_section: &str,
        _transfer_encoding: &str,
        on_done: Box<dyn FnOnce(Result<Vec<u8>, StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_done(Err(e)),
        };
        let section = imap_section.trim();
        let Some(rest) = section.strip_prefix(GMAIL_ATTACHMENT_SECTION_PREFIX) else {
            return on_done(Err(StoreError::new(
                "Gmail attachment download: imapSection must be gmail:<attachmentId> (from message detail JSON)",
            )));
        };
        let attachment_id = rest.trim();
        if attachment_id.is_empty() {
            return on_done(Err(StoreError::new(
                "Gmail attachment download: empty attachment id",
            )));
        }
        let mid = id.to_string();
        let aid = attachment_id.to_string();
        self.runtime_handle.spawn(async move {
            let r = gmail_fetch_attachment_body(&mid, &aid, &token).await;
            on_done(r);
        });
    }

    fn delete_message(
        &self,
        id: &MessageId,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let path = format!("{}/messages/{}/trash", GMAIL_BASE, id);
        self.runtime_handle.spawn(async move {
            on_complete(raw_request(Method::Post, &path, &token, None).await.map(|_| ()));
        });
    }

    fn copy_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let Some(dest_label_id) = self.label_id_by_name(dest_folder_name) else {
            return on_complete(Err(StoreError::new("destination folder not found")));
        };
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let current = self.label_id.clone();
        let ids = ids.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
        self.runtime_handle.spawn(async move {
            for id in ids {
                let body = gmail_modify_labels_body(&[&dest_label_id], &[&current]);
                let path = format!("{}/messages/{}/modify", GMAIL_BASE, id);
                if let Err(e) = json_request(Method::Post, &path, &token, Some(body)).await {
                    return on_complete(Err(e));
                }
            }
            on_complete(Ok(()));
        });
    }

    fn move_messages_to(
        &self,
        ids: &[&str],
        dest_folder_name: &str,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        self.copy_messages_to(ids, dest_folder_name, on_complete);
    }

    fn store_flags(
        &self,
        ids: &[&str],
        add: &[Flag],
        remove: &[Flag],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let mut add_labels = Vec::new();
        let mut rem_labels = Vec::new();
        for f in add {
            match f {
                Flag::Seen => rem_labels.push("UNREAD"),
                Flag::Flagged => add_labels.push("STARRED"),
                _ => {}
            }
        }
        for f in remove {
            match f {
                Flag::Seen => add_labels.push("UNREAD"),
                Flag::Flagged => rem_labels.push("STARRED"),
                _ => {}
            }
        }
        let ids = ids.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
        self.runtime_handle.spawn(async move {
            for id in ids {
                let body = gmail_modify_labels_body(&add_labels, &rem_labels);
                let path = format!("{}/messages/{}/modify", GMAIL_BASE, id);
                if let Err(e) = json_request(Method::Post, &path, &token, Some(body)).await {
                    return on_complete(Err(e));
                }
            }
            on_complete(Ok(()));
        });
    }

    fn mark_all_read(&self, on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let body = gmail_modify_labels_body(&[], &["UNREAD"]);
        let path = format!("{}/messages/batchModify", GMAIL_BASE);
        self.runtime_handle.spawn(async move {
            on_complete(json_request(Method::Post, &path, &token, Some(body)).await.map(|_| ()));
        });
    }
}

pub struct GmailTransport {
    email: String,
    uri: String,
    credential_key: String,
    use_keychain: bool,
    client_id: String,
    client_secret: String,
    runtime_handle: Handle,
    credentials_path: PathBuf,
}

impl GmailTransport {
    pub fn new(
        email: impl Into<String>,
        credential_key: impl Into<String>,
        use_keychain: bool,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        runtime_handle: Handle,
    ) -> Result<Self, StoreError> {
        let email = email.into();
        let uri = crate::uri::gmail_store_uri(&email);
        let credentials_path = default_credentials_path()
            .ok_or_else(|| StoreError::new("no credentials path available"))?;
        Ok(Self {
            email,
            uri,
            credential_key: credential_key.into(),
            use_keychain,
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            runtime_handle,
            credentials_path,
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn access_token(&self) -> Result<String, StoreError> {
        let provider = GoogleOAuthProvider::new(&self.client_id, &self.client_secret);
        get_valid_access_token_for_store_credential(
            &self.credentials_path,
            &provider,
            self.credential_key.as_str(),
            self.use_keychain,
            &self.runtime_handle,
        )
        .map_err(|_| StoreError::NeedsCredential {
            username: self.email.clone(),
            is_plaintext: false,
            advertised_capabilities: None,
        })
    }
}

impl Transport for GmailTransport {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Email
    }

    fn send(
        &self,
        payload: &SendPayload,
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let (message, _) = crate::protocol::smtp::build_rfc822_from_payload(payload);
        let body = gmail_send_body_raw_base64(&base64url_encode(message.as_slice()));
        self.runtime_handle.spawn(async move {
            let path = format!("{}/messages/send", GMAIL_BASE);
            on_complete(json_request(Method::Post, &path, &token, Some(body)).await.map(|_| ()));
        });
    }
}

impl GmailTransport {
    /// Send a complete RFC 822 message (e.g. after S/MIME or OpenPGP transforms).
    pub fn send_prebuilt_rfc822(
        &self,
        raw: &[u8],
        on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send>,
    ) {
        let token = match self.access_token() {
            Ok(t) => t,
            Err(e) => return on_complete(Err(e)),
        };
        let body = gmail_send_body_raw_base64(&base64url_encode(raw));
        self.runtime_handle.spawn(async move {
            let path = format!("{}/messages/send", GMAIL_BASE);
            on_complete(json_request(Method::Post, &path, &token, Some(body)).await.map(|_| ()));
        });
    }
}

fn gmail_envelope_from_parsed(meta: &parse::GmailMessageMetadataParsed) -> Envelope {
    let headers = &meta.headers;
    Envelope {
        from: parse_address_list(headers.get("From").cloned().unwrap_or_default().as_str()),
        to: parse_address_list(headers.get("To").cloned().unwrap_or_default().as_str()),
        cc: parse_address_list(headers.get("Cc").cloned().unwrap_or_default().as_str()),
        date: headers
            .get("Date")
            .and_then(|s| parse_rfc822_datetime(s))
            .or_else(|| {
                meta.internal_date_ms.map(|ms| DateTime {
                    timestamp: ms / 1000,
                    tz_offset_secs: Some(0),
                })
            }),
        subject: headers.get("Subject").cloned(),
        message_id: headers.get("Message-ID").cloned(),
        in_reply_to: headers.get("In-Reply-To").cloned(),
        references: headers.get("References").cloned(),
    }
}

fn gmail_envelope_from_full(f: &parse::GmailMessageFullParsed) -> Envelope {
    let headers = &f.envelope_headers;
    Envelope {
        from: parse_address_list(headers.get("From").cloned().unwrap_or_default().as_str()),
        to: parse_address_list(headers.get("To").cloned().unwrap_or_default().as_str()),
        cc: parse_address_list(headers.get("Cc").cloned().unwrap_or_default().as_str()),
        date: headers
            .get("Date")
            .and_then(|s| parse_rfc822_datetime(s))
            .or_else(|| {
                f.internal_date_ms.map(|ms| DateTime {
                    timestamp: ms / 1000,
                    tz_offset_secs: Some(0),
                })
            }),
        subject: headers.get("Subject").cloned(),
        message_id: headers.get("Message-ID").cloned(),
        in_reply_to: headers.get("In-Reply-To").cloned(),
        references: headers.get("References").cloned(),
    }
}

fn gmail_summary_from_parsed(
    message_id: &str,
    meta: &parse::GmailMessageMetadataParsed,
) -> ConversationSummary {
    let envelope = gmail_envelope_from_parsed(meta);
    let mut flags = HashSet::new();
    let has_label = |needle: &str| meta.label_ids.iter().any(|s| s == needle);
    if !has_label("UNREAD") {
        flags.insert(Flag::Seen);
    }
    if has_label("STARRED") {
        flags.insert(Flag::Flagged);
    }
    if has_label("DRAFT") {
        flags.insert(Flag::Draft);
    }
    ConversationSummary {
        id: MessageId::new(message_id),
        envelope,
        flags,
        size: meta.size_estimate,
    }
}

fn gmail_message_for_display_from_full(full: parse::GmailMessageFullParsed) -> MessageForDisplay {
    let envelope = gmail_envelope_from_full(&full);
    let body_html = full.payload.find_part_body("text/html");
    let body_plain = full.payload.find_part_body("text/plain");
    let mut attachments = Vec::new();
    full.payload.collect_attachments(&mut attachments);
    MessageForDisplay {
        envelope,
        body_plain,
        body_html,
        attachments,
    }
}

fn parse_address_list(raw: &str) -> Vec<Address> {
    raw.split(',')
        .filter_map(|part| {
            let t = part.trim();
            if t.is_empty() {
                return None;
            }
            if let (Some(lt), Some(gt)) = (t.find('<'), t.rfind('>')) {
                let name = t[..lt].trim().trim_matches('"').to_string();
                return parse_email_addr(&t[lt + 1..gt], if name.is_empty() { None } else { Some(name) });
            }
            parse_email_addr(t, None)
        })
        .collect()
}

fn parse_email_addr(raw: &str, display_name: Option<String>) -> Option<Address> {
    let t = raw.trim();
    let at = t.find('@')?;
    Some(Address {
        display_name,
        local_part: t[..at].to_string(),
        domain: Some(t[at + 1..].to_string()),
    })
}

fn parse_rfc822_datetime(_raw: &str) -> Option<DateTime> {
    None
}

async fn json_request(
    method: Method,
    path: &str,
    token: &str,
    body: Option<Vec<u8>>,
) -> Result<Vec<u8>, StoreError> {
    let (_, response_body) = raw_request(method, path, token, body).await?;
    Ok(response_body)
}

async fn raw_request(
    method: Method,
    path: &str,
    token: &str,
    body: Option<Vec<u8>>,
) -> Result<(u16, Vec<u8>), StoreError> {
    let mut conn = HttpClient::connect(GMAIL_HOST, GMAIL_PORT, true)
        .await
        .map_err(|e| StoreError::new(format!("gmail connect: {e}")))?;
    let mut req: RequestBuilder = conn.request(method, path.to_string());
    req.header("Authorization", format!("Bearer {token}"));
    req.header("Accept", "application/json");
    if let Some(b) = body {
        req.header("Content-Type", "application/json");
        req.header("Content-Length", b.len().to_string());
        req.body(b);
    }
    log_http_request(
        "gmail",
        method.as_str(),
        path,
        req.body.as_deref(),
    );
    let status = Arc::new(Mutex::new(0u16));
    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    let failed = Arc::new(Mutex::new(None::<String>));
    let handler = CollectResponseHandler {
        status: status.clone(),
        body: output.clone(),
        failed: failed.clone(),
    };
    conn.send(req, handler)
        .await
        .map_err(|e| StoreError::new(format!("gmail http send: {e}")))?;
    if let Some(e) = failed.lock().ok().and_then(|g| g.clone()) {
        return Err(StoreError::new(e));
    }
    let code = *status.lock().map_err(|_| StoreError::new("gmail status lock"))?;
    let body = output.lock().map_err(|_| StoreError::new("gmail body lock"))?.clone();
    log_http_response("gmail", code, &body);
    if (200..300).contains(&code) {
        Ok((code, body))
    } else {
        Err(StoreError::new(format!(
            "gmail http {}: {}",
            code,
            String::from_utf8_lossy(&body)
        )))
    }
}

struct CollectResponseHandler {
    status: Arc<Mutex<u16>>,
    body: Arc<Mutex<Vec<u8>>>,
    failed: Arc<Mutex<Option<String>>>,
}

impl ResponseHandler for CollectResponseHandler {
    fn ok(&mut self, response: Response) {
        if let Ok(mut s) = self.status.lock() {
            *s = response.code;
        }
    }

    fn error(&mut self, response: Response) {
        if let Ok(mut s) = self.status.lock() {
            *s = response.code;
        }
    }

    fn header(&mut self, _name: &str, _value: &str) {}
    fn start_body(&mut self) {}
    fn body_chunk(&mut self, data: &[u8]) {
        if let Ok(mut b) = self.body.lock() {
            b.extend_from_slice(data);
        }
    }
    fn end_body(&mut self) {}
    fn complete(&mut self) {}
    fn failed(&mut self, error: &std::io::Error) {
        if let Ok(mut f) = self.failed.lock() {
            *f = Some(format!("gmail request failed: {error}"));
        }
    }
}

pub(super) fn base64url_decode(input: &str) -> Vec<u8> {
    let mut s = input.replace('-', "+").replace('_', "/");
    while s.len() % 4 != 0 {
        s.push('=');
    }
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    STANDARD.decode(s).unwrap_or_default()
}

fn base64url_encode(input: &[u8]) -> String {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    STANDARD
        .encode(input)
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}
