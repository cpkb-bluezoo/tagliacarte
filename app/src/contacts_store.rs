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
//!
//! Schema v3 aligns with common vCard fields: multiple emails per contact (`UNIQUE(contact_id, email)`),
//! child rows for `TEL`, `URL`, `ADR`-style postal lines, and `PHOTO` (embedded bytes and/or `VALUE=uri`).
//! Full-text search aggregates names, emails, phones, URLs, and address text via [contact_fts_document].

use crate::vcard_lite;
use rusqlite::{params, Connection};
use std::path::Path;

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

/// Full-text search document per contact (kept in sync via triggers).
fn sql_create_contact_fts_document_view() -> &'static str {
    r#"
            CREATE VIEW contact_fts_document AS
            SELECT c.id AS contact_id,
              trim(
                COALESCE(c.display_name, '') || ' ' ||
                COALESCE(c.nickname, '') || ' ' ||
                COALESCE(c.organization, '') || ' ' ||
                COALESCE(c.title, '') || ' ' ||
                COALESCE(c.birthday, '') || ' ' ||
                IFNULL((SELECT group_concat(email, ' ') FROM contact_emails e WHERE e.contact_id = c.id), '') || ' ' ||
                IFNULL((SELECT group_concat(number, ' ') FROM contact_phones p WHERE p.contact_id = c.id), '') || ' ' ||
                IFNULL((SELECT group_concat(url, ' ') FROM contact_urls u WHERE u.contact_id = c.id), '') || ' ' ||
                IFNULL((SELECT group_concat(
                  trim(COALESCE(a.po_box, '') || ' ' || COALESCE(a.extended, '') || ' ' || COALESCE(a.street, '') || ' ' ||
                  COALESCE(a.locality, '') || ' ' || COALESCE(a.region, '') || ' ' || COALESCE(a.postal_code, '') || ' ' || COALESCE(a.country, ''))
                , ' ') FROM contact_postal_addresses a WHERE a.contact_id = c.id), '')
              ) AS body
            FROM contacts c;
    "#
}

fn sql_drop_contact_fts_triggers() -> &'static str {
    r#"
            DROP TRIGGER IF EXISTS tr_contacts_ai_fts;
            DROP TRIGGER IF EXISTS tr_contacts_au_fts;
            DROP TRIGGER IF EXISTS tr_contacts_ad_fts;
            DROP TRIGGER IF EXISTS tr_emails_ai_fts;
            DROP TRIGGER IF EXISTS tr_emails_au_fts;
            DROP TRIGGER IF EXISTS tr_emails_ad_fts;
            DROP TRIGGER IF EXISTS tr_phones_ai_fts;
            DROP TRIGGER IF EXISTS tr_phones_au_fts;
            DROP TRIGGER IF EXISTS tr_phones_ad_fts;
            DROP TRIGGER IF EXISTS tr_urls_ai_fts;
            DROP TRIGGER IF EXISTS tr_urls_au_fts;
            DROP TRIGGER IF EXISTS tr_urls_ad_fts;
            DROP TRIGGER IF EXISTS tr_postal_ai_fts;
            DROP TRIGGER IF EXISTS tr_postal_au_fts;
            DROP TRIGGER IF EXISTS tr_postal_ad_fts;
    "#
}

