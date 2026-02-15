/*
 * tagliacarte.h
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

/* C API for tagliacarte core. Stores, folders, and transports are identified by URI.
 * Create functions return a newly allocated URI string; free with tagliacarte_free_string.
 * All string parameters are UTF-8 NUL-terminated. */

#ifndef TAGLIACARTE_H
#define TAGLIACARTE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

/* Version string (static, do not free). */
const char *tagliacarte_version(void);

/* Last error message from a failed call. Valid until next FFI call. Do not free. */
const char *tagliacarte_last_error(void);

/* Free a string returned by tagliacarte_store_maildir_new, tagliacarte_store_imap_new, tagliacarte_store_pop3_new, tagliacarte_store_nostr_new, tagliacarte_store_matrix_new, tagliacarte_store_open_folder, tagliacarte_transport_smtp_new, tagliacarte_transport_nostr_new, tagliacarte_transport_matrix_new. No-op if ptr is NULL. */
void tagliacarte_free_string(char *ptr);

/* Free a NULL-terminated array of strings from tagliacarte_store_list_folders. */
void tagliacarte_free_string_list(char **ptr);

/* Conversation summary for list view. Free with tagliacarte_free_conversation_summary_list. */
typedef struct TagliacarteConversationSummary {
    char *id;
    char *subject;
    char *from_;
    uint64_t size;
} TagliacarteConversationSummary;

void tagliacarte_free_conversation_summary_list(
    TagliacarteConversationSummary *ptr,
    size_t count
);

/* Attachment in a received message (owned; freed by tagliacarte_free_message). */
typedef struct TagliacarteMessageAttachment {
    char *filename;   /* NULL if not present */
    char *mime_type;
    uint8_t *data;
    size_t data_len;
} TagliacarteMessageAttachment;

/* Full message (envelope + structured body + attachments). Free with tagliacarte_free_message. */
typedef struct TagliacarteMessage {
    char *subject;
    char *from_;
    char *to;
    char *date;
    char *body_html;   /* preferred for display; NULL if not present */
    char *body_plain;
    size_t attachment_count;
    TagliacarteMessageAttachment *attachments;  /* NULL if attachment_count 0 */
} TagliacarteMessage;

void tagliacarte_free_message(TagliacarteMessage *msg);

/* Store: identified by URI (e.g. maildir:///path, imaps://user@host:993). */
char *tagliacarte_store_maildir_new(const char *root_path);  /* caller frees with tagliacarte_free_string */
char *tagliacarte_store_imap_new(const char *user_at_host, const char *host, uint16_t port);  /* imaps: for 993, imap: otherwise; caller frees URI */
char *tagliacarte_store_pop3_new(const char *user_at_host, const char *host, uint16_t port);  /* pop3s for 995; auth via Authenticate flow; caller frees URI */
char *tagliacarte_store_nostr_new(const char *relays_comma_separated, const char *key_path);  /* key_path NULL = use env; caller frees URI */
char *tagliacarte_store_matrix_new(const char *homeserver, const char *user_id, const char *access_token);  /* access_token NULL = must log in; caller frees URI */
void tagliacarte_store_free(const char *store_uri);

/* Credential callback: when core needs a password it calls this (store_uri, auth_type, is_plaintext, username, user_data). UI shows dialog, then calls tagliacarte_credential_provide or tagliacarte_credential_cancel. Pass NULL to clear. */
#define TAGLIACARTE_AUTH_TYPE_AUTO 0
#define TAGLIACARTE_NEEDS_CREDENTIAL (-2)  /* returned from list_folders / on_complete when credential required */
typedef void (*tagliacarte_credential_request_cb)(const char *store_uri, int auth_type, int is_plaintext, const char *username, void *user_data);
void tagliacarte_set_credential_request_callback(tagliacarte_credential_request_cb callback, void *user_data);
int tagliacarte_credential_provide(const char *store_uri, const char *password);  /* 0 success, -1 error */
void tagliacarte_credential_cancel(const char *store_uri);  /* no-op; next connect will request again */

void tagliacarte_set_credentials_backend(int use_keychain);  /* 1 = keychain, 0 = encrypted file; call at startup */
int tagliacarte_keychain_available(void);  /* 1 if system keychain available, 0 otherwise */
int tagliacarte_migrate_credentials_to_keychain(const char *path);  /* file -> keychain; 0 success, -1 error */
int tagliacarte_migrate_credentials_to_file(const char *path, size_t uri_count, const char **uris);  /* keychain -> file; 0 success, -1 error */

int tagliacarte_store_list_folders(
    const char *store_uri,
    size_t *out_count,
    char ***out_names
);
char *tagliacarte_store_open_folder(const char *store_uri, const char *name);  /* returns folder URI; caller frees with tagliacarte_free_string */

/* Store kind: 0 = Email, 1 = Nostr, 2 = Matrix. Returns -1 if store_uri is NULL or not found. */
#define TAGLIACARTE_STORE_KIND_EMAIL  0
#define TAGLIACARTE_STORE_KIND_NOSTR  1
#define TAGLIACARTE_STORE_KIND_MATRIX 2
int tagliacarte_store_kind(const char *store_uri);

