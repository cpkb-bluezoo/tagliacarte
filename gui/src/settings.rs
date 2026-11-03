//! Preferences: one setting at a time.
//!
//! The page sends `{key, value}` for the control the user changed; nothing
//! here receives or returns the whole configuration. Each change is a
//! read-modify-write of `config.xml` under a lock, and only after the file is
//! known to parse, so a config that could not be read is never replaced by an
//! empty one.

use std::path::Path;
use std::sync::Mutex;

use tagliacarte_app::frb_api::{FrbConfig, frb_load_config, frb_save_config};
use tagliacarte_core::tagliacarte_config_xml::load_tagliacarte_config;

use crate::args::Args;

static CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// The configuration, or why it must not be edited. A missing file is a fresh
/// install and fine; one that exists but does not parse is left alone.
pub fn load_for_edit(config_path: &str) -> Result<FrbConfig, String> {
    let p = Path::new(config_path);
    if p.is_file() {
        load_tagliacarte_config(p)
            .map_err(|e| format!("config.xml could not be read, so nothing was changed: {e}"))?;
    }
    Ok(frb_load_config(config_path.to_owned()))
}

/// Runs `edit` on the configuration and saves the result, serialised with
/// every other edit. Returns whatever `edit` returns plus the saved config.
pub fn edit_config<T>(
    config_path: &str,
    edit: impl FnOnce(&mut FrbConfig) -> Result<T, String>,
) -> Result<(T, FrbConfig), String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cfg = load_for_edit(config_path)?;
    let out = edit(&mut cfg)?;
    frb_save_config(config_path.to_owned(), cfg.clone())?;
    Ok((out, cfg))
}

/// Notes where the user is: the account in view and, when given, the folder
/// in it and the message in that folder. Returns whether anything changed.
///
/// Giving a folder sets the account's folder and message (no message means
/// none in the new folder). Giving only the account leaves both of that
/// account's own places alone, so that switching to it, or quitting right
/// there, does not forget where it was left.
pub fn remember_location(
    cfg: &mut FrbConfig,
    account_id: &str,
    folder: Option<&str>,
    message: Option<&str>,
) -> Result<bool, String> {
    let acc = cfg
        .accounts
        .iter_mut()
        .find(|a| a.id == account_id)
        .ok_or_else(|| format!("no such account '{account_id}'"))?;
    let mut changed = false;
    if let Some(f) = folder {
        let (f, m) = (Some(f.to_owned()), message.map(str::to_owned));
        if acc.last_folder != f || acc.last_message_id != m {
            acc.last_folder = f;
            acc.last_message_id = m;
            changed = true;
        }
    }
    if cfg.selected_store_id.as_deref() != Some(account_id) {
        cfg.selected_store_id = Some(account_id.to_owned());
        changed = true;
    }
    Ok(changed)
}

/// `remember_location` on `config.xml`, written only when it changed anything.
/// This is the file alone: unlike an account's edit it does not touch the
/// running session, which has no business reconnecting because a message was opened.
pub fn save_location(
    config_path: &str,
    account_id: &str,
    folder: Option<&str>,
    message: Option<&str>,
) -> Result<bool, String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cfg = load_for_edit(config_path)?;
    if !remember_location(&mut cfg, account_id, folder, message)? {
        return Ok(false);
    }
    frb_save_config(config_path.to_owned(), cfg)?;
    Ok(true)
}

fn text(a: &Args) -> Result<String, String> {
    a.str("value")
}

fn flag(a: &Args) -> Result<bool, String> {
    a.opt_bool("value").ok_or_else(|| "missing boolean argument 'value'".to_owned())
}