fn sql_create_contact_fts_triggers() -> &'static str {
    r#"
            CREATE TRIGGER tr_contacts_ai_fts AFTER INSERT ON contacts BEGIN
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.id;
            END;
            CREATE TRIGGER tr_contacts_au_fts AFTER UPDATE ON contacts BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.id;
            END;
            CREATE TRIGGER tr_contacts_ad_fts AFTER DELETE ON contacts BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.id;
            END;
            CREATE TRIGGER tr_emails_ai_fts AFTER INSERT ON contact_emails BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_emails_au_fts AFTER UPDATE ON contact_emails BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_emails_ad_fts AFTER DELETE ON contact_emails BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = old.contact_id;
            END;
            CREATE TRIGGER tr_phones_ai_fts AFTER INSERT ON contact_phones BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_phones_au_fts AFTER UPDATE ON contact_phones BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_phones_ad_fts AFTER DELETE ON contact_phones BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = old.contact_id;
            END;
            CREATE TRIGGER tr_urls_ai_fts AFTER INSERT ON contact_urls BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_urls_au_fts AFTER UPDATE ON contact_urls BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_urls_ad_fts AFTER DELETE ON contact_urls BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = old.contact_id;
            END;
            CREATE TRIGGER tr_postal_ai_fts AFTER INSERT ON contact_postal_addresses BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_postal_au_fts AFTER UPDATE ON contact_postal_addresses BEGIN
                DELETE FROM contacts_fts WHERE contact_id = new.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = new.contact_id;
            END;
            CREATE TRIGGER tr_postal_ad_fts AFTER DELETE ON contact_postal_addresses BEGIN
                DELETE FROM contacts_fts WHERE contact_id = old.contact_id;
                INSERT INTO contacts_fts(contact_id, body)
                SELECT contact_id, body FROM contact_fts_document WHERE contact_id = old.contact_id;
            END;
    "#
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let ver: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver < 1 {
        let schema_init = String::new()
            + r#"
            CREATE TABLE IF NOT EXISTS contacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                display_name TEXT NOT NULL DEFAULT '',
                nickname TEXT NOT NULL DEFAULT '',
                birthday TEXT,
                organization TEXT NOT NULL DEFAULT '',
                title TEXT NOT NULL DEFAULT '',
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
                compose_to_count INTEGER NOT NULL DEFAULT 0,
                UNIQUE(contact_id, email)
            );

            CREATE TABLE IF NOT EXISTS contact_phones (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                number TEXT NOT NULL DEFAULT '',
                label TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS contact_urls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                url TEXT NOT NULL DEFAULT '',
                label TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS contact_photos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                mime_type TEXT NOT NULL DEFAULT '',
                data BLOB,
                source_uri TEXT,
                sort_order INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS contact_postal_addresses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                label TEXT NOT NULL DEFAULT '',
                po_box TEXT NOT NULL DEFAULT '',
                extended TEXT NOT NULL DEFAULT '',
                street TEXT NOT NULL DEFAULT '',
                locality TEXT NOT NULL DEFAULT '',
                region TEXT NOT NULL DEFAULT '',
                postal_code TEXT NOT NULL DEFAULT '',
                country TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
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
            CREATE INDEX IF NOT EXISTS idx_contact_phones_contact ON contact_phones(contact_id);
            CREATE INDEX IF NOT EXISTS idx_contact_urls_contact ON contact_urls(contact_id);
            CREATE INDEX IF NOT EXISTS idx_contact_photos_contact ON contact_photos(contact_id);
            CREATE INDEX IF NOT EXISTS idx_contact_postal_contact ON contact_postal_addresses(contact_id);
            CREATE INDEX IF NOT EXISTS idx_repo_state_dirty ON contact_repository_state(repository_id, local_dirty);

            CREATE VIRTUAL TABLE IF NOT EXISTS contacts_fts USING fts5(
                contact_id UNINDEXED,
                body,
                tokenize = 'unicode61'
            );
            "#
            + sql_create_contact_fts_document_view()
            + sql_create_contact_fts_triggers();
        conn.execute_batch(&schema_init)
        .map_err(|e| e.to_string())?;
        conn.execute("PRAGMA user_version = 3", [])
            .map_err(|e| e.to_string())?;
    }
    let ver2: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver2 < 2 {
        conn.execute(
            "ALTER TABLE contact_emails ADD COLUMN compose_to_count INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute("PRAGMA user_version = 2", [])
            .map_err(|e| e.to_string())?;
    }
    let ver3: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver3 < 3 {
        migrate_v2_to_v3(conn)?;
    }
    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, col: &str) -> Result<bool, String> {
    let sql = format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1");
    let n: i64 = conn
        .query_row(&sql, [col], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

/// Upgrade existing DB (user_version 1 or 2) to v3: vCard-shaped fields, child tables, FTS view.
fn migrate_v2_to_v3(conn: &Connection) -> Result<(), String> {
    if !table_has_column(conn, "contacts", "nickname")? {
        conn.execute(
            "ALTER TABLE contacts ADD COLUMN nickname TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    if !table_has_column(conn, "contacts", "birthday")? {
        conn.execute("ALTER TABLE contacts ADD COLUMN birthday TEXT", [])
            .map_err(|e| e.to_string())?;
    }
    if !table_has_column(conn, "contacts", "organization")? {
        conn.execute(
            "ALTER TABLE contacts ADD COLUMN organization TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    if !table_has_column(conn, "contacts", "title")? {
        conn.execute(
            "ALTER TABLE contacts ADD COLUMN title TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute_batch(
        r#"
            CREATE TABLE IF NOT EXISTS contact_phones (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                number TEXT NOT NULL DEFAULT '',
                label TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS contact_urls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                url TEXT NOT NULL DEFAULT '',
                label TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS contact_photos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                mime_type TEXT NOT NULL DEFAULT '',
                data BLOB,
                source_uri TEXT,
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS contact_postal_addresses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                label TEXT NOT NULL DEFAULT '',
                po_box TEXT NOT NULL DEFAULT '',
                extended TEXT NOT NULL DEFAULT '',
                street TEXT NOT NULL DEFAULT '',
                locality TEXT NOT NULL DEFAULT '',
                region TEXT NOT NULL DEFAULT '',
                postal_code TEXT NOT NULL DEFAULT '',
                country TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
            );
        "#,
    )
    .map_err(|e| e.to_string())?;

    conn.execute("DROP VIEW IF EXISTS contact_fts_document", [])
        .map_err(|e| e.to_string())?;
    conn.execute_batch(sql_drop_contact_fts_triggers())
        .map_err(|e| e.to_string())?;

    conn
        .execute_batch(
            r#"
            DROP TABLE IF EXISTS contact_emails_v3;
            CREATE TABLE contact_emails_v3 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                email TEXT NOT NULL,
                label TEXT NOT NULL DEFAULT '',
                compose_to_count INTEGER NOT NULL DEFAULT 0,
                UNIQUE(contact_id, email)
            );
            INSERT INTO contact_emails_v3 SELECT * FROM contact_emails;
            DROP TABLE contact_emails;
            ALTER TABLE contact_emails_v3 RENAME TO contact_emails;
        "#,
        )
        .map_err(|e| e.to_string())?;

    conn.execute_batch(
        r#"
            CREATE INDEX IF NOT EXISTS idx_contact_emails_contact ON contact_emails(contact_id);
            CREATE INDEX IF NOT EXISTS idx_contact_phones_contact ON contact_phones(contact_id);
            CREATE INDEX IF NOT EXISTS idx_contact_urls_contact ON contact_urls(contact_id);
            CREATE INDEX IF NOT EXISTS idx_contact_photos_contact ON contact_photos(contact_id);
            CREATE INDEX IF NOT EXISTS idx_contact_postal_contact ON contact_postal_addresses(contact_id);
        "#,
    )
    .map_err(|e| e.to_string())?;

    let batch = sql_create_contact_fts_document_view().to_string() + sql_create_contact_fts_triggers();
    conn.execute_batch(&batch).map_err(|e| e.to_string())?;

    conn.execute_batch(
        r#"
            DELETE FROM contacts_fts;
            INSERT INTO contacts_fts(contact_id, body) SELECT contact_id, body FROM contact_fts_document;
        "#,
    )
    .map_err(|e| e.to_string())?;

    conn.execute("PRAGMA user_version = 3", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Rebuild FTS row for one contact (used when triggers might miss edge cases).
pub fn refresh_contact_fts(conn: &Connection, contact_id: i64) -> Result<(), String> {
    let body: String = conn
        .query_row(
            "SELECT body FROM contact_fts_document WHERE contact_id = ?1",
            [contact_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
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
    let (display_name, nickname, organization, title, birthday, notes): (
        String,
        String,
        String,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            r#"SELECT display_name, nickname, organization, title, birthday, notes
               FROM contacts WHERE id = ?1"#,
            [contact_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT email, label FROM contact_emails WHERE contact_id = ?1 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let emails: Vec<(String, String)> = stmt
        .query_map([contact_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let email_pairs: Vec<(&str, &str)> = emails
        .iter()
        .map(|(a, l)| (a.as_str(), l.as_str()))
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT number, label FROM contact_phones WHERE contact_id = ?1 ORDER BY sort_order, id",
        )
        .map_err(|e| e.to_string())?;
    let tels: Vec<(String, String)> = stmt
        .query_map([contact_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let tel_pairs: Vec<(&str, &str)> = tels
        .iter()
        .map(|(a, l)| (a.as_str(), l.as_str()))
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT url, label FROM contact_urls WHERE contact_id = ?1 ORDER BY sort_order, id",
        )
        .map_err(|e| e.to_string())?;
    let urls: Vec<(String, String)> = stmt
        .query_map([contact_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let url_pairs: Vec<(&str, &str)> = urls
        .iter()
        .map(|(a, l)| (a.as_str(), l.as_str()))
        .collect();

    let mut stmt = conn
        .prepare(
            r#"SELECT label, po_box, extended, street, locality, region, postal_code, country
               FROM contact_postal_addresses WHERE contact_id = ?1 ORDER BY sort_order, id"#,
        )
        .map_err(|e| e.to_string())?;
    let adrs: Vec<vcard_lite::ParsedAdr> = stmt
        .query_map([contact_id], |r| {
            Ok(vcard_lite::ParsedAdr {
                label: r.get::<_, String>(0)?,
                po_box: r.get::<_, String>(1)?,
                extended: r.get::<_, String>(2)?,
                street: r.get::<_, String>(3)?,
                locality: r.get::<_, String>(4)?,
                region: r.get::<_, String>(5)?,
                postal_code: r.get::<_, String>(6)?,
                country: r.get::<_, String>(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT mime_type, data, source_uri FROM contact_photos WHERE contact_id = ?1 ORDER BY sort_order, id",
        )
        .map_err(|e| e.to_string())?;
    let photos: Vec<vcard_lite::ParsedPhoto> = stmt
        .query_map([contact_id], |r| {
            let mime: String = r.get(0)?;
            let data: Option<Vec<u8>> = r.get(1)?;
            let source_uri: Option<String> = r.get(2)?;
            Ok(vcard_lite::ParsedPhoto {
                mime_type: mime,
                data,
                source_uri,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let bd = birthday
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    Ok(vcard_lite::build_vcard_full(
        &display_name,
        &nickname,
        &organization,
        &title,
        bd,
        &notes,
        &email_pairs,
        &tel_pairs,
        &url_pairs,
        &adrs,
        &photos,
    ))
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
    let esc = sq_like_escape(q);
    let pattern = format!("%{esc}%");
    let prefix = format!("{esc}%");
    let mut stmt = conn
        .prepare(
            r#"SELECT DISTINCT c.id, c.display_name
               FROM contacts c
               LEFT JOIN contact_emails e ON e.contact_id = c.id
               LEFT JOIN contact_phones p ON p.contact_id = c.id
               LEFT JOIN contact_urls u ON u.contact_id = c.id
               WHERE c.display_name LIKE ?1 ESCAPE '\'
                  OR e.email LIKE ?2 ESCAPE '\'
                  OR e.email LIKE ?3 ESCAPE '\'
                  OR p.number LIKE ?2 ESCAPE '\'
                  OR p.number LIKE ?3 ESCAPE '\'
                  OR u.url LIKE ?2 ESCAPE '\'
                  OR u.url LIKE ?3 ESCAPE '\'
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

fn sq_like_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn format_recipient_one_line(display_name: &str, email: &str) -> String {
    let name = display_name.trim();
    let em = email.trim();
    if em.is_empty() {
        return String::new();
    }
    if name.is_empty() {
        return em.to_string();
    }
    format!("{} <{}>", name, em)
}

fn char_eq_case_insensitive(a: char, b: char) -> bool {
    if a == b {
        return true;
    }
    a.to_lowercase().to_string() == b.to_lowercase().to_string()
}

/// Returns the remainder of [whole] after a case-insensitive character-by-character match of [typed].
fn strip_case_insensitive_prefix<'a>(whole: &'a str, typed: &str) -> Option<&'a str> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    let mut w = whole.chars();
    for tc in typed.chars() {
        let wc = w.next()?;
        if !char_eq_case_insensitive(tc, wc) {
            return None;
        }
    }
    Some(w.as_str())
}

/// Best single completion for compose recipient fields: full RFC-like line and grey inline suffix.
/// Ranked by [compose_to_count] and whether the typed prefix matches email vs display name first.
pub fn recipient_completion_best(
    conn: &Connection,
    prefix: &str,
) -> Result<Option<(String, String)>, String> {
    let p = prefix.trim();
    if p.is_empty() {
        return Ok(None);
    }
    let esc = sq_like_escape(p);
    let pl = p.to_lowercase();
    let prefer_email = p.contains('@');

    let mut stmt = conn
        .prepare(
            r#"SELECT e.email, c.display_name, e.compose_to_count
               FROM contact_emails e
               JOIN contacts c ON c.id = e.contact_id
               WHERE lower(e.email) LIKE lower(?1) || '%' ESCAPE '\'
                  OR lower(trim(c.display_name)) LIKE lower(?2) || '%' ESCAPE '\'
               ORDER BY e.compose_to_count DESC
               LIMIT 64"#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![&esc, &esc], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut best: Option<(i64, String, String)> = None;
    for row in rows {
        let (email, display_name, compose_to_count) = row.map_err(|e| e.to_string())?;
        let full = format_recipient_one_line(&display_name, &email);
        if full.is_empty() {
            continue;
        }
        let el = email.to_lowercase();
        let dl = display_name.trim().to_lowercase();
        let email_pref = el.starts_with(pl.as_str());
        let display_pref = dl.starts_with(pl.as_str());
        if !email_pref && !display_pref {
            continue;
        }
        let mut score: i64 = compose_to_count.saturating_mul(1000);
        if prefer_email {
            if email_pref {
                score += 500;
            }
            if display_pref {
                score += 100;
            }
        } else {
            if display_pref {
                score += 500;
            }
            if email_pref {
                score += 200;
            }
        }
        let replace = match &best {
            None => true,
            Some((s, _, prev_full)) => {
                if score > *s {
                    true
                } else if score == *s && full.len() < prev_full.len() {
                    true
                } else {
                    false
                }
            }
        };
        if replace {
            best = Some((score, full, email));
        }
    }

    let Some((_, full, email)) = best else {
        return Ok(None);
    };

    let ghost = strip_case_insensitive_prefix(&full, p)
        .map(str::to_string)
        .or_else(|| strip_case_insensitive_prefix(&email, p).map(str::to_string))
        .unwrap_or_default();

    Ok(Some((full, ghost)))
}

/// Increment per-address compose frequency (normalized lower-case match to [contact_emails.email]).
pub fn bump_compose_to_counts(conn: &Connection, normalized_emails: &[String]) -> Result<(), String> {
    for e in normalized_emails {
        let n = e.trim();
        if n.is_empty() {
            continue;
        }
        conn.execute(
            "UPDATE contact_emails SET compose_to_count = compose_to_count + 1 WHERE lower(email) = lower(?1)",
            [n],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
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

    #[test]
    fn recipient_completion_prefers_compose_count() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO contacts (display_name, import_origin, externally_share_ok, created_at, updated_at) VALUES ('Alice One', 'manual', 1, 0, 0)",
            [],
        )
        .unwrap();
        let id1: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO contact_emails (contact_id, email, compose_to_count) VALUES (?1, 'alice@a.test', 5)",
            [id1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO contacts (display_name, import_origin, externally_share_ok, created_at, updated_at) VALUES ('Alice Two', 'manual', 1, 0, 0)",
            [],
        )
        .unwrap();
        let id2: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO contact_emails (contact_id, email, compose_to_count) VALUES (?1, 'alice@b.test', 1)",
            [id2],
        )
        .unwrap();
        refresh_contact_fts(&conn, id1).unwrap();
        refresh_contact_fts(&conn, id2).unwrap();
        let c = recipient_completion_best(&conn, "ali").unwrap().unwrap();
        assert_eq!(c.0, "Alice One <alice@a.test>");
        assert!(c.1.contains('<') || !c.1.is_empty());
    }
}
