//! Tagliacarte GUI: a wry webview over the app layer.
//!
//! Everything the page shows is served by the `app` custom protocol from
//! this process; the page fetches nothing else (the CSP and the navigation
//! handler enforce that). Calls go to `POST /api/<name>` as JSON, events come
//! back through a held `POST /poll`. Handlers hand their responder to
//! another thread and return, so the UI thread never waits on the app layer.

mod appicon;
mod args;
mod assets;
mod calendar;
mod commands;
mod compose;
mod contacts;
mod entities;
mod events;
mod hub;
mod mail;
mod menu;
mod save;
mod settings;
mod uploads;
#[cfg(test)]
mod value;

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use muda::MenuEvent;
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::http::{Method, Request, Response, header};
use wry::{NewWindowResponse, RequestAsyncResponder, WebViewBuilder};

use commands::{Ctx, PickMode, UiRequest};
use hub::Hub;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "ios"))]
pub const BASE: &str = "app://localhost";
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "ios")))]
pub const BASE: &str = "http://app.localhost";

/// The UI loads script, style, images and frames from itself and nothing
/// else, and may call only itself.
const CSP_UI: &str = "default-src 'self'; img-src 'self' data:; script-src 'self'; \
                      style-src 'self'; frame-src 'self'; connect-src 'self'; \
                      base-uri 'none'; form-action 'none'";

#[derive(Debug)]
enum UserEvent {
    Ui(UiRequest),
}

struct Shared {
    /// Per-launch secret the page must present on the command channel.
    token: String,
    ctx: Ctx,
}

fn status(code: u16) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .body(Vec::new())
        .expect("static response is valid")
}

fn handle(sh: &Arc<Shared>, req: Request<Vec<u8>>, responder: RequestAsyncResponder) {
    let path = req.uri().path().to_owned();
    if std::env::var_os("TAGLIACARTE_TRACE").is_some() {
        let inm = req.headers().get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok());
        eprintln!("[trace] {} {} if-none-match={:?}", req.method(), path, inm);
    }
    match (req.method(), path.as_str()) {
        (&Method::GET, p) if p.starts_with("/mail/") => mail::serve(p, req.uri().query(), &sh.ctx.hub, responder),
        (&Method::GET, p) if p.starts_with("/remote/") => mail::serve_remote(p, responder),
        (&Method::GET, p) if p.starts_with("/avatar/") => mail::serve_avatar(p, responder),
        (&Method::GET, p) => match assets::lookup(p) {
            Some(asset) => {
                // Standard validation: `no-cache` lets a client keep the
                // file but ask each time, and a matching ETag costs a 304.
                let fresh = req
                    .headers()
                    .get(header::IF_NONE_MATCH)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| assets::etag_matches(v, &asset.etag));
                let builder = Response::builder()
                    .header(header::ETAG, &asset.etag)
                    .header(header::CACHE_CONTROL, "no-cache");
                responder.respond(if fresh {
                    builder.status(304).body(Cow::Borrowed(&[][..]))
                } else {
                    builder
                        .status(200)
                        .header(header::CONTENT_TYPE, asset.content_type)
                        .header(header::CONTENT_SECURITY_POLICY, CSP_UI)
                        .body(asset.body)
                }
                .expect("static response is valid"));
            }
            None => responder.respond(status(404)),
        },
        (&Method::POST, "/poll") if authorised(sh, &req) => sh.ctx.hub.poll(responder),
        // A file as the raw request body: `X-Filename` (percent-encoded) names it
        // and `Content-Type` types it. Nothing is encoded into JSON.
        (&Method::POST, "/upload") if authorised(sh, &req) => {
            let sh = Arc::clone(sh);
            std::thread::spawn(move || {
                let name = req
                    .headers()
                    .get("x-filename")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| percent_encoding::percent_decode_str(v).decode_utf8_lossy().into_owned())
                    .unwrap_or_default();
                let mime = req.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
                let (status, body) = match sh.ctx.uploads.put_bytes(&name, mime, req.body()) {
                    Ok(info) => (200, info.to_json()),
                    Err(e) => (413, format!("{{\"ok\":false,\"error\":{}}}", json_string(&e))),
                };
                responder.respond(
                    Response::builder()
                        .status(status)
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::CACHE_CONTROL, "no-store")
                        .body(body.into_bytes())
                        .expect("static response is valid"),
                );
            });
        }
        (&Method::POST, p) if p.starts_with("/api/") && authorised(sh, &req) => {
            let sh = Arc::clone(sh);
            let name = p[5..].to_owned();
            std::thread::spawn(move || {
                let body = String::from_utf8_lossy(req.body()).into_owned();
                let reply = commands::call(&sh.ctx, &name, &body);
                responder.respond(
                    Response::builder()
                        .status(200)
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::CACHE_CONTROL, "no-store")
                        .body(reply.into_bytes())
                        .expect("static response is valid"),
                );
            });
        }
        (&Method::POST, _) => responder.respond(status(403)),
        _ => responder.respond(status(404)),
    }
}

