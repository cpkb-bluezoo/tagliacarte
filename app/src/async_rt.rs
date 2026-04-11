/*
 * async_rt.rs
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

//! Shared Tokio runtime for the app layer: mail I/O, `tokio::fs` config/credentials loads from
//! synchronous FRB entrypoints, and async protocol work. Prefer [`block_on_app`] for sync FFI that
//! needs non-blocking file I/O (see `ARCHITECTURE.md`).

use std::future::Future;

use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Handle, Runtime};

static APP_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    let n = std::thread::available_parallelism()
        .map(|p| p.get().clamp(4, 32))
        .unwrap_or(8);
    Builder::new_multi_thread()
        .worker_threads(n)
        .enable_all()
        .build()
        .expect("app tokio runtime")
});

/// Handle for spawning async work (same pool as [block_on_app]).
pub fn app_runtime_handle() -> Handle {
    APP_RUNTIME.handle().clone()
}

/// Run a future on the shared runtime. Used when synchronous Flutter FRB (or other sync) callers
/// need `tokio::fs` and other async I/O without blocking a worker on POSIX `read`/`write`.
///
/// If the current thread is already a worker of this runtime, [`Runtime::block_on`] would panic
/// (“Cannot start a runtime from within a runtime”). In that case the future is spawned onto the
/// pool and this call blocks on a channel until it completes.
pub fn block_on_app<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send,
{
    if Handle::try_current().is_ok() {
        let app = app_runtime_handle();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        app.spawn(async move {
            let out = future.await;
            let _ = tx.send(out);
        });
        rx.recv()
            .expect("block_on_app: spawned task dropped sender without sending")
    } else {
        APP_RUNTIME.block_on(future)
    }
}
