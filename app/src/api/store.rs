/*
 * store.rs
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
use crate::api::types::FolderEvent;

pub fn refresh_folders(state: AppState, store_uri: String) -> Vec<FolderEvent> {
    let maybe_store = state
        .stores
        .read()
        .ok()
        .and_then(|m| m.get(&store_uri).cloned());

    let Some(store) = maybe_store else {
        return vec![FolderEvent::Error {
            message: format!("store not found: {store_uri}"),
        }];
    };

    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<FolderEvent>::new()));
    let events_for_folder = events.clone();
    let events_for_done = events.clone();

    store.list_folders(
        Box::new(move |folder| {
            if let Ok(mut lock) = events_for_folder.lock() {
                lock.push(FolderEvent::Found {
                    name: folder.name,
                    delimiter: folder.delimiter,
                    attributes: folder.attributes.join(","),
                });
            }
        }),
        Box::new(move |result| {
            if let Ok(mut lock) = events_for_done.lock() {
                match result {
                    Ok(()) => lock.push(FolderEvent::Complete),
                    Err(err) => lock.push(FolderEvent::Error {
                        message: err.to_string(),
                    }),
                }
            }
        }),
    );

    events.lock().map(|v| v.clone()).unwrap_or_default()
}
