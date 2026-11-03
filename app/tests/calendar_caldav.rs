//! Pulling a CalDAV collection with each kind of authentication, against an
//! in-process hopf-webdav server that asks for it (or does not).

use std::net::SocketAddr;
use std::sync::Arc;

use hopf_auth::{PasswordStore, TrustPolicy};
use hopf_core::{ProtocolHandler, TcpListenerConfig};
use hopf_http::{BasicAuthConfig, BasicAuthFactory, BearerAuthFactory, CleartextHttpEndpoint, HttpLimits, ServerHandlerFactory};
use hopf_webdav::{WebDavConfig, WebDavFactory};
use tagliacarte_app::calendar_caldav_sync::{CaldavAuth, CaldavPullOutcome, needs_fetch, pull_calendar_collection};
use tagliacarte_app::calendar_sources::{
    CALENDAR_SCOPE, CalendarSource, SourceAuth, calendar_id, check_calendar_scope, delete_source, get_source, list_sources, save_source,
    sync_sources_with,
};
use tagliacarte_app::calendar_store::{list_events_in_range, open_calendar_db};
use tagliacarte_core::net::hopf_runtime;

#[derive(Clone, Copy)]
enum Guard {
    Open,
    Basic,
    Bearer,
}

fn start_server(root: &std::path::Path, guard: Guard) -> SocketAddr {
    let config = WebDavConfig {
        root_path: root.to_path_buf(),
        allow_write: true,
        webdav_enabled: true,
        ..WebDavConfig::default()
    }
    .allow_unauthenticated_access();
    let rt = hopf_runtime();
    let dav: Arc<dyn ServerHandlerFactory> = Arc::new(WebDavFactory::new(config, Arc::clone(rt.storage())).unwrap());
    let store = Arc::new(PasswordStore::new().with_user("alice", "secret").with_token("tok-1", "alice"));
    let factory: Arc<dyn ServerHandlerFactory> = match guard {
        Guard::Open => dav,
        Guard::Basic => Arc::new(BasicAuthFactory::new(dav, store, BasicAuthConfig::new("cal"))),
        Guard::Bearer => {
            let policy: Arc<dyn TrustPolicy> = store;
            Arc::new(BearerAuthFactory::new(dav, policy))
        }
    };
    let (addr, _) = rt
        .add_tcp_listener(TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
            Box::new(CleartextHttpEndpoint::new(Arc::clone(&factory), HttpLimits::default())) as Box<dyn ProtocolHandler>
        }))
        .unwrap();
    addr
}

fn ics(uid: &str, summary: &str, start: &str, end: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20260101T000000Z\r\n\
         DTSTART:{start}\r\nDTEND:{end}\r\nSUMMARY:{summary}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

/// A served directory with two events in it, and where to find it.
fn calendar(guard: Guard) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("work")).unwrap();
    std::fs::write(dir.path().join("work/a.ics"), ics("a@test", "Standup", "20260601T100000Z", "20260601T103000Z")).unwrap();
    std::fs::write(dir.path().join("work/b.ics"), ics("b@test", "Review", "20260602T140000Z", "20260602T150000Z")).unwrap();
    let addr = start_server(dir.path(), guard);
    let url = format!("http://{}:{}/work/", addr.ip(), addr.port());
    (dir, url)
}

fn titles(conn: &rusqlite::Connection, calendar_id: &str) -> Vec<String> {
    let mut t: Vec<String> = list_events_in_range(conn, 0, i64::MAX / 2, &[calendar_id.to_owned()])
        .unwrap()
        .into_iter()
        .map(|e| e.title)
        .collect();
    t.sort();
    t
}

fn pull(url: &str, auth: &CaldavAuth) -> (tempfile::TempDir, rusqlite::Connection, Result<u64, String>) {
    let data = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(data.path()).unwrap();
    let r = pull_calendar_collection(&conn, "caldav:t", "t", "Test", url, auth).map(|o| o.imported_events);
    (data, conn, r)
}

