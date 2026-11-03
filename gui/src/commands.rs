//! `POST /api/<name>`: the page's calls into the app layer.
//!
//! Arguments arrive as a JSON object and the reply is
//! `{"ok":true}`, `{"ok":true,"result":...}` or `{"ok":false,"error":"..."}`.
//! A call returns as soon as the work is scheduled; anything slow reports on
//! the event stream (`session.*` commands answer with a `commandResult` or
//! their own events). Commands that produce data do the same: `config.load`
//! replies `{"ok":true}` and then streams the configuration as events.

use std::sync::Arc;

use tagliacarte_app::frb_api::frb_load_config;
use tagliacarte_app::mail_view::{self, MessageRef};
use tagliacarte_app::session::{self, AppCommand};
use tagliacarte_core::json::{JsonWriter, writer_into_string};

use crate::args::Args;
use crate::events;
use crate::hub::Hub;
use crate::settings;

/// Things only the UI thread can do. A command posts one and the shell
/// carries it out; any answer comes back as an event.
pub enum UiRequest {
    /// Close the window and exit.
    Quit,
    /// Show a native file or folder dialog; the answer is a `pathPicked` event.
    PickPath { request_id: String, mode: PickMode, title: String, file_name: String },
    /// Show a save dialog and hand the chosen path (or `None`) to `reply`,
    /// which is called on the UI thread and must not block.
    SaveFile { title: String, file_name: String, reply: Box<dyn FnOnce(Option<std::path::PathBuf>) + Send> },
    /// Words for menu items, as (id, label).
    MenuLabels(Vec<(String, String)>),
    /// Exactly these of the menu's enabled-and-disabled items can be used.
    MenuEnable(Vec<String>),
    /// The window's title, from the page's `document.title`.
    SetTitle(String),
}

impl std::fmt::Debug for UiRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiRequest::Quit => f.write_str("Quit"),
            UiRequest::PickPath { request_id, .. } => write!(f, "PickPath({request_id})"),
            UiRequest::SaveFile { file_name, .. } => write!(f, "SaveFile({file_name})"),
            UiRequest::MenuLabels(l) => write!(f, "MenuLabels({})", l.len()),
            UiRequest::MenuEnable(i) => write!(f, "MenuEnable({})", i.len()),
            UiRequest::SetTitle(t) => write!(f, "SetTitle({t})"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickMode {
    File,
    Folder,
    /// Choose where to write a new file.
    Save,
}

pub struct Ctx {
    pub config_path: String,
    pub selftest: bool,
    /// Where events for the page go.
    pub hub: Arc<Hub>,
    /// Attachments waiting to be sent.
    pub uploads: Arc<crate::uploads::Uploads>,
    /// Hands a request to the UI thread.
    pub ui: Box<dyn Fn(UiRequest) + Send + Sync>,
}

/// Runs one command and returns the whole reply body.
pub fn call(ctx: &Ctx, name: &str, body: &str) -> String {
    match run(ctx, name, body) {
        Ok(None) => r#"{"ok":true}"#.to_owned(),
        Ok(Some(result)) => format!(r#"{{"ok":true,"result":{result}}}"#),
        Err(e) => error_reply(&e),
    }
}

fn error_reply(message: &str) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("ok");
    w.write_bool(false);
    w.write_key("error");
    w.write_string(message);
    w.write_end_object();
    writer_into_string(w)
}

fn info(ctx: &Ctx) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("version");
    w.write_string(env!("CARGO_PKG_VERSION"));
    w.write_key("configPath");
    w.write_string(&ctx.config_path);
    w.write_end_object();
    writer_into_string(w)
}

