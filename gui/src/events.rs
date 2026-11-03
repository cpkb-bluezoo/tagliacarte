//! Events for the page, one small JSON object each. Every object has a
//! `type` member, and keys are camelCase. Session events come from
//! `AppEvent`; the configuration is delivered the same way, as a sequence
//! (`configStarted`, one `configAccount` or `configTransport` per entry,
//! `configLoaded`) so nothing has to be built or held as one large document.

use std::collections::HashMap;

use tagliacarte_app::frb_api::{FrbAccount, FrbConfig, FrbTransport};
use tagliacarte_app::mail_view::{AttachmentInfo, Headers, Mailbox, OpenEvent};
use tagliacarte_app::session::{AppEvent, MessageListRowSummary, SubscriptionAvailableRow};
use tagliacarte_core::json::{JsonWriter, writer_into_string};

pub(crate) fn opt_str(w: &mut JsonWriter, key: &str, v: &Option<String>) {
    if let Some(s) = v {
        w.write_key(key);
        w.write_string(s);
    }
}

pub(crate) fn str(w: &mut JsonWriter, key: &str, v: &str) {
    w.write_key(key);
    w.write_string(v);
}

pub(crate) fn int(w: &mut JsonWriter, key: &str, v: i64) {
    w.write_key(key);
    w.write_i64(v);
}

pub(crate) fn flag(w: &mut JsonWriter, key: &str, v: bool) {
    w.write_key(key);
    w.write_bool(v);
}

pub(crate) fn string_array(w: &mut JsonWriter, key: &str, items: &[String]) {
    w.write_key(key);
    w.write_start_array();
    for s in items {
        w.write_string(s);
    }
    w.write_end_array();
}

fn count_map(w: &mut JsonWriter, key: &str, map: &HashMap<String, u32>) {
    w.write_key(key);
    w.write_start_object();
    for (k, v) in map {
        w.write_key(k);
        w.write_i64(i64::from(*v));
    }
    w.write_end_object();
}

fn name_map(w: &mut JsonWriter, key: &str, map: &HashMap<String, String>) {
    w.write_key(key);
    w.write_start_object();
    for (k, v) in map {
        str(w, k, v);
    }
    w.write_end_object();
}

fn available_row(w: &mut JsonWriter, row: &SubscriptionAvailableRow) {
    w.write_start_object();
    str(w, "id", &row.id);
    w.write_key("isSubscribed");
    w.write_bool(row.is_subscribed);
    opt_str(w, "displayName", &row.display_name);
    if let Some(u) = row.unread {
        int(w, "unread", i64::from(u));
    }
    w.write_key("allowUnsubscribe");
    w.write_bool(row.allow_unsubscribe);
    w.write_end_object();
}

fn row_summary(w: &mut JsonWriter, s: &MessageListRowSummary) {
    w.write_key("summary");
    w.write_start_object();
    str(w, "id", &s.id);
    str(w, "from", &s.from);
    str(w, "subject", &s.subject);
    if let Some(ms) = s.date_ms {
        int(w, "dateMs", ms);
    }
    w.write_key("isRead");
    w.write_bool(s.is_read);
    w.write_key("markedForDeletion");
    w.write_bool(s.marked_for_deletion);
    opt_str(w, "nostrSenderPubkeyHex", &s.nostr_sender_pubkey_hex);
    w.write_end_object();
}