#[test]
fn a_collection_is_pulled_with_basic_authentication() {
    let (_served, url) = calendar(Guard::Basic);
    let (_d, conn, r) = pull(&url, &CaldavAuth::Basic { username: "alice".into(), password: "secret".into() });
    assert_eq!(r.unwrap(), 2);
    assert_eq!(titles(&conn, "caldav:t"), ["Review", "Standup"]);
}

#[test]
fn a_collection_is_pulled_with_a_bearer_token() {
    let (_served, url) = calendar(Guard::Bearer);
    let (_d, conn, r) = pull(&url, &CaldavAuth::Bearer("tok-1".into()));
    assert_eq!(r.unwrap(), 2);
    assert_eq!(titles(&conn, "caldav:t"), ["Review", "Standup"]);
}

#[test]
fn a_collection_is_pulled_with_no_authentication_where_none_is_asked_for() {
    let (_served, url) = calendar(Guard::Open);
    let (_d, conn, r) = pull(&url, &CaldavAuth::None);
    assert_eq!(r.unwrap(), 2);
    assert_eq!(titles(&conn, "caldav:t").len(), 2);
}

#[test]
fn the_wrong_credentials_are_refused_and_import_nothing() {
    let (_served, basic) = calendar(Guard::Basic);
    for auth in [CaldavAuth::None, CaldavAuth::Basic { username: "alice".into(), password: "wrong".into() }, CaldavAuth::Bearer("tok-1".into())] {
        let (_d, conn, r) = pull(&basic, &auth);
        assert!(r.as_ref().err().is_some_and(|e| e.contains("401")), "{r:?}");
        assert!(titles(&conn, "caldav:t").is_empty());
    }
    let (_served, bearer) = calendar(Guard::Bearer);
    let (_d, conn, r) = pull(&bearer, &CaldavAuth::Bearer("not-the-token".into()));
    assert!(r.as_ref().err().is_some_and(|e| e.contains("401")), "{r:?}");
    assert!(titles(&conn, "caldav:t").is_empty());
}

#[test]
fn the_calendar_is_named_for_the_source_not_its_id() {
    let (_served, url) = calendar(Guard::Open);
    let (_d, conn, r) = pull(&url, &CaldavAuth::None);
    r.unwrap();
    let cal = tagliacarte_app::calendar_view::list_calendars(&conn).unwrap().into_iter().find(|c| c.calendar_id == "caldav:t").unwrap();
    assert_eq!((cal.name.as_str(), cal.source_id.as_str()), ("Test", "t"));
}

fn source(id: &str, name: &str, url: &str, auth: SourceAuth) -> CalendarSource {
    CalendarSource { id: id.into(), name: name.into(), url: url.into(), auth, enabled: true }
}

