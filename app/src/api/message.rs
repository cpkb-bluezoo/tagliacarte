/*
 * message.rs
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

use tagliacarte_core::message_id::MessageId;

use crate::api::app::AppState;
use crate::api::types::{ComposeData, DisplayMessage, MessageEvent};

pub fn request_message(
    state: AppState,
    folder_uri: String,
    message_id: String,
) -> Vec<MessageEvent> {
    let maybe_folder = state
        .folders
        .read()
        .ok()
        .and_then(|m| m.get(&folder_uri).cloned());

    let Some(folder) = maybe_folder else {
        return vec![MessageEvent::Error {
            message: format!("folder not found: {folder_uri}"),
        }];
    };

    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<MessageEvent>::new()));
    let events_meta = events.clone();
    let events_body = events.clone();
    let events_done = events.clone();

    folder.get_message(
        &MessageId::new(message_id),
        Box::new(move |envelope| {
            let from = envelope
                .from
                .first()
                .map(|a| format!("{}@{}", a.local_part, a.domain.clone().unwrap_or_default()))
                .unwrap_or_default();
            let to = envelope
                .to
                .first()
                .map(|a| format!("{}@{}", a.local_part, a.domain.clone().unwrap_or_default()))
                .unwrap_or_default();
            if let Ok(mut lock) = events_meta.lock() {
                lock.push(MessageEvent::Metadata {
                    subject: envelope.subject.unwrap_or_default(),
                    from,
                    to,
                    date: envelope
                        .date
                        .map(|d| d.timestamp.to_string())
                        .unwrap_or_default(),
                });
            }
        }),
        Box::new(move |bytes| {
            if let Ok(mut lock) = events_body.lock() {
                lock.push(MessageEvent::BodyReady {
                    html: None,
                    plain: Some(String::from_utf8_lossy(bytes).to_string()),
                });
            }
        }),
        Box::new(move |result| {
            if let Ok(mut lock) = events_done.lock() {
                match result {
                    Ok(()) => lock.push(MessageEvent::Complete),
                    Err(err) => lock.push(MessageEvent::Error {
                        message: err.to_string(),
                    }),
                }
            }
        }),
    );

    events.lock().map(|v| v.clone()).unwrap_or_default()
}

pub fn build_reply(original: DisplayMessage, _reply_all: bool) -> ComposeData {
    ComposeData {
        to: vec![original.from.clone()],
        subject: if original.subject.starts_with("Re:") {
            original.subject
        } else {
            format!("Re: {}", original.subject)
        },
        body_plain: format!("\n\n---\n{}", original.body_plain.unwrap_or_default()),
        ..ComposeData::default()
    }
}

pub fn build_forward(original: DisplayMessage) -> ComposeData {
    ComposeData {
        subject: if original.subject.starts_with("Fwd:") {
            original.subject
        } else {
            format!("Fwd: {}", original.subject)
        },
        body_plain: format!("\n\n---\n{}", original.body_plain.unwrap_or_default()),
        ..ComposeData::default()
    }
}