/* Store: event-driven folder list. Callbacks may run on a backend thread; marshal to main thread if needed. */
typedef void (*TagliacarteOnFolderFound)(const char *name, char delimiter, const char *attributes, void *user_data);
typedef void (*TagliacarteOnFolderRemoved)(const char *name, void *user_data);
typedef void (*TagliacarteOnFolderListComplete)(int error, void *user_data);
void tagliacarte_store_set_folder_list_callbacks(
    const char *store_uri,
    TagliacarteOnFolderFound on_folder_found,
    TagliacarteOnFolderRemoved on_folder_removed,
    TagliacarteOnFolderListComplete on_complete,
    void *user_data
);
void tagliacarte_store_refresh_folders(const char *store_uri);  /* returns immediately */

/* Hierarchy delimiter for a store. Returns '\0' if unknown or not applicable. */
char tagliacarte_store_hierarchy_delimiter(const char *store_uri);

/* Folder management: returns immediately; on success the existing on_folder_found / on_folder_removed
 * callback fires from a backend thread. On error, on_error(message, user_data) is called. */
typedef void (*TagliacarteOnFolderOpError)(const char *message, void *user_data);
void tagliacarte_store_create_folder(
    const char *store_uri, const char *name,
    TagliacarteOnFolderOpError on_error, void *user_data);
void tagliacarte_store_rename_folder(
    const char *store_uri, const char *old_name, const char *new_name,
    TagliacarteOnFolderOpError on_error, void *user_data);
void tagliacarte_store_delete_folder(
    const char *store_uri, const char *name,
    TagliacarteOnFolderOpError on_error, void *user_data);

/* Store: async open folder. Returns immediately; callbacks run on a backend thread (marshal to main thread if needed).
 * on_select_event: optional (may be NULL). Called for each SELECT response item. string_value is valid only during the call; copy if keeping.
 * on_folder_ready: called once with folder_uri (caller must free with tagliacarte_free_string).
 * on_error: called on failure; message valid only during the call. */
#define TAGLIACARTE_OPEN_FOLDER_EXISTS       0
#define TAGLIACARTE_OPEN_FOLDER_RECENT       1
#define TAGLIACARTE_OPEN_FOLDER_FLAGS        2
#define TAGLIACARTE_OPEN_FOLDER_UID_VALIDITY 3
#define TAGLIACARTE_OPEN_FOLDER_UID_NEXT     4
#define TAGLIACARTE_OPEN_FOLDER_OTHER        5
typedef void (*TagliacarteOnOpenFolderSelectEvent)(int event_type, uint32_t number_value, const char *string_value, void *user_data);
typedef void (*TagliacarteOnFolderReady)(const char *folder_uri, void *user_data);
typedef void (*TagliacarteOnOpenFolderError)(const char *message, void *user_data);
void tagliacarte_store_start_open_folder(
    const char *store_uri,
    const char *mailbox_name,
    TagliacarteOnOpenFolderSelectEvent on_select_event,
    TagliacarteOnFolderReady on_folder_ready,
    TagliacarteOnOpenFolderError on_error,
    void *user_data
);

/* Folder: identified by URI (store_uri + "/" + encoded name). */
void tagliacarte_folder_free(const char *folder_uri);

/* Folder: event-driven message list. Callbacks may run on a backend thread. */
typedef void (*TagliacarteOnMessageSummary)(const char *id, const char *subject, const char *from_, int64_t date_timestamp_secs, uint64_t size, void *user_data);  /* date_timestamp_secs: Unix time, or -1 if no date */
typedef void (*TagliacarteOnMessageListComplete)(int error, void *user_data);
void tagliacarte_folder_set_message_list_callbacks(
    const char *folder_uri,
    TagliacarteOnMessageSummary on_message_summary,
    TagliacarteOnMessageListComplete on_complete,
    void *user_data
);
void tagliacarte_folder_request_message_list(const char *folder_uri, uint64_t start, uint64_t end);  /* returns immediately */

/* Folder: event-driven get message.
 * Flow: on_metadata(envelope), then MIME entity events mirroring MimeHandler:
 *   on_start_entity, on_content_type, on_content_disposition, on_content_id, on_end_headers,
 *   on_body_content (multiple, chunked), on_end_entity — for each MIME entity.
 * Finally on_complete. Body content arrives in chunks for streaming display. */
typedef void (*TagliacarteOnMessageMetadata)(const char *subject, const char *from_, const char *to, const char *date, void *user_data);
typedef void (*TagliacarteOnStartEntity)(void *user_data);
typedef void (*TagliacarteOnContentType)(const char *value, void *user_data);
typedef void (*TagliacarteOnContentDisposition)(const char *value, void *user_data);
typedef void (*TagliacarteOnContentId)(const char *value, void *user_data);
typedef void (*TagliacarteOnEndHeaders)(void *user_data);
typedef void (*TagliacarteOnBodyContent)(const uint8_t *data, size_t len, void *user_data);
typedef void (*TagliacarteOnEndEntity)(void *user_data);
typedef void (*TagliacarteOnMessageComplete)(int error, void *user_data);
void tagliacarte_folder_set_message_callbacks(
    const char *folder_uri,
    TagliacarteOnMessageMetadata on_metadata,
    TagliacarteOnStartEntity on_start_entity,
    TagliacarteOnContentType on_content_type,
    TagliacarteOnContentDisposition on_content_disposition,
    TagliacarteOnContentId on_content_id,
    TagliacarteOnEndHeaders on_end_headers,
    TagliacarteOnBodyContent on_body_content,
    TagliacarteOnEndEntity on_end_entity,
    TagliacarteOnMessageComplete on_complete,
    void *user_data
);
void tagliacarte_folder_request_message(const char *folder_uri, const char *message_id);  /* returns immediately */