/// Applies one named preference. Unknown names are an error, so the page
/// cannot write arbitrary fields.
pub fn apply(cfg: &mut FrbConfig, key: &str, a: &Args) -> Result<(), String> {
    match key {
        "useKeychain" => cfg.use_keychain = flag(a)?,
        "loadRemoteImages" => {
            cfg.load_remote_images = flag(a)?;
            cfg.resource_policy =
                if cfg.load_remote_images { "allow-remote" } else { "block-remote" }.to_owned();
        }
        "threadedView" => cfg.threaded_view = flag(a)?,
        "quoteOriginal" => cfg.quote_original = flag(a)?,
        "notifyNewMessages" => cfg.notify_new_messages = flag(a)?,
        "composeUseRichText" => cfg.compose_use_rich_text = flag(a)?,
        "matrixChatUseRichText" => cfg.matrix_chat_use_rich_text = flag(a)?,
        "replyHeaderTemplate" => cfg.reply_header_template = text(a)?,
        "replyDateFormat" => cfg.reply_date_format = text(a)?,
        "replyTimeFormat" => cfg.reply_time_format = text(a)?,
        "replyLinePrefix" => {
            let v = text(a)?;
            cfg.reply_line_prefix = if v.is_empty() { "> ".to_owned() } else { v };
        }
        "replyQuoteMode" => cfg.reply_quote_mode = text(a)?,
        "replyPlainPosition" => cfg.reply_plain_position = text(a)?,
        "messageListSort" => cfg.message_list_sort = text(a)?,
        "mailCryptoStack" => cfg.mail_crypto_stack = text(a)?,
        "mailCryptoPgpSecretKeyPath" => cfg.mail_crypto_pgp_secret_key_path = text(a)?.trim().to_owned(),
        // Write-only: the page can set it but is never sent it back.
        "mailCryptoPgpPassphrase" => cfg.mail_crypto_pgp_passphrase = text(a)?,
        "mailCryptoSmimeCertPath" => cfg.mail_crypto_smime_cert_path = text(a)?.trim().to_owned(),
        "mailCryptoSmimeKeyPath" => cfg.mail_crypto_smime_key_path = text(a)?.trim().to_owned(),
        _ => return Err(format!("unknown setting '{key}'")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Args {
        Args::parse(json).unwrap()
    }

    #[test]
    fn applies_known_settings_and_refuses_others() {
        let mut cfg = FrbConfig::default();
        apply(&mut cfg, "threadedView", &args(r#"{"value":false}"#)).unwrap();
        assert!(!cfg.threaded_view);
        apply(&mut cfg, "replyHeaderTemplate", &args(r#"{"value":"$sender wrote:"}"#)).unwrap();
        assert_eq!(cfg.reply_header_template, "$sender wrote:");
        assert!(apply(&mut cfg, "accounts", &args(r#"{"value":"x"}"#)).is_err());
        assert!(apply(&mut cfg, "threadedView", &args(r#"{"value":"yes"}"#)).is_err());
    }

    #[test]
    fn remote_images_drives_the_resource_policy_and_an_empty_prefix_is_defaulted() {
        let mut cfg = FrbConfig::default();
        apply(&mut cfg, "loadRemoteImages", &args(r#"{"value":true}"#)).unwrap();
        assert_eq!(cfg.resource_policy, "allow-remote");
        apply(&mut cfg, "loadRemoteImages", &args(r#"{"value":false}"#)).unwrap();
        assert_eq!(cfg.resource_policy, "block-remote");
        apply(&mut cfg, "replyLinePrefix", &args(r#"{"value":""}"#)).unwrap();
        assert_eq!(cfg.reply_line_prefix, "> ");
    }

    #[test]
    fn an_unreadable_config_is_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("tc-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.xml");
        std::fs::write(&path, "<tagliacarte><stores><store ").unwrap();
        let before = std::fs::read(&path).unwrap();

        let r = edit_config(path.to_str().unwrap(), |c| {
            c.threaded_view = false;
            Ok(())
        });
        assert!(r.unwrap_err().contains("nothing was changed"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn two_accounts() -> FrbConfig {
        let mut cfg = FrbConfig::default();
        for id in ["s1", "s2"] {
            cfg.accounts.push(tagliacarte_app::frb_api::FrbAccount { id: id.into(), label: id.into(), ..Default::default() });
        }
        cfg
    }

    #[test]
    fn where_the_user_was_is_remembered_per_account() {
        let mut cfg = two_accounts();
        // A folder and a message in it.
        assert!(remember_location(&mut cfg, "s1", Some("INBOX"), Some("42")).unwrap());
        assert_eq!(cfg.selected_store_id.as_deref(), Some("s1"));
        assert_eq!(cfg.accounts[0].last_folder.as_deref(), Some("INBOX"));
        assert_eq!(cfg.accounts[0].last_message_id.as_deref(), Some("42"));

        // The same again is nothing to write.
        assert!(!remember_location(&mut cfg, "s1", Some("INBOX"), Some("42")).unwrap());

        // Another folder, no message yet: the old message is not that folder's.
        assert!(remember_location(&mut cfg, "s1", Some("Archive"), None).unwrap());
        assert_eq!(cfg.accounts[0].last_folder.as_deref(), Some("Archive"));
        assert_eq!(cfg.accounts[0].last_message_id, None);

        // Moving to another account notes the account and leaves both accounts'
        // own places alone, so that coming back (or quitting there) loses nothing.
        remember_location(&mut cfg, "s1", Some("INBOX"), Some("7")).unwrap();
        assert!(remember_location(&mut cfg, "s2", None, None).unwrap());
        assert_eq!(cfg.selected_store_id.as_deref(), Some("s2"));
        assert_eq!(cfg.accounts[0].last_folder.as_deref(), Some("INBOX"));
        assert_eq!(cfg.accounts[0].last_message_id.as_deref(), Some("7"));
        assert_eq!(cfg.accounts[1].last_folder, None);
        assert!(!remember_location(&mut cfg, "s2", None, None).unwrap());

        // An account that is not there is the page's mistake, not a place to remember.
        assert!(remember_location(&mut cfg, "nope", Some("INBOX"), None).is_err());
        assert_eq!(cfg.selected_store_id.as_deref(), Some("s2"));
    }

    #[test]
    fn an_unchanged_place_does_not_rewrite_the_file() {
        let dir = std::env::temp_dir().join(format!("tc-location-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.xml");
        std::fs::write(
            &path,
            r#"<tagliacarte><security use-keychain="false"/><stores><store id="s1" type="maildir" display-name="One" path="/tmp/x"/></stores></tagliacarte>"#,
        )
        .unwrap();
        let p = path.to_str().unwrap();

        assert!(save_location(p, "s1", Some("INBOX"), Some("m1")).unwrap());
        let written = std::fs::read(&path).unwrap();
        assert!(String::from_utf8_lossy(&written).contains("INBOX"));
        let stamp = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!save_location(p, "s1", Some("INBOX"), Some("m1")).unwrap());
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), stamp, "not rewritten");

        // It comes back from disk.
        let cfg = load_for_edit(p).unwrap();
        assert_eq!(cfg.selected_store_id.as_deref(), Some("s1"));
        assert_eq!(cfg.accounts[0].last_folder.as_deref(), Some("INBOX"));
        assert_eq!(cfg.accounts[0].last_message_id.as_deref(), Some("m1"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
