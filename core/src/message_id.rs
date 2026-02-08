/*
 * message_id.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
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

//! Stable message identifier (opaque + URI form). Keyed by Store/Folder; not folder index or Message-ID header.

use std::fmt;

/// Opaque stable message id. Unique within a folder (or store). URI form for parsing/cross-reference.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MessageId(String);

impl MessageId {
    pub fn new(uri_or_opaque: impl Into<String>) -> Self {
        Self(uri_or_opaque.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Build MessageId for IMAP (uid + mailbox + host).
pub fn imap_message_id(user_at_host: &str, mailbox_name: &str, uid: u32) -> MessageId {
    MessageId::new(format!("imap://{}/{}/{}", user_at_host, mailbox_name, uid))
}

/// Build MessageId for POP3 (uidl).
pub fn pop3_message_id(user_at_host: &str, uidl: &str) -> MessageId {
    MessageId::new(format!("pop3://{}/{}", user_at_host, uidl))
}

/// Build MessageId for Maildir (path + folder + filename).
pub fn maildir_message_id(path: &str, folder: &str, filename: &str) -> MessageId {
    MessageId::new(format!("maildir://{}/{}/{}", path, folder, filename))
}

/// Build MessageId for mbox (path + offset or id).
pub fn mbox_message_id(path: &str, id: &str) -> MessageId {
    MessageId::new(format!("mbox://{}/#{}", path, id))
}

// --- Nostr ---

/// Build MessageId for a Nostr event (NIP-19 nevent or raw event id).
pub fn nostr_nevent_message_id(nevent_or_event_id: &str) -> MessageId {
    MessageId::new(format!("nostr:nevent:{}", nevent_or_event_id))
}

/// Build MessageId for a Nostr DM (direct message event).
pub fn nostr_dm_message_id(event_id_or_dm_id: &str) -> MessageId {
    MessageId::new(format!("nostr:dm:{}", event_id_or_dm_id))
}

// --- Matrix ---

/// Build MessageId for a Matrix message (room + event id).
pub fn matrix_message_id(room_id: &str, event_id: &str) -> MessageId {
    MessageId::new(format!("matrix://{}/{}", room_id, event_id))
}

/// Folder identifier for a Matrix room (use as folder id in store).
pub fn matrix_room_folder_id(room_id: &str) -> String {
    format!("matrix:room:{}", room_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_id_uri_roundtrip_imap() {
        let id = imap_message_id("user@host", "INBOX", 42);
        let s = id.as_str();
        assert!(s.starts_with("imap://"));
        assert!(s.contains("/INBOX/42"));
        let id2 = MessageId::new(s);
        assert_eq!(id.as_str(), id2.as_str());
    }

    #[test]
    fn message_id_uri_roundtrip_maildir() {
        let id = maildir_message_id("/var/mail", "INBOX", "1234567890.M12345P67890.host");
        let s = id.as_str();
        assert!(s.starts_with("maildir://"));
        let id2 = MessageId::new(s);
        assert_eq!(id.as_str(), id2.as_str());
    }

    #[test]
    fn message_id_nostr_matrix() {
        let nevent = nostr_nevent_message_id("nevent1abc...");
        assert!(nevent.as_str().starts_with("nostr:nevent:"));
        let dm = nostr_dm_message_id("event_id_xyz");
        assert!(dm.as_str().starts_with("nostr:dm:"));
        let mx = matrix_message_id("!room:server", "$event:server");
        assert_eq!(mx.as_str(), "matrix://!room:server/$event:server");
        assert_eq!(matrix_room_folder_id("!room:server"), "matrix:room:!room:server");
    }
}
