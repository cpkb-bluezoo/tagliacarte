#include "Callbacks.h"
#include "EventBridge.h"
#include "Config.h"
#include "Tr.h"
#include "tagliacarte.h"

#include <QMetaObject>
#include <QString>
#include <QDateTime>
#include <QLocale>

void on_folder_found_cb(const char *name, char delimiter, const char *attributes, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString n = QString::fromUtf8(name);
    QString delim = delimiter ? QString(QChar(delimiter)) : QString();
    QString attrs = attributes ? QString::fromUtf8(attributes) : QString();
    QMetaObject::invokeMethod(b, "addFolder", Qt::QueuedConnection,
        Q_ARG(QString, n), Q_ARG(QString, delim), Q_ARG(QString, attrs));
}

void on_folder_removed_cb(const char *name, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString n = QString::fromUtf8(name);
    QMetaObject::invokeMethod(b, "removeFolder", Qt::QueuedConnection, Q_ARG(QString, n));
}

void on_folder_op_error_cb(const char *message, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString msg = message ? QString::fromUtf8(message) : QStringLiteral("Unknown error");
    QMetaObject::invokeMethod(b, "onFolderOpError", Qt::QueuedConnection, Q_ARG(QString, msg));
}

void on_folder_list_complete_cb(int error, const char *error_message, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString msg = error_message ? QString::fromUtf8(error_message) : QString();
    QMetaObject::invokeMethod(b, "onFolderListComplete", Qt::QueuedConnection,
        Q_ARG(int, error), Q_ARG(QString, msg));
}

void on_message_summary_cb(const char *id, const char *subject, const char *from_, qint64 date_timestamp_secs, uint64_t size, uint32_t flags, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString dateStr;
    if (date_timestamp_secs >= 0) {
        QDateTime dt = QDateTime::fromSecsSinceEpoch(date_timestamp_secs);
        Config c = loadConfig();
        if (c.dateFormat.isEmpty()) {
            dateStr = QLocale().toString(dt, QLocale::ShortFormat);
        } else {
            dateStr = dt.toString(c.dateFormat);
        }
    }
    QMetaObject::invokeMethod(b, "addMessageSummary", Qt::QueuedConnection,
        Q_ARG(QString, QString::fromUtf8(id)),
        Q_ARG(QString, subject ? QString::fromUtf8(subject) : QString()),
        Q_ARG(QString, from_ ? QString::fromUtf8(from_) : QString()),
        Q_ARG(QString, dateStr),
        Q_ARG(qint64, date_timestamp_secs),
        Q_ARG(quint64, size),
        Q_ARG(quint32, flags));
}

void on_bulk_complete_cb(int ok, const char *error_message, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString errMsg = (ok != 0 && error_message) ? QString::fromUtf8(error_message) : QString();
    QMetaObject::invokeMethod(b, "onBulkComplete", Qt::QueuedConnection,
        Q_ARG(int, ok), Q_ARG(QString, errMsg));
}

void on_message_list_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onMessageListComplete", Qt::QueuedConnection, Q_ARG(int, error));
}

void on_message_metadata_cb(const char *subject, const char *from_, const char *to, const char *date, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "showMessageMetadata", Qt::QueuedConnection,
        Q_ARG(QString, subject ? QString::fromUtf8(subject) : QString()),
        Q_ARG(QString, from_ ? QString::fromUtf8(from_) : QString()),
        Q_ARG(QString, to ? QString::fromUtf8(to) : QString()),
        Q_ARG(QString, date ? QString::fromUtf8(date) : QString()));
}

void on_start_entity_cb(void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onStartEntity", Qt::QueuedConnection);
}

void on_content_type_cb(const char *value, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onContentType", Qt::QueuedConnection,
        Q_ARG(QString, value ? QString::fromUtf8(value) : QString()));
}

void on_content_disposition_cb(const char *value, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onContentDisposition", Qt::QueuedConnection,
        Q_ARG(QString, value ? QString::fromUtf8(value) : QString()));
}

void on_content_id_cb(const char *value, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onContentId", Qt::QueuedConnection,
        Q_ARG(QString, value ? QString::fromUtf8(value) : QString()));
}

void on_end_headers_cb(void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onEndHeaders", Qt::QueuedConnection);
}

void on_body_content_cb(const uint8_t *data, size_t len, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QByteArray ba;
    if (data && len > 0) {
        ba = QByteArray(reinterpret_cast<const char *>(data), static_cast<int>(len));
    }
    QMetaObject::invokeMethod(b, "onBodyContent", Qt::QueuedConnection, Q_ARG(QByteArray, ba));
}

void on_end_entity_cb(void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onEndEntity", Qt::QueuedConnection);
}

void on_message_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onMessageComplete", Qt::QueuedConnection, Q_ARG(int, error));
}

void on_send_progress_cb(const char *status, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString s = status ? QString::fromUtf8(status) : QString();
    QMetaObject::invokeMethod(b, "onSendProgress", Qt::QueuedConnection, Q_ARG(QString, s));
}

void on_send_complete_cb(int ok, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onSendComplete", Qt::QueuedConnection, Q_ARG(int, ok));
}

void on_folder_ready_cb(const char *folder_uri, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString uri = QString::fromUtf8(folder_uri);
    tagliacarte_free_string(const_cast<char *>(folder_uri));
    QMetaObject::invokeMethod(b, "onFolderReady", Qt::QueuedConnection, Q_ARG(QString, uri));
}

void on_open_folder_error_cb(const char *message, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString msg = message ? QString::fromUtf8(message) : TR("error.unknown");
    QMetaObject::invokeMethod(b, "onOpenFolderError", Qt::QueuedConnection, Q_ARG(QString, msg));
}

void on_open_folder_select_event_cb(int event_type, uint32_t number_value, const char *, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    if (event_type == TAGLIACARTE_OPEN_FOLDER_EXISTS && b->statusBar) {
        QMetaObject::invokeMethod(b, "showOpeningMessageCount", Qt::QueuedConnection, Q_ARG(quint32, number_value));
    }
}

void on_credential_request_cb(const char *store_uri, int auth_type, int is_plaintext, const char *username, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "requestCredentialSlot", Qt::QueuedConnection,
        Q_ARG(QString, QString::fromUtf8(store_uri ? store_uri : "")),
        Q_ARG(QString, QString::fromUtf8(username ? username : "")),
        Q_ARG(int, is_plaintext),
        Q_ARG(int, auth_type));
}
