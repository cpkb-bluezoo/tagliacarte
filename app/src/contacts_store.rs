/*
 * contacts_store.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
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

//! SQLite contacts database: local canonical store with repositories, groups, sync state.

use crate::vcard_lite;
use rusqlite::{params, Connection};
use std::path::Path;

const SCHEMA_VERSION: i32 = 1;

/// Open or create the contacts database at `{data_dir}/contacts.db`.
pub fn open_contacts_db(data_dir: impl AsRef<Path>) -> Result<Connection, String> {
    let p = data_dir.as_ref().join("contacts.db");
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(&p).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let ver: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver < SCHEMA_VERSION {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS contacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                display_name TEXT NOT NULL DEFAULT '',
                notes TEXT NOT NULL DEFAULT '',
                import_origin TEXT NOT NULL DEFAULT 'manual',
                externally_share_ok INTEGER NOT NULL DEFAULT 1,
                pgp_fingerprint TEXT,
                pgp_key_path TEXT,
                smime_cert_path TEXT,
                smime_notes TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS contact_emails (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                email TEXT NOT NULL,
                label TEXT NOT NULL DEFAULT '',
                UNIQUE(email)
            );

            CREATE TABLE IF NOT EXISTS contact_repositories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                base_url TEXT NOT NULL DEFAULT '',
                collection_path TEXT NOT NULL DEFAULT '',
                credential_key TEXT NOT NULL DEFAULT '',
                default_new_contact INTEGER NOT NULL DEFAULT 0,
                ctag TEXT NOT NULL DEFAULT '',
                last_collection_sync_at INTEGER,
                sync_error TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS contact_repository_state (
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                repository_id INTEGER NOT NULL REFERENCES contact_repositories(id) ON DELETE CASCADE,
                remote_href TEXT NOT NULL DEFAULT '',
                etag TEXT NOT NULL DEFAULT '',
                last_synced_at INTEGER,
                local_dirty INTEGER NOT NULL DEFAULT 0,
                pending_delete INTEGER NOT NULL DEFAULT 0,
                last_error TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (contact_id, repository_id)
            );

            CREATE TABLE IF NOT EXISTS contact_groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                color_argb INTEGER
            );

            CREATE TABLE IF NOT EXISTS contact_group_members (
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                group_id INTEGER NOT NULL REFERENCES contact_groups(id) ON DELETE CASCADE,
                PRIMARY KEY (contact_id, group_id)
            );

            CREATE TABLE IF NOT EXISTS group_repository_targets (
                group_id INTEGER NOT NULL REFERENCES contact_groups(id) ON DELETE CASCADE,
                repository_id INTEGER NOT NULL REFERENCES contact_repositories(id) ON DELETE CASCADE,
                PRIMARY KEY (group_id, repository_id)
            );

            CREATE INDEX IF NOT EXISTS idx_contact_emails_contact ON contact_emails(contact_id);
            CREATE INDEX IF NOT EXISTS idx_repo_state_dirty ON contact_repository_state(repository_id, local_dirty);

            CREATE VIRTUAL TABLE IF NOT EXISTS contacts_fts USING fts5(
                contact_id UNINDEXED,
                body,
                tokenize = 'unicode61'
            );

            CREATE TRIGGER IF NOT EXISTS tr_contacts_ai_fts AFTER INSERT ON contacts BEGIN
                INSERT INTO contacts_fts(contact_id, body) VALUES (new.id, new.display_name);
            END;
            CREATE TRIGGER IF NOT EXISTS tr_contacts_au_fts AFTER UPDATE ON contacts BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.id;
                INSERT INTO contacts_fts(contact_id, body) VALUES (new.id, new.display_name);
            END;
            CREATE TRIGGER IF NOT EXISTS tr_contacts_ad_fts AFTER DELETE ON contacts BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS tr_emails_ai_fts AFTER INSERT ON contact_emails BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT new.contact_id,
                    c.display_name || ' ' || IFNULL((SELECT group_concat(email, ' ') FROM contact_emails WHERE contact_id = new.contact_id), '')
                FROM contacts c WHERE c.id = new.contact_id;
            END;
            CREATE TRIGGER IF NOT EXISTS tr_emails_au_fts AFTER UPDATE ON contact_emails BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT new.contact_id,
                    c.display_name || ' ' || IFNULL((SELECT group_concat(email, ' ') FROM contact_emails WHERE contact_id = new.contact_id), '')
                FROM contacts c WHERE c.id = new.contact_id;
            END;
            CREATE TRIGGER IF NOT EXISTS tr_emails_ad_fts AFTER DELETE ON contact_emails BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT old.contact_id,
                    c.display_name || ' ' || IFNULL((SELECT group_concat(email, ' ') FROM contact_emails WHERE contact_id = old.contact_id), '')
                FROM contacts c WHERE c.id = old.contact_id;
            END;
            "#,
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            &format!("PRAGMA user_version = {SCHEMA_VERSION}"),
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Rebuild FTS row for one contact (used when triggers might miss edge cases).
pub fn refresh_contact_fts(conn: &Connection, contact_id: i64) -> Result<(), String> {
    let display: String = conn
        .query_row(
            "SELECT display_name FROM contacts WHERE id = ?1",
            [contact_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let emails: String = conn
        .query_row(
            "SELECT IFNULL(group_concat(email, ' '), '') FROM contact_emails WHERE contact_id = ?1",
            [contact_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let body = format!("{display} {emails}");
    conn.execute(
        "DELETE FROM contacts_fts WHERE contact_id = ?1",
        [contact_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO contacts_fts(contact_id, body) VALUES (?1, ?2)",
        params![contact_id, body],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Serialize one contact as a vCard (for CardDAV PUT).
pub fn contact_vcard_body(conn: &Connection, contact_id: i64) -> Result<String, String> {
    let (name, notes): (String, String) = conn
        .query_row(
            "SELECT display_name, notes FROM contacts WHERE id = ?1",
            [contact_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT email, label FROM contact_emails WHERE contact_id = ?1 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([contact_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let emails: Vec<(String, String)> = rows
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let pairs: Vec<(&str, &str)> = emails
        .iter()
        .map(|(a, l)| (a.as_str(), l.as_str()))
        .collect();
    Ok(vcard_lite::build_vcard(&name, pairs.as_slice(), &notes))
}

pub fn contact_may_sync_externally(conn: &Connection, contact_id: i64) -> Result<bool, String> {
    let ok: i32 = conn
        .query_row(
            "SELECT externally_share_ok FROM contacts WHERE id = ?1",
            [contact_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(ok != 0)
}

pub fn search_contacts(conn: &Connection, query: &str, limit: i64) -> Result<Vec<(i64, String, Vec<String>)>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let esc = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let pattern = format!("%{esc}%");
    let prefix = format!("{esc}%");
    let mut stmt = conn
        .prepare(
            r#"SELECT DISTINCT c.id, c.display_name
               FROM contacts c
               LEFT JOIN contact_emails e ON e.contact_id = c.id
               WHERE c.display_name LIKE ?1 ESCAPE '\'
                  OR e.email LIKE ?2 ESCAPE '\'
                  OR e.email LIKE ?3 ESCAPE '\'
               LIMIT ?4"#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern, pattern, prefix, limit], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name) = row.map_err(|e| e.to_string())?;
        let emails = emails_for_contact(conn, id)?;
        out.push((id, name, emails));
    }
    Ok(out)
}

fn emails_for_contact(conn: &Connection, contact_id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT email FROM contact_emails WHERE contact_id = ?1 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([contact_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows.map(|x| x.map_err(|e| e.to_string())).collect()
}

pub fn upsert_repository_membership(
    conn: &Connection,
    contact_id: i64,
    repository_id: i64,
    include: bool,
) -> Result<(), String> {
    if include {
        if !contact_may_sync_externally(conn, contact_id)? {
            return Err(
                "contact must be validated for external sharing before linking to a repository"
                    .to_string(),
            );
        }
        conn.execute(
            r#"INSERT INTO contact_repository_state (contact_id, repository_id, local_dirty, last_error)
               VALUES (?1, ?2, 1, '')
               ON CONFLICT(contact_id, repository_id) DO UPDATE SET local_dirty = 1"#,
            params![contact_id, repository_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "DELETE FROM contact_repository_state WHERE contact_id = ?1 AND repository_id = ?2",
            params![contact_id, repository_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn materialize_group_repository_targets(conn: &Connection) -> Result<u32, String> {
    let mut n = 0u32;
    let mut stmt = conn
        .prepare(
            "SELECT g.group_id, g.repository_id FROM group_repository_targets g",
        )
        .map_err(|e| e.to_string())?;
    let pairs: Vec<(i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    for (group_id, repository_id) in pairs {
        let mut m = conn
            .prepare("SELECT contact_id FROM contact_group_members WHERE group_id = ?1")
            .map_err(|e| e.to_string())?;
        let cids: Vec<i64> = m
            .query_map([group_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        for contact_id in cids {
            if !contact_may_sync_externally(conn, contact_id)? {
                continue;
            }
            conn.execute(
                r#"INSERT INTO contact_repository_state (contact_id, repository_id, local_dirty, last_error)
                   VALUES (?1, ?2, 1, '')
                   ON CONFLICT(contact_id, repository_id) DO UPDATE SET local_dirty = 1"#,
                params![contact_id, repository_id],
            )
            .map_err(|e| e.to_string())?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO contacts (display_name, import_origin, externally_share_ok, created_at, updated_at) VALUES ('A', 'manual', 1, 0, 0)",
            [],
        )
        .unwrap();
        let id: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO contact_emails (contact_id, email) VALUES (?1, 'a@b.co')",
            [id],
        )
        .unwrap();
        refresh_contact_fts(&conn, id).unwrap();
        let r = search_contacts(&conn, "a@b", 10).unwrap();
        assert!(!r.is_empty());
    }
}
