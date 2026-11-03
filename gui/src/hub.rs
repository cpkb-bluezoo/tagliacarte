//! Back end to page event delivery by long-poll.
//!
//! The page keeps one `POST /poll` outstanding. The hub holds its responder
//! until there is something to say, then answers with a JSON array of every
//! queued event. `poll` and `push` return at once and complete the HTTP
//! response from whichever thread calls them. A poll held too long is
//! released with `[]` so that no request sits in the webview for ever.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use wry::RequestAsyncResponder;
use wry::http::{Response, header};

#[derive(Default)]
struct Inner {
    pending: Option<(RequestAsyncResponder, Instant)>,
    queue: Vec<String>,
}

#[derive(Default)]
pub struct Hub {
    inner: Mutex<Inner>,
    sequences: Mutex<()>,
}

/// `["a","b"]` from already-serialised JSON values.
fn array(events: &[String]) -> Vec<u8> {
    let mut out = String::with_capacity(2 + events.iter().map(|e| e.len() + 1).sum::<usize>());
    out.push('[');
    for (i, e) in events.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(e);
    }
    out.push(']');
    out.into_bytes()
}

fn finish(responder: RequestAsyncResponder, events: &[String]) {
    let resp = Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(array(events))
        .expect("static response is valid");
    responder.respond(resp);
}

impl Hub {
    /// The page asks for events.
    pub fn poll(&self, responder: RequestAsyncResponder) {
        let mut g = self.inner.lock().unwrap();
        if !g.queue.is_empty() {
            let events = std::mem::take(&mut g.queue);
            drop(g);
            finish(responder, &events);
            return;
        }
        // A second poll while one is held means the page reloaded; release
        // the stale one so it does not leak.
        let stale = g.pending.replace((responder, Instant::now()));
        drop(g);
        if let Some((old, _)) = stale {
            finish(old, &[]);
        }
    }

    /// The back end has an event, already serialised as one JSON value.
    pub fn push(&self, event: String) {
        let mut g = self.inner.lock().unwrap();
        g.queue.push(event);
        if let Some((responder, _)) = g.pending.take() {
            let events = std::mem::take(&mut g.queue);
            drop(g);
            finish(responder, &events);
        }
    }

    /// Runs `emit`, which pushes one whole event sequence (`xStarted` ...
    /// `xLoaded`), so that no other sequence is pushed in between. The page
    /// keeps one pending copy per kind and replaces it at the end, so two
    /// interleaved sequences would corrupt it. Single events need no gate.
    pub fn sequence<R>(&self, emit: impl FnOnce() -> R) -> R {
        let _one_at_a_time = self.sequences.lock().unwrap_or_else(PoisonError::into_inner);
        emit()
    }

    /// Answers a poll that has been held longer than `max_age` with `[]`; the
    /// page immediately polls again.
    pub fn release_stale(&self, max_age: Duration) {
        let mut g = self.inner.lock().unwrap();
        let stale = matches!(&g.pending, Some((_, since)) if since.elapsed() >= max_age);
        let held = if stale { g.pending.take() } else { None };
        drop(g);
        if let Some((responder, _)) = held {
            finish(responder, &[]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Hub, array};
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn events_are_joined_as_a_json_array() {
        assert_eq!(array(&[]), b"[]");
        let two = ["{\"a\":1}".to_owned(), "{\"b\":2}".to_owned()];
        assert_eq!(array(&two), b"[{\"a\":1},{\"b\":2}]");
    }

    /// Two page-bound sequences (`contactsStarted` ... `contactsLoaded`) that
    /// overlap in time must not overlap in the queue: the page keeps one
    /// pending copy, and an interleaved end event made it forget the address book.
    #[test]
    fn a_sequence_is_not_interleaved_with_another() {
        let hub = Arc::new(Hub::default());
        let (entered_tx, entered_rx) = mpsc::channel();
        let first = {
            let hub = Arc::clone(&hub);
            std::thread::spawn(move || {
                hub.sequence(|| {
                    hub.push("a1".to_owned());
                    entered_tx.send(()).unwrap();
                    std::thread::sleep(Duration::from_millis(100));
                    hub.push("a2".to_owned());
                });
            })
        };
        entered_rx.recv().unwrap();
        hub.sequence(|| {
            hub.push("b1".to_owned());
            hub.push("b2".to_owned());
        });
        first.join().unwrap();
        assert_eq!(hub.inner.lock().unwrap().queue, ["a1", "a2", "b1", "b2"]);
    }
}
