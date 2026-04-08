//! Application state, navigation, and input handling.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{ListState, ScrollbarState};
use ratatui::Frame;
use serde_json::json;
use tagliacarte_app::frb_api::{FrbAccount, FrbConfig};
use tagliacarte_app::mail_kind::{is_imap_like_store, is_nostr_store};
use tagliacarte_app::session;

use crate::bridge::{
    default_transport_id, get_message, list_folders, list_messages_window, load_config, mark_read,
    nostr_hex_to_npub, nostr_public_key_from_secret_hex, nostr_secret_key_to_hex, save_config,
    save_store_credential, save_transport_credential, send_nntp, send_smtp, transfer_messages,
    MessageDetail, MessageSummary,
};
use crate::l10n::{self, format_template, tr, Locale};
use crate::screens::accounts_folders::FocusPane;
use crate::widgets::{Dialog, DialogChoice, TextArea, TextInput};

const TAB_TITLES: &[&str] = &[
    "Accounts",
    "Outgoing",
    "Security",
    "Viewing",
    "Composing",
    "About",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DialogAction {
    DiscardCompose,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsSubview {
    Account(usize),
    Transport(usize),
}

pub struct NostrNsecPrompt {
    pub account_id: String,
    pub secret: TextInput,
    pub error_line: String,
}

pub struct MailCredPrompt {
    pub title: String,
    pub store_account_id: Option<String>,
    pub transport_id: Option<String>,
    pub username: TextInput,
    pub password: TextInput,
    pub field: usize,
    pub error_line: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    MessageList,
    AccountsFolders,
    MessageDetail,
    Compose,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComposeKind {
    New,
    Reply,
    ReplyAll,
    Forward,
}

pub struct ComposeModel {
    pub kind: ComposeKind,
    pub field: usize,
    pub from: TextInput,
    pub to: TextInput,
    pub cc: TextInput,
    pub bcc: TextInput,
    pub subject: TextInput,
    pub body: TextArea,
    pub transport_id: Option<String>,
}

impl ComposeModel {
    fn new() -> Self {
        Self {
            kind: ComposeKind::New,
            field: 0,
            from: TextInput::default(),
            to: TextInput::default(),
            cc: TextInput::default(),
            bcc: TextInput::default(),
            subject: TextInput::default(),
            body: TextArea::from_text(""),
            transport_id: None,
        }
    }
}

pub struct App {
    pub locale: Locale,
    pub config_path: PathBuf,
    pub config: FrbConfig,
    pub screen: Screen,
    pub status_line: String,
    /// Recent diagnostics (session errors, bridge failures); view with Ctrl+L.
    pub log_lines: VecDeque<String>,
    pub log_viewer_open: bool,
    pub log_viewer_scroll: usize,

    pub account_id: String,
    pub folder: String,
    pub folders_cache: HashMap<String, Vec<String>>,
    pub messages: Vec<MessageSummary>,
    pub list_selected: usize,
    pub list_state: ListState,
    pub multi_selected: BTreeSet<String>,

    pub detail: Option<MessageDetail>,
    pub detail_scroll: usize,
    pub detail_scrollbar: ScrollbarState,

    pub af_account_sel: usize,
    pub af_folder_sel: usize,
    pub af_focus: FocusPane,
    pub af_account_state: ListState,
    pub af_folder_state: ListState,

    pub compose: ComposeModel,
    pub pending_reply_detail: Option<MessageDetail>,

    pub settings_tab: usize,
    pub settings_sel: usize,
    pub settings_state: ListState,

    pub dialog: Option<Dialog>,
    pub dialog_action: Option<DialogAction>,
    /// Move/copy: `(is_move, message_ids, source_account_id, source_folder)`.
    pub pending_transfer: Option<(bool, Vec<String>, String, String)>,
    pub sort_menu_open: bool,
    pub sort_cursor: usize,

    pub message_list_sort: String,

    pub settings_subview: Option<SettingsSubview>,
    pub nostr_nsec_prompt: Option<NostrNsecPrompt>,
    pub mail_cred_prompt: Option<MailCredPrompt>,
}

static SORT_OPTIONS: &[(&str, &str)] = &[
    ("date_desc", "sortDateNewest"),
    ("date_asc", "sortDateOldest"),
    ("from_asc", "sortFromAz"),
    ("from_desc", "sortFromZa"),
    ("subject_asc", "sortSubjectAz"),
    ("subject_desc", "sortSubjectZa"),
];

impl App {
    pub fn new(locale: Locale, config_path: PathBuf, config: FrbConfig) -> Self {
        let message_list_sort = config.message_list_sort.clone();
        let account_id = config
            .selected_store_id
            .clone()
            .or_else(|| config.accounts.first().map(|a| a.id.clone()))
            .unwrap_or_default();
        let folder = config
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .and_then(|a| a.last_folder.clone())
            .unwrap_or_else(|| "INBOX".to_string());

        Self {
            locale,
            config_path,
            config,
            screen: Screen::MessageList,
            status_line: String::new(),
            log_lines: VecDeque::new(),
            log_viewer_open: false,
            log_viewer_scroll: 0,
            account_id,
            folder,
            folders_cache: HashMap::new(),
            messages: Vec::new(),
            list_selected: 0,
            list_state: ListState::default(),
            multi_selected: BTreeSet::new(),
            detail: None,
            detail_scroll: 0,
            detail_scrollbar: ScrollbarState::default(),
            af_account_sel: 0,
            af_folder_sel: 0,
            af_focus: FocusPane::Accounts,
            af_account_state: ListState::default(),
            af_folder_state: ListState::default(),
            compose: ComposeModel::new(),
            pending_reply_detail: None,
            settings_tab: 0,
            settings_sel: 0,
            settings_state: ListState::default(),
            dialog: None,
            dialog_action: None,
            pending_transfer: None,
            sort_menu_open: false,
            sort_cursor: 0,
            message_list_sort,
            settings_subview: None,
            nostr_nsec_prompt: None,
            mail_cred_prompt: None,
        }
    }

    const MAX_LOG_LINES: usize = 200;

    pub fn push_log(&mut self, line: String) {
        while self.log_lines.len() >= Self::MAX_LOG_LINES {
            self.log_lines.pop_front();
        }
        self.log_lines.push_back(line);
    }

    pub fn report_error(&mut self, msg: String) {
        self.push_log(msg.clone());
        self.status_line = msg;
    }

    /// Pull session push JSON into the in-app log (errors only).
    pub fn ingest_session_json_line(&mut self, line: &str) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        match v.get("type").and_then(|x| x.as_str()) {
            Some("accountConnectionChanged") => {
                if v.get("connectionState").and_then(|x| x.as_str()) == Some("error") {
                    let aid = v.get("accountId").and_then(|x| x.as_str()).unwrap_or("?");
                    if let Some(msg) = v.get("message").and_then(|x| x.as_str()) {
                        if !msg.is_empty() {
                            self.push_log(format!("[{aid}] {msg}"));
                        }
                    }
                }
            }
            Some("messageListWindowComplete") => {
                if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
                    if !err.is_empty() {
                        self.push_log(err.to_string());
                    }
                }
            }
            Some("commandResult") => {
                if v.get("ok").and_then(|x| x.as_bool()) == Some(false) {
                    if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
                        if !err.is_empty() {
                            self.push_log(err.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn is_nostr_account(&self, account_id: &str) -> bool {
        self.config
            .accounts
            .iter()
            .any(|a| a.id == account_id && is_nostr_store(a.backend_type.as_str()))
    }

    /// If [err] indicates missing Nostr secret, open the nsec prompt instead of only status text.
    fn offer_nostr_nsec_if_needed(&mut self, account_id: &str, err: &str) -> bool {
        if !self.is_nostr_account(account_id) {
            return false;
        }
        let e = err.to_lowercase();
        let credish = err.contains("credential required")
            || err.contains("no saved credential")
            || e.contains("secret")
            || e.contains("nsec");
        if !credish {
            return false;
        }
        self.open_nostr_nsec_prompt(account_id.to_string());
        true
    }

    fn open_nostr_nsec_prompt(&mut self, account_id: String) {
        self.nostr_nsec_prompt = Some(NostrNsecPrompt {
            account_id,
            secret: TextInput::default(),
            error_line: String::new(),
        });
    }

    pub fn refresh_folders_for(&mut self, account_id: &str) {
        match list_folders(account_id) {
            Ok(v) => {
                self.folders_cache.insert(account_id.to_string(), v);
            }
            Err(e) => {
                if !self.offer_nostr_nsec_if_needed(account_id, &e) {
                    self.report_error(format_template(
                        tr(self.locale, "foldersLoadError"),
                        &[("error", &e)],
                    ));
                }
            }
        }
    }

    pub fn refresh_messages(&mut self) {
        if self.account_id.is_empty() || self.folder.is_empty() {
            self.messages.clear();
            return;
        }
        match list_messages_window(
            &self.account_id,
            &self.folder,
            0,
            500,
            &self.message_list_sort,
        ) {
            Ok(w) => {
                self.messages = w.messages;
                if self.list_selected >= self.messages.len() {
                    self.list_selected = self.messages.len().saturating_sub(1);
                }
            }
            Err(e) => {
                self.messages.clear();
                let aid = self.account_id.clone();
                if !self.offer_nostr_nsec_if_needed(aid.as_str(), &e) {
                    self.report_error(e);
                }
            }
        }
    }

    pub fn open_detail(&mut self) {
        let Some(m) = self.messages.get(self.list_selected) else {
            return;
        };
        match get_message(&self.account_id, &self.folder, &m.id) {
            Ok(d) => {
                let _ = mark_read(&self.account_id, &self.folder, &m.id);
                self.detail = Some(d);
                self.detail_scroll = 0;
                self.screen = Screen::MessageDetail;
            }
            Err(e) => {
                let aid = self.account_id.clone();
                if !self.offer_nostr_nsec_if_needed(aid.as_str(), &e) {
                    self.report_error(e);
                }
            }
        }
    }

    pub fn start_compose_new(&mut self) {
        let acc = match self.config.accounts.iter().find(|a| a.id == self.account_id) {
            Some(a) => a.clone(),
            None => return,
        };
        let tid = default_transport_id(&acc);
        if tid.is_none() && !is_nntp(&acc) {
            self.status_line = l10n::trs(self.locale, "composeNeedTransportTooltip");
            return;
        }
        let mut c = ComposeModel::new();
        c.kind = ComposeKind::New;
        c.transport_id = tid;
        if let Some(tid) = &c.transport_id {
            if let Some(tr) = self.config.transports.iter().find(|t| t.id == *tid) {
                c.from.set(tr.default_from.trim());
            }
        }
        self.compose = c;
        self.screen = Screen::Compose;
    }

    pub fn start_compose_reply(&mut self, all: bool) {
        let detail = self
            .detail
            .clone()
            .or_else(|| {
                self.messages
                    .get(self.list_selected)
                    .and_then(|m| get_message(&self.account_id, &self.folder, &m.id).ok())
            });
        let Some(d) = detail else {
            return;
        };
        let acc = match self.config.accounts.iter().find(|a| a.id == self.account_id) {
            Some(a) => a.clone(),
            None => return,
        };
        let tid = default_transport_id(&acc);
        if tid.is_none() && !is_nntp(&acc) {
            self.status_line = l10n::trs(self.locale, "composeNeedTransportTooltip");
            return;
        }
        let mut c = ComposeModel::new();
        c.kind = if all {
            ComposeKind::ReplyAll
        } else {
            ComposeKind::Reply
        };
        c.transport_id = tid;
        if let Some(tid) = &c.transport_id {
            if let Some(tr) = self.config.transports.iter().find(|t| t.id == *tid) {
                c.from.set(tr.default_from.trim());
            }
        }
        c.to.set(&d.from);
        if all {
            let mut cc_parts = Vec::new();
            if let Some(cc) = &d.cc {
                cc_parts.push(cc.as_str());
            }
            if !d.to.is_empty() {
                cc_parts.push(d.to.as_str());
            }
            c.cc.set(&cc_parts.join(", "));
        }
        let subj = if d.subject.to_lowercase().starts_with("re:") {
            d.subject.clone()
        } else {
            format!("Re: {}", d.subject)
        };
        c.subject.set(&subj);
        let quote = d
            .body_plain
            .as_deref()
            .or(d.body_html.as_deref().map(|h| h.as_ref()))
            .unwrap_or("");
        let body = if self.config.quote_original {
            format!("\n\n> {}", quote.lines().collect::<Vec<_>>().join("\n> "))
        } else {
            String::new()
        };
        c.body = TextArea::from_text(&body);
        self.compose = c;
        self.pending_reply_detail = Some(d);
        self.screen = Screen::Compose;
    }

    pub fn start_compose_forward(&mut self) {
        let detail = self
            .detail
            .clone()
            .or_else(|| {
                self.messages
                    .get(self.list_selected)
                    .and_then(|m| get_message(&self.account_id, &self.folder, &m.id).ok())
            });
        let Some(d) = detail else {
            return;
        };
        let acc = match self.config.accounts.iter().find(|a| a.id == self.account_id) {
            Some(a) => a.clone(),
            None => return,
        };
        let tid = default_transport_id(&acc);
        if tid.is_none() && !is_nntp(&acc) {
            self.status_line = l10n::trs(self.locale, "composeNeedTransportTooltip");
            return;
        }
        let mut c = ComposeModel::new();
        c.kind = ComposeKind::Forward;
        c.transport_id = tid;
        if let Some(tid) = &c.transport_id {
            if let Some(tr) = self.config.transports.iter().find(|t| t.id == *tid) {
                c.from.set(tr.default_from.trim());
            }
        }
        let subj = if d.subject.to_lowercase().starts_with("fwd:") {
            d.subject.clone()
        } else {
            format!("Fwd: {}", d.subject)
        };
        c.subject.set(&subj);
        let plain = d.body_plain.clone().unwrap_or_default();
        c.body = TextArea::from_text(&plain);
        self.compose = c;
        self.screen = Screen::Compose;
    }

    pub fn send_compose(&mut self) {
        let acc = match self.config.accounts.iter().find(|a| a.id == self.account_id) {
            Some(a) => a.clone(),
            None => return,
        };
        let c = &self.compose;
        if is_nntp(&acc) {
            let groups: Vec<String> = c
                .to
                .text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if groups.is_empty() {
                self.status_line = l10n::trs(self.locale, "composeMissingNewsgroups");
                return;
            }
            let compose = json!({
                "from": c.from.text,
                "newsgroups": groups,
                "subject": c.subject.text,
                "bodyPlain": c.body.as_text(),
            });
            match send_nntp(&acc.id, &compose) {
                Ok(()) => {
                    self.status_line = l10n::trs(self.locale, "composeSendSucceeded");
                    self.screen = Screen::MessageList;
                    self.refresh_messages();
                }
                Err(e) => self.report_error(e),
            }
            return;
        }
        let Some(tid) = c.transport_id.clone() else {
            self.status_line = l10n::trs(self.locale, "composeSendCancelledNoSmtpCredentials");
            return;
        };
        if c.from.text.trim().is_empty() {
            self.status_line = l10n::trs(self.locale, "composeMissingFrom");
            return;
        }
        if c.to.text.trim().is_empty() {
            self.status_line = l10n::trs(self.locale, "composeMissingTo");
            return;
        }
        let to: Vec<String> = c
            .to
            .text
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let cc: Vec<String> = c
            .cc
            .text
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let bcc: Vec<String> = c
            .bcc
            .text
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let compose = json!({
            "from": c.from.text,
            "to": to,
            "cc": cc,
            "bcc": bcc,
            "subject": c.subject.text,
            "bodyPlain": c.body.as_text(),
        });
        match send_smtp(&tid, &compose) {
            Ok(()) => {
                self.status_line = l10n::trs(self.locale, "composeSendSucceeded");
                self.screen = Screen::MessageList;
                self.refresh_messages();
            }
            Err(e) => self.report_error(e),
        }
    }

    pub fn delete_or_junk(&mut self, junk: bool) {
        let ids: Vec<String> = if self.multi_selected.is_empty() {
            self.messages
                .get(self.list_selected)
                .map(|m| vec![m.id.clone()])
                .unwrap_or_default()
        } else {
            self.multi_selected.iter().cloned().collect()
        };
        if ids.is_empty() {
            return;
        }
        let trash: &str = if junk {
            "Junk"
        } else {
            self.config
                .accounts
                .iter()
                .find(|a| a.id == self.account_id)
                .and_then(|a| a.attrs.get("imapTrashFolderName").map(|s| s.as_str()))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Trash")
        };
        let dest_folder = if self.folders_cache.get(&self.account_id).is_some_and(|f| {
            f.iter().any(|x| x.eq_ignore_ascii_case(trash))
        }) {
            self.folders_cache[&self.account_id]
                .iter()
                .find(|x| x.eq_ignore_ascii_case(trash))
                .cloned()
                .unwrap_or_else(|| trash.to_string())
        } else {
            trash.to_string()
        };
        match transfer_messages(
            &self.account_id,
            &self.folder,
            &self.account_id,
            &dest_folder,
            ids,
            true,
        ) {
            Ok(s) => {
                self.status_line = s;
                self.multi_selected.clear();
                self.refresh_messages();
                if self.screen == Screen::MessageDetail {
                    self.detail = None;
                    self.screen = Screen::MessageList;
                }
            }
            Err(e) => self.report_error(e),
        }
    }

    pub fn complete_transfer_to_folder(&mut self, dest_account_id: &str, dest_folder: &str) {
        let Some((is_move, ids, src_acc, src_fold)) = self.pending_transfer.take() else {
            return;
        };
        match transfer_messages(
            &src_acc,
            &src_fold,
            dest_account_id,
            dest_folder,
            ids,
            is_move,
        ) {
            Ok(s) => {
                self.status_line = s;
                self.multi_selected.clear();
                self.refresh_messages();
            }
            Err(e) => self.report_error(e),
        }
    }

    pub fn settings_lines(&self) -> Vec<String> {
        match self.settings_tab {
            0 => self
                .config
                .accounts
                .iter()
                .map(|a| format!("[{}] {} — {}", a.id, a.label, a.backend_type))
                .collect(),
            1 => self
                .config
                .transports
                .iter()
                .map(|t| format!("[{}] {} {}:{}", t.id, t.display_name, t.host, t.port))
                .collect(),
            2 => vec![
                format!(
                    "[toggle] {} → {}",
                    tr(self.locale, "useSystemKeychain"),
                    self.config.use_keychain
                ),
                l10n::trs(self.locale, "oauthSection") + " — (use GUI for OAuth)",
            ],
            3 => vec![
                format!(
                    "[toggle] {} → {}",
                    tr(self.locale, "loadRemoteImages"),
                    self.config.load_remote_images
                ),
                format!(
                    "[toggle] {} → {}",
                    tr(self.locale, "threadedView"),
                    self.config.threaded_view
                ),
                format!(
                    "[info] {}: {}",
                    tr(self.locale, "sort"),
                    self.config.message_list_sort
                ),
            ],
            4 => vec![format!(
                "[{}] {}",
                tr(self.locale, "quoteOriginalOnReply"),
                self.config.quote_original
            )],
            5 => vec![
                tr(self.locale, "appTitle").to_string(),
                tr(self.locale, "aboutSubtitle").to_string(),
                tr(self.locale, "supportedBackendsList").to_string(),
                tr(self.locale, "copyrightLine").to_string(),
            ],
            _ => vec![],
        }
    }

    pub fn apply_settings_enter(&mut self) {
        match self.settings_tab {
            0 => {
                if self.settings_sel < self.config.accounts.len() {
                    self.settings_subview = Some(SettingsSubview::Account(self.settings_sel));
                    return;
                }
            }
            1 => {
                if self.settings_sel < self.config.transports.len() {
                    self.settings_subview = Some(SettingsSubview::Transport(self.settings_sel));
                    return;
                }
            }
            // Security — matches Flutter tab order (Security = index 2).
            2 => {
                if self.settings_sel == 0 {
                    self.config.use_keychain = !self.config.use_keychain;
                }
            }
            3 => match self.settings_sel {
                0 => self.config.load_remote_images = !self.config.load_remote_images,
                1 => self.config.threaded_view = !self.config.threaded_view,
                _ => {}
            },
            4 => {
                if self.settings_sel == 0 {
                    self.config.quote_original = !self.config.quote_original;
                }
            }
            _ => {}
        }
        let path = self.config_path.to_str().unwrap_or("");
        match save_config(path, &self.config) {
            Ok(()) => {
                let _ = session::reload_session_accounts(path);
            }
            Err(e) => self.report_error(e),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(2)])
            .split(area);

        let main = chunks[0];
        match self.screen {
            Screen::MessageList => {
                let title = format!(
                    "{} / {} · {}",
                    self.account_id,
                    self.folder,
                    self.message_list_sort
                );
                crate::screens::message_list::draw(
                    f,
                    main,
                    &self.messages,
                    self.list_selected,
                    &mut self.list_state,
                    &title,
                );
            }
            Screen::AccountsFolders => {
                let folders = self
                    .folders_cache
                    .get(
                        &self.config
                            .accounts
                            .get(self.af_account_sel)
                            .map(|a| a.id.clone())
                            .unwrap_or_default(),
                    )
                    .cloned()
                    .unwrap_or_default();
                crate::screens::accounts_folders::draw(
                    f,
                    main,
                    &self.config.accounts,
                    &folders,
                    self.af_account_sel,
                    self.af_folder_sel,
                    &mut self.af_account_state,
                    &mut self.af_folder_state,
                    self.af_focus,
                    (
                        tr(self.locale, "accountsListTitle"),
                        tr(self.locale, "folderLabel"),
                    ),
                );
            }
            Screen::MessageDetail => {
                if let Some(ref d) = self.detail {
                    crate::screens::message_detail::draw(
                        f,
                        main,
                        d,
                        self.detail_scroll,
                        &mut self.detail_scrollbar,
                        false,
                    );
                }
            }
            Screen::Compose => {
                let tlabel = self
                    .compose
                    .transport_id
                    .as_ref()
                    .and_then(|id| self.config.transports.iter().find(|t| t.id == *id))
                    .map(|t| format!("{} ({})", t.display_name, t.id))
                    .unwrap_or_else(|| tr(self.locale, "unknownTransport").to_string());
                let draw = crate::screens::compose::ComposeDraw {
                    field: self.compose.field,
                    from: &self.compose.from,
                    to: &self.compose.to,
                    cc: &self.compose.cc,
                    bcc: &self.compose.bcc,
                    subject: &self.compose.subject,
                    body: &self.compose.body,
                    transport_label: &tlabel,
                };
                crate::screens::compose::draw(f, main, draw);
            }
            Screen::Settings => {
                match self.settings_subview {
                    Some(SettingsSubview::Account(i)) => {
                        if let Some(acc) = self.config.accounts.get(i) {
                            crate::screens::settings_detail::draw_account(f, main, acc);
                        }
                    }
                    Some(SettingsSubview::Transport(i)) => {
                        if let Some(t) = self.config.transports.get(i) {
                            crate::screens::settings_detail::draw_transport(f, main, t);
                        }
                    }
                    None => {
                        let lines = self.settings_lines();
                        crate::screens::settings::draw(
                            f,
                            main,
                            self.settings_tab,
                            TAB_TITLES,
                            &lines,
                            &mut self.settings_state,
                            self.settings_sel,
                        );
                    }
                }
            }
        }

        let hint = match self.screen {
            Screen::MessageList => {
                if self.sort_menu_open {
                    "↑/↓ sort · Enter apply · Esc cancel".to_string()
                } else if self.pending_transfer.is_some() {
                    format!(
                        "{}",
                        tr(self.locale, "pendingMoveTagged")
                            .replace("{count}", &self.multi_selected.len().to_string())
                    )
                } else {
                    format!(
                        "Esc folders · Enter open · c compose · r/R reply · f fwd · d del · j junk · m move · y copy · s sort · Space multi · Ctrl+L log · q quit"
                    )
                }
            }
            Screen::AccountsFolders => "Tab pane · Enter folder · s settings · Esc list".to_string(),
            Screen::MessageDetail => "Esc back · r reply · f forward · d del".to_string(),
            Screen::Compose => "Tab fields · Ctrl+S send · Esc".to_string(),
            Screen::Settings => {
                if self.settings_subview.is_some() {
                    "detail · e / m / p · Esc back to list".to_string()
                } else {
                    "←/→ tab · Enter open row or apply · Esc".to_string()
                }
            }
        };
        let status = format!("{hint} │ {}", self.status_line);
        crate::widgets::draw_status_bar(f, chunks[1], &status);

        if let Some(ref dlg) = self.dialog {
            let area = centered_rect(60, 25, area);
            dlg.draw(f, area);
        }

        if self.sort_menu_open {
            let area = centered_rect(50, 12, area);
            let items: Vec<String> = SORT_OPTIONS
                .iter()
                .enumerate()
                .map(|(i, (val, key))| {
                    let label = tr(self.locale, key);
                    if i == self.sort_cursor {
                        format!("> {label} ({val})")
                    } else {
                        format!("  {label} ({val})")
                    }
                })
                .collect();
            let block = ratatui::widgets::Clear;
            f.render_widget(block, area);
            let list = ratatui::widgets::Paragraph::new(items.join("\n"))
                .block(
                    ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title(tr(self.locale, "sort")),
                );
            f.render_widget(list, area);
        }

        if self.log_viewer_open {
            let log_area = centered_rect(85, 70, area);
            f.render_widget(ratatui::widgets::Clear, log_area);
            let block = ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Log · Esc close · ↑↓ scroll · Ctrl+L toggle");
            let inner = block.inner(log_area);
            f.render_widget(block, log_area);
            let h = inner.height.max(1) as usize;
            let lines: Vec<&str> = self.log_lines.iter().map(|s| s.as_str()).collect();
            let max_start = lines.len().saturating_sub(h);
            if self.log_viewer_scroll == usize::MAX {
                self.log_viewer_scroll = max_start;
            } else {
                self.log_viewer_scroll = self.log_viewer_scroll.min(max_start);
            }
            let text = lines
                .iter()
                .skip(self.log_viewer_scroll)
                .take(h)
                .copied()
                .collect::<Vec<_>>()
                .join("\n");
            let p = ratatui::widgets::Paragraph::new(text)
                .wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(p, inner);
        }

        if let Some(ref p) = self.nostr_nsec_prompt {
            crate::screens::settings_detail::draw_nostr_nsec_dialog(
                f,
                area,
                p.account_id.as_str(),
                p.secret.text.as_str(),
                p.error_line.as_str(),
            );
        } else if let Some(ref p) = self.mail_cred_prompt {
            crate::screens::settings_detail::draw_mail_credential_dialog(
                f,
                area,
                p.title.as_str(),
                p.username.text.as_str(),
                p.password.text.as_str(),
                p.field,
                p.error_line.as_str(),
            );
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        if self.dialog.is_some() {
            self.on_key_dialog(key);
            return false;
        }
        if self.mail_cred_prompt.is_some() {
            self.on_key_mail_cred_prompt(key);
            return false;
        }
        if self.nostr_nsec_prompt.is_some() {
            self.on_key_nostr_nsec_prompt(key);
            return false;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            self.log_viewer_open = !self.log_viewer_open;
            if self.log_viewer_open {
                self.log_viewer_scroll = usize::MAX;
            }
            return false;
        }
        if self.log_viewer_open {
            self.on_key_log_viewer(key);
            return false;
        }
        if self.sort_menu_open {
            return self.on_key_sort_menu(key);
        }

        match self.screen {
            Screen::MessageList => self.on_key_message_list(key),
            Screen::AccountsFolders => self.on_key_accounts_folders(key),
            Screen::MessageDetail => self.on_key_message_detail(key),
            Screen::Compose => self.on_key_compose(key),
            Screen::Settings => {
                if self.settings_subview.is_some() {
                    self.on_key_settings_subview(key);
                } else {
                    self.on_key_settings(key);
                }
                false
            }
        }
    }

    fn on_key_dialog(&mut self, key: KeyEvent) {
        let Some(ref mut d) = self.dialog else {
            return;
        };
        match key.code {
            KeyCode::Tab | KeyCode::Left | KeyCode::Right => d.toggle(),
            KeyCode::Enter => {
                let yes = d.choice() == DialogChoice::Yes;
                let action = self.dialog_action.take();
                self.dialog = None;
                if yes {
                    if matches!(action, Some(DialogAction::DiscardCompose)) {
                        self.screen = Screen::MessageList;
                        self.compose = ComposeModel::new();
                    }
                }
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let action = self.dialog_action.take();
                self.dialog = None;
                if matches!(action, Some(DialogAction::DiscardCompose)) {
                    self.screen = Screen::MessageList;
                    self.compose = ComposeModel::new();
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.dialog = None;
                self.dialog_action = None;
            }
            _ => {}
        }
    }

    fn on_key_log_viewer(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.log_viewer_open = false,
            KeyCode::Up => {
                self.log_viewer_scroll = self.log_viewer_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                self.log_viewer_scroll = self.log_viewer_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.log_viewer_scroll = self.log_viewer_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.log_viewer_scroll = self.log_viewer_scroll.saturating_add(10);
            }
            KeyCode::Home => self.log_viewer_scroll = 0,
            KeyCode::End => self.log_viewer_scroll = usize::MAX,
            _ => {}
        }
    }

    fn on_key_sort_menu(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => self.sort_menu_open = false,
            KeyCode::Up => {
                self.sort_cursor = self.sort_cursor.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.sort_cursor + 1 < SORT_OPTIONS.len() {
                    self.sort_cursor += 1;
                }
            }
            KeyCode::Enter => {
                let (val, _) = SORT_OPTIONS[self.sort_cursor];
                self.message_list_sort = val.to_string();
                self.config.message_list_sort = val.to_string();
                let _ = save_config(self.config_path.to_str().unwrap_or(""), &self.config);
                self.sort_menu_open = false;
                self.refresh_messages();
            }
            _ => {}
        }
        false
    }

    fn on_key_message_list(&mut self, key: KeyEvent) -> bool {
        if self.pending_transfer.is_some() {
            return self.on_key_pending_transfer(key);
        }
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Esc => {
                let aids: Vec<String> = self.config.accounts.iter().map(|a| a.id.clone()).collect();
                for aid in aids {
                    self.refresh_folders_for(&aid);
                }
                if let Some(i) = self
                    .config
                    .accounts
                    .iter()
                    .position(|a| a.id == self.account_id)
                {
                    self.af_account_sel = i;
                }
                if let Some(folders) = self.folders_cache.get(&self.account_id) {
                    if let Some(j) = folders.iter().position(|x| x == &self.folder) {
                        self.af_folder_sel = j;
                    }
                }
                self.af_focus = FocusPane::Accounts;
                self.screen = Screen::AccountsFolders;
            }
            KeyCode::Down => {
                if self.list_selected + 1 < self.messages.len() {
                    self.list_selected += 1;
                }
            }
            KeyCode::Up => {
                self.list_selected = self.list_selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.list_selected = (self.list_selected + 10).min(self.messages.len().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.list_selected = self.list_selected.saturating_sub(10);
            }
            KeyCode::Home => self.list_selected = 0,
            KeyCode::End => {
                self.list_selected = self.messages.len().saturating_sub(1);
            }
            KeyCode::Enter => self.open_detail(),
            KeyCode::Char('c') => self.start_compose_new(),
            KeyCode::Char('r') => self.start_compose_reply(false),
            KeyCode::Char('R') => self.start_compose_reply(true),
            KeyCode::Char('f') => self.start_compose_forward(),
            KeyCode::Char('d') => self.delete_or_junk(false),
            KeyCode::Char('j') => self.delete_or_junk(true),
            KeyCode::Char('m') => self.start_transfer(true),
            KeyCode::Char('y') => self.start_transfer(false),
            KeyCode::Char('s') => {
                self.sort_menu_open = true;
                self.sort_cursor = SORT_OPTIONS
                    .iter()
                    .position(|(v, _)| *v == self.message_list_sort)
                    .unwrap_or(0);
            }
            KeyCode::Char(' ') => {
                if let Some(m) = self.messages.get(self.list_selected) {
                    if self.multi_selected.contains(&m.id) {
                        self.multi_selected.remove(&m.id);
                    } else {
                        self.multi_selected.insert(m.id.clone());
                    }
                }
            }
            KeyCode::Char('n') => {
                if let Some(m) = self.messages.get(self.list_selected) {
                    // mark unread — no direct API; skip or use session command if exists
                    let _ = m;
                }
            }
            _ => {}
        }
        false
    }

    fn on_key_pending_transfer(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.pending_transfer = None;
                self.status_line.clear();
            }
            _ => {}
        }
        false
    }

    fn on_key_accounts_folders(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::MessageList;
            }
            KeyCode::Tab => {
                self.af_focus = match self.af_focus {
                    FocusPane::Accounts => FocusPane::Folders,
                    FocusPane::Folders => FocusPane::Accounts,
                };
            }
            KeyCode::Char('s') => {
                self.settings_tab = 0;
                self.settings_sel = 0;
                self.screen = Screen::Settings;
            }
            KeyCode::Up => match self.af_focus {
                FocusPane::Accounts => {
                    self.af_account_sel = self.af_account_sel.saturating_sub(1);
                }
                FocusPane::Folders => {
                    self.af_folder_sel = self.af_folder_sel.saturating_sub(1);
                }
            },
            KeyCode::Down => match self.af_focus {
                FocusPane::Accounts => {
                    if self.af_account_sel + 1 < self.config.accounts.len() {
                        self.af_account_sel += 1;
                    }
                    let aid = self
                        .config
                        .accounts
                        .get(self.af_account_sel)
                        .map(|a| a.id.clone())
                        .unwrap_or_default();
                    self.refresh_folders_for(&aid);
                }
                FocusPane::Folders => {
                    let n = self
                        .folders_cache
                        .get(
                            &self
                                .config
                                .accounts
                                .get(self.af_account_sel)
                                .map(|a| a.id.clone())
                                .unwrap_or_default(),
                        )
                        .map(|v| v.len())
                        .unwrap_or(0);
                    if self.af_folder_sel + 1 < n {
                        self.af_folder_sel += 1;
                    }
                }
            },
            KeyCode::Enter => {
                if self.af_focus == FocusPane::Folders {
                    let aid = self
                        .config
                        .accounts
                        .get(self.af_account_sel)
                        .map(|a| a.id.clone())
                        .unwrap_or_default();
                    if let Some(folders) = self.folders_cache.get(&aid).cloned() {
                        if let Some(fname) = folders.get(self.af_folder_sel) {
                            let dest_folder = fname.clone();
                            if self.pending_transfer.is_some() {
                                self.complete_transfer_to_folder(&aid, &dest_folder);
                                self.account_id = aid;
                                self.folder = dest_folder;
                                self.screen = Screen::MessageList;
                                self.refresh_messages();
                                return false;
                            }
                            self.account_id = aid;
                            self.folder = dest_folder;
                            self.config.selected_store_id = Some(self.account_id.clone());
                            let _ = save_config(self.config_path.to_str().unwrap_or(""), &self.config);
                            self.screen = Screen::MessageList;
                            self.refresh_messages();
                        }
                    }
                } else {
                    self.af_focus = FocusPane::Folders;
                    let aid = self
                        .config
                        .accounts
                        .get(self.af_account_sel)
                        .map(|a| a.id.clone())
                        .unwrap_or_default();
                    self.refresh_folders_for(&aid);
                }
            }
            _ => {}
        }
        false
    }

    fn on_key_message_detail(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::MessageList;
                self.detail = None;
            }
            KeyCode::Down | KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
            KeyCode::Char('r') => self.start_compose_reply(false),
            KeyCode::Char('R') => self.start_compose_reply(true),
            KeyCode::Char('f') => self.start_compose_forward(),
            KeyCode::Char('d') => self.delete_or_junk(false),
            KeyCode::Char('j') => self.delete_or_junk(true),
            KeyCode::Char('m') => self.start_transfer(true),
            KeyCode::Char('y') => self.start_transfer(false),
            _ => {}
        }
        false
    }

    fn on_key_compose(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.send_compose();
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.dialog = Some(Dialog::new(
                    l10n::trs(self.locale, "discardChangesTitle"),
                    l10n::trs(self.locale, "discardChangesBody"),
                ));
                self.dialog_action = Some(DialogAction::DiscardCompose);
            }
            KeyCode::Tab => {
                self.compose.field = (self.compose.field + 1) % 7;
            }
            KeyCode::BackTab => {
                self.compose.field = (self.compose.field + 6) % 7;
            }
            _ => {
                let f = self.compose.field;
                let handled = match f {
                    0 => self.compose.from.handle_key(key),
                    1 => self.compose.to.handle_key(key),
                    2 => self.compose.cc.handle_key(key),
                    3 => self.compose.bcc.handle_key(key),
                    4 => self.compose.subject.handle_key(key),
                    5 => false,
                    6 => self.compose.body.handle_key(key),
                    _ => false,
                };
                if !handled {
                    let _ = handled;
                }
            }
        }
        false
    }

    fn reload_config_and_session(&mut self) {
        let path = self.config_path.to_str().unwrap_or("");
        if let Ok(c) = load_config(path) {
            self.config = c;
        }
        let _ = session::reload_session_accounts(path);
    }

    fn on_key_settings_subview(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.settings_subview = None;
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(SettingsSubview::Account(i)) = self.settings_subview {
                    if let Some(acc) = self.config.accounts.get(i) {
                        if is_nostr_store(acc.backend_type.as_str()) {
                            self.open_nostr_nsec_prompt(acc.id.clone());
                        }
                    }
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(SettingsSubview::Account(i)) = self.settings_subview {
                    if let Some(acc) = self.config.accounts.get(i) {
                        if is_imap_like_store(acc.backend_type.as_str()) {
                            self.open_mail_cred_store(i);
                        }
                    }
                }
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if let Some(SettingsSubview::Transport(i)) = self.settings_subview {
                    self.open_mail_cred_transport(i);
                }
            }
            _ => {}
        }
    }

    fn open_mail_cred_store(&mut self, account_idx: usize) {
        let Some(acc) = self.config.accounts.get(account_idx) else {
            return;
        };
        let title = format!("Mail login — {}", acc.label);
        let pre = acc
            .attrs
            .get("user")
            .or_else(|| acc.attrs.get("username"))
            .cloned()
            .unwrap_or_default();
        let mut username = TextInput::default();
        username.set(pre.as_str());
        self.mail_cred_prompt = Some(MailCredPrompt {
            title,
            store_account_id: Some(acc.id.clone()),
            transport_id: None,
            username,
            password: TextInput::default(),
            field: 0,
            error_line: String::new(),
        });
    }

    fn open_mail_cred_transport(&mut self, transport_idx: usize) {
        let Some(t) = self.config.transports.get(transport_idx) else {
            return;
        };
        let title = format!("SMTP — {}", t.display_name);
        let mut username = TextInput::default();
        username.set(t.default_from.as_str());
        self.mail_cred_prompt = Some(MailCredPrompt {
            title,
            store_account_id: None,
            transport_id: Some(t.id.clone()),
            username,
            password: TextInput::default(),
            field: 0,
            error_line: String::new(),
        });
    }

    fn submit_nostr_nsec(&mut self) {
        let Some(mut p) = self.nostr_nsec_prompt.take() else {
            return;
        };
        let raw = p.secret.text.trim();
        if raw.is_empty() {
            p.error_line = tr(self.locale, "validationUsernameRequired").to_string();
            self.nostr_nsec_prompt = Some(p);
            return;
        }
        let hex = match nostr_secret_key_to_hex(raw) {
            Ok(h) => h,
            Err(e) => {
                p.error_line = e;
                self.nostr_nsec_prompt = Some(p);
                return;
            }
        };
        let pk = match nostr_public_key_from_secret_hex(&hex) {
            Ok(x) => x,
            Err(e) => {
                p.error_line = e;
                self.nostr_nsec_prompt = Some(p);
                return;
            }
        };
        let npub = match nostr_hex_to_npub(&pk) {
            Ok(x) => x,
            Err(e) => {
                p.error_line = e;
                self.nostr_nsec_prompt = Some(p);
                return;
            }
        };
        let aid = p.account_id.clone();
        match save_store_credential(&aid, &npub, &hex) {
            Ok(()) => {
                self.reload_config_and_session();
                self.refresh_folders_for(&aid);
                if self.account_id == aid {
                    self.refresh_messages();
                }
                self.status_line = format!("Nostr credentials saved ({npub})");
            }
            Err(e) => {
                p.error_line = e;
                self.nostr_nsec_prompt = Some(p);
            }
        }
    }

    fn on_key_nostr_nsec_prompt(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.nostr_nsec_prompt = None;
            }
            KeyCode::Enter => {
                self.submit_nostr_nsec();
            }
            _ => {
                if let Some(ref mut p) = self.nostr_nsec_prompt {
                    p.error_line.clear();
                    p.secret.handle_key(key);
                }
            }
        }
    }

    fn submit_mail_cred(&mut self) {
        let Some(mut p) = self.mail_cred_prompt.take() else {
            return;
        };
        let pass = p.password.text.trim();
        if pass.is_empty() {
            p.error_line = "Password required".to_string();
            self.mail_cred_prompt = Some(p);
            return;
        }

        let res = if let Some(ref aid) = p.store_account_id {
            let u = p.username.text.trim();
            if u.is_empty() {
                p.error_line = "Username required".to_string();
                self.mail_cred_prompt = Some(p);
                return;
            }
            save_store_credential(aid, u, pass)
        } else if let Some(ref tid) = p.transport_id {
            let mut u = p.username.text.trim().to_string();
            if u.is_empty() {
                u = self
                    .config
                    .transports
                    .iter()
                    .find(|t| t.id == *tid)
                    .map(|t| t.default_from.clone())
                    .unwrap_or_default();
            }
            if u.is_empty() {
                p.error_line = "Username or default From required".to_string();
                self.mail_cred_prompt = Some(p);
                return;
            }
            save_transport_credential(tid, &u, pass)
        } else {
            Err("missing credential target".to_string())
        };

        match res {
            Ok(()) => {
                self.reload_config_and_session();
                self.status_line = "Credentials saved".to_string();
            }
            Err(e) => {
                p.error_line = e;
                self.mail_cred_prompt = Some(p);
            }
        }
    }

    fn on_key_mail_cred_prompt(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mail_cred_prompt = None;
            }
            KeyCode::Enter => {
                self.submit_mail_cred();
            }
            KeyCode::Tab => {
                if let Some(ref mut p) = self.mail_cred_prompt {
                    p.field = 1 - p.field;
                    p.error_line.clear();
                }
            }
            _ => {
                if let Some(ref mut p) = self.mail_cred_prompt {
                    p.error_line.clear();
                    if p.field == 0 {
                        p.username.handle_key(key);
                    } else {
                        p.password.handle_key(key);
                    }
                }
            }
        }
    }

    fn on_key_settings(&mut self, key: KeyEvent) -> bool {
        let n = self.settings_lines().len();
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::AccountsFolders;
            }
            KeyCode::Right => {
                if self.settings_tab + 1 < TAB_TITLES.len() {
                    self.settings_tab += 1;
                    self.settings_sel = 0;
                }
            }
            KeyCode::Left => {
                self.settings_tab = self.settings_tab.saturating_sub(1);
                self.settings_sel = 0;
            }
            KeyCode::Down => {
                if self.settings_sel + 1 < n {
                    self.settings_sel += 1;
                }
            }
            KeyCode::Up => {
                self.settings_sel = self.settings_sel.saturating_sub(1);
            }
            KeyCode::Enter => self.apply_settings_enter(),
            _ => {}
        }
        false
    }

    fn start_transfer(&mut self, is_move: bool) {
        let ids: Vec<String> = if self.multi_selected.is_empty() {
            self.messages
                .get(self.list_selected)
                .map(|m| vec![m.id.clone()])
                .unwrap_or_default()
        } else {
            self.multi_selected.iter().cloned().collect()
        };
        if ids.is_empty() {
            return;
        }
        let count = ids.len();
        self.pending_transfer = Some((
            is_move,
            ids,
            self.account_id.clone(),
            self.folder.clone(),
        ));
        self.status_line = if is_move {
            l10n::trs(self.locale, "pendingMoveTagged")
        } else {
            l10n::trs(self.locale, "pendingCopyTagged")
        }
        .replace("{count}", &count.to_string());
        let aids: Vec<String> = self.config.accounts.iter().map(|a| a.id.clone()).collect();
        for aid in aids {
            self.refresh_folders_for(&aid);
        }
        self.screen = Screen::AccountsFolders;
    }
}

fn is_nntp(acc: &FrbAccount) -> bool {
    let t = acc.backend_type.to_lowercase();
    t.contains("nntp")
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
