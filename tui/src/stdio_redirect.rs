/*!
 * Redirect **stderr** to a log file while the TUI runs; restore on drop.
 *
 * **Important:** [ratatui] draws to **stdout**. Redirecting stdout to a file would hide the UI and
 * break the alternate screen; only stderr is redirected so `eprintln!` from the app layer does not
 * corrupt the terminal.
 * Copyright (C) 2026 Chris Burdess
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use std::fs;
use std::io;
use std::path::PathBuf;

/// Restores original stderr when dropped.
pub struct StdioLogGuard {
    #[cfg(unix)]
    saved_err: libc::c_int,
}

impl StdioLogGuard {
    /// Append **stderr** to `path` (creates file if needed). Stdout is unchanged. No-op on non-Unix.
    pub fn try_new(path: PathBuf) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::io::AsRawFd;

            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }

            let log = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            let log_fd = log.as_raw_fd();

            let saved_err = unsafe { libc::dup(libc::STDERR_FILENO) };
            if saved_err < 0 {
                return Err(io::Error::last_os_error());
            }
            unsafe {
                if libc::dup2(log_fd, libc::STDERR_FILENO) < 0 {
                    let e = io::Error::last_os_error();
                    libc::dup2(saved_err, libc::STDERR_FILENO);
                    libc::close(saved_err);
                    return Err(e);
                }
            }
            drop(log);
            Ok(Self { saved_err })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(Self {})
        }
    }
}

#[cfg(not(unix))]
impl StdioLogGuard {}

impl Drop for StdioLogGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::dup2(self.saved_err, libc::STDERR_FILENO);
            libc::close(self.saved_err);
        }
    }
}