#[test]
fn sources_are_kept_listed_edited_and_deleted_with_their_calendar_and_events() {
    let (_served, url) = calendar(Guard::Open);
    let data = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(data.path()).unwrap();
    assert!(list_sources(&conn).unwrap().is_empty());

    save_source(&conn, &source("s2", "Zed", "https://example.org/cal/", SourceAuth::Basic { username: "alice".into() })).unwrap();
    save_source(&conn, &source("s1", "Alpha", &url, SourceAuth::None)).unwrap();
    let names: Vec<String> = list_sources(&conn).unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["Alpha", "Zed"], "listed by name");

    // Editing is saving again under the same id.
    save_source(&conn, &source("s2", "Zulu", "https://example.org/other/", SourceAuth::Account { account_id: "g1".into() })).unwrap();
    let again = get_source(&conn, "s2").unwrap().unwrap();
    assert_eq!((again.name.as_str(), again.url.as_str(), again.auth.clone()), ("Zulu", "https://example.org/other/", SourceAuth::Account { account_id: "g1".into() }));
    assert_eq!(list_sources(&conn).unwrap().len(), 2);
    assert!(get_source(&conn, "nope").unwrap().is_none());

    // Only web addresses, and a name.
    for bad in ["", "ftp://example.org/x", "not a url", "file:///etc/passwd"] {
        assert!(save_source(&conn, &source("s3", "Bad", bad, SourceAuth::None)).is_err(), "{bad:?}");
    }
    assert!(save_source(&conn, &source("s3", "  ", "https://example.org/", SourceAuth::None)).is_err());
    assert_eq!(list_sources(&conn).unwrap().len(), 2);

    // Deleting a source takes its calendar and the events with it, and nothing else.
    pull_calendar_collection(&conn, &calendar_id("s1"), "s1", "Alpha", &url, &CaldavAuth::None).unwrap();
    pull_calendar_collection(&conn, &calendar_id("s2"), "s2", "Zulu", &url, &CaldavAuth::None).unwrap();
    delete_source(&conn, "s1").unwrap();
    assert!(get_source(&conn, "s1").unwrap().is_none());
    assert!(titles(&conn, &calendar_id("s1")).is_empty());
    assert_eq!(titles(&conn, &calendar_id("s2")).len(), 2);
    let calendars: Vec<String> = tagliacarte_app::calendar_view::list_calendars(&conn).unwrap().into_iter().map(|c| c.calendar_id).collect();
    assert!(!calendars.contains(&calendar_id("s1")) && calendars.contains(&calendar_id("s2")), "{calendars:?}");
}

#[test]
fn syncing_goes_through_every_enabled_source_and_reports_the_ones_that_fail() {
    let (_open_dir, open) = calendar(Guard::Open);
    let (_locked_dir, locked) = calendar(Guard::Basic);
    let data = tempfile::tempdir().unwrap();
    let conn = open_calendar_db(data.path()).unwrap();
    save_source(&conn, &source("ok", "Fine", &open, SourceAuth::None)).unwrap();
    save_source(&conn, &source("locked", "Locked", &locked, SourceAuth::Basic { username: "alice".into() })).unwrap();
    save_source(&conn, &source("nopw", "No password", &locked, SourceAuth::Basic { username: "alice".into() })).unwrap();
    let mut off = source("off", "Switched off", &open, SourceAuth::None);
    off.enabled = false;
    save_source(&conn, &off).unwrap();

    let mut asked = Vec::new();
    let report = sync_sources_with(&conn, &mut |s: &CalendarSource| {
        asked.push(s.id.clone());
        match s.id.as_str() {
            "ok" => Ok(CaldavAuth::None),
            "locked" => Ok(CaldavAuth::Basic { username: "alice".into(), password: "wrong".into() }),
            _ => Err("no saved password for this calendar".to_owned()),
        }
    });
    asked.sort();
    assert_eq!(asked, ["locked", "nopw", "ok"], "a source that is off is not even asked about");
    assert_eq!((report.calendars, report.events), (1, 2));
    let mut failed: Vec<(String, String)> = report.failed.clone();
    failed.sort();
    assert_eq!(failed.len(), 2, "{failed:?}");
    assert_eq!(failed[0].0, "Locked");
    assert!(failed[0].1.contains("401"), "{failed:?}");
    assert_eq!(failed[1], ("No password".to_owned(), "no saved password for this calendar".to_owned()));
    assert_eq!(titles(&conn, &calendar_id("ok")), ["Review", "Standup"]);
    assert!(titles(&conn, &calendar_id("locked")).is_empty());
}

