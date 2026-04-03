/*
 * auth.rs
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

use crate::api::app::AppState;
use crate::api::types::OAuthEvent;

pub fn provide_credential(
    state: AppState,
    store_uri: String,
    password: String,
) -> Result<(), String> {
    let mut lock = state
        .credentials
        .lock()
        .map_err(|_| "credentials lock poisoned".to_owned())?;
    lock.insert(store_uri, password);
    Ok(())
}

pub fn start_oauth(provider: String, email_hint: Option<String>) -> Vec<OAuthEvent> {
    let provider = provider.trim().to_lowercase();
    if provider != "google" && provider != "microsoft" {
        return vec![OAuthEvent::Error {
            message: format!("unknown oauth provider: {provider}"),
        }];
    }

    let hint = email_hint.unwrap_or_default();
    let auth_url =
        format!("https://example.invalid/oauth/start?provider={provider}&email_hint={hint}");
    vec![
        OAuthEvent::AuthorizationUrl { url: auth_url },
        OAuthEvent::Complete,
    ]
}
