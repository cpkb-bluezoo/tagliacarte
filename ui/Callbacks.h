#ifndef CALLBACKS_H
#define CALLBACKS_H

#include <QtTypes>
#include <cstdint>
#include <cstddef>

// C callbacks (run on backend thread); marshal to main thread via EventBridge.
// These are passed to FFI registration calls.

void on_folder_found_cb(const char *name, char delimiter, const char *attributes, void *user_data);
void on_folder_removed_cb(const char *name, void *user_data);
void on_folder_op_error_cb(const char *message, void *user_data);
void on_folder_list_complete_cb(int error, void *user_data);
void on_message_summary_cb(const char *id, const char *subject, const char *from_, qint64 date_timestamp_secs, uint64_t size, void *user_data);
void on_message_list_complete_cb(int error, void *user_data);
void on_message_metadata_cb(const char *subject, const char *from_, const char *to, const char *date, void *user_data);
void on_start_entity_cb(void *user_data);
void on_content_type_cb(const char *value, void *user_data);
void on_content_disposition_cb(const char *value, void *user_data);
void on_content_id_cb(const char *value, void *user_data);
void on_end_headers_cb(void *user_data);
void on_body_content_cb(const uint8_t *data, size_t len, void *user_data);
void on_end_entity_cb(void *user_data);
void on_message_complete_cb(int error, void *user_data);
void on_send_progress_cb(const char *status, void *user_data);
void on_send_complete_cb(int ok, void *user_data);
void on_folder_ready_cb(const char *folder_uri, void *user_data);
void on_open_folder_error_cb(const char *message, void *user_data);
void on_open_folder_select_event_cb(int event_type, uint32_t number_value, const char *string_value, void *user_data);
void on_credential_request_cb(const char *store_uri, int auth_type, int is_plaintext, const char *username, void *user_data);

#endif // CALLBACKS_H