#[test]
fn a_google_grant_without_the_calendar_scope_is_turned_down_with_a_way_forward() {
    use tagliacarte_core::oauth::OAuthTokenEntry;
    let entry = |scopes: &str| {
        OAuthTokenEntry { provider: "google".into(), access_token: "a".into(), refresh_token: "r".into(), expires_at: 0, scopes: scopes.into() }.to_json()
    };
    assert!(check_calendar_scope(&entry(&format!("https://mail.google.com/ {CALENDAR_SCOPE} openid"))).is_ok());
    let err = check_calendar_scope(&entry("https://mail.google.com/ openid")).unwrap_err();
    assert!(err.contains("calendar") && err.contains("Sign in"), "{err}");
    let readonly = check_calendar_scope(&entry("https://www.googleapis.com/auth/calendar.readonly")).unwrap_err();
    assert!(readonly.contains("Sign in"), "calendar.readonly is not the calendar scope");
    // A bare access token says nothing about its scopes; it is tried.
    assert!(check_calendar_scope("ya29.a0Af").is_ok());
}

// ── pulling only what has changed ──

fn pull_again(conn: &rusqlite::Connection, url: &str) -> CaldavPullOutcome {
    pull_calendar_collection(conn, "caldav:t", "t", "Test", url, &CaldavAuth::None).unwrap()
}

fn kept_calendar(dir: &tempfile::TempDir) -> rusqlite::Connection {
    open_calendar_db(dir.path()).unwrap()
}

#[test]
fn a_second_pull_fetches_nothing_that_has_not_changed() {
    let (_served, url) = calendar(Guard::Open);
    let data = tempfile::tempdir().unwrap();
    let conn = kept_calendar(&data);
    let first = pull_again(&conn, &url);
    assert_eq!((first.fetched_resources, first.imported_events, first.unchanged_resources, first.removed_resources), (2, 2, 0, 0));
    let second = pull_again(&conn, &url);
    assert_eq!((second.fetched_resources, second.imported_events, second.unchanged_resources, second.removed_resources), (0, 0, 2, 0), "nothing was fetched");
    assert_eq!(titles(&conn, "caldav:t"), ["Review", "Standup"], "and nothing was lost");
}

#[test]
fn a_changed_file_is_fetched_again_and_what_it_used_to_give_is_replaced() {
    let (served, url) = calendar(Guard::Open);
    let data = tempfile::tempdir().unwrap();
    let conn = kept_calendar(&data);
    pull_again(&conn, &url);
    // b.ics becomes a different event altogether (another UID), in a longer file.
    std::fs::write(served.path().join("work/b.ics"), ics("b2@test", "Retrospective with the whole team", "20260603T140000Z", "20260603T150000Z")).unwrap();
    let again = pull_again(&conn, &url);
    assert_eq!((again.fetched_resources, again.unchanged_resources, again.removed_resources), (1, 1, 0));
    assert_eq!(titles(&conn, "caldav:t"), ["Retrospective with the whole team", "Standup"], "the old Review is gone, not left beside the new");
}

#[test]
fn a_file_that_is_gone_from_the_server_takes_its_events_with_it() {
    let (served, url) = calendar(Guard::Open);
    let data = tempfile::tempdir().unwrap();
    let conn = kept_calendar(&data);
    pull_again(&conn, &url);
    std::fs::remove_file(served.path().join("work/a.ics")).unwrap();
    let again = pull_again(&conn, &url);
    assert_eq!((again.fetched_resources, again.unchanged_resources, again.removed_resources), (0, 1, 1));
    assert_eq!(titles(&conn, "caldav:t"), ["Review"]);
}

#[test]
fn a_new_file_is_fetched_and_the_rest_are_not() {
    let (served, url) = calendar(Guard::Open);
    let data = tempfile::tempdir().unwrap();
    let conn = kept_calendar(&data);
    pull_again(&conn, &url);
    std::fs::write(served.path().join("work/c.ics"), ics("c@test", "Planning", "20260604T090000Z", "20260604T100000Z")).unwrap();
    let again = pull_again(&conn, &url);
    assert_eq!((again.fetched_resources, again.unchanged_resources, again.imported_events), (1, 2, 1));
    assert_eq!(titles(&conn, "caldav:t"), ["Planning", "Review", "Standup"]);
}