pub fn event_json(ev: &AppEvent) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    match ev {
        AppEvent::AccountConnectionChanged {
            account_id,
            store_kind,
            connection_state,
            message,
        } => {
            str(&mut w, "type", "accountConnectionChanged");
            str(&mut w, "accountId", account_id);
            str(&mut w, "storeKind", store_kind);
            str(&mut w, "connectionState", connection_state);
            opt_str(&mut w, "message", message);
        }
        AppEvent::FolderListUpdated {
            account_id,
            folders,
            hierarchy_delimiter,
            unread_by_folder,
            folder_display_names,
            subscription_available,
            matrix_dm_folder_ids,
        } => {
            str(&mut w, "type", "folderListUpdated");
            str(&mut w, "accountId", account_id);
            string_array(&mut w, "folders", folders);
            opt_str(&mut w, "hierarchyDelimiter", hierarchy_delimiter);
            count_map(&mut w, "unreadByFolder", unread_by_folder);
            name_map(&mut w, "folderDisplayNames", folder_display_names);
            if let Some(rows) = subscription_available {
                w.write_key("subscriptionAvailable");
                w.write_start_array();
                for row in rows {
                    available_row(&mut w, row);
                }
                w.write_end_array();
            }
            if let Some(ids) = matrix_dm_folder_ids {
                string_array(&mut w, "matrixDmFolderIds", ids);
            }
        }
        AppEvent::FolderFound {
            account_id,
            folder_name,
            unread,
        } => {
            str(&mut w, "type", "folderFound");
            str(&mut w, "accountId", account_id);
            str(&mut w, "folderName", folder_name);
            int(&mut w, "unread", i64::from(*unread));
        }
        AppEvent::FolderListFailed { account_id, message } => {
            str(&mut w, "type", "folderListFailed");
            str(&mut w, "accountId", account_id);
            str(&mut w, "message", message);
        }
        AppEvent::MessageFlagsChanged {
            account_id,
            folder,
            message_id,
            is_read,
        } => {
            str(&mut w, "type", "messageFlagsChanged");
            str(&mut w, "accountId", account_id);
            str(&mut w, "folder", folder);
            str(&mut w, "messageId", message_id);
            w.write_key("isRead");
            w.write_bool(*is_read);
        }
        AppEvent::MessageListWindowStarted {
            request_id,
            account_id,
            folder_name,
            message_list_sort,
            total,
            start_index,
            list_strategy,
            row_count,
            list_ready,
        } => {
            str(&mut w, "type", "messageListWindowStarted");
            str(&mut w, "requestId", request_id);
            str(&mut w, "accountId", account_id);
            str(&mut w, "folderName", folder_name);
            str(&mut w, "messageListSort", message_list_sort);
            int(&mut w, "total", *total as i64);
            int(&mut w, "startIndex", *start_index as i64);
            str(&mut w, "listStrategy", list_strategy);
            int(&mut w, "rowCount", i64::from(*row_count));
            w.write_key("listReady");
            w.write_bool(*list_ready);
        }
        AppEvent::MessageListRowFound {
            request_id,
            account_id,
            folder_name,
            message_list_sort,
            rank,
            summary,
        } => {
            str(&mut w, "type", "messageListRowFound");
            str(&mut w, "requestId", request_id);
            str(&mut w, "accountId", account_id);
            str(&mut w, "folderName", folder_name);
            str(&mut w, "messageListSort", message_list_sort);
            int(&mut w, "rank", *rank as i64);
            row_summary(&mut w, summary);
        }
        AppEvent::MessageListWindowComplete {
            request_id,
            account_id,
            folder_name,
            message_list_sort,
            error,
        } => {
            str(&mut w, "type", "messageListWindowComplete");
            str(&mut w, "requestId", request_id);
            str(&mut w, "accountId", account_id);
            str(&mut w, "folderName", folder_name);
            str(&mut w, "messageListSort", message_list_sort);
            opt_str(&mut w, "error", error);
        }
        AppEvent::CommandResult {
            request_id,
            ok,
            error,
        } => {
            str(&mut w, "type", "commandResult");
            opt_str(&mut w, "requestId", request_id);
            w.write_key("ok");
            w.write_bool(*ok);
            opt_str(&mut w, "error", error);
        }
        AppEvent::NostrProfileUpdated {
            account_id,
            pubkey_hex,
            npub,
            display_name,
            nip05,
            picture,
        } => {
            str(&mut w, "type", "nostrProfileUpdated");
            str(&mut w, "accountId", account_id);
            str(&mut w, "pubkeyHex", pubkey_hex);
            str(&mut w, "npub", npub);
            opt_str(&mut w, "displayName", display_name);
            opt_str(&mut w, "nip05", nip05);
            opt_str(&mut w, "picture", picture);
        }
    }
    w.write_end_object();
    writer_into_string(w)
}

