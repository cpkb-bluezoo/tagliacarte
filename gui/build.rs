//! Embeds everything under `ui/` into the binary as a table of
//! (url path, bytes), so a release build is one file with no resources to
//! locate at run time. In development `TAGLIACARTE_UI_DIR` serves the same
//! tree from disk instead (see `src/assets.rs`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .expect("ui directory is readable")
        .map(|e| e.expect("ui entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let rel = path
                .strip_prefix(base)
                .expect("entry is under ui")
                .to_string_lossy()
                .replace('\\', "/");
            out.push((format!("/{rel}"), path));
        }
    }
}

/// FNV-1a, 64 bit: a cheap, stable content hash for ETags.
fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let ui = manifest.join("ui");
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=build.rs");

    let mut files = Vec::new();
    walk(&ui, &ui, &mut files);

    // The string tables are the ARB files, served as they are at
    // /locales/<lang>.arb. They stay where Flutter's generator and the TUI read
    // them until the Flutter UI is removed.
    let l10n = manifest.join("../flutter_ui/lib/src/l10n");
    println!("cargo:rerun-if-changed={}", l10n.display());
    if let Ok(entries) = fs::read_dir(&l10n) {
        let mut arbs: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        arbs.sort();
        for path in arbs {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(lang) = name.strip_prefix("app_").and_then(|n| n.strip_suffix(".arb")) {
                files.push((format!("/locales/{lang}.arb"), path.canonicalize().expect("arb path")));
            }
        }
    }

    // (url path, bytes, strong ETag)
    let mut src = String::from("pub static ASSETS: &[(&str, &[u8], &str)] = &[\n");
    for (url, path) in &files {
        let bytes = fs::read(path).expect("ui file is readable");
        src.push_str(&format!(
            "    ({:?}, include_bytes!({:?}), {:?}),\n",
            url,
            path.to_string_lossy(),
            format!("\"{:016x}\"", fnv1a64(&bytes))
        ));
    }
    src.push_str("];\n");

    let out = PathBuf::from(env::var("OUT_DIR").expect("set by cargo")).join("ui_assets.rs");
    fs::write(out, src).expect("OUT_DIR is writable");
}
