/*
 * session.rs
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

//! One persistent IMAP connection on hopf (`hopf-imap`), driven by a queue of
//! jobs.
//!
//! The store and its folders push [`Job`]s onto the session from whatever
//! thread they run on; each job carries its own completion callback. The
//! session's driver runs on hopf's reactor and is woken by a
//! [`ConnHandle::poke`] after every push: in `on_wake` it receives the
//! staged state object for the connection's current state and issues the
//! next job's command, selecting the job's mailbox first when a different
//! one (or none) is selected. Jobs run one at a time, in order.
//!
//! While the queue is empty and a mailbox is selected, the session goes
//! into RFC 2177 `IDLE` after the configured quiet period (or sends `NOOP`
//! on servers without IDLE). Unsolicited `EXISTS` / `RECENT` / `EXPUNGE`
//! set the shared folder-list-stale flag the app polls. A job arriving
//! during IDLE ends it (`DONE`) and runs once the server acknowledges.
//!
//! Nothing here blocks hopf's reactor: the `*_blocking` helpers wait on a
//! channel from the caller's own thread.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::Duration;

use base64::Engine as _;
use hopf_auth::{create_client, SaslClient, SaslClientStep, SaslMechanism as HopfMechanism};
use hopf_core::{ConnHandle, Endpoint, TimerHandle};
use hopf_imap::{
    ImapAppendUid, ImapCapabilities, ImapClient, ImapClientAppend, ImapClientAuthExchange,
    ImapClientAuthenticated, ImapClientDriver, ImapClientHandlerFactory, ImapClientIdle,
    ImapClientNotAuthenticated, ImapClientPostStarttls, ImapClientSelected, ImapClientTimeouts,
    ImapClientWakeState, ImapCopyUid, ImapFetchData, ImapListEntry, ImapListOptions,
    ImapMailboxInfo, ImapStatus, ImapStatusData, MailboxEventListener,
};

use crate::net::{hopf_runtime, hopf_tls_connector};
use crate::sasl::SaslMechanism;

/// Outbound buffer limit for a session: an `APPEND` literal is written in
/// one call, so this bounds the largest message that can be saved to Sent
/// or Drafts.
const MAX_NET_OUT: usize = 128 * 1024 * 1024;
/// Re-issue IDLE before the RFC 2177 29-minute server limit.
const IDLE_RESTART: Duration = Duration::from_secs(25 * 60);

/// IMAP client error (network, protocol, auth).
#[derive(Debug, Clone)]
pub struct ImapClientError {
    pub message: String,
}

impl ImapClientError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

impl fmt::Display for ImapClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ImapClientError {}

impl From<io::Error> for ImapClientError {
    fn from(e: io::Error) -> Self {
        Self::new(e.to_string())
    }
}

/// Completion callback of a job.
pub type Done<T> = Box<dyn FnOnce(Result<T, ImapClientError>) + Send>;

/// How to reach and log in to the server.
#[derive(Clone)]
pub struct ImapSessionConfig {
    pub host: String,
    pub port: u16,
    /// TLS from the first byte (port 993).
    pub implicit_tls: bool,
    /// On a plain connection, STARTTLS is mandatory.
    pub starttls: bool,
    /// Username, secret, mechanism family. `XOAuth2` means the secret is a
    /// bearer token.
    pub auth: Option<(String, String, SaslMechanism)>,
    /// Quiet seconds on the wire before auto-IDLE / NOOP.
    pub min_idle_secs: Arc<AtomicU32>,
    /// Set when the server reports mailbox activity (EXISTS, RECENT,
    /// EXPUNGE), solicited or not.
    pub folder_list_stale: Arc<AtomicBool>,
    /// Whether to go idle at all. The mail-body streaming session does not.
    pub auto_idle: bool,
}

/// One message's data from a FETCH: the parsed attributes plus the first
/// literal in the response (the header fields or body section asked for).
#[derive(Debug, Clone, Default)]
pub struct FetchItem {
    pub seq: u32,
    pub uid: Option<u32>,
    pub flags: Vec<String>,
    pub size: Option<u64>,
    /// `BODY[…]` / `RFC822*` literal octets, when not streamed.
    pub literal: Vec<u8>,
    /// Captured `BODYSTRUCTURE` text, when requested.
    pub bodystructure: Option<String>,
}

/// Streamed delivery of a FETCH literal instead of accumulation into
/// [`FetchItem::literal`]: `on_begin(size)` then `on_chunk` per piece.
/// `on_chunk` gets the endpoint so it can apply read backpressure.
pub struct FetchStream {
    pub on_begin: Box<dyn FnMut(u64) + Send>,
    pub on_chunk: Box<dyn FnMut(&[u8], &mut dyn Endpoint) + Send>,
}

/// A unit of work for the session. Jobs that act on a mailbox carry its
/// name; the session selects it first when needed.
pub enum Job {
    List { pattern: String, status_unseen: bool, on_row: Box<dyn Fn(&ImapListEntry, Option<u32>) + Send>, done: Done<()> },
    Lsub { on_row: Box<dyn Fn(&ImapListEntry) + Send>, done: Done<()> },
    Status { mailbox: String, done: Done<ImapStatusData> },
    Select { mailbox: String, done: Done<ImapMailboxInfo> },
    Fetch {
        mailbox: String,
        uid: bool,
        set: String,
        items: String,
        on_item: Box<dyn Fn(FetchItem) + Send>,
        stream: Option<FetchStream>,
        done: Done<()>,
    },
    UidSort { mailbox: String, criteria: String, done: Done<Vec<u32>> },
    UidSearch { mailbox: String, criteria: String, done: Done<Vec<u32>> },
    UidStore { mailbox: String, set: String, action: String, flags: String, done: Done<()> },
    UidCopy { mailbox: String, set: String, dest: String, done: Done<Option<ImapCopyUid>> },
    UidMove { mailbox: String, set: String, dest: String, done: Done<Option<ImapCopyUid>> },
    Expunge { mailbox: String, done: Done<()> },
    UidExpunge { mailbox: String, set: String, done: Done<()> },
    Append { mailbox: String, flags: Option<String>, data: Vec<u8>, done: Done<Option<ImapAppendUid>> },
    Create { mailbox: String, done: Done<()> },
    Delete { mailbox: String, done: Done<()> },
    Rename { from: String, to: String, done: Done<()> },
    Subscribe { mailbox: String, done: Done<()> },
    Unsubscribe { mailbox: String, done: Done<()> },
    /// Keep-alive on servers without IDLE; internal.
    Noop,
    Logout,
}

impl Job {
    /// Variant name for tracing.
    fn kind(&self) -> &'static str {
        match self {
            Job::List { .. } => "LIST",
            Job::Lsub { .. } => "LSUB",
            Job::Status { .. } => "STATUS",
            Job::Select { .. } => "SELECT",
            Job::Fetch { uid: true, .. } => "UID FETCH",
            Job::Fetch { .. } => "FETCH",
            Job::UidSort { .. } => "UID SORT",
            Job::UidSearch { .. } => "UID SEARCH",
            Job::UidStore { .. } => "UID STORE",
            Job::UidCopy { .. } => "UID COPY",
            Job::UidMove { .. } => "UID MOVE",
            Job::Expunge { .. } => "EXPUNGE",
            Job::UidExpunge { .. } => "UID EXPUNGE",
            Job::Append { .. } => "APPEND",
            Job::Create { .. } => "CREATE",
            Job::Delete { .. } => "DELETE",
            Job::Rename { .. } => "RENAME",
            Job::Subscribe { .. } => "SUBSCRIBE",
            Job::Unsubscribe { .. } => "UNSUBSCRIBE",
            Job::Noop => "NOOP",
            Job::Logout => "LOGOUT",
        }
    }

    /// The mailbox that must be selected before this job's command.
    fn needs_mailbox(&self) -> Option<&str> {
        match self {
            Job::Fetch { mailbox, .. }
            | Job::UidSort { mailbox, .. }
            | Job::UidSearch { mailbox, .. }
            | Job::UidStore { mailbox, .. }
            | Job::UidCopy { mailbox, .. }
            | Job::UidMove { mailbox, .. }
            | Job::Expunge { mailbox, .. }
            | Job::UidExpunge { mailbox, .. } => Some(mailbox),
            _ => None,
        }
    }

    fn fail(self, err: ImapClientError) {
        match self {
            Job::List { done, .. } | Job::Lsub { done, .. } | Job::UidStore { done, .. } | Job::Expunge { done, .. }
            | Job::UidExpunge { done, .. } | Job::Create { done, .. } | Job::Delete { done, .. }
            | Job::Rename { done, .. } | Job::Subscribe { done, .. } | Job::Unsubscribe { done, .. }
            | Job::Fetch { done, .. } => done(Err(err)),
            Job::Status { done, .. } => done(Err(err)),
            Job::Select { done, .. } => done(Err(err)),
            Job::UidSort { done, .. } | Job::UidSearch { done, .. } => done(Err(err)),
            Job::UidCopy { done, .. } | Job::UidMove { done, .. } => done(Err(err)),
            Job::Append { done, .. } => done(Err(err)),
            Job::Noop | Job::Logout => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Connecting,
    Ready,
    Closed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdleState {
    Off,
    Active,
    Ending,
}

struct Inner {
    phase: Phase,
    handle: Option<ConnHandle>,
    caps: ImapCapabilities,
    selected: Option<String>,
    queue: VecDeque<Job>,
    /// The job whose command is on the wire.
    current: Option<Job>,
    /// A job waiting for the SELECT issued on its behalf.
    pending_select: Option<Job>,
    idle: IdleState,
    idle_due: bool,
    idle_restart_due: bool,
    idle_timer: Option<TimerHandle>,
    sasl: Option<Box<dyn SaslClient>>,
    // Per-job accumulators.
    fetch_cur: Option<FetchItem>,
    list_rows: Vec<ImapListEntry>,
    status_rows: HashMap<String, ImapStatusData>,
    numbers: Vec<u32>,
    append_data: Option<Vec<u8>>,
}

struct Shared {
    config: ImapSessionConfig,
    inner: Mutex<Inner>,
    /// Signalled when the phase leaves `Connecting`.
    ready: Condvar,
}

/// A live (or dying) session. Cheap to clone through the `Arc`.
pub struct ImapSession {
    shared: Arc<Shared>,
}

impl ImapSession {
    /// Dial, log in, and return at once; jobs may be submitted immediately
    /// and run once the session is ready (or fail if it never is).
    pub fn connect(config: ImapSessionConfig) -> Arc<ImapSession> {
        let shared = Arc::new(Shared {
            inner: Mutex::new(Inner {
                phase: Phase::Connecting,
                handle: None,
                caps: ImapCapabilities::default(),
                selected: None,
                queue: VecDeque::new(),
                current: None,
                pending_select: None,
                idle: IdleState::Off,
                idle_due: false,
                idle_restart_due: false,
                idle_timer: None,
                sasl: None,
                fetch_cur: None,
                list_rows: Vec::new(),
                status_rows: HashMap::new(),
                numbers: Vec::new(),
                append_data: None,
            }),
            config: config.clone(),
            ready: Condvar::new(),
        });
        let session = Arc::new(ImapSession { shared: Arc::clone(&shared) });
        crate::trace_log!(
            "imap",
            "session connect host={} port={} implicit_tls={} starttls={} auth={}",
            config.host,
            config.port,
            config.implicit_tls,
            config.starttls,
            config.auth.is_some()
        );
        let client = ImapClient::new(config.host.as_str(), config.port)
            .timeouts(ImapClientTimeouts::default())
            .resolver(crate::net::hopf_dns_resolver())
            .max_net_out(MAX_NET_OUT);
        let client = if config.implicit_tls {
            client.implicit_tls(hopf_tls_connector(), config.host.as_str())
        } else if config.starttls {
            client.starttls(hopf_tls_connector(), config.host.as_str())
        } else {
            client
        };
        if let Err(e) = client.connect(&hopf_runtime(), Arc::new(Factory(Arc::clone(&shared)))) {
            shared.close(format!("dial refused: {e}"));
        }
        session
    }

    /// `false` once the connection is gone; the store then opens a new one.
    pub fn is_alive(&self) -> bool {
        !matches!(self.shared.inner.lock().unwrap().phase, Phase::Closed(_))
    }

    /// Wait for login to complete; the capability set the server
    /// advertised after authentication.
    pub fn wait_ready(&self, timeout: Duration) -> Result<ImapCapabilities, ImapClientError> {
        let mut inner = self.shared.inner.lock().unwrap();
        let deadline = std::time::Instant::now() + timeout;
        while inner.phase == Phase::Connecting {
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(ImapClientError::new("timeout waiting for IMAP login"));
            }
            let (guard, _) = self.shared.ready.wait_timeout(inner, deadline - now).unwrap();
            inner = guard;
        }
        match &inner.phase {
            Phase::Ready => Ok(inner.caps.clone()),
            Phase::Closed(m) => Err(ImapClientError::new(m.clone())),
            Phase::Connecting => unreachable!(),
        }
    }

    /// Capabilities seen so far (empty before login).
    pub fn capabilities(&self) -> ImapCapabilities {
        self.shared.inner.lock().unwrap().caps.clone()
    }

    /// The mailbox currently selected on the wire, if any.
    pub fn selected_mailbox(&self) -> Option<String> {
        self.shared.inner.lock().unwrap().selected.clone()
    }

    /// Queue a job and wake the driver.
    pub fn submit(&self, job: Job) {
        let handle = {
            let mut inner = self.shared.inner.lock().unwrap();
            if let Phase::Closed(m) = &inner.phase {
                let m = m.clone();
                drop(inner);
                job.fail(ImapClientError::new(m));
                return;
            }
            crate::trace_log!("imap", "submit {} (queued={}, current={})", job.kind(), inner.queue.len(), inner.current.is_some());
            inner.queue.push_back(job);
            inner.handle.clone()
        };
        if let Some(h) = handle {
            h.poke();
        }
    }

    /// Run a job and wait for its result on this thread.
    pub fn run_blocking<T: Send + 'static>(
        &self,
        make: impl FnOnce(Done<T>) -> Job,
        timeout: Duration,
    ) -> Result<T, ImapClientError> {
        let (tx, rx) = mpsc::sync_channel::<Result<T, ImapClientError>>(1);
        self.submit(make(Box::new(move |r| {
            let _ = tx.send(r);
        })));
        match rx.recv_timeout(timeout) {
            Ok(r) => r,
            Err(_) => Err(ImapClientError::new(format!("timeout after {}s waiting for IMAP", timeout.as_secs()))),
        }
    }

    /// LOGOUT; the connection closes and every queued job fails.
    pub fn close(&self) {
        self.submit(Job::Logout);
    }
}

impl Shared {
    /// The connection is over: fail everything outstanding.
    fn close(&self, why: String) {
        let (jobs, timer) = {
            let mut inner = self.inner.lock().unwrap();
            if matches!(inner.phase, Phase::Closed(_)) {
                return;
            }
            crate::trace_log!("imap", "session closed: {why}");
            inner.phase = Phase::Closed(why.clone());
            let mut jobs: Vec<Job> = inner.queue.drain(..).collect();
            if let Some(j) = inner.pending_select.take() {
                jobs.push(j);
            }
            if let Some(j) = inner.current.take() {
                jobs.push(j);
            }
            (jobs, inner.idle_timer.take())
        };
        self.ready.notify_all();
        if let Some(t) = timer {
            t.cancel();
        }
        for j in jobs {
            j.fail(ImapClientError::new(why.clone()));
        }
    }
}

struct Factory(Arc<Shared>);

impl ImapClientHandlerFactory for Factory {
    fn create(&self) -> Box<dyn ImapClientDriver> {
        Box::new(Driver { shared: Arc::clone(&self.0), events: Events { stale: Arc::clone(&self.0.config.folder_list_stale) } })
    }

    fn connect_failed(&self, host: &str, error: &io::Error) {
        self.0.close(format!("connect to {host} failed: {error}"));
    }
}

/// Unsolicited mailbox events: everything that means "the folder list or a
/// message window may be out of date".
struct Events {
    stale: Arc<AtomicBool>,
}

impl MailboxEventListener for Events {
    fn on_exists(&mut self, _count: u32) {
        self.stale.store(true, Ordering::Release);
    }
    fn on_recent(&mut self, _count: u32) {
        self.stale.store(true, Ordering::Release);
    }
    fn on_expunge(&mut self, _seq: u32) {
        self.stale.store(true, Ordering::Release);
    }
    fn on_flags(&mut self, _seq: u32, _flags: &[String]) {}
}

struct Driver {
    shared: Arc<Shared>,
    events: Events,
}

/// hopf-auth's name for one of ours.
fn hopf_mechanism(m: SaslMechanism) -> HopfMechanism {
    match m {
        SaslMechanism::Plain => HopfMechanism::Plain,
        SaslMechanism::Login => HopfMechanism::Login,
        SaslMechanism::CramMd5 => HopfMechanism::CramMd5,
        SaslMechanism::ScramSha256 => HopfMechanism::ScramSha256,
        SaslMechanism::XOAuth2 => HopfMechanism::XOauth2,
    }
}

impl Driver {
    fn fail_session(&self, why: impl Into<String>) {
        self.shared.close(why.into());
    }

    /// Pick and start authentication given the server's capability set.
    fn authenticate(&self, auth: &mut dyn ImapClientNotAuthenticated, caps: &ImapCapabilities) {
        let Some((user, secret, mech)) = self.shared.config.auth.clone() else {
            self.fail_session("no credentials");
            auth.logout();
            return;
        };
        let advertised = |m: HopfMechanism| caps.has(&format!("AUTH={}", m.name()));
        let candidates: Vec<HopfMechanism> = match mech {
            SaslMechanism::XOAuth2 => vec![HopfMechanism::XOauth2, HopfMechanism::OauthBearer],
            m => {
                let mut v = vec![hopf_mechanism(m)];
                for extra in [HopfMechanism::ScramSha256, HopfMechanism::CramMd5, HopfMechanism::Plain, HopfMechanism::Login] {
                    if !v.contains(&extra) {
                        v.push(extra);
                    }
                }
                v
            }
        };
        if let Some(m) = candidates.into_iter().find(|m| advertised(*m)) {
            let mut client = create_client(m, &user, &secret, &self.shared.config.host, "imap", None);
            let initial = if client.has_initial_response() {
                match client.evaluate(None) {
                    SaslClientStep::Response(r) => Some(r),
                    SaslClientStep::Complete(r) => Some(r),
                    SaslClientStep::Failure => None,
                }
            } else {
                None
            };
            self.shared.inner.lock().unwrap().sasl = Some(client);
            crate::trace_log!("imap", "AUTHENTICATE {}", m.name());
            auth.authenticate(m.name(), initial.as_deref());
            return;
        }
        if mech == SaslMechanism::XOAuth2 {
            self.fail_session("server offers neither AUTH=XOAUTH2 nor AUTH=OAUTHBEARER");
            auth.logout();
            return;
        }
        if caps.login_disabled {
            self.fail_session("server offers no usable AUTH mechanism and LOGIN is disabled");
            auth.logout();
            return;
        }
        crate::trace_log!("imap", "LOGIN {user}");
        auth.login(&user, &secret);
    }

    /// Called from `on_wake` and after every completion: run the next job
    /// when nothing is in flight, or go idle.
    fn drive_authenticated(&mut self, session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint) {
        let mut inner = self.shared.inner.lock().unwrap();
        if inner.phase != Phase::Ready || inner.current.is_some() || inner.pending_select.is_some() {
            return;
        }
        if inner.idle != IdleState::Off {
            return;
        }
        let Some(job) = inner.queue.pop_front() else {
            self.maybe_idle(&mut inner, session, ep);
            return;
        };
        if let Some(mb) = job.needs_mailbox() {
            if inner.selected.as_deref() != Some(mb) {
                let mb = mb.to_string();
                inner.pending_select = Some(job);
                inner.selected = None;
                drop(inner);
                session.select(&mb);
                return;
            }
            // Selected and matching, but this state object cannot issue
            // mailbox commands; the next wake arrives as `Selected`.
            inner.queue.push_front(job);
            drop(inner);
            ep.handle().poke();
            return;
        }
        self.issue_authenticated(&mut inner, job, session);
    }

    fn drive_selected(&mut self, selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint) {
        let mut inner = self.shared.inner.lock().unwrap();
        if inner.phase != Phase::Ready || inner.current.is_some() || inner.pending_select.is_some() {
            return;
        }
        if inner.idle != IdleState::Off {
            return;
        }
        let Some(job) = inner.queue.pop_front() else {
            self.maybe_idle(&mut inner, selected, ep);
            return;
        };
        match job.needs_mailbox() {
            Some(mb) if inner.selected.as_deref() != Some(mb) => {
                let mb = mb.to_string();
                inner.pending_select = Some(job);
                inner.selected = None;
                drop(inner);
                selected.select(&mb);
            }
            Some(_) => self.issue_selected(&mut inner, job, selected),
            None => self.issue_authenticated(&mut inner, job, selected),
        }
    }

    /// Nothing to do: arm the quiet timer, or, when it has fired, IDLE / NOOP.
    fn maybe_idle(&self, inner: &mut Inner, session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint) {
        if !self.shared.config.auto_idle {
            return;
        }
        if inner.idle_due {
            inner.idle_due = false;
            if inner.selected.is_some() && inner.caps.idle {
                crate::trace_log!("imap", "IDLE");
                session.idle();
            } else {
                inner.current = Some(Job::Noop);
                session.noop();
            }
            return;
        }
        if inner.idle_timer.is_none() {
            let secs = self.shared.config.min_idle_secs.load(Ordering::Relaxed).clamp(1, 864_000);
            inner.idle_timer = Some(self.arm_timer(ep, Duration::from_secs(u64::from(secs)), false));
        }
    }

    /// A timer that sets a flag and pokes the connection so `on_wake` acts.
    fn arm_timer(&self, ep: &mut dyn Endpoint, after: Duration, restart: bool) -> TimerHandle {
        let shared = Arc::clone(&self.shared);
        let handle = ep.handle();
        ep.schedule_timer(
            after,
            Box::new(move || {
                {
                    let mut inner = shared.inner.lock().unwrap();
                    inner.idle_timer = None;
                    if restart {
                        inner.idle_restart_due = true;
                    } else {
                        inner.idle_due = true;
                    }
                }
                handle.poke();
            }),
        )
    }

    fn cancel_idle_timer(inner: &mut Inner) {
        if let Some(t) = inner.idle_timer.take() {
            t.cancel();
        }
        inner.idle_due = false;
    }

    fn issue_authenticated<S: ImapClientAuthenticated + ?Sized>(&self, inner: &mut Inner, job: Job, s: &mut S) {
        crate::trace_log!("imap", "issue {}", job.kind());
        Self::cancel_idle_timer(inner);
        inner.list_rows.clear();
        inner.status_rows.clear();
        inner.numbers.clear();
        match &job {
            Job::List { pattern, status_unseen, .. } => {
                if *status_unseen {
                    let opts = ImapListOptions { return_status: vec!["UNSEEN".into()], ..Default::default() };
                    s.list_extended("", pattern, &opts);
                } else {
                    s.list("", pattern);
                }
            }
            Job::Lsub { .. } => s.lsub("", "*"),
            Job::Status { mailbox, .. } => s.status(mailbox, "(UNSEEN)"),
            Job::Select { mailbox, .. } => {
                inner.selected = None;
                s.select(mailbox);
            }
            Job::Append { mailbox, flags, data, .. } => {
                // Always the synchronizing literal: the data goes out on the
                // `+` continuation (`on_append_continue`), which every server
                // sends. LITERAL- would save one round trip on small
                // messages but needs the literal pushed right behind the
                // command, which this state object cannot do.
                inner.append_data = Some(data.clone());
                s.append(mailbox, flags.as_deref(), None, data.len() as u64, false);
            }
            Job::Create { mailbox, .. } => s.create(mailbox),
            Job::Delete { mailbox, .. } => s.delete(mailbox),
            Job::Rename { from, to, .. } => s.rename(from, to),
            Job::Subscribe { mailbox, .. } => s.subscribe(mailbox),
            Job::Unsubscribe { mailbox, .. } => s.unsubscribe(mailbox),
            Job::Noop => s.noop(),
            Job::Logout => s.logout(),
            _ => unreachable!("mailbox job issued without a selected mailbox"),
        }
        inner.current = Some(job);
    }

    fn issue_selected(&self, inner: &mut Inner, job: Job, s: &mut dyn ImapClientSelected) {
        crate::trace_log!("imap", "issue {}", job.kind());
        Self::cancel_idle_timer(inner);
        inner.fetch_cur = None;
        inner.numbers.clear();
        match &job {
            Job::Fetch { uid, set, items, .. } => {
                if *uid {
                    s.uid_fetch(set, items);
                } else {
                    s.fetch(set, items);
                }
            }
            Job::UidSort { criteria, .. } => s.uid_sort(criteria, "UTF-8", "ALL"),
            Job::UidSearch { criteria, .. } => s.uid_search(criteria),
            Job::UidStore { set, action, flags, .. } => s.uid_store(set, action, flags),
            Job::UidCopy { set, dest, .. } => s.uid_copy(set, dest),
            Job::UidMove { set, dest, .. } => s.uid_move(set, dest),
            Job::Expunge { .. } => s.expunge(),
            Job::UidExpunge { set, .. } => s.uid_expunge(set),
            _ => {
                self.issue_authenticated(inner, job, s);
                return;
            }
        }
        inner.current = Some(job);
    }

    /// Take the job in flight, hand it its result, then wake the driver
    /// for the next one.
    fn finish<F: FnOnce(Job)>(&self, ep: &mut dyn Endpoint, deliver: F) {
        let job = self.shared.inner.lock().unwrap().current.take();
        if let Some(job) = job {
            crate::trace_log!("imap", "complete {}", job.kind());
            deliver(job);
        }
        ep.handle().poke();
    }

    fn status_err(status: ImapStatus, message: &str, what: &str) -> Result<(), ImapClientError> {
        if status == ImapStatus::Ok {
            Ok(())
        } else {
            Err(ImapClientError::new(format!("{what}: {status:?} {message}")))
        }
    }
}

impl ImapClientDriver for Driver {
    fn mailbox_events(&mut self) -> Option<&mut dyn MailboxEventListener> {
        Some(&mut self.events)
    }

    fn on_greeting(&mut self, auth: &mut dyn ImapClientNotAuthenticated, ep: &mut dyn Endpoint, _text: &str, preauth: bool, caps: &ImapCapabilities) {
        if preauth {
            return; // on_authenticated follows
        }
        let cfg = &self.shared.config;
        if !cfg.implicit_tls && cfg.starttls && !ep.is_secure() {
            if caps.tokens.is_empty() {
                auth.capability();
            } else if caps.starttls {
                auth.starttls();
            } else {
                self.fail_session("STARTTLS required but the server does not offer it");
                auth.logout();
            }
            return;
        }
        if caps.tokens.is_empty() {
            auth.capability();
        } else {
            self.authenticate(auth, caps);
        }
    }

    fn on_capability(&mut self, auth: &mut dyn ImapClientNotAuthenticated, ep: &mut dyn Endpoint, caps: &ImapCapabilities) {
        crate::trace_log!("imap", "capability: {}", caps.tokens.join(" "));
        let ready = {
            let mut inner = self.shared.inner.lock().unwrap();
            inner.caps = caps.clone();
            inner.phase == Phase::Ready
        };
        if ready {
            // Post-login refresh; nothing to issue from here.
            ep.handle().poke();
            return;
        }
        let cfg = &self.shared.config;
        if !cfg.implicit_tls && cfg.starttls && !ep.is_secure() {
            if caps.starttls {
                auth.starttls();
            } else {
                self.fail_session("STARTTLS required but the server does not offer it");
                auth.logout();
            }
            return;
        }
        self.authenticate(auth, caps);
    }

    fn on_tls_established(&mut self, post: &mut dyn ImapClientPostStarttls, _ep: &mut dyn Endpoint) {
        post.capability();
    }

    fn on_tls_unavailable(&mut self, auth: &mut dyn ImapClientNotAuthenticated, _ep: &mut dyn Endpoint, message: &str) {
        self.fail_session(format!("STARTTLS refused: {message}"));
        auth.logout();
    }

    fn on_authenticated(&mut self, session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint, caps: &ImapCapabilities) {
        let need_caps = {
            let mut inner = self.shared.inner.lock().unwrap();
            inner.sasl = None;
            if !caps.tokens.is_empty() {
                inner.caps = caps.clone();
            }
            inner.handle = Some(ep.handle());
            inner.phase = Phase::Ready;
            inner.caps.tokens.is_empty()
        };
        self.shared.ready.notify_all();
        crate::trace_log!("imap", "authenticated; caps={}", self.shared.inner.lock().unwrap().caps.tokens.join(" "));
        if need_caps {
            session.capability();
        } else {
            ep.handle().poke();
        }
    }

    fn on_auth_failed(&mut self, auth: &mut dyn ImapClientNotAuthenticated, _ep: &mut dyn Endpoint, message: &str) {
        self.fail_session(format!("authentication failed: {message}"));
        auth.logout();
    }

    fn on_auth_continue(&mut self, exchange: &mut dyn ImapClientAuthExchange, _ep: &mut dyn Endpoint, text: &str) {
        let raw = base64::engine::general_purpose::STANDARD.decode(text.trim()).unwrap_or_default();
        let mut inner = self.shared.inner.lock().unwrap();
        let Some(mut client) = inner.sasl.take() else {
            drop(inner);
            exchange.abort();
            return;
        };
        match client.evaluate(Some(&raw)) {
            SaslClientStep::Response(r) => {
                inner.sasl = Some(client);
                drop(inner);
                exchange.respond(&r);
            }
            SaslClientStep::Complete(r) => {
                inner.sasl = Some(client);
                drop(inner);
                if !r.is_empty() {
                    exchange.respond(&r);
                } else {
                    exchange.respond(&[]);
                }
            }
            SaslClientStep::Failure => {
                drop(inner);
                exchange.abort();
            }
        }
    }

    fn on_selected(&mut self, selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, info: &ImapMailboxInfo, _read_only: bool) {
        crate::trace_log!("imap", "on_selected {:?} exists={}", info.name, info.exists);
        let mut inner = self.shared.inner.lock().unwrap();
        let name = match (&inner.current, &inner.pending_select) {
            (Some(Job::Select { mailbox, .. }), _) => mailbox.clone(),
            (_, Some(job)) => job.needs_mailbox().unwrap_or("").to_string(),
            _ => info.name.clone(),
        };
        inner.selected = Some(name);
        if let Some(job) = inner.pending_select.take() {
            self.issue_selected(&mut inner, job, selected);
            return;
        }
        if matches!(inner.current, Some(Job::Select { .. })) {
            let job = inner.current.take();
            drop(inner);
            if let Some(Job::Select { done, .. }) = job {
                done(Ok(info.clone()));
            }
            ep.handle().poke();
        }
    }

    fn on_select_failed(&mut self, _session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint, message: &str) {
        crate::trace_log!("imap", "on_select_failed: {message}");
        let (pending, current) = {
            let mut inner = self.shared.inner.lock().unwrap();
            inner.selected = None;
            (inner.pending_select.take(), if matches!(inner.current, Some(Job::Select { .. })) { inner.current.take() } else { None })
        };
        let err = ImapClientError::new(format!("SELECT failed: {message}"));
        if let Some(j) = pending {
            j.fail(err.clone());
        }
        if let Some(j) = current {
            j.fail(err);
        }
        ep.handle().poke();
    }

    fn on_fetch_literal_begin(&mut self, _ep: &mut dyn Endpoint, seq: u32, _section: &str, size: u64) {
        let mut inner = self.shared.inner.lock().unwrap();
        if let Some(Job::Fetch { stream: Some(stream), .. }) = inner.current.as_mut() {
            (stream.on_begin)(size);
            return;
        }
        match inner.fetch_cur.as_mut() {
            Some(cur) if cur.seq == seq => {}
            _ => inner.fetch_cur = Some(FetchItem { seq, ..Default::default() }),
        }
    }

    fn on_fetch_literal(&mut self, data: &[u8], ep: &mut dyn Endpoint) {
        let mut inner = self.shared.inner.lock().unwrap();
        if let Some(Job::Fetch { stream: Some(stream), .. }) = inner.current.as_mut() {
            (stream.on_chunk)(data, ep);
            return;
        }
        if let Some(cur) = inner.fetch_cur.as_mut() {
            cur.literal.extend_from_slice(data);
        }
    }

    fn on_fetch_data(&mut self, data: &ImapFetchData) {
        let mut inner = self.shared.inner.lock().unwrap();
        let mut item = match inner.fetch_cur.take() {
            Some(cur) if cur.seq == data.seq => cur,
            _ => FetchItem { seq: data.seq, ..Default::default() },
        };
        item.uid = data.uid;
        item.flags = data.flags.clone();
        item.size = data.size;
        item.bodystructure = data.bodystructure.clone();
        if item.literal.is_empty() && !data.body.is_empty() {
            item.literal = data.body.clone();
        }
        if let Some(Job::Fetch { on_item, .. }) = inner.current.as_ref() {
            on_item(item);
        }
    }

    fn on_fetch_complete(&mut self, _selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        crate::trace_log!("imap", "on_fetch_complete {status:?} {message}");
        self.finish(ep, |job| {
            if let Job::Fetch { done, .. } = job {
                done(Self::status_err(status, message, "FETCH"));
            }
        });
    }

    fn on_search_numbers(&mut self, numbers: &[u32]) {
        self.shared.inner.lock().unwrap().numbers.extend_from_slice(numbers);
    }

    fn on_search_complete(&mut self, _selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        let nums = std::mem::take(&mut self.shared.inner.lock().unwrap().numbers);
        self.finish(ep, |job| {
            if let Job::UidSearch { done, .. } = job {
                done(Self::status_err(status, message, "SEARCH").map(|_| nums));
            }
        });
    }

    fn on_sort_numbers(&mut self, numbers: &[u32]) {
        self.shared.inner.lock().unwrap().numbers.extend_from_slice(numbers);
    }

    fn on_sort_complete(&mut self, _selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        crate::trace_log!("imap", "on_sort_complete {status:?} {message}");
        let nums = std::mem::take(&mut self.shared.inner.lock().unwrap().numbers);
        self.finish(ep, |job| {
            if let Job::UidSort { done, .. } = job {
                done(Self::status_err(status, message, "SORT").map(|_| nums));
            }
        });
    }

    fn on_list_entry(&mut self, entry: &ImapListEntry) {
        let mut inner = self.shared.inner.lock().unwrap();
        match inner.current.as_ref() {
            Some(Job::List { status_unseen: true, .. }) => inner.list_rows.push(entry.clone()),
            Some(Job::List { on_row, .. }) => on_row(entry, None),
            Some(Job::Lsub { on_row, .. }) => on_row(entry),
            _ => {}
        }
    }

    fn on_list_complete(&mut self, _session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        let (rows, statuses) = {
            let mut inner = self.shared.inner.lock().unwrap();
            (std::mem::take(&mut inner.list_rows), std::mem::take(&mut inner.status_rows))
        };
        self.finish(ep, |job| match job {
            Job::List { on_row, done, status_unseen, .. } => {
                if status_unseen {
                    for e in &rows {
                        on_row(e, Some(statuses.get(&e.name).and_then(|s| s.unseen).unwrap_or(0)));
                    }
                }
                done(Self::status_err(status, message, "LIST"));
            }
            Job::Lsub { done, .. } => done(Self::status_err(status, message, "LSUB")),
            _ => {}
        });
    }

    fn on_status_data(&mut self, data: &ImapStatusData) {
        self.shared.inner.lock().unwrap().status_rows.insert(data.mailbox.clone(), data.clone());
    }

    fn on_status_complete(&mut self, _session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        let statuses = std::mem::take(&mut self.shared.inner.lock().unwrap().status_rows);
        self.finish(ep, |job| {
            if let Job::Status { mailbox, done } = job {
                done(Self::status_err(status, message, "STATUS").and_then(|_| {
                    statuses
                        .get(&mailbox)
                        .cloned()
                        .or_else(|| statuses.values().next().cloned())
                        .ok_or_else(|| ImapClientError::new("STATUS: no data in reply"))
                }));
            }
        });
    }

    fn on_store_complete(&mut self, _selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        self.finish(ep, |job| {
            if let Job::UidStore { done, .. } = job {
                done(Self::status_err(status, message, "STORE"));
            }
        });
    }

    fn on_copy_complete(&mut self, _selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, status: ImapStatus, copyuid: Option<&ImapCopyUid>, message: &str) {
        let cu = copyuid.cloned();
        self.finish(ep, |job| {
            if let Job::UidCopy { done, .. } = job {
                done(Self::status_err(status, message, "COPY").map(|_| cu));
            }
        });
    }

    fn on_move_complete(&mut self, _selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, status: ImapStatus, copyuid: Option<&ImapCopyUid>, message: &str) {
        let cu = copyuid.cloned();
        self.finish(ep, |job| {
            if let Job::UidMove { done, .. } = job {
                done(Self::status_err(status, message, "MOVE").map(|_| cu));
            }
        });
    }

    fn on_expunge_complete(&mut self, _selected: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        self.finish(ep, |job| match job {
            Job::Expunge { done, .. } | Job::UidExpunge { done, .. } => done(Self::status_err(status, message, "EXPUNGE")),
            _ => {}
        });
    }

    fn on_idle_started(&mut self, _idle: &mut dyn ImapClientIdle, ep: &mut dyn Endpoint) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.idle = IdleState::Active;
        inner.idle_timer = Some(self.arm_timer(ep, IDLE_RESTART, true));
        if !inner.queue.is_empty() {
            drop(inner);
            ep.handle().poke();
        }
    }

    fn on_idle_complete(&mut self, _session: &mut dyn ImapClientSelected, ep: &mut dyn Endpoint, _status: ImapStatus, _message: &str) {
        {
            let mut inner = self.shared.inner.lock().unwrap();
            inner.idle = IdleState::Off;
            Self::cancel_idle_timer(&mut inner);
            inner.idle_restart_due = false;
        }
        ep.handle().poke();
    }

    fn on_append_continue(&mut self, append: &mut dyn ImapClientAppend, _ep: &mut dyn Endpoint, _text: &str) {
        let data = self.shared.inner.lock().unwrap().append_data.take();
        if let Some(d) = data {
            append.send_literal(&d);
        }
    }

    fn on_append_complete(&mut self, _session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint, status: ImapStatus, appenduid: Option<&ImapAppendUid>, message: &str) {
        let au = appenduid.cloned();
        self.shared.inner.lock().unwrap().append_data = None;
        self.finish(ep, |job| {
            if let Job::Append { done, .. } = job {
                done(Self::status_err(status, message, "APPEND").map(|_| au));
            }
        });
    }

    fn on_mailbox_op_complete(&mut self, _session: &mut dyn ImapClientAuthenticated, ep: &mut dyn Endpoint, status: ImapStatus, message: &str) {
        self.finish(ep, |job| match job {
            Job::Create { done, .. } => done(Self::status_err(status, message, "CREATE")),
            Job::Delete { done, .. } => done(Self::status_err(status, message, "DELETE")),
            Job::Rename { done, .. } => done(Self::status_err(status, message, "RENAME")),
            Job::Subscribe { done, .. } => done(Self::status_err(status, message, "SUBSCRIBE")),
            Job::Unsubscribe { done, .. } => done(Self::status_err(status, message, "UNSUBSCRIBE")),
            _ => {}
        });
    }

    fn on_command_complete(&mut self, ep: &mut dyn Endpoint, _tag: &str, status: ImapStatus, _code: Option<&str>, message: &str) {
        crate::trace_log!("imap", "on_command_complete {status:?} {message}");
        // NOOP and anything without a dedicated completion.
        let is_noop = matches!(self.shared.inner.lock().unwrap().current, Some(Job::Noop));
        if is_noop {
            self.finish(ep, |_| {});
            let _ = (status, message);
        }
    }

    fn on_wake(&mut self, state: ImapClientWakeState<'_>, ep: &mut dyn Endpoint) {
        match state {
            ImapClientWakeState::Authenticated(a) => self.drive_authenticated(a, ep),
            ImapClientWakeState::Selected(s) => self.drive_selected(s, ep),
            ImapClientWakeState::Idle(i) => {
                let mut inner = self.shared.inner.lock().unwrap();
                if inner.idle == IdleState::Active && (!inner.queue.is_empty() || inner.idle_restart_due) {
                    inner.idle = IdleState::Ending;
                    drop(inner);
                    i.done();
                }
            }
            ImapClientWakeState::NotAuthenticated(_) | ImapClientWakeState::Busy => {}
        }
    }

    fn on_error(&mut self, ep: &mut dyn Endpoint, err: &io::Error) {
        crate::trace_log!("imap", "on_error: {err}");
        self.fail_session(format!("connection error: {err}"));
        ep.close();
    }

    fn on_timeout(&mut self, ep: &mut dyn Endpoint) {
        crate::trace_log!("imap", "on_timeout");
        self.fail_session("timed out waiting for the server");
        ep.close();
    }

    fn on_disconnected(&mut self, _ep: &mut dyn Endpoint) {
        self.fail_session("connection closed");
    }
}

// ── Capability probe (no login) ─────────────────────────────────────────────

/// Connect, reach the point where credentials would be needed, read the
/// capability list, log out. For the credential prompt, which shows the
/// server's AUTH mechanisms.
pub fn probe_capabilities_blocking(config: &ImapSessionConfig, timeout: Duration) -> Result<Vec<String>, ImapClientError> {
    struct Probe {
        cfg: ImapSessionConfig,
        tx: Mutex<Option<mpsc::SyncSender<Result<Vec<String>, ImapClientError>>>>,
    }
    impl Probe {
        fn report(&self, r: Result<Vec<String>, ImapClientError>) {
            if let Some(tx) = self.tx.lock().unwrap().take() {
                let _ = tx.send(r);
            }
        }
    }
    struct ProbeFactory(Arc<Probe>);
    impl ImapClientHandlerFactory for ProbeFactory {
        fn create(&self) -> Box<dyn ImapClientDriver> {
            Box::new(ProbeDriver(Arc::clone(&self.0)))
        }
        fn connect_failed(&self, host: &str, error: &io::Error) {
            self.0.report(Err(ImapClientError::new(format!("connect to {host} failed: {error}"))));
        }
    }
    struct ProbeDriver(Arc<Probe>);
    impl ProbeDriver {
        fn settle(&self, auth: &mut dyn ImapClientNotAuthenticated, ep: &mut dyn Endpoint, caps: &ImapCapabilities) {
            let cfg = &self.0.cfg;
            if !cfg.implicit_tls && cfg.starttls && !ep.is_secure() {
                if caps.tokens.is_empty() {
                    auth.capability();
                } else if caps.starttls {
                    auth.starttls();
                } else {
                    self.0.report(Err(ImapClientError::new("STARTTLS required but the server does not offer it")));
                    auth.logout();
                }
                return;
            }
            if caps.tokens.is_empty() {
                auth.capability();
                return;
            }
            self.0.report(Ok(caps.tokens.clone()));
            auth.logout();
        }
    }
    impl ImapClientDriver for ProbeDriver {
        fn on_greeting(&mut self, auth: &mut dyn ImapClientNotAuthenticated, ep: &mut dyn Endpoint, _t: &str, preauth: bool, caps: &ImapCapabilities) {
            if preauth {
                self.0.report(Ok(caps.tokens.clone()));
                auth.logout();
                return;
            }
            self.settle(auth, ep, caps);
        }
        fn on_capability(&mut self, auth: &mut dyn ImapClientNotAuthenticated, ep: &mut dyn Endpoint, caps: &ImapCapabilities) {
            self.settle(auth, ep, caps);
        }
        fn on_tls_established(&mut self, post: &mut dyn ImapClientPostStarttls, _ep: &mut dyn Endpoint) {
            post.capability();
        }
        fn on_tls_unavailable(&mut self, auth: &mut dyn ImapClientNotAuthenticated, _ep: &mut dyn Endpoint, m: &str) {
            self.0.report(Err(ImapClientError::new(format!("STARTTLS refused: {m}"))));
            auth.logout();
        }
        fn on_authenticated(&mut self, session: &mut dyn ImapClientAuthenticated, _ep: &mut dyn Endpoint, caps: &ImapCapabilities) {
            self.0.report(Ok(caps.tokens.clone()));
            session.logout();
        }
        fn on_auth_failed(&mut self, auth: &mut dyn ImapClientNotAuthenticated, _ep: &mut dyn Endpoint, m: &str) {
            self.0.report(Err(ImapClientError::new(m)));
            auth.logout();
        }
        fn on_auth_continue(&mut self, exchange: &mut dyn ImapClientAuthExchange, _ep: &mut dyn Endpoint, _t: &str) {
            exchange.abort();
        }
        fn on_selected(&mut self, _s: &mut dyn ImapClientSelected, _ep: &mut dyn Endpoint, _i: &ImapMailboxInfo, _ro: bool) {}
        fn on_select_failed(&mut self, _s: &mut dyn ImapClientAuthenticated, _ep: &mut dyn Endpoint, _m: &str) {}
        fn on_fetch_literal(&mut self, _d: &[u8], _ep: &mut dyn Endpoint) {}
        fn on_fetch_complete(&mut self, _s: &mut dyn ImapClientSelected, _ep: &mut dyn Endpoint, _st: ImapStatus, _m: &str) {}
        fn on_append_continue(&mut self, append: &mut dyn ImapClientAppend, _ep: &mut dyn Endpoint, _t: &str) {
            append.send_literal(&[]);
        }
        fn on_append_complete(&mut self, _s: &mut dyn ImapClientAuthenticated, _ep: &mut dyn Endpoint, _st: ImapStatus, _a: Option<&ImapAppendUid>, _m: &str) {}
        fn on_error(&mut self, ep: &mut dyn Endpoint, err: &io::Error) {
            self.0.report(Err(ImapClientError::new(err.to_string())));
            ep.close();
        }
        fn on_timeout(&mut self, ep: &mut dyn Endpoint) {
            self.0.report(Err(ImapClientError::new("timed out waiting for the server")));
            ep.close();
        }
        fn on_disconnected(&mut self, _ep: &mut dyn Endpoint) {
            self.0.report(Err(ImapClientError::new("connection closed")));
        }
    }

    let (tx, rx) = mpsc::sync_channel(1);
    let probe = Arc::new(Probe { cfg: config.clone(), tx: Mutex::new(Some(tx)) });
    let client = ImapClient::new(config.host.as_str(), config.port)
        .timeouts(ImapClientTimeouts::default())
        .resolver(crate::net::hopf_dns_resolver());
    let client = if config.implicit_tls {
        client.implicit_tls(hopf_tls_connector(), config.host.as_str())
    } else if config.starttls {
        client.starttls(hopf_tls_connector(), config.host.as_str())
    } else {
        client
    };
    client.connect(&hopf_runtime(), Arc::new(ProbeFactory(probe)))?;
    rx.recv_timeout(timeout).unwrap_or_else(|_| Err(ImapClientError::new("timeout probing IMAP capabilities")))
}
