/*
 * types.rs
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

//! Matrix Client-Server API types and constants.

// ── API paths (v3) ───────────────────────────────────────────────────

pub const API_PREFIX: &str = "/_matrix/client/v3";
pub const MEDIA_PREFIX: &str = "/_matrix/media/v3";
pub const WELL_KNOWN_PATH: &str = "/.well-known/matrix/client";

pub const PATH_LOGIN: &str = "/_matrix/client/v3/login";
pub const PATH_SYNC: &str = "/_matrix/client/v3/sync";
pub const PATH_JOINED_ROOMS: &str = "/_matrix/client/v3/joined_rooms";

/// `/_matrix/client/v3/profile/{userId}`
pub fn path_profile(user_id: &str) -> String {
    format!("{}/profile/{}", API_PREFIX, url_encode(user_id))
}

/// `/_matrix/client/v3/profile/{userId}/displayname`
pub fn path_display_name(user_id: &str) -> String {
    format!("{}/profile/{}/displayname", API_PREFIX, url_encode(user_id))
}

/// `/_matrix/client/v3/profile/{userId}/avatar_url`
pub fn path_avatar_url(user_id: &str) -> String {
    format!("{}/profile/{}/avatar_url", API_PREFIX, url_encode(user_id))
}

/// `/_matrix/client/v3/rooms/{roomId}/send/m.room.message/{txnId}`
pub fn path_send_message(room_id: &str, txn_id: &str) -> String {
    format!(
        "{}/rooms/{}/send/m.room.message/{}",
        API_PREFIX,
        url_encode(room_id),
        url_encode(txn_id),
    )
}

/// `/_matrix/client/v3/rooms/{roomId}/messages?dir=b&limit={limit}`
pub fn path_room_messages(room_id: &str, limit: u64, from: Option<&str>) -> String {
    let mut path = format!(
        "{}/rooms/{}/messages?dir=b&limit={}",
        API_PREFIX,
        url_encode(room_id),
        limit,
    );
    if let Some(token) = from {
        path.push_str("&from=");
        path.push_str(&url_encode(token));
    }
    path
}

/// `/_matrix/client/v3/rooms/{roomId}/event/{eventId}`
pub fn path_room_event(room_id: &str, event_id: &str) -> String {
    format!(
        "{}/rooms/{}/event/{}",
        API_PREFIX,
        url_encode(room_id),
        url_encode(event_id),
    )
}

/// `/_matrix/client/v3/join/{roomIdOrAlias}`
pub fn path_join(room_id_or_alias: &str) -> String {
    format!("{}/join/{}", API_PREFIX, url_encode(room_id_or_alias))
}

/// `/_matrix/client/v3/rooms/{roomId}/leave`
pub fn path_leave(room_id: &str) -> String {
    format!("{}/rooms/{}/leave", API_PREFIX, url_encode(room_id))
}

/// `/_matrix/media/v3/upload`
pub fn path_media_upload() -> String {
    format!("{}/upload", MEDIA_PREFIX)
}

/// `/_matrix/media/v3/thumbnail/{serverName}/{mediaId}?width={w}&height={h}&method=crop`
pub fn path_thumbnail(server_name: &str, media_id: &str, width: u32, height: u32) -> String {
    format!(
        "{}/thumbnail/{}/{}?width={}&height={}&method=crop",
        MEDIA_PREFIX, server_name, media_id, width, height,
    )
}

/// `/_matrix/media/v3/download/{serverName}/{mediaId}`
pub fn path_media_download(server_name: &str, media_id: &str) -> String {
    format!("{}/download/{}/{}", MEDIA_PREFIX, server_name, media_id)
}

// ── mxc:// URI handling ──────────────────────────────────────────────

/// Parse an `mxc://server/mediaId` URI into `(server_name, media_id)`.
pub fn parse_mxc_uri(mxc: &str) -> Option<(&str, &str)> {
    let rest = mxc.strip_prefix("mxc://")?;
    let slash = rest.find('/')?;
    let server = &rest[..slash];
    let media_id = &rest[slash + 1..];
    if server.is_empty() || media_id.is_empty() {
        return None;
    }
    Some((server, media_id))
}

/// Convert an `mxc://` URI to an HTTP thumbnail URL on the given homeserver.
pub fn mxc_to_thumbnail_url(homeserver: &str, mxc: &str, width: u32, height: u32) -> Option<String> {
    let (server, media_id) = parse_mxc_uri(mxc)?;
    Some(format!(
        "{}{}",
        homeserver.trim_end_matches('/'),
        path_thumbnail(server, media_id, width, height),
    ))
}

/// Convert an `mxc://` URI to an HTTP download URL on the given homeserver.
pub fn mxc_to_download_url(homeserver: &str, mxc: &str) -> Option<String> {
    let (server, media_id) = parse_mxc_uri(mxc)?;
    Some(format!(
        "{}{}",
        homeserver.trim_end_matches('/'),
        path_media_download(server, media_id),
    ))
}

// ── E2EE paths ──────────────────────────────────────────────────────

pub const PATH_KEYS_UPLOAD: &str = "/_matrix/client/v3/keys/upload";
pub const PATH_KEYS_QUERY: &str = "/_matrix/client/v3/keys/query";
pub const PATH_KEYS_CLAIM: &str = "/_matrix/client/v3/keys/claim";
pub const PATH_DEVICE_SIGNING_UPLOAD: &str = "/_matrix/client/v3/keys/device_signing/upload";
pub const PATH_SIGNATURES_UPLOAD: &str = "/_matrix/client/v3/keys/signatures/upload";
pub const PATH_ROOM_KEYS_VERSION: &str = "/_matrix/client/v3/room_keys/version";

/// `/_matrix/client/v3/sendToDevice/{eventType}/{txnId}`
pub fn path_send_to_device(event_type: &str, txn_id: &str) -> String {
    format!("{}/sendToDevice/{}/{}", API_PREFIX, url_encode(event_type), url_encode(txn_id))
}

/// `/_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}`
pub fn path_send_event(room_id: &str, event_type: &str, txn_id: &str) -> String {
    format!(
        "{}/rooms/{}/send/{}/{}",
        API_PREFIX, url_encode(room_id), url_encode(event_type), url_encode(txn_id),
    )
}

/// `/_matrix/client/v3/user/{userId}/account_data/{type}`
pub fn path_account_data(user_id: &str, event_type: &str) -> String {
    format!("{}/user/{}/account_data/{}", API_PREFIX, url_encode(user_id), url_encode(event_type))
}

/// `/_matrix/client/v3/room_keys/keys?version={version}`
pub fn path_room_keys(version: &str) -> String {
    format!("{}/room_keys/keys?version={}", API_PREFIX, url_encode(version))
}

/// `/_matrix/client/v3/rooms/{roomId}/state/m.room.encryption`
pub fn path_room_encryption_state(room_id: &str) -> String {
    format!("{}/rooms/{}/state/m.room.encryption", API_PREFIX, url_encode(room_id))
}

// ── Event / message types ────────────────────────────────────────────

pub const EVENT_ROOM_MESSAGE: &str = "m.room.message";
pub const EVENT_ROOM_NAME: &str = "m.room.name";
pub const EVENT_ROOM_AVATAR: &str = "m.room.avatar";
pub const EVENT_ROOM_TOPIC: &str = "m.room.topic";
pub const EVENT_ROOM_MEMBER: &str = "m.room.member";
pub const EVENT_ROOM_CREATE: &str = "m.room.create";

pub const EVENT_ROOM_ENCRYPTED: &str = "m.room.encrypted";
pub const EVENT_ROOM_ENCRYPTION: &str = "m.room.encryption";
pub const EVENT_ROOM_KEY: &str = "m.room_key";
pub const EVENT_TO_DEVICE_ENCRYPTED: &str = "m.room.encrypted";
pub const EVENT_VERIFICATION_REQUEST: &str = "m.key.verification.request";
pub const EVENT_VERIFICATION_READY: &str = "m.key.verification.ready";
pub const EVENT_VERIFICATION_START: &str = "m.key.verification.start";
pub const EVENT_VERIFICATION_ACCEPT: &str = "m.key.verification.accept";
pub const EVENT_VERIFICATION_KEY: &str = "m.key.verification.key";
pub const EVENT_VERIFICATION_MAC: &str = "m.key.verification.mac";
pub const EVENT_VERIFICATION_DONE: &str = "m.key.verification.done";
pub const EVENT_VERIFICATION_CANCEL: &str = "m.key.verification.cancel";

pub const ALGORITHM_OLM: &str = "m.olm.v1.curve25519-aes-sha2";
pub const ALGORITHM_MEGOLM: &str = "m.megolm.v1.aes-sha2";

pub const MSG_TYPE_TEXT: &str = "m.text";
pub const MSG_TYPE_IMAGE: &str = "m.image";
pub const MSG_TYPE_FILE: &str = "m.file";
pub const MSG_TYPE_AUDIO: &str = "m.audio";
pub const MSG_TYPE_VIDEO: &str = "m.video";
pub const MSG_TYPE_NOTICE: &str = "m.notice";
pub const MSG_TYPE_EMOTE: &str = "m.emote";

// ── Error codes ──────────────────────────────────────────────────────

pub const ERR_UNKNOWN_TOKEN: &str = "M_UNKNOWN_TOKEN";
pub const ERR_MISSING_TOKEN: &str = "M_MISSING_TOKEN";
pub const ERR_FORBIDDEN: &str = "M_FORBIDDEN";
pub const ERR_LIMIT_EXCEEDED: &str = "M_LIMIT_EXCEEDED";

// ── Data types ───────────────────────────────────────────────────────

/// Result of a successful login.
#[derive(Debug, Clone)]
pub struct LoginResponse {
    pub access_token: String,
    pub user_id: String,
    pub device_id: String,
}

/// User profile from the profile API.
#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub displayname: Option<String>,
    pub avatar_url: Option<String>,
}

/// Summary of a joined room, extracted from sync state.
#[derive(Debug, Clone)]
pub struct RoomSummary {
    pub room_id: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub topic: Option<String>,
}

/// A timeline event from a room.
#[derive(Debug, Clone)]
pub struct RoomEvent {
    pub event_id: String,
    pub event_type: String,
    pub sender: String,
    pub origin_server_ts: i64,
    pub body: Option<String>,
    pub msgtype: Option<String>,
    pub url: Option<String>,
    pub room_id: String,
    pub algorithm: Option<String>,
    pub sender_key: Option<String>,
    pub session_id: Option<String>,
    pub ciphertext: Option<String>,
    pub device_id: Option<String>,
}

/// A parsed Matrix error response: `{"errcode": "...", "error": "..."}`.
#[derive(Debug, Clone)]
pub struct MatrixApiError {
    pub status: u16,
    pub errcode: String,
    pub error: String,
}

impl std::fmt::Display for MatrixApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.errcode.is_empty() && self.error.is_empty() {
            write!(f, "HTTP {}", self.status)
        } else if self.errcode.is_empty() {
            write!(f, "HTTP {}: {}", self.status, self.error)
        } else {
            write!(f, "HTTP {} {}: {}", self.status, self.errcode, self.error)
        }
    }
}

/// Result of a well-known lookup.
#[derive(Debug, Clone)]
pub struct WellKnown {
    pub homeserver_base_url: String,
}

// ── E2EE data types ──────────────────────────────────────────────────

/// Parsed one-time key counts from `/keys/upload` or sync.
#[derive(Debug, Clone, Default)]
pub struct KeyUploadCounts {
    pub signed_curve25519: usize,
}

/// Parsed result of `/keys/query`.
#[derive(Debug, Clone, Default)]
pub struct KeyQueryResult {
    /// user_id -> { device_id -> DeviceKeysResponse }
    pub device_keys: std::collections::HashMap<String, std::collections::HashMap<String, DeviceKeysResponse>>,
}

/// One device's keys from `/keys/query`.
#[derive(Debug, Clone)]
pub struct DeviceKeysResponse {
    pub user_id: String,
    pub device_id: String,
    pub algorithms: Vec<String>,
    pub ed25519_key: Option<String>,
    pub curve25519_key: Option<String>,
    /// user_id -> { key_id -> signature_b64 }
    pub signatures: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

/// Parsed result of `/keys/claim`.
#[derive(Debug, Clone, Default)]
pub struct KeyClaimResult {
    /// user_id -> { device_id -> (key_id, key_b64) }
    pub one_time_keys: std::collections::HashMap<String, std::collections::HashMap<String, (String, String)>>,
}

/// Fields from an `m.room.encrypted` event.
#[derive(Debug, Clone)]
pub struct EncryptedEventContent {
    pub algorithm: String,
    pub sender_key: String,
    pub ciphertext: String,
    pub session_id: Option<String>,
    pub device_id: Option<String>,
}

/// A to-device event received via sync.
#[derive(Debug, Clone)]
pub struct ToDeviceEvent {
    pub event_type: String,
    pub sender: String,
    pub content: ToDeviceContent,
}

/// Content variants for to-device events.
#[derive(Debug, Clone)]
pub enum ToDeviceContent {
    /// m.room.encrypted (Olm-encrypted to-device message)
    Encrypted {
        algorithm: String,
        sender_key: String,
        ciphertext_for_us: Option<OlmCiphertext>,
    },
    /// m.room_key (plaintext inside Olm, after decryption)
    RoomKey {
        algorithm: String,
        room_id: String,
        session_id: String,
        session_key: String,
    },
    /// Verification events
    Verification {
        transaction_id: String,
        /// Raw JSON content bytes for the verification handler
        raw_content: Vec<u8>,
    },
    /// Unknown / unhandled
    Other,
}

/// Olm ciphertext for our device from a to-device encrypted event.
#[derive(Debug, Clone)]
pub struct OlmCiphertext {
    pub msg_type: u64,
    pub body: String,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Minimal percent-encoding for Matrix identifiers in URL path segments.
/// Encodes characters that are not unreserved per RFC 3986.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX_UPPER[(b >> 4) as usize] as char);
                out.push(HEX_UPPER[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

const HEX_UPPER: [u8; 16] = *b"0123456789ABCDEF";

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mxc_uri() {
        let (server, id) = parse_mxc_uri("mxc://matrix.org/AbCdEfG").unwrap();
        assert_eq!(server, "matrix.org");
        assert_eq!(id, "AbCdEfG");
    }

    #[test]
    fn test_parse_mxc_uri_invalid() {
        assert!(parse_mxc_uri("https://matrix.org/AbCdEfG").is_none());
        assert!(parse_mxc_uri("mxc://").is_none());
        assert!(parse_mxc_uri("mxc:///media_id").is_none());
        assert!(parse_mxc_uri("mxc://server/").is_none());
    }

    #[test]
    fn test_mxc_to_thumbnail_url() {
        let url = mxc_to_thumbnail_url(
            "https://matrix.example.org",
            "mxc://matrix.org/AbCdEfG",
            96, 96,
        ).unwrap();
        assert!(url.starts_with("https://matrix.example.org/_matrix/media/v3/thumbnail/"));
        assert!(url.contains("matrix.org/AbCdEfG"));
        assert!(url.contains("width=96"));
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("!abc:server"), "%21abc%3Aserver");
        assert_eq!(url_encode("@user:matrix.org"), "%40user%3Amatrix.org");
    }

    #[test]
    fn test_path_send_message() {
        let path = path_send_message("!room:server", "txn1");
        assert!(path.contains("rooms/%21room%3Aserver/send/m.room.message/txn1"));
    }
}