uint64_t tagliacarte_folder_message_count(const char *folder_uri);

/* Append raw message bytes (e.g. from .eml file) to a folder. Supported for Maildir. Returns 0 on success, -1 on error. */
int tagliacarte_folder_append_message(const char *folder_uri, const unsigned char *data, size_t data_len);

/* Delete a message by id. Supported for Maildir. Returns 0 on success, -1 on error. */
int tagliacarte_folder_delete_message(const char *folder_uri, const char *message_id);

int tagliacarte_folder_get_message(
    const char *folder_uri,
    const char *message_id,
    TagliacarteMessage **out_message
);
int tagliacarte_folder_list_conversations(
    const char *folder_uri,
    uint64_t start,
    uint64_t end,
    size_t *out_count,
    TagliacarteConversationSummary **out_summaries
);

/* Transport: identified by URI (e.g. smtps://host:465, smtp://host:587). */
#define TAGLIACARTE_TRANSPORT_KIND_EMAIL  0
#define TAGLIACARTE_TRANSPORT_KIND_NOSTR  1
#define TAGLIACARTE_TRANSPORT_KIND_MATRIX 2
int tagliacarte_transport_kind(const char *transport_uri);
char *tagliacarte_transport_smtp_new(const char *host, uint16_t port);  /* smtps: for 465, smtp: otherwise; caller frees URI */
char *tagliacarte_transport_nostr_new(const char *relays_comma_separated, const char *key_path);  /* key_path NULL = use env; caller frees URI */
char *tagliacarte_transport_matrix_new(const char *homeserver, const char *user_id, const char *access_token);  /* access_token NULL = must log in; caller frees URI */
typedef struct {
    const char *filename;   /* NULL ok */
    const char *mime_type;
    const uint8_t *data;
    size_t data_len;
} TagliacarteAttachment;
int tagliacarte_transport_send(
    const char *transport_uri,
    const char *from,
    const char *to,
    const char *cc,         /* NULL or comma-separated */
    const char *subject,    /* NULL ok */
    const char *body_plain, /* NULL ok */
    const char *body_html,  /* NULL ok */
    size_t attachment_count,
    const TagliacarteAttachment *attachments  /* NULL if attachment_count 0 */
);

/* Async send: returns immediately; callbacks run on a background thread (marshal to main thread if needed).
 * Do not free the transport until on_complete has been called.
 * on_progress: optional (may be NULL). status e.g. "connecting", "sending"; valid only during the call.
 * on_complete: ok 0 = success, non-zero = error. */
typedef void (*TagliacarteOnSendProgress)(const char *status, void *user_data);
typedef void (*TagliacarteOnSendComplete)(int ok, void *user_data);
void tagliacarte_transport_send_async(
    const char *transport_uri,
    const char *from,
    const char *to,
    const char *cc,
    const char *subject,
    const char *body_plain,
    const char *body_html,
    size_t attachment_count,
    const TagliacarteAttachment *attachments,
    TagliacarteOnSendProgress on_progress,
    TagliacarteOnSendComplete on_complete,
    void *user_data
);

/* Streaming send (non-blocking). Order: start_send → metadata → body chunks → (start_attachment → attachment chunks → end_attachment)* → end_send. Free session_id with tagliacarte_free_string. */
char *tagliacarte_transport_start_send(const char *transport_uri);  /* NULL if not supported */
typedef void (*TagliacarteOnSendComplete)(int ok, void *user_data);  /* ok: 0 = success */
int tagliacarte_send_session_metadata(const char *session_id, const char *from, const char *to, const char *cc, const char *subject);
int tagliacarte_send_session_body_plain_chunk(const char *session_id, const uint8_t *data, size_t data_len);
int tagliacarte_send_session_body_html_chunk(const char *session_id, const uint8_t *data, size_t data_len);
int tagliacarte_send_session_start_attachment(const char *session_id, const char *filename, const char *mime_type);
int tagliacarte_send_session_attachment_chunk(const char *session_id, const uint8_t *data, size_t data_len);
int tagliacarte_send_session_end_attachment(const char *session_id);
void tagliacarte_send_session_end_send(const char *session_id, TagliacarteOnSendComplete on_complete, void *user_data);  /* returns immediately; on_complete called from background thread */
void tagliacarte_send_session_free(const char *session_id);  /* discard without sending */

void tagliacarte_transport_free(const char *transport_uri);

#ifdef __cplusplus
}
#endif

#endif /* TAGLIACARTE_H */