#[test]
fn a_series_and_what_replaces_one_of_its_occurrences_arrive_in_one_file_and_show_in_their_places() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("work")).unwrap();
    std::fs::write(
        dir.path().join("work/series.ics"),
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\n\
         BEGIN:VEVENT\r\nUID:w\r\nDTSTAMP:20260101T000000Z\r\nSUMMARY:Weekly\r\nDTSTART;TZID=Europe/London:20260601T100000\r\nDTEND;TZID=Europe/London:20260601T110000\r\nRRULE:FREQ=WEEKLY;COUNT=4\r\n\
         BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:w\r\nDTSTAMP:20260101T000000Z\r\nSUMMARY:Weekly (late)\r\nRECURRENCE-ID;TZID=Europe/London:20260615T100000\r\nDTSTART;TZID=Europe/London:20260615T140000\r\nDTEND;TZID=Europe/London:20260615T150000\r\nEND:VEVENT\r\n\
         END:VCALENDAR\r\n",
    )
    .unwrap();
    let addr = start_server(dir.path(), Guard::Open);
    let url = format!("http://{}:{}/work/", addr.ip(), addr.port());
    let data = tempfile::tempdir().unwrap();
    let conn = kept_calendar(&data);
    let out = pull_again(&conn, &url);
    assert_eq!((out.fetched_resources, out.imported_events), (1, 2));
    let june = tagliacarte_app::calendar_view::events_in_range(&conn, 0, i64::MAX / 4, &[]).unwrap();
    let shown: Vec<(String, i64)> = june.iter().map(|e| (e.title.clone(), e.starts_at_utc_ms)).collect();
    let utc = |d: u32, h: u32| chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 6, d, h, 0, 0).unwrap().timestamp_millis();
    assert_eq!(
        shown,
        [("Weekly".to_owned(), utc(1, 9)), ("Weekly".to_owned(), utc(8, 9)), ("Weekly (late)".to_owned(), utc(15, 13)), ("Weekly".to_owned(), utc(22, 9))],
        "four weeks, the third one later in the day"
    );
    assert!(june.iter().all(|e| e.description.is_none()), "the reminder's text is not the event's");
}

#[test]
fn what_an_older_version_left_in_a_calendar_is_cleared_by_the_first_pull_that_tracks_files() {
    let (_served, url) = calendar(Guard::Open);
    let data = tempfile::tempdir().unwrap();
    let conn = kept_calendar(&data);
    // An event the server no longer has, from before files were tracked: no record of which file it came from.
    tagliacarte_app::calendar_store::upsert_calendar(&conn, &tagliacarte_core::calendar::CalendarDescriptor {
        calendar_id: "caldav:t".into(), source_id: "t".into(), name: "Test".into(), color_hex: None, enabled: true,
    })
    .unwrap();
    tagliacarte_app::calendar_store::upsert_event(&conn, &tagliacarte_core::calendar::CalendarEvent {
        event_id: "caldav:t:stale".into(), calendar_id: "caldav:t".into(), uid: "stale".into(), title: "Stale".into(),
        starts_at_utc_ms: 1000, ends_at_utc_ms: 2000, ..Default::default()
    })
    .unwrap();
    pull_again(&conn, &url);
    assert_eq!(titles(&conn, "caldav:t"), ["Review", "Standup"]);
}

#[test]
fn a_server_that_gives_no_tag_is_always_asked_and_one_that_does_is_asked_only_when_it_changed() {
    assert!(needs_fetch(None, None));
    assert!(needs_fetch(Some("\"a\""), None), "no tag now: fetch");
    assert!(needs_fetch(None, Some("\"a\"")), "never fetched: fetch");
    assert!(needs_fetch(Some("\"a\""), Some("\"b\"")));
    assert!(!needs_fetch(Some("W/\"a\""), Some("W/\"a\"")));
    assert!(needs_fetch(Some(""), Some("")), "an empty tag says nothing");
}