/// Only the UI document may use the command channel. WKWebView sends no
/// `Origin` on same-origin fetches to a custom scheme, so the check is a
/// per-launch token that a main-frame-only init script gives the UI page and
/// nothing else (a sandboxed message body never sees it).
fn authorised(sh: &Shared, req: &Request<Vec<u8>>) -> bool {
    req.headers()
        .get("x-token")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t == sh.token)
}

fn json_string(s: &str) -> String {
    let mut w = tagliacarte_core::json::JsonWriter::new();
    w.write_string(s);
    tagliacarte_core::json::writer_into_string(w)
}

/// Random per process: `RandomState` is seeded from the OS.
fn make_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..4)
        .map(|_| {
            let h = std::collections::hash_map::RandomState::new()
                .build_hasher()
                .finish();
            format!("{h:016x}")
        })
        .collect()
}

/// A web or mail link, as opposed to `about:blank` and the like, which are
/// refused without troubling the user.
fn is_link(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["http://", "https://", "mailto:"].iter().any(|p| lower.starts_with(p))
}

/// Same resolution as the TUI: argv[1], `TAGLIACARTE_CONFIG`, then the
/// default data directory.
fn config_path() -> Result<PathBuf, String> {
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("TAGLIACARTE_CONFIG").map(PathBuf::from))
        .or_else(tagliacarte_core::config::default_config_xml_path)
        .ok_or_else(|| {
            "could not resolve the config path (pass it as an argument, set TAGLIACARTE_CONFIG, \
             or set TAGLIACARTE_DATA_DIR)"
                .to_owned()
        })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selftest = std::env::args().any(|a| a == "--selftest");
    let config_path = config_path()?.to_string_lossy().into_owned();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();
    let title_proxy = proxy.clone();
    let hub = Arc::new(Hub::default());
    let shared = Arc::new(Shared {
        token: make_token(),
        ctx: Ctx {
            config_path: config_path.clone(),
            selftest,
            hub: Arc::clone(&hub),
            uploads: Arc::new(uploads::Uploads::new()),
            ui: Box::new(move |req| {
                let _ = proxy.send_event(UserEvent::Ui(req));
            }),
        },
    });

    // The session reports on a std channel; a thread turns each event into
    // JSON for the page.
    let (tx, rx) = std::sync::mpsc::channel::<tagliacarte_app::session::AppEvent>();
    tagliacarte_app::session::start_session_native(tx, config_path)
        .map_err(|e| format!("session start: {e}"))?;
    {
        let hub = Arc::clone(&hub);
        std::thread::spawn(move || {
            for ev in rx {
                hub.push(events::event_json(&ev));
            }
        });
    }
    {
        let hub = Arc::clone(&hub);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(5));
                hub.release_stale(Duration::from_secs(25));
            }
        });
    }

    let builder = WindowBuilder::new()
        .with_title("Tagliacarte")
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_min_inner_size(LogicalSize::new(720.0, 480.0));
    // macOS takes the icon from the app bundle, or from `appicon::set_dock_icon`.
    #[cfg(not(target_os = "macos"))]
    let builder = builder.with_window_icon(appicon::window_icon());
    let window = builder.build(&event_loop)?;

    let handler_shared = Arc::clone(&shared);
    let webview = WebViewBuilder::new()
        .with_initialization_script_for_main_only(
            format!(
                "window.__TOKEN__ = '{}'; window.__SELFTEST__ = {};",
                shared.token, selftest
            ),
            true,
        )
        .with_asynchronous_custom_protocol("app".into(), move |_id, req, responder| {
            handle(&handler_shared, req, responder)
        })
        // Documents are `app` documents and nothing else. A web or mail link
        // followed in a message does not navigate; the page is told, and asks
        // the user before anything is opened.
        .with_navigation_handler({
            let hub = Arc::clone(&hub);
            move |url| {
                if url == BASE || url.starts_with(&format!("{BASE}/")) {
                    return true;
                }
                if is_link(&url) {
                    hub.push(events::link_json(&url));
                }
                false
            }
        })
        .with_new_window_req_handler({
            let hub = Arc::clone(&hub);
            move |url, _features| {
                if is_link(&url) {
                    hub.push(events::link_json(&url));
                }
                NewWindowResponse::Deny
            }
        })
        // Files dropped on the window arrive as paths, which become attachments
        // without being read here. Text dragged inside the page is left alone.
        .with_drag_drop_handler({
            let hub = Arc::clone(&hub);
            move |ev| match ev {
                wry::DragDropEvent::Enter { paths, .. } if !paths.is_empty() => {
                    hub.push(events::files_dragging_json(true));
                    true
                }
                wry::DragDropEvent::Drop { paths, .. } if !paths.is_empty() => {
                    hub.push(events::files_dragging_json(false));
                    hub.push(events::files_dropped_json(&paths));
                    true
                }
                wry::DragDropEvent::Leave => {
                    hub.push(events::files_dragging_json(false));
                    false
                }
                _ => false,
            }
        })
        // The window is titled by the page, through the ordinary `document.title`.
        .with_document_title_changed_handler(move |title| {
            let _ = title_proxy.send_event(UserEvent::Ui(UiRequest::SetTitle(title)));
        })
        .with_devtools(cfg!(debug_assertions))
        .with_url(format!("{BASE}/"))
        .build(&window)?;

    let menu = menu::MenuBar::build()?;
    // The system's own items (Hide, Quit, Copy) never get here; these are the
    // ones the page acts on.
    MenuEvent::set_event_handler(Some({
        let hub = Arc::clone(&hub);
        move |e: MenuEvent| {
            if menu::is_action(&e.id.0) {
                hub.push(events::menu_action_json(&e.id.0));
            }
        }
    }));
    let dialog_hub = Arc::clone(&hub);
    // When the self-test is to read the window's title (see SetTitle).
    let mut read_title_at: Option<Instant> = None;

    event_loop.run(move |event, _, flow| {
        if read_title_at.is_some_and(|at| Instant::now() >= at) {
            read_title_at = None;
            dialog_hub.push(events::window_title_json(&window.title()));
        }
        *flow = match read_title_at {
            Some(at) => ControlFlow::WaitUntil(at),
            None => ControlFlow::Wait,
        };
        let _keep_alive = (&webview, &window, &menu);
        match event {
            Event::NewEvents(StartCause::Init) => {
                #[cfg(target_os = "macos")]
                {
                    menu.menu.init_for_nsapp();
                    appicon::set_dock_icon();
                }
            }
            Event::UserEvent(UserEvent::Ui(UiRequest::MenuLabels(labels))) => menu.set_labels(&labels),
            Event::UserEvent(UserEvent::Ui(UiRequest::MenuEnable(ids))) => menu.set_enabled(&ids),
            Event::UserEvent(UserEvent::Ui(UiRequest::SetTitle(title))) => {
                if !title.is_empty() {
                    window.set_title(&title);
                    // The self-test reads the window's own title back. The system applies a
                    // new title a little after it is set, so the reading is taken a moment later.
                    if selftest {
                        let at = Instant::now() + Duration::from_millis(150);
                        read_title_at = Some(at);
                        *flow = ControlFlow::WaitUntil(at);
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::UserEvent(UserEvent::Ui(UiRequest::Quit)) => *flow = ControlFlow::Exit,
            // A native dialog runs here, on the UI thread, and blocks it
            // while it is open, as a modal dialog should.
            Event::UserEvent(UserEvent::Ui(UiRequest::PickPath { request_id, mode, title, file_name })) => {
                let mut dialog = rfd::FileDialog::new().set_title(title);
                if !file_name.is_empty() {
                    dialog = dialog.set_file_name(file_name);
                }
                let picked = match mode {
                    PickMode::File => dialog.pick_file(),
                    PickMode::Folder => dialog.pick_folder(),
                    PickMode::Save => dialog.save_file(),
                };
                let path = picked.map(|p| p.to_string_lossy().into_owned());
                dialog_hub.push(events::path_picked_json(&request_id, path.as_deref()));
            }
            Event::UserEvent(UserEvent::Ui(UiRequest::SaveFile { title, file_name, reply })) => {
                let mut dialog = rfd::FileDialog::new().set_title(title);
                if !file_name.is_empty() {
                    dialog = dialog.set_file_name(file_name);
                }
                reply(dialog.save_file());
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_mail_links_reach_the_page() {
        assert!(is_link("https://example.com/a?b=1"));
        assert!(is_link("HTTP://example.com"));
        assert!(is_link("mailto:ann@example.org"));
        assert!(!is_link("about:blank"));
        assert!(!is_link("javascript:alert(1)"));
        assert!(!is_link("file:///etc/passwd"));
        assert!(!is_link("app://localhost/"));
    }

    #[test]
    fn tokens_differ_between_calls() {
        assert_ne!(make_token(), make_token());
        assert_eq!(make_token().len(), 64);
    }
}
