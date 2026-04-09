/*
 * nntp_newsrc.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Per-account `.newsrc`-style subscription file under the app data directory.
 */

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tagliacarte_core::config::tagliacarte_data_dir;

fn sanitize_account_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `{data_dir}/nntp/{account_id}.newsrc`
pub fn newsrc_path_for_account(account_id: &str) -> Option<PathBuf> {
    let base = tagliacarte_data_dir()?;
    let dir = base.join("nntp");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join(format!("{}.newsrc", sanitize_account_id(account_id))))
}

/// Parse `.newsrc` lines: `groupname: …` subscribed, `groupname! …` unsubscribed (article ranges ignored).
fn parse_newsrc(content: &str) -> HashMap<String, bool> {
    let mut m = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(colon) = line.find(':') {
            let left = line[..colon].trim();
            if !left.ends_with('!') && !left.is_empty() {
                m.insert(left.to_string(), true);
                continue;
            }
        }
        if let Some(bang) = line.find('!') {
            let left = line[..bang].trim();
            if !left.is_empty() {
                m.insert(left.to_string(), false);
            }
        }
    }
    m
}

pub fn read_subscription_map(account_id: &str) -> io::Result<HashMap<String, bool>> {
    let Some(path) = newsrc_path_for_account(account_id) else {
        return Ok(HashMap::new());
    };
    if !path.is_file() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(&path)?;
    Ok(parse_newsrc(&content))
}

pub fn subscribed_group_names(account_id: &str) -> io::Result<Vec<String>> {
    let m = read_subscription_map(account_id)?;
    let mut v: Vec<String> = m
        .into_iter()
        .filter(|(_, sub)| *sub)
        .map(|(n, _)| n)
        .collect();
    v.sort();
    Ok(v)
}

fn write_map(path: &Path, map: &HashMap<String, bool>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut f = fs::File::create(path)?;
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    for k in keys {
        let sub = *map.get(k).unwrap_or(&false);
        if sub {
            writeln!(f, "{}:", k)?;
        } else {
            writeln!(f, "{}!", k)?;
        }
    }
    Ok(())
}

pub fn set_group_subscribed(account_id: &str, group: &str, subscribed: bool) -> io::Result<()> {
    let Some(path) = newsrc_path_for_account(account_id) else {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no application data directory",
        ));
    };
    let mut map = read_subscription_map(account_id)?;
    let g = group.trim();
    if g.is_empty() {
        return Ok(());
    }
    if subscribed {
        map.insert(g.to_string(), true);
    } else {
        map.insert(g.to_string(), false);
    }
    write_map(&path, &map)
}

pub fn merge_wildmat_results(
    account_id: &str,
    server_groups: &[String],
) -> io::Result<Vec<(String, bool)>> {
    let map = read_subscription_map(account_id)?;
    let mut out: Vec<(String, bool)> = server_groups
        .iter()
        .map(|g| {
            let sub = map.get(g).copied().unwrap_or(false);
            (g.clone(), sub)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