fn account_event(a: &FrbAccount) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "configAccount");
    str(&mut w, "id", &a.id);
    str(&mut w, "label", &a.label);
    str(&mut w, "backendType", &a.backend_type);
    opt_str(&mut w, "avatarUrl", &a.avatar_url);
    opt_str(&mut w, "lastFolder", &a.last_folder);
    opt_str(&mut w, "lastMessageId", &a.last_message_id);
    name_map(&mut w, "attrs", &a.attrs);
    w.write_key("lists");
    w.write_start_object();
    for (k, items) in &a.lists {
        string_array(&mut w, k, items);
    }
    w.write_end_object();
    w.write_end_object();
    writer_into_string(w)
}

fn transport_event(t: &FrbTransport) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "configTransport");
    str(&mut w, "id", &t.id);
    str(&mut w, "transportType", &t.transport_type);
    str(&mut w, "displayName", &t.display_name);
    str(&mut w, "host", &t.host);
    int(&mut w, "port", i64::from(t.port));
    str(&mut w, "security", &t.security);
    str(&mut w, "defaultFrom", &t.default_from);
    str(&mut w, "dsnNotify", &t.dsn_notify);
    str(&mut w, "oauthProvider", &t.oauth_provider);
    w.write_end_object();
    writer_into_string(w)
}

/// The settings that are not accounts or transports. The OpenPGP passphrase
/// is a secret and never leaves the back end.
pub fn config_loaded_event(c: &FrbConfig) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "configLoaded");
    opt_str(&mut w, "selectedStoreId", &c.selected_store_id);
    str(&mut w, "dateFormat", &c.date_format);
    str(&mut w, "resourcePolicy", &c.resource_policy);
    flag(&mut w, "useKeychain", c.use_keychain);
    flag(&mut w, "loadRemoteImages", c.load_remote_images);
    flag(&mut w, "threadedView", c.threaded_view);
    flag(&mut w, "quoteOriginal", c.quote_original);
    str(&mut w, "replyHeaderTemplate", &c.reply_header_template);
    str(&mut w, "replyDateFormat", &c.reply_date_format);
    str(&mut w, "replyTimeFormat", &c.reply_time_format);
    str(&mut w, "replyLinePrefix", &c.reply_line_prefix);
    str(&mut w, "replyQuoteMode", &c.reply_quote_mode);
    str(&mut w, "replyPlainPosition", &c.reply_plain_position);
    str(&mut w, "messageListSort", &c.message_list_sort);
    flag(&mut w, "notifyNewMessages", c.notify_new_messages);
    flag(&mut w, "composeUseRichText", c.compose_use_rich_text);
    flag(&mut w, "matrixChatUseRichText", c.matrix_chat_use_rich_text);
    str(&mut w, "mailCryptoPgpSecretKeyPath", &c.mail_crypto_pgp_secret_key_path);
    flag(&mut w, "mailCryptoPgpHasPassphrase", !c.mail_crypto_pgp_passphrase.is_empty());
    str(&mut w, "mailCryptoSmimeCertPath", &c.mail_crypto_smime_cert_path);
    str(&mut w, "mailCryptoSmimeKeyPath", &c.mail_crypto_smime_key_path);
    str(&mut w, "mailCryptoStack", &c.mail_crypto_stack);
    w.write_end_object();
    writer_into_string(w)
}

pub(crate) fn mailboxes(w: &mut JsonWriter, key: &str, list: &[Mailbox]) {
    w.write_key(key);
    w.write_start_array();
    for m in list {
        w.write_start_object();
        opt_str(w, "name", &m.name);
        str(w, "email", &m.email);
        w.write_end_object();
    }
    w.write_end_array();
}

fn headers_event(request_id: &str, h: &Headers) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "messageHeaders");
    str(&mut w, "requestId", request_id);
    mailboxes(&mut w, "from", &h.from);
    mailboxes(&mut w, "to", &h.to);
    mailboxes(&mut w, "cc", &h.cc);
    opt_str(&mut w, "subject", &h.subject);
    if let Some(ms) = h.date_ms {
        int(&mut w, "dateMs", ms);
    }
    opt_str(&mut w, "messageId", &h.message_id);
    w.write_end_object();
    writer_into_string(w)
}

