/*
 * xoauth2.rs
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

//! XOAUTH2 SASL mechanism for Gmail and Outlook IMAP/SMTP.
//!
//! The XOAUTH2 mechanism is a single-shot SASL mechanism (no challenge-response rounds).
//! The initial client response is:
//!
//! ```text
//! base64("user=" {user} "\x01" "auth=Bearer " {access_token} "\x01\x01")
//! ```
//!
//! See <https://developers.google.com/gmail/imap/xoauth2-protocol>

/// Build the raw XOAUTH2 initial response (before base64 encoding).
///
/// Format: `user={user}\x01auth=Bearer {access_token}\x01\x01`
pub fn xoauth2_initial_response(user: &str, access_token: &str) -> Vec<u8> {
    format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xoauth2_initial_response() {
        let raw = xoauth2_initial_response("user@example.com", "ya29.token123");
        let expected = b"user=user@example.com\x01auth=Bearer ya29.token123\x01\x01";
        assert_eq!(raw, expected.to_vec());
    }
}
