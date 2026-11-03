//! A source's password goes in the credential vault under its own key and comes back
//! when the source is signed in to. One test, because loading a config is process-wide.
//! The file backend is forced, so the system keychain is never touched.

use tagliacarte_app::calendar_caldav_sync::CaldavAuth;
use tagliacarte_app::calendar_sources::{CalendarSource, SourceAuth, credential_key, has_password, resolve_auth, save_password};

fn basic(id: &str, username: &str) -> CalendarSource {
    CalendarSource { id: id.into(), name: "Work".into(), url: "https://example.org/cal/".into(), auth: SourceAuth::Basic { username: username.into() }, enabled: true }
}

#[test]
fn a_password_is_kept_in_the_vault_and_found_again() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.xml");
    std::fs::write(&config, "<tagliacarte><security use-keychain=\"false\"/></tagliacarte>\n").unwrap();
    // SAFETY: the only test in this process, and nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("TAGLIACARTE_DATA_DIR", dir.path()) };
    let cfg = tagliacarte_app::frb_api::frb_load_config(config.to_string_lossy().into_owned());
    assert!(!cfg.use_keychain, "the file backend, never the keychain");

    // Nothing yet: a clear way forward, not a crash.
    assert!(!has_password("s1"));
    let none = resolve_auth(&basic("s1", "alice")).unwrap_err();
    assert!(none.contains("no saved password"), "{none}");

    save_password("s1", "alice", "s3cret").unwrap();
    assert!(has_password("s1") && !has_password("s2"));
    match resolve_auth(&basic("s1", "alice")).unwrap() {
        CaldavAuth::Basic { username, password } => assert_eq!((username.as_str(), password.as_str()), ("alice", "s3cret")),
        other => panic!("{other:?}"),
    }
    // The name on the source wins; without one, the one saved with the password is used.
    match resolve_auth(&basic("s1", "")).unwrap() {
        CaldavAuth::Basic { username, .. } => assert_eq!(username, "alice"),
        other => panic!("{other:?}"),
    }
    assert_eq!(credential_key("s1"), "caldav:s1");
    // The password is in the vault file, not in anything written for the calendar.
    assert!(!std::fs::read_to_string(&config).unwrap().contains("s3cret"));
    assert!(!matches!(resolve_auth(&CalendarSource { auth: SourceAuth::None, ..basic("s1", "") }).unwrap(), CaldavAuth::Basic { .. }));
}
