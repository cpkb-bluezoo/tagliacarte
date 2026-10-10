/*
 * body_stream.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */
//! The mail-body HTTPS server's IMAP session: a dedicated connection on
//! which one `BODY.PEEK[section]` (or `BODY[]`) literal at a time is pushed
//! to a [`LiteralSink`] straight from hopf's reactor.
//!
//! Backpressure runs both ways: a sink that cannot take more (its HTTP
//! client is slow) pauses the IMAP connection's reads and resumes them
//! when it can, so a slow WebView never makes the client buffer a whole
//! attachment. Requests take turns on the session through [`Lease`]s, so
//! one request's SELECT and FETCH never interleave with another's.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use hopf_core::{ConnHandle, Endpoint};

use super::session::{Done, FetchItem, FetchStream, ImapClientError, ImapSession, ImapSessionConfig, Job};

/// Hears one literal, from the IMAP connection's reactor thread.
pub trait LiteralSink: Send {
    /// The literal's size; the pieces follow.
    fn begin(&mut self, size: u64) {
        let _ = size;
    }
    /// A piece of the literal. Pause the connection's reads through
    /// `reads` when the consumer is behind, and resume them later.
    fn chunk(&mut self, data: &[u8], reads: &mut Reads<'_>);
    /// The tagged completion, or why the fetch ended early. Nothing else
    /// is delivered afterwards.
    fn done(&mut self, r: Result<(), ImapClientError>);
}

/// Read backpressure for the connection a literal arrives on.
pub struct Reads<'a> {
    ep: &'a mut dyn Endpoint,
}

impl Reads<'_> {
    /// Stop reading from the server until [`Resume::resume`].
    pub fn pause(&mut self) -> Resume {
        self.ep.pause_read();
        Resume { handle: self.ep.handle() }
    }
}

/// Resumes reads paused by [`Reads::pause`], from any thread.
pub struct Resume {
    handle: ConnHandle,
}

impl Resume {
    pub fn resume(self) {
        self.handle.with_endpoint(|ep| ep.resume_read());
    }
}

/// Who has the session: at most one request at a time, the rest queued.
struct Lane {
    state: Mutex<LaneState>,
}

struct LaneState {
    busy: bool,
    waiting: VecDeque<Box<dyn FnOnce(Lease) + Send>>,
}

/// Exclusive use of the mail-body session for one request. Dropping it
/// hands the session to the next request waiting.
pub struct Lease {
    lane: Arc<Lane>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let next = {
            let mut g = self.lane.state.lock().unwrap();
            match g.waiting.pop_front() {
                Some(job) => Some(job),
                None => {
                    g.busy = false;
                    None
                }
            }
        };
        if let Some(job) = next {
            job(Lease { lane: Arc::clone(&self.lane) });
        }
    }
}

/// The dedicated session plus the push API the mail-body server is
/// written against. Cheap to clone.
#[derive(Clone)]
pub struct MailBodySession {
    session: Arc<ImapSession>,
    lane: Arc<Lane>,
}

impl MailBodySession {
    pub fn connect(mut config: ImapSessionConfig) -> Self {
        config.auto_idle = false;
        Self {
            session: ImapSession::connect(config),
            lane: Arc::new(Lane { state: Mutex::new(LaneState { busy: false, waiting: VecDeque::new() }) }),
        }
    }

    pub fn is_alive(&self) -> bool {
        self.session.is_alive()
    }

    /// Run `job` with the session to itself: at once when it is free,
    /// otherwise once the requests before it have dropped their leases.
    pub fn acquire(&self, job: Box<dyn FnOnce(Lease) + Send>) {
        let job = {
            let mut g = self.lane.state.lock().unwrap();
            if g.busy {
                g.waiting.push_back(job);
                return;
            }
            g.busy = true;
            job
        };
        job(Lease { lane: Arc::clone(&self.lane) });
    }

