/*
 * send.rs
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

use tagliacarte_core::store::{Address, SendPayload};

use crate::api::app::AppState;
use crate::api::types::{ComposeData, SendEvent};

pub fn send_message(
    state: AppState,
    transport_uri: String,
    compose: ComposeData,
) -> Vec<SendEvent> {
    let maybe_transport = state
        .transports
        .read()
        .ok()
        .and_then(|m| m.get(&transport_uri).cloned());

    let Some(transport) = maybe_transport else {
        return vec![SendEvent::Error {
            message: format!("transport not found: {transport_uri}"),
        }];
    };

    let events = std::sync::Arc::new(std::sync::Mutex::new(vec![SendEvent::Progress {
        status: "sending".to_owned(),
    }]));
    let events_done = events.clone();
    transport.send(
        &to_payload(compose),
        Box::new(move |result| {
            if let Ok(mut lock) = events_done.lock() {
                match result {
                    Ok(()) => lock.push(SendEvent::Complete),
                    Err(err) => lock.push(SendEvent::Error {
                        message: err.to_string(),
                    }),
                }
            }
        }),
    );

    events.lock().map(|v| v.clone()).unwrap_or_default()
}

fn to_payload(compose: ComposeData) -> SendPayload {
    SendPayload {
        from: parse_addrs(compose.from),
        to: compose.to.into_iter().flat_map(parse_addrs).collect(),
        cc: compose.cc.into_iter().flat_map(parse_addrs).collect(),
        bcc: compose.bcc.into_iter().flat_map(parse_addrs).collect(),
        subject: Some(compose.subject),
        body_plain: Some(compose.body_plain),
        body_html: compose.body_html,
        attachments: vec![],
        newsgroups: vec![],
        nntp_in_reply_to: None,
        nntp_references: None,
        smtp_notify: None,
        smtp_in_reply_to: None,
        smtp_references: None,
        smtp_message_id: None,
    }
}

fn parse_addrs(raw: impl AsRef<str>) -> Vec<Address> {
    raw.as_ref()
        .split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.split('@');
            let local = parts.next()?.trim().to_owned();
            let domain = parts.next().map(|v| v.trim().to_owned());
            Some(Address {
                display_name: None,
                local_part: local,
                domain,
            })
        })
        .collect()
}
