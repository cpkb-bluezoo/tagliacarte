//! Pulling a CalDAV collection with each kind of authentication, against an
//! in-process hopf-webdav server that asks for it (or does not).

use std::net::SocketAddr;
use std::sync::Arc;

use hopf_auth::{PasswordStore, TrustPolicy};
use hopf_core::{ProtocolHandler, TcpListenerConfig};
use hopf_http::{BasicAuthConfig, BasicAuthFactory, BearerAuthFactory, CleartextHttpEndpoint, HttpLimits, ServerHandlerFactory};
use hopf_webdav::{WebDavConfig, WebDavFactory};
use tagliacarte_app::calendar_caldav_sync::{CaldavAuth, pull_calendar_collection};
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