    /// SELECT the mailbox the next fetches refer to.
    pub fn select(&self, mailbox: &str, done: Done<()>) {
        let mailbox = mailbox.to_string();
        self.session.submit(Job::Select { mailbox, done: Box::new(move |r| done(r.map(|_| ()))) });
    }

    /// `* n FETCH (UID uid BODYSTRUCTURE (…))` as one line, for
    /// [`plan_body_fetch`](super::plan_body_fetch).
    pub fn fetch_bodystructure_line(&self, uid: u32, done: Done<String>) {
        let mailbox = match self.selected() {
            Ok(m) => m,
            Err(e) => return done(Err(e)),
        };
        let captured: Arc<Mutex<Option<FetchItem>>> = Arc::new(Mutex::new(None));
        let c2 = Arc::clone(&captured);
        self.session.submit(Job::Fetch {
            mailbox,
            uid: true,
            set: uid.to_string(),
            items: "(UID BODYSTRUCTURE)".into(),
            on_item: Box::new(move |it| {
                if it.uid == Some(uid) {
                    *c2.lock().unwrap() = Some(it);
                }
            }),
            stream: None,
            done: Box::new(move |r| {
                if let Err(e) = r {
                    return done(Err(e));
                }
                let item = captured.lock().unwrap().take();
                done(match item.and_then(|it| it.bodystructure.map(|b| (it.seq, b))) {
                    Some((seq, bs)) => Ok(format!("* {seq} FETCH (UID {uid} BODYSTRUCTURE {bs})")),
                    None => Err(ImapClientError::new("UID FETCH BODYSTRUCTURE: no matching untagged response")),
                })
            }),
        });
    }

    fn selected(&self) -> Result<String, ImapClientError> {
        self.session
            .selected_mailbox()
            .ok_or_else(|| ImapClientError::new("no mailbox selected on the mail-body session"))
    }

    fn fetch_literal(&self, uid: u32, items: String, mut sink: Box<dyn LiteralSink>) {
        let mailbox = match self.selected() {
            Ok(m) => m,
            Err(e) => return sink.done(Err(e)),
        };
        let sink = Arc::new(Mutex::new(Some(sink)));
        let began = Arc::new(AtomicBool::new(false));
        let (sink_begin, sink_chunk, began_begin) = (Arc::clone(&sink), Arc::clone(&sink), Arc::clone(&began));
        let stream = FetchStream {
            on_begin: Box::new(move |size| {
                began_begin.store(true, Ordering::Release);
                if let Some(s) = sink_begin.lock().unwrap().as_mut() {
                    s.begin(size);
                }
            }),
            on_chunk: Box::new(move |data, ep| {
                if let Some(s) = sink_chunk.lock().unwrap().as_mut() {
                    s.chunk(data, &mut Reads { ep });
                }
            }),
        };
        self.session.submit(Job::Fetch {
            mailbox,
            uid: true,
            set: uid.to_string(),
            items,
            on_item: Box::new(|_| {}),
            stream: Some(stream),
            done: Box::new(move |r| {
                // A reply with no literal at all (NIL body, or NO) is a
                // failure for the sink.
                let r = match r {
                    Ok(()) if !began.load(Ordering::Acquire) => {
                        Err(ImapClientError::new("UID FETCH: expected body literal, got OK without literal"))
                    }
                    r => r,
                };
                let taken = sink.lock().unwrap().take();
                if let Some(mut s) = taken {
                    s.done(r);
                }
            }),
        });
    }

    /// `UID FETCH uid (BODY.PEEK[section])`, the literal pushed to `sink`.
    pub fn fetch_body_peek_section(&self, uid: u32, section: &str, sink: Box<dyn LiteralSink>) {
        self.fetch_literal(uid, format!("(BODY.PEEK[{section}])"), sink)
    }

    /// `UID FETCH uid (BODY[])` for the whole message, pushed to `sink`.
    pub fn fetch_body_by_uid(&self, uid: u32, sink: Box<dyn LiteralSink>) {
        self.fetch_literal(uid, "(BODY[])".into(), sink)
    }
}