fn attachment_event(request_id: &str, a: &AttachmentInfo) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "messageAttachment");
    str(&mut w, "requestId", request_id);
    int(&mut w, "index", i64::from(a.index));
    opt_str(&mut w, "filename", &a.filename);
    str(&mut w, "contentType", &a.content_type);
    int(&mut w, "size", a.size as i64);
    opt_str(&mut w, "contentId", &a.content_id);
    flag(&mut w, "invite", is_calendar_invite(&a.content_type, a.filename.as_deref()));
    w.write_end_object();
    writer_into_string(w)
}

/// Whether an attachment is an invitation or other iCalendar file, which can be
/// added to the calendar: by its type, or by `.ics` for a sender that says
/// `application/octet-stream` of everything.
pub fn is_calendar_invite(content_type: &str, filename: Option<&str>) -> bool {
    let media = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    media == "text/calendar"
        || media == "application/ics"
        || filename.is_some_and(|f| f.trim().to_ascii_lowercase().ends_with(".ics"))
}

/// What `mail_view::open_message` reports, as page events.
pub fn open_event_json(request_id: &str, ev: &OpenEvent) -> String {
    match ev {
        OpenEvent::Headers(h) => headers_event(request_id, h),
        OpenEvent::Attachment(a) => attachment_event(request_id, a),
        OpenEvent::Signature(status) => {
            let mut w = JsonWriter::new();
            w.write_start_object();
            str(&mut w, "type", "messageSignature");
            str(&mut w, "requestId", request_id);
            str(&mut w, "status", status);
            w.write_end_object();
            writer_into_string(w)
        }
        OpenEvent::Body { html } => {
            let mut w = JsonWriter::new();
            w.write_start_object();
            str(&mut w, "type", "messageBody");
            str(&mut w, "requestId", request_id);
            flag(&mut w, "html", *html);
            w.write_end_object();
            writer_into_string(w)
        }
    }
}

/// The end of a `messageHeaders` / `messageAttachment` sequence.
pub fn opened_json(request_id: &str, error: Option<&str>) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "messageOpened");
    str(&mut w, "requestId", request_id);
    if let Some(e) = error {
        str(&mut w, "error", e);
    }
    w.write_end_object();
    writer_into_string(w)
}

/// The end of something that ran in the background (`kind` is its event
/// type): the request id it was started with and, if it failed, why.
pub fn done_json(kind: &str, request_id: &str, error: Option<&str>) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", kind);
    str(&mut w, "requestId", request_id);
    if let Some(e) = error {
        str(&mut w, "error", e);
    }
    w.write_end_object();
    writer_into_string(w)
}

/// How a save of an attachment ended.
pub enum Saved {
    Done { path: String, bytes: u64 },
    /// The save dialog was dismissed.
    Cancelled,
    Failed(String),
}

/// The end of `attachment.save`: `path` and `bytes`, `cancelled`, or `error`.
pub fn saved_json(request_id: &str, outcome: &Saved) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "attachmentSaved");
    str(&mut w, "requestId", request_id);
    match outcome {
        Saved::Done { path, bytes } => {
            str(&mut w, "path", path);
            int(&mut w, "bytes", i64::try_from(*bytes).unwrap_or(i64::MAX));
        }
        Saved::Cancelled => flag(&mut w, "cancelled", true),
        Saved::Failed(e) => str(&mut w, "error", e),
    }
    w.write_end_object();
    writer_into_string(w)
}

/// Files were dropped on the window. They arrive as paths; their bytes are not read.
pub fn files_dropped_json(paths: &[std::path::PathBuf]) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "filesDropped");
    w.write_key("paths");
    w.write_start_array();
    for p in paths {
        w.write_string(&p.to_string_lossy());
    }
    w.write_end_array();
    w.write_end_object();
    writer_into_string(w)
}

/// Files are being dragged over the window, or no longer are.
pub fn files_dragging_json(active: bool) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "filesDragging");
    flag(&mut w, "active", active);
    w.write_end_object();
    writer_into_string(w)
}

