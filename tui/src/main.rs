/*!
 * Tagliacarte terminal UI (ratatui).
 * Copyright (C) 2026 Chris Burdess
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

mod app;
mod bridge;
mod l10n;
mod screens;
mod stdio_redirect;
mod widgets;

use std::io::{stdout, Stdout};
use std::path::PathBuf;

use app::App;
use crossterm::event::{Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let locale = l10n::detect_locale();
    let config_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("TAGLIACARTE_CONFIG").map(PathBuf::from))
        .or_else(|| tagliacarte_core::config::default_config_xml_path())
        .ok_or(
            "Could not resolve config path (pass argv[1], set TAGLIACARTE_CONFIG, or TAGLIACARTE_DATA_DIR / app data dir)",
        )?;

    let path_str = config_path.to_string_lossy().to_string();
    let config = tagliacarte_app::frb_api::frb_load_config(path_str.clone());

    let (sess_tx, mut sess_rx) =
        tokio::sync::mpsc::unbounded_channel::<tagliacarte_app::session::AppEvent>();
    tagliacarte_app::session::start_session_native(sess_tx, path_str.clone())
        .map_err(|e| format!("session start: {e}"))?;

    let log_path = std::env::var_os("TAGLIACARTE_TUI_LOG")
        .map(PathBuf::from)
        .or_else(|| {
            config_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.join("tui.log"))
        })
        .or_else(|| {
            tagliacarte_core::config::tagliacarte_data_dir().map(|d| d.join("tui.log"))
        })
        .ok_or("Could not resolve TUI log path (set TAGLIACARTE_TUI_LOG or ensure config path has a parent directory)")?;
    let _stdio_log = stdio_redirect::StdioLogGuard::try_new(log_path)
        .map_err(|e| format!("stdio redirect to log: {e}"))?;

    enable_raw_mode()?;
    let mut stdout: Stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(locale, config_path, config);
    // Folder lists arrive from the session (`FolderListUpdated`) after each account’s worker runs.
    app.refresh_messages();

    let mut reader = crossterm::event::EventStream::new();

    let (sig_tx, mut sig_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = sig_tx.send(());
        }
    });

    loop {
        terminal.draw(|f| {
            app.draw(f, f.area());
        })?;

        tokio::select! {
            biased;
            Some(()) = sig_rx.recv() => {
                break;
            }
            Some(Ok(ev)) = reader.next() => {
                if let Event::Key(k) = ev {
                    if k.kind == KeyEventKind::Release {
                        continue;
                    }
                    if app.on_key(k) {
                        break;
                    }
                }
            }
            Some(ev) = sess_rx.recv() => {
                app.ingest_session_event(&ev);
                if let tagliacarte_app::session::AppEvent::FolderListUpdated {
                    account_id,
                    folders,
                    ..
                } = ev
                {
                    app.folders_cache.insert(account_id, folders);
                }
            }
        }
    }

    Ok(())
}
