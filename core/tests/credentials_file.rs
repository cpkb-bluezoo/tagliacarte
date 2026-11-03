//! The credentials file is shared by every account worker, and several of
//! them save at once at start-up (OAuth tokens are refreshed and written
//! back). These tests pin down that it never loses an entry, never shows a
//! reader a half-written file, and never overwrites a file it cannot read.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use tagliacarte_core::config::{load_credentials, save_credential, set_credentials_backend};

fn file_backend() -> (tempfile::TempDir, std::path::PathBuf) {
    set_credentials_backend(false);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("credentials");
    // The key is created by the first save; start with it in place.
    save_credential(&path, "seed", "u", "p").unwrap();
    (dir, path)
}

#[test]
fn concurrent_saves_all_survive() {
    let (_dir, path) = file_backend();
    let workers: Vec<_> = (0..24)
        .map(|i| {
            let path = path.clone();
            thread::spawn(move || {
                save_credential(&path, &format!("k{i}"), "user", &format!("pw{i}")).unwrap();
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    let all = load_credentials(&path, None).unwrap();
    assert_eq!(all.len(), 25, "entries lost: {:?}", all.keys().collect::<Vec<_>>());
    assert_eq!(all["k7"].password_or_token, "pw7");
}

#[test]
fn a_reader_never_sees_a_half_written_file() {
    let (_dir, path) = file_backend();
    let big = "x".repeat(64 * 1024);
    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (path, stop) = (path.clone(), Arc::clone(&stop));
        thread::spawn(move || {
            let mut n = 0;
            while !stop.load(Ordering::Relaxed) && n < 300 {
                save_credential(&path, "writer", "u", &big).unwrap();
                n += 1;
            }
        })
    };
    let mut failures = Vec::new();
    for _ in 0..600 {
        match load_credentials(&path, None) {
            Ok(m) if m.contains_key("seed") => {}
            Ok(m) => failures.push(format!("read succeeded but lost 'seed': {} entries", m.len())),
            Err(e) => failures.push(e),
        }
    }
    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();
    assert!(failures.is_empty(), "{} bad reads, first: {:?}", failures.len(), failures.first());
}

#[test]
fn a_file_that_cannot_be_read_is_not_overwritten() {
    let (_dir, path) = file_backend();
    // A real credentials file cut short, as a crash or a full disk leaves it.
    let mut cut = std::fs::read(&path).unwrap();
    cut.truncate(cut.len() / 2);
    std::fs::write(&path, &cut).unwrap();
    assert!(load_credentials(&path, None).is_err(), "the fixture must be unreadable");

    let r = save_credential(&path, "new", "u", "p");
    assert!(r.is_err(), "saving over an unreadable file must fail, not replace it");
    assert_eq!(std::fs::read(&path).unwrap(), cut, "the file must be left exactly as it was");
}