/// The answer to a file dialog; `path` is absent when it was cancelled.
pub fn path_picked_json(request_id: &str, path: Option<&str>) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "pathPicked");
    str(&mut w, "requestId", request_id);
    if let Some(p) = path {
        str(&mut w, "path", p);
    }
    w.write_end_object();
    writer_into_string(w)
}

/// A link in a message body was followed; the webview did not navigate.
pub fn link_json(url: &str) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "linkClicked");
    str(&mut w, "url", url);
    w.write_end_object();
    writer_into_string(w)
}

/// How many remote images the body of a message holds. The body is loaded by the
/// page's frame, not asked for with a request, so this is by message.
pub fn remote_content_json(account_id: &str, folder: &str, message_id: &str, count: usize) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "messageRemote");
    str(&mut w, "accountId", account_id);
    str(&mut w, "folder", folder);
    str(&mut w, "messageId", message_id);
    int(&mut w, "count", i64::try_from(count).unwrap_or(i64::MAX));
    w.write_end_object();
    writer_into_string(w)
}

/// The title the window really has (self-test only).
pub fn window_title_json(title: &str) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "windowTitle");
    str(&mut w, "title", title);
    w.write_end_object();
    writer_into_string(w)
}

/// An item of the native menu that the page acts on was chosen.
pub fn menu_action_json(id: &str) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    str(&mut w, "type", "menuAction");
    str(&mut w, "id", id);
    w.write_end_object();
    writer_into_string(w)
}

