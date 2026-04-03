/*
 * app_api.rs
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

use tagliacarte_app::api::auth::start_oauth;
use tagliacarte_app::api::config::{load_config, save_config};
use tagliacarte_app::api::message::{build_forward, build_reply};
use tagliacarte_app::api::types::DisplayMessage;

#[test]
fn oauth_reports_unknown_provider() {
    let events = start_oauth("unknown".to_owned(), None);
    assert!(!events.is_empty());
}

#[test]
fn config_round_trip_path() {
    let path = std::env::temp_dir()
        .join("tagliacarte_app_test")
        .join("config.xml");
    let saved = save_config(path.to_string_lossy().to_string(), Default::default());
    assert!(saved.is_ok());
    let cfg = load_config(path.to_string_lossy().to_string());
    assert!(cfg.accounts.is_empty());
}

#[test]
fn reply_and_forward_subjects() {
    let original = DisplayMessage {
        subject: "Subject".to_owned(),
        from: "alice@example.com".to_owned(),
        body_plain: Some("Body".to_owned()),
        ..Default::default()
    };
    let reply = build_reply(original.clone(), false);
    let fwd = build_forward(original);
    assert!(reply.subject.starts_with("Re:"));
    assert!(fwd.subject.starts_with("Fwd:"));
}
