//! The UI, served from the binary with an ETag per file (a hash of its
//! content, computed at build time). `TAGLIACARTE_UI_DIR` serves the same
//! tree from disk instead, so editing HTML, CSS or JS needs no rebuild; its
//! ETags are weak, from the file's size and modification time.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "arb" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

pub struct Asset {
    pub body: Cow<'static, [u8]>,
    pub content_type: &'static str,
    pub etag: String,
}

/// Whether an `If-None-Match` header value matches `etag` (weak comparison,
/// as RFC 9110 prescribes for this header; `*` matches anything).
pub fn etag_matches(if_none_match: &str, etag: &str) -> bool {
    let bare = |t: &str| t.trim().trim_start_matches("W/").to_owned();
    let ours = bare(etag);
    if_none_match
        .split(',')
        .any(|candidate| candidate.trim() == "*" || bare(candidate) == ours)
}

/// The asset at a URL path. `/` is the index page. With `TAGLIACARTE_UI_DIR`
/// set, files are read from that directory.
pub fn lookup(url_path: &str) -> Option<Asset> {
    let dir = std::env::var_os("TAGLIACARTE_UI_DIR").map(PathBuf::from);
    lookup_in(dir.as_deref(), url_path)
}

/// Disk first, when there is a directory, then what is embedded. Not every
/// embedded file is under `ui/` (the string tables are the ARB files), so a
/// file missing on disk is not necessarily missing.
fn lookup_in(ui_dir: Option<&Path>, url_path: &str) -> Option<Asset> {
    let path = if url_path == "/" { "/index.html" } else { url_path };
    if path.split('/').any(|seg| seg == "..") {
        return None;
    }
    if let Some(dir) = ui_dir {
        if let Some(asset) = from_disk(dir, path) {
            return Some(asset);
        }
    }
    ASSETS.iter().find(|(p, _, _)| *p == path).map(|(_, bytes, etag)| Asset {
        body: Cow::Borrowed(*bytes),
        content_type: content_type(path),
        etag: (*etag).to_owned(),
    })
}

fn from_disk(dir: &Path, path: &str) -> Option<Asset> {
    let file = dir.join(path.trim_start_matches('/'));
    let modified = std::fs::metadata(&file).and_then(|m| m.modified()).ok()?;
    let age = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    let bytes = std::fs::read(&file).ok()?;
    let etag = format!("W/\"{}-{}-{}\"", bytes.len(), age.as_secs(), age.subsec_nanos());
    Some(Asset { body: Cow::Owned(bytes), content_type: content_type(path), etag })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_is_embedded_and_traversal_is_refused() {
        let a = lookup("/").expect("index.html is embedded");
        assert!(a.content_type.starts_with("text/html"));
        assert!(!a.body.is_empty());
        assert!(a.etag.starts_with('"') && a.etag.ends_with('"'), "strong ETag: {}", a.etag);
        assert!(lookup("/../Cargo.toml").is_none());
        assert!(lookup("/modules/../../Cargo.toml").is_none());
        assert!(lookup("/no-such-file.js").is_none());
    }

    #[test]
    fn serving_from_disk_still_finds_the_embedded_string_tables() {
        let dir = std::env::temp_dir().join(format!("tc-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<p>from disk</p>").unwrap();

        // A file on disk wins, and is not the embedded copy.
        let page = lookup_in(Some(&dir), "/").unwrap();
        assert_eq!(&*page.body, b"<p>from disk</p>");
        assert!(page.etag.starts_with("W/"), "disk files get weak ETags: {}", page.etag);

        // The ARB files are not under ui/, so they are never on disk. That
        // must not make them 404: the page would then show message keys.
        let arb = lookup_in(Some(&dir), "/locales/en.arb").expect("string table is embedded");
        assert!(arb.content_type.starts_with("application/json"));
        assert!(std::str::from_utf8(&arb.body).unwrap().contains("\"settings\""));

        // Nothing anywhere is still a miss, and traversal is refused.
        assert!(lookup_in(Some(&dir), "/nothing-here.js").is_none());
        assert!(lookup_in(Some(&dir), "/../Cargo.toml").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn etag_comparison_follows_if_none_match_rules() {
        assert!(etag_matches("\"abc\"", "\"abc\""));
        assert!(etag_matches("W/\"abc\"", "\"abc\""));
        assert!(etag_matches("\"x\", \"abc\"", "\"abc\""));
        assert!(etag_matches("*", "\"abc\""));
        assert!(!etag_matches("\"abd\"", "\"abc\""));
        assert!(!etag_matches("", "\"abc\""));
    }
}