/// The configuration as a sequence of events: `configStarted`, then one
/// event per account and transport, then `configLoaded` as the authoritative
/// end (entries the page held but did not see again no longer exist).
pub fn config_events(cfg: &FrbConfig, mut emit: impl FnMut(String)) {
    emit(r#"{"type":"configStarted"}"#.to_owned());
    for a in &cfg.accounts {
        emit(account_event(a));
    }
    for t in &cfg.transports {
        emit(transport_event(t));
    }
    emit(config_loaded_event(cfg));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn folder_list_round_trips_through_the_parser() {
        let ev = AppEvent::FolderListUpdated {
            account_id: "a\"1".into(),
            folders: vec!["INBOX".into(), "Archive/2026".into()],
            hierarchy_delimiter: Some("/".into()),
            unread_by_folder: HashMap::from([("INBOX".to_owned(), 3)]),
            folder_display_names: HashMap::new(),
            subscription_available: None,
            matrix_dm_folder_ids: None,
        };
        let v = Value::parse(&event_json(&ev)).unwrap();
        assert_eq!(v.opt_str("type"), Some("folderListUpdated"));
        assert_eq!(v.opt_str("accountId"), Some("a\"1"));
        assert_eq!(v.strings("folders").unwrap(), vec!["INBOX", "Archive/2026"]);
        assert_eq!(v.get("unreadByFolder").unwrap().u64("INBOX").unwrap(), 3);
    }

    #[test]
    fn config_arrives_as_a_sequence_and_keeps_the_passphrase_back() {
        let mut cfg = FrbConfig::default();
        cfg.selected_store_id = Some("s2".into());
        cfg.mail_crypto_pgp_passphrase = "hunter2".into();
        cfg.accounts = vec![
            FrbAccount { id: "s1".into(), label: "One".into(), ..FrbAccount::default() },
            FrbAccount {
                id: "s2".into(),
                label: "Two".into(),
                attrs: HashMap::from([("path".to_owned(), "/m".to_owned())]),
                lists: HashMap::from([("relayUrls".to_owned(), vec!["wss://r".to_owned()])]),
                ..FrbAccount::default()
            },
        ];
        let mut out = Vec::new();
        config_events(&cfg, |e| out.push(e));

        let types: Vec<String> = out
            .iter()
            .map(|e| Value::parse(e).unwrap().opt_str("type").unwrap().to_owned())
            .collect();
        assert_eq!(types, ["configStarted", "configAccount", "configAccount", "configLoaded"]);
        assert!(out.iter().all(|e| !e.contains("hunter2")), "passphrase must not be sent");

        let two = Value::parse(&out[2]).unwrap();
        assert_eq!(two.get("attrs").unwrap().opt_str("path"), Some("/m"));
        assert_eq!(two.get("lists").unwrap().strings("relayUrls").unwrap(), vec!["wss://r"]);
        let loaded = Value::parse(&out[3]).unwrap();
        assert_eq!(loaded.opt_str("selectedStoreId"), Some("s2"));
        assert!(loaded.bool_or("mailCryptoPgpHasPassphrase", false));
    }

    #[test]
    fn message_open_events_round_trip() {
        let h = Headers {
            from: vec![Mailbox { name: Some("Ann \"A\"".into()), email: "ann@example.org".into() }],
            to: vec![Mailbox { name: None, email: "bob@example.org".into() }],
            subject: Some("Hi".into()),
            date_ms: Some(5),
            ..Headers::default()
        };
        let v = Value::parse(&open_event_json("m1", &OpenEvent::Headers(h))).unwrap();
        assert_eq!(v.opt_str("type"), Some("messageHeaders"));
        assert_eq!(v.opt_str("requestId"), Some("m1"));
        assert_eq!(v.opt_str("subject"), Some("Hi"));
        let a = Value::parse(&open_event_json(
            "m1",
            &OpenEvent::Attachment(AttachmentInfo {
                index: 2,
                filename: None,
                content_type: "image/png".into(),
                size: 9,
                content_id: Some("c1".into()),
            }),
        ))
        .unwrap();
        assert_eq!(a.u64("index").unwrap(), 2);
        assert_eq!(a.opt_str("contentId"), Some("c1"));
        assert_eq!(a.opt_str("filename"), None);

        let done = Value::parse(&opened_json("m1", Some("boom \"x\""))).unwrap();
        assert_eq!(done.opt_str("error"), Some("boom \"x\""));
        let link = Value::parse(&link_json("https://e.test/?a=1&b=\"2\"")).unwrap();
        assert_eq!(link.opt_str("url"), Some("https://e.test/?a=1&b=\"2\""));
    }

    #[test]
    fn drop_events_carry_paths_and_state() {
        let paths = vec![std::path::PathBuf::from("/tmp/a \"b\".txt"), std::path::PathBuf::from("/tmp/c.pdf")];
        let v = Value::parse(&files_dropped_json(&paths)).unwrap();
        assert_eq!(v.opt_str("type"), Some("filesDropped"));
        assert_eq!(v.strings("paths").unwrap(), vec!["/tmp/a \"b\".txt", "/tmp/c.pdf"]);
        assert!(Value::parse(&files_dragging_json(true)).unwrap().bool_or("active", false));
        assert!(!Value::parse(&files_dragging_json(false)).unwrap().bool_or("active", true));
    }

    #[test]
    fn row_and_result_events_carry_their_fields() {
        let row = AppEvent::MessageListRowFound {
            request_id: "w1".into(),
            account_id: "a".into(),
            folder_name: "INBOX".into(),
            message_list_sort: "date_desc".into(),
            rank: 7,
            summary: MessageListRowSummary {
                id: "m1".into(),
                from: "Ann <ann@example.org>".into(),
                subject: "Hello \u{1F600}\n".into(),
                date_ms: Some(1_700_000_000_000),
                is_read: false,
                marked_for_deletion: false,
                nostr_sender_pubkey_hex: None,
            },
        };
        let v = Value::parse(&event_json(&row)).unwrap();
        assert_eq!(v.u64("rank").unwrap(), 7);
        let s = v.get("summary").unwrap();
        assert_eq!(s.opt_str("subject"), Some("Hello \u{1F600}\n"));
        assert_eq!(s.u64("dateMs").unwrap(), 1_700_000_000_000);

        let res = AppEvent::CommandResult {
            request_id: None,
            ok: false,
            error: Some("nope".into()),
        };
        let v = Value::parse(&event_json(&res)).unwrap();
        assert!(!v.bool_or("ok", true));
        assert_eq!(v.opt_str("error"), Some("nope"));
        assert_eq!(v.opt_str("requestId"), None);
    }

    #[test]
    fn the_end_of_a_save_says_what_happened() {
        let done = Value::parse(&saved_json("r1", &Saved::Done { path: "/tmp/a \"b\".pdf".into(), bytes: 42 })).unwrap();
        assert_eq!(done.opt_str("type"), Some("attachmentSaved"));
        assert_eq!(done.opt_str("requestId"), Some("r1"));
        assert_eq!(done.opt_str("path"), Some("/tmp/a \"b\".pdf"));
        assert_eq!(done.u64("bytes").unwrap(), 42);
        assert!(done.opt_str("error").is_none() && !done.bool_or("cancelled", false));

        assert!(Value::parse(&saved_json("r2", &Saved::Cancelled)).unwrap().bool_or("cancelled", false));
        let failed = Value::parse(&saved_json("r3", &Saved::Failed("no space".into()))).unwrap();
        assert_eq!(failed.opt_str("error"), Some("no space"));
    }

    #[test]
    fn the_body_event_says_whether_there_is_html() {
        let v = Value::parse(&open_event_json("m1", &OpenEvent::Body { html: true })).unwrap();
        assert_eq!(v.opt_str("type"), Some("messageBody"));
        assert_eq!(v.opt_str("requestId"), Some("m1"));
        assert!(v.bool_or("html", false));
        assert!(!Value::parse(&open_event_json("m1", &OpenEvent::Body { html: false })).unwrap().bool_or("html", true));
    }

    #[test]
    fn an_attachment_is_an_invitation_by_its_type_or_its_name() {
        for (ct, name) in [
            ("text/calendar", None),
            ("TEXT/CALENDAR; method=REQUEST", None),
            ("application/ics", None),
            ("application/octet-stream", Some("invite.ics")),
            ("application/octet-stream", Some("Meeting.ICS")),
        ] {
            assert!(is_calendar_invite(ct, name), "{ct} {name:?}");
        }
        for (ct, name) in [("text/plain", None), ("application/pdf", Some("ics.pdf")), ("image/png", Some("pic.png")), ("text/x-vcard", Some("a.vcf"))] {
            assert!(!is_calendar_invite(ct, name), "{ct} {name:?}");
        }
        let a = AttachmentInfo { index: 1, filename: Some("invite.ics".into()), content_type: "text/calendar".into(), size: 10, content_id: None };
        let v = Value::parse(&open_event_json("m1", &OpenEvent::Attachment(a))).unwrap();
        assert!(v.bool_or("invite", false));
        let b = AttachmentInfo { index: 0, filename: Some("a.pdf".into()), content_type: "application/pdf".into(), size: 10, content_id: None };
        assert!(!Value::parse(&open_event_json("m1", &OpenEvent::Attachment(b))).unwrap().bool_or("invite", true));
    }

    #[test]
    fn a_signature_verdict_and_remote_content_are_page_events() {
        let v = Value::parse(&open_event_json("m1", &OpenEvent::Signature("valid".into()))).unwrap();
        assert_eq!(v.opt_str("type"), Some("messageSignature"));
        assert_eq!(v.opt_str("requestId"), Some("m1"));
        assert_eq!(v.opt_str("status"), Some("valid"));

        // The body says how many remote images it holds, by message since the body is
        // loaded by the frame and not asked for with a request.
        let r = Value::parse(&remote_content_json("acc 1", "INBOX/sub", "42", 3)).unwrap();
        assert_eq!(r.opt_str("type"), Some("messageRemote"));
        assert_eq!(r.opt_str("accountId"), Some("acc 1"));
        assert_eq!(r.opt_str("folder"), Some("INBOX/sub"));
        assert_eq!(r.opt_str("messageId"), Some("42"));
        assert_eq!(r.u64("count").unwrap(), 3);
    }

    #[test]
    fn a_menu_choice_names_the_item() {
        let v = Value::parse(&menu_action_json("replyAll")).unwrap();
        assert_eq!(v.opt_str("type"), Some("menuAction"));
        assert_eq!(v.opt_str("id"), Some("replyAll"));
    }
}