fn run(ctx: &Ctx, name: &str, body: &str) -> Result<Option<String>, String> {
    if let Some(r) = crate::entities::run(ctx, name, body) {
        return r;
    }
    // Each module claims only the names it handles; compose first, since it
    // owns `contacts.complete` and contacts claims the rest of `contacts.*`.
    if let Some(r) = crate::compose::run(ctx, name, body) {
        return r;
    }
    if let Some(r) = crate::contacts::run(ctx, name, body) {
        return r;
    }
    if let Some(r) = crate::calendar::run(ctx, name, body) {
        return r;
    }
    match name {
        "app.info" => Ok(Some(info(ctx))),
        "app.report" => {
            let line = Args::parse(body)?.str("line")?;
            println!("[page] {line}");
            if ctx.selftest && line.starts_with("FINAL") {
                (ctx.ui)(UiRequest::Quit);
            }
            Ok(None)
        }
        "config.load" => {
            let cfg = frb_load_config(ctx.config_path.clone());
            ctx.hub.sequence(|| events::config_events(&cfg, |e| ctx.hub.push(e)));
            Ok(None)
        }
        // One preference at a time. Replies at once; the saved settings come
        // back as a configLoaded event (a secret is never among them).
        "settings.set" => {
            let a = Args::parse(body)?;
            let key = a.str("key")?;
            let (_, cfg) = settings::edit_config(&ctx.config_path, |cfg| settings::apply(cfg, &key, &a))?;
            ctx.hub.push(events::config_loaded_event(&cfg));
            Ok(None)
        }
        // An attachment chosen in a native dialog: referenced by path, read at send.
        "compose.attachPath" => {
            let path = Args::parse(body)?.str("path")?;
            Ok(Some(ctx.uploads.add_path(std::path::Path::new(&path))?.to_json()))
        }
        "compose.removeAttachment" => {
            ctx.uploads.remove(&Args::parse(body)?.str("id")?);
            Ok(None)
        }
        "dialog.pickPath" => {
            let a = Args::parse(body)?;
            let mode = match a.opt_str("mode").unwrap_or("file") {
                "file" => PickMode::File,
                "folder" => PickMode::Folder,
                "save" => PickMode::Save,
                other => return Err(format!("unknown dialog mode '{other}'")),
            };
            (ctx.ui)(UiRequest::PickPath {
                request_id: a.str("requestId")?,
                mode,
                title: a.opt_str("title").unwrap_or("").to_owned(),
                file_name: a.opt_str("fileName").unwrap_or("").to_owned(),
            });
            Ok(None)
        }
        "attachment.save" => crate::save::start(ctx, body),
        // Where the user is, so that the next start can return there. Replies
        // at once; nothing is sent back, because the page already knows.
        "location.save" => {
            let a = Args::parse(body)?;
            settings::save_location(&ctx.config_path, &a.str("accountId")?, a.opt_str("folder"), a.opt_str("messageId"))?;
            Ok(None)
        }
        // The native menu: its words, and which of its items can be used now.
        "menu.labels" => {
            let a = Args::parse(body)?;
            let pairs = crate::menu::check_labels(&a.strings("ids")?, &a.strings("labels")?)?;
            (ctx.ui)(UiRequest::MenuLabels(pairs));
            Ok(None)
        }
        "menu.enable" => {
            let ids = crate::menu::check_enabled(&Args::parse(body)?.strings("ids")?)?;
            (ctx.ui)(UiRequest::MenuEnable(ids));
            Ok(None)
        }
        "app.openUrl" => {
            open_url(&Args::parse(body)?.str("url")?)?;
            Ok(None)
        }
        // Replies at once; the envelope and attachments follow as events,
        // ending with messageOpened.
        "session.openMessage" => {
            let a = Args::parse(body)?;
            let request_id = a.str("requestId")?;
            let msg = MessageRef {
                account_id: a.str("accountId")?,
                folder: a.str("folder")?,
                message_id: a.str("messageId")?,
            };
            let (hub, rid) = (Arc::clone(&ctx.hub), request_id.clone());
            let on_event = Arc::new(move |ev| hub.push(events::open_event_json(&rid, &ev)));
            let hub = Arc::clone(&ctx.hub);
            mail_view::open_message(
                msg,
                on_event,
                Box::new(move |r| hub.push(events::opened_json(&request_id, r.err().as_deref()))),
            );
            Ok(None)
        }
        "session.refreshFolders" => session_cmd(AppCommand::RefreshFolders {
            account_id: Args::parse(body)?.str("accountId")?,
        }),
        "session.markRead" => {
            let a = Args::parse(body)?;
            session_cmd(AppCommand::MarkRead {
                account_id: a.str("accountId")?,
                folder: a.str("folder")?,
                message_id: a.str("messageId")?,
                request_id: a.opt_str("requestId").map(str::to_owned),
            })
        }
        "session.listMessagesWindow" => {
            let a = Args::parse(body)?;
            session_cmd(AppCommand::ListMessagesWindow {
                account_id: a.str("accountId")?,
                folder_name: a.str("folderName")?,
                start_index: a.u64("startIndex")?,
                limit: a.u64("limit")?,
                message_list_sort: a.str("messageListSort")?,
                request_id: a.str("requestId")?,
                list_ready: a.bool_or("listReady", false),
                visible_first_rank: a.opt_u64("visibleFirstRank"),
                visible_last_rank: a.opt_u64("visibleLastRank"),
            })
        }
        "session.transferMessages" => {
            let a = Args::parse(body)?;
            session_cmd(AppCommand::TransferMessages {
                source_account_id: a.str("sourceAccountId")?,
                source_folder: a.str("sourceFolder")?,
                dest_account_id: a.str("destAccountId")?,
                dest_folder: a.str("destFolder")?,
                message_ids: a.strings("messageIds")?,
                is_move: a.bool_or("isMove", false),
                request_id: a.opt_str("requestId").map(str::to_owned),
            })
        }
        "session.sendChatMessage" => {
            let a = Args::parse(body)?;
            session_cmd(AppCommand::SendChatMessage {
                account_id: a.str("accountId")?,
                folder: a.str("folder")?,
                text: a.str("text")?,
                body_html: a.opt_str("bodyHtml").map(str::to_owned),
                request_id: a.opt_str("requestId").map(str::to_owned),
            })
        }
        _ => Err(format!("unknown command '{name}'")),
    }
}

