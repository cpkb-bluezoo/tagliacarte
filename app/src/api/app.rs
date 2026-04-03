/*
 * app.rs
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

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use once_cell::sync::OnceCell;
use tagliacarte_core::store::{Folder, Store, Transport};
use tokio::runtime::{Builder, Runtime};

pub struct AppState {
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) stores: Arc<RwLock<HashMap<String, Arc<dyn Store>>>>,
    pub(crate) folders: Arc<RwLock<HashMap<String, Arc<dyn Folder>>>>,
    pub(crate) transports: Arc<RwLock<HashMap<String, Arc<dyn Transport>>>>,
    pub(crate) credentials: Arc<Mutex<HashMap<String, String>>>,
    pub(crate) config_path: Arc<RwLock<String>>,
}

static GLOBAL_STATE: OnceCell<AppState> = OnceCell::new();

pub fn init_app(config_path: String) -> AppState {
    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    let state = AppState {
        runtime: Arc::new(runtime),
        stores: Arc::new(RwLock::new(HashMap::new())),
        folders: Arc::new(RwLock::new(HashMap::new())),
        transports: Arc::new(RwLock::new(HashMap::new())),
        credentials: Arc::new(Mutex::new(HashMap::new())),
        config_path: Arc::new(RwLock::new(config_path)),
    };

    let _ = GLOBAL_STATE.set(state.clone());
    state
}

pub fn shutdown(_state: &AppState) {}

pub fn global_state() -> Option<AppState> {
    GLOBAL_STATE.get().cloned()
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
            stores: self.stores.clone(),
            folders: self.folders.clone(),
            transports: self.transports.clone(),
            credentials: self.credentials.clone(),
            config_path: self.config_path.clone(),
        }
    }
}
