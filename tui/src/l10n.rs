//! Localization from Flutter ARB (generated in `OUT_DIR`).

include!(concat!(env!("OUT_DIR"), "/l10n_generated.rs"));

/// Resolve UI locale from `LANG` / `LC_MESSAGES` (falls back to English).
pub fn detect_locale() -> Locale {
    let tag = std::env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LC_MESSAGES").ok().filter(|s| !s.is_empty()))
        .or_else(|| std::env::var("LANG").ok().filter(|s| !s.is_empty()))
        .unwrap_or_default();
    Locale::from_lang_tag(&tag)
}

pub fn trs(locale: Locale, key: &str) -> String {
    let s = tr(locale, key);
    if s.is_empty() {
        key.to_string()
    } else {
        s.to_string()
    }
}