/// Hands a link to the system browser. Only web and mail links are opened,
/// and the URL goes to the opener as one argument, never through a shell.
fn open_url(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    let allowed = ["http://", "https://", "mailto:"].iter().any(|p| lower.starts_with(p));
    if !allowed || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("only http, https and mailto links can be opened".to_owned());
    }
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(url)
        .status()
        .map_err(|e| format!("could not open the link: {e}"))
        .and_then(|s| if s.success() { Ok(()) } else { Err(format!("opener exited with {s}")) })
}

fn session_cmd(cmd: AppCommand) -> Result<Option<String>, String> {
    session::session_command(cmd)?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::value::Value;

    fn ctx() -> Ctx {
        Ctx {
            config_path: "/nonexistent/config.xml".to_owned(),
            selftest: false,
            hub: Arc::new(Hub::default()),
            uploads: Arc::new(crate::uploads::Uploads::new()),
            ui: Box::new(|_| {}),
        }
    }

    #[test]
    fn replies_are_well_formed_json() {
        let c = ctx();
        let v = Value::parse(&call(&c, "app.info", "")).unwrap();
        assert!(v.bool_or("ok", false));
        assert_eq!(v.get("result").unwrap().opt_str("configPath"), Some("/nonexistent/config.xml"));

        let v = Value::parse(&call(&c, "no.such", "")).unwrap();
        assert!(!v.bool_or("ok", true));
        assert!(v.opt_str("error").unwrap().contains("no.such"));

        // Only web and mail links are ever handed to the system.
        for bad in ["javascript:alert(1)", "file:///etc/passwd", "https://x/ y", "ftp://h/f"] {
            let body = format!(r#"{{"url":"{bad}"}}"#);
            let v = Value::parse(&call(&c, "app.openUrl", &body)).unwrap();
            assert!(!v.bool_or("ok", true), "{bad} must be refused");
        }

        // Argument errors name the argument and quote nothing unescaped.
        let v = Value::parse(&call(&c, "session.markRead", r#"{"accountId":"a\"b"}"#)).unwrap();
        assert!(v.opt_str("error").unwrap().contains("folder"));
        let v = Value::parse(&call(&c, "session.markRead", "{broken")).unwrap();
        assert!(!v.bool_or("ok", true));
    }

    /// A context whose requests to the UI thread are kept, for looking at.
    fn capturing() -> (Ctx, Arc<std::sync::Mutex<Vec<String>>>) {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let mut c = ctx();
        c.ui = Box::new(move |r| sink.lock().unwrap().push(format!("{r:?}")));
        (c, seen)
    }

    #[test]
    fn the_page_sets_menu_words_and_which_message_actions_can_be_used() {
        let (c, seen) = capturing();
        let ok = |body: &str, name: &str| Value::parse(&call(&c, name, body)).unwrap();

        assert!(ok(r#"{"ids":["reply","quit"],"labels":["Antworten","Beenden"]}"#, "menu.labels").bool_or("ok", false));
        assert!(ok(r#"{"ids":["reply","forward"]}"#, "menu.enable").bool_or("ok", false));
        assert!(ok(r#"{"ids":[]}"#, "menu.enable").bool_or("ok", false));
        assert_eq!(*seen.lock().unwrap(), ["MenuLabels(2)", "MenuEnable(2)", "MenuEnable(0)"]);

        // What is wrong is refused, and nothing reaches the UI thread.
        seen.lock().unwrap().clear();
        for (name, body) in [
            ("menu.labels", r#"{"ids":["nope"],"labels":["x"]}"#),
            ("menu.labels", r#"{"ids":["reply"],"labels":[]}"#),
            ("menu.enable", r#"{"ids":["settings"]}"#),
            ("menu.enable", r#"{}"#),
        ] {
            assert!(!ok(body, name).bool_or("ok", true), "{name} {body}");
        }
        assert!(seen.lock().unwrap().is_empty());
    }
}
