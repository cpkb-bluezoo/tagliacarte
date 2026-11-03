//! Saving an attachment to disk.
//!
//! The page only says which attachment and what to call it. The back end asks
//! for the destination itself, with a native save dialog, and writes only to
//! the path that comes back, so a page cannot name a file to overwrite. The
//! bytes never pass through the page: they are streamed from the store to a
//! temporary file beside the destination, which replaces it once complete. A
//! failed or abandoned save leaves any existing file as it was.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tagliacarte_app::mail_view::{self, MessageRef};

use crate::args::Args;
use crate::commands::{Ctx, UiRequest};
use crate::events::{self, Saved};
use crate::uploads::clean_name;

type Done = Box<dyn FnOnce(Result<u64, String>) + Send>;

/// A file the writer and the finishing code both hold, so that it can be
/// closed before it is renamed (a file cannot be renamed while open on Windows).
#[derive(Clone)]
struct Shared(Arc<Mutex<Option<File>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0.lock().unwrap().as_mut() {
            Some(f) => f.write(buf),
            None => Err(io::Error::other("the file was already closed")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.0.lock().unwrap().as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

fn partial_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Hands `fetch` a writer on a temporary file beside `dest` and a completion;
/// when the completion reports success the file replaces `dest`, and when it
/// reports failure the file is removed. `finished` hears the outcome.
fn write_beside(
    dest: &Path,
    fetch: impl FnOnce(Box<dyn Write + Send>, Done),
    finished: Done,
) {
    let temp = partial_path(dest);
    let file = match File::create(&temp) {
        Ok(f) => f,
        Err(e) => return finished(Err(format!("could not create {}: {e}", temp.display()))),
    };
    let shared = Shared(Arc::new(Mutex::new(Some(file))));
    let (closer, dest) = (shared.clone(), dest.to_path_buf());
    fetch(
        Box::new(shared),
        Box::new(move |r| {
            drop(closer.0.lock().unwrap().take());
            finished(match r {
                Ok(n) => std::fs::rename(&temp, &dest)
                    .map(|()| n)
                    .map_err(|e| {
                        let _ = std::fs::remove_file(&temp);
                        format!("could not save {}: {e}", dest.display())
                    }),
                Err(e) => {
                    let _ = std::fs::remove_file(&temp);
                    Err(e)
                }
            });
        }),
    );
}

/// `attachment.save`: replies at once; the dialog, the transfer and the end
/// are reported by an `attachmentSaved` event carrying the request id.
pub fn start(ctx: &Ctx, body: &str) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    let request_id = a.str("requestId")?;
    let msg = MessageRef {
        account_id: a.str("accountId")?,
        folder: a.str("folder")?,
        message_id: a.str("messageId")?,
    };
    let index = u32::try_from(a.u64("index")?).map_err(|_| "no such attachment".to_owned())?;
    let file_name = clean_name(a.opt_str("fileName").unwrap_or(""));

    let hub = Arc::clone(&ctx.hub);
    let reply: Box<dyn FnOnce(Option<PathBuf>) + Send> = Box::new(move |chosen: Option<PathBuf>| {
        let Some(dest) = chosen else {
            return hub.push(events::saved_json(&request_id, &Saved::Cancelled));
        };
        // Off the UI thread, which is still inside the dialog's callback.
        std::thread::spawn(move || {
            let shown = dest.to_string_lossy().into_owned();
            write_beside(
                &dest,
                |out, done| mail_view::save_attachment(msg, index, out, done),
                Box::new(move |r| {
                    let outcome = match r {
                        Ok(bytes) => Saved::Done { path: shown, bytes },
                        Err(e) => Saved::Failed(e),
                    };
                    hub.push(events::saved_json(&request_id, &outcome));
                }),
            );
        });
    });

    if ctx.selftest {
        // No one is there to use a dialog; the file goes beside the scratch config.
        let dir = Path::new(&ctx.config_path).parent().unwrap_or(Path::new("."));
        reply(Some(dir.join(format!("saved-{file_name}"))));
    } else {
        (ctx.ui)(UiRequest::SaveFile { title: String::new(), file_name, reply });
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn run(dest: &Path, fetch: impl FnOnce(Box<dyn Write + Send>, Done)) -> Result<u64, String> {
        let (tx, rx) = channel();
        write_beside(dest, fetch, Box::new(move |r| tx.send(r).unwrap()));
        rx.recv().unwrap()
    }

    #[test]
    fn a_completed_save_replaces_the_destination_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("report.pdf");
        std::fs::write(&dest, b"old").unwrap();
        let r = run(&dest, |mut out, done| {
            out.write_all(b"new ").unwrap();
            out.write_all(b"contents").unwrap();
            done(Ok(12));
        });
        assert_eq!(r, Ok(12));
        assert_eq!(std::fs::read(&dest).unwrap(), b"new contents");
        assert!(!partial_path(&dest).exists());
    }

    #[test]
    fn a_failed_save_keeps_what_was_there_and_removes_the_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("report.pdf");
        std::fs::write(&dest, b"old").unwrap();
        let r = run(&dest, |mut out, done| {
            out.write_all(b"half").unwrap();
            done(Err("connection lost".to_owned()));
        });
        assert_eq!(r, Err("connection lost".to_owned()));
        assert_eq!(std::fs::read(&dest).unwrap(), b"old");
        assert!(!partial_path(&dest).exists());
    }

    #[test]
    fn nothing_is_created_when_the_folder_cannot_be_written() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("no-such-folder").join("a.bin");
        let r = run(&dest, |_, _| panic!("no transfer should start"));
        assert!(r.unwrap_err().contains("could not create"));
    }
}
