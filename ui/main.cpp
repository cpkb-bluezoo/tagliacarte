/*
 * main.cpp
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

// Tagliacarte UI – Qt 6, Phase 2: conversation view, message body, compose.

#include <QApplication>
#include <QMainWindow>
#include <QWidget>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPushButton>
#include <QToolButton>
#include <QListWidget>
#include <QTreeWidget>
#include <QSplitter>
#include <QFileDialog>
#include <QMessageBox>
#include <QStatusBar>
#include <QLabel>
#include <QTextBrowser>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QLineEdit>
#include <QTextEdit>
#include <QDateTime>
#include <QMetaObject>
#include <QTimer>
#include <QFrame>
#include <QStackedWidget>
#include <QTabWidget>
#include <QScrollArea>
#include <QSizePolicy>
#include <QSpinBox>
#include <QCheckBox>
#include <QComboBox>
#include <QDir>
#include <QStandardPaths>
#include <QFile>
#include <QSaveFile>
#include <QXmlStreamReader>
#include <QXmlStreamWriter>
#include <QSvgRenderer>
#include <QPainter>
#include <QPaintEvent>
#include <QColor>
#include <QTranslator>
#include <QLocale>
#include <QInputDialog>
#include <QMap>
#include <QRegularExpression>

#include "tagliacarte.h"
#include "EventBridge.h"
#include "Tr.h"

#include <cstring>
#include <cstdint>

void showError(QWidget *parent, const char *contextKey) {
    const char *msg = tagliacarte_last_error();
    QString msgStr = msg ? QString::fromUtf8(msg) : TR("error.unknown");
    QMessageBox::critical(parent, TR("common.error"), QString("%1: %2").arg(TR(contextKey)).arg(msgStr));
}

// Resolve icon path: try Qt resource first, then filesystem (app dir, bundle Resources, build dir).
static QString resolveIconPath(const QString &resourcePath) {
    QFile f(resourcePath);
    if (f.open(QIODevice::ReadOnly)) {
        f.close();
        return resourcePath;
    }
    QString base = QCoreApplication::applicationDirPath();
    int lastSlash = resourcePath.lastIndexOf(QLatin1Char('/'));
    QString name = lastSlash >= 0 ? resourcePath.mid(lastSlash + 1) : resourcePath;
    QStringList tries;
    tries << (base + QLatin1String("/icons/") + name);
#if defined(Q_OS_MACOS) || defined(Q_OS_DARWIN)
    tries << (base + QLatin1String("/../Resources/icons/") + name);
#endif
    tries << (base + QLatin1String("/../ui/icons/") + name);
    tries << (base + QLatin1String("/../../ui/icons/") + name);
    for (const QString &p : tries) {
        if (QFile::exists(p)) {
            return p;
        }
    }
    return resourcePath;
}

// Render SVG from resource to a QPixmap at exact size (no QIcon; we control pixels).
static QPixmap renderSvgToPixmap(const QString &path, const QColor &color, int w, int h, qreal scaleFactor = 1.0) {
    QString resolved = resolveIconPath(path);
    QFile f(resolved);
    if (!f.open(QIODevice::ReadOnly)) {
        return QPixmap();
    }
    QByteArray svg = f.readAll();
    f.close();
    svg.replace("currentColor", color.name(QColor::HexRgb).toLatin1());
    QSvgRenderer r(svg);
    if (!r.isValid()) {
        return QPixmap();
    }
    QImage img(w, h, QImage::Format_ARGB32);
    img.fill(Qt::transparent);
    QPainter p(&img);
    if (scaleFactor > 1.0) {
        p.translate(w / 2.0, h / 2.0);
        p.scale(scaleFactor, scaleFactor);
        p.translate(-w / 2.0, -h / 2.0);
    }
    r.render(&p, QRectF(0, 0, w, h));
    p.end();
    return QPixmap::fromImage(img);
}

// Web-safe colours for store/account circles (unselected = border, selected = background). Order determines assignment.
static const char *const storeCircleColours[] = {
    "#6699CC", "#996633", "#339966", "#993366", "#666699", "#CC9933", "#33CC99", "#CC6699"
};
static const int storeCircleColourCount = int(sizeof(storeCircleColours) / sizeof(storeCircleColours[0]));

// Stylesheet for a store/account circle: unselected = thin border of colour, selected = full background of colour.
static QString storeCircleStyleSheet(int colourIndex) {
    QString hex(storeCircleColours[colourIndex % storeCircleColourCount]);
    return QStringLiteral("QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); font-weight: bold; padding: 0; min-width: 40px; min-height: 40px; border: 2px solid %1; }"
                         "QToolButton:hover { background-color: palette(light); }"
                         "QToolButton:checked { background-color: %1; color: #fff; border-color: %1; }"
                         "QToolButton:checked:hover { background-color: %1; color: #fff; }").arg(hex);
}

// Config under ~/.tagliacarte/config.xml. Core fields on <store>; store-specific data in <param key="..." value="..."/>.
struct StoreEntry {
    QString id;
    QString type;
    QString displayName;
    QString emailAddress;  // From address; also NIP-05 for Nostr
    QString picture;       // Optional; for future use
    QMap<QString, QString> params;  // Store-specific: hostname, path, username, port, security, transport*, imap*, etc.
};
struct Config {
    QList<StoreEntry> stores;
    QString lastSelectedStoreId;
    bool useKeychain = false;  // true = system keychain, false = encrypted file
    QString dateFormat;        // empty = locale default; otherwise Qt date format string for message list
};

static QString param(const StoreEntry &e, const char *key) {
    return e.params.value(QLatin1String(key));
}
static int paramInt(const StoreEntry &e, const char *key, int defaultVal) {
    QString v = e.params.value(QLatin1String(key));
    return v.isEmpty() ? defaultVal : v.toInt();
}
// Hostname for IMAP/POP3, path for Maildir/Nostr/Matrix/mbox
static QString storeHostOrPath(const StoreEntry &e) {
    if (e.type == QLatin1String("imap") || e.type == QLatin1String("pop3"))
        return param(e, "hostname");
    return param(e, "path");
}

static QString tagliacarteConfigDir() {
    QString path = QStandardPaths::writableLocation(QStandardPaths::HomeLocation) + QStringLiteral("/.tagliacarte");
    QDir().mkpath(path);
    return path;
}
static QString tagliacarteConfigPath() { return tagliacarteConfigDir() + QStringLiteral("/config.xml"); }

static Config loadConfig() {
    Config c;
    QFile f(tagliacarteConfigPath());
    if (!f.open(QIODevice::ReadOnly))
        return c;
    QXmlStreamReader r(&f);
    auto attr = [](const QXmlStreamAttributes &a, const char *kebab, const char *oldCamel = nullptr) {
        QString v = a.value(QLatin1String(kebab)).toString();
        if (v.isEmpty() && oldCamel) {
            v = a.value(QLatin1String(oldCamel)).toString();
        }
        return v;
    };
    while (!r.atEnd()) {
        r.readNext();
        if (r.isStartElement()) {
            if (r.name() == QLatin1String("selected-store")) {
                c.lastSelectedStoreId = r.attributes().value(QLatin1String("id")).toString().trimmed();
            } else if (r.name() == QLatin1String("lastSelectedStoreId")) {
                if (c.lastSelectedStoreId.isEmpty()) {
                    c.lastSelectedStoreId = r.readElementText().trimmed();
                } else {
                    r.readElementText();
                }
            } else if (r.name() == QLatin1String("store")) {
                const QXmlStreamAttributes a = r.attributes();
                StoreEntry e;
                e.id = a.value(QLatin1String("id")).toString();
                e.type = a.value(QLatin1String("type")).toString();
                e.displayName = attr(a, "display-name", "displayName");
                e.emailAddress = attr(a, "email-address", "emailAddress");
                e.picture = attr(a, "picture", "picture");
                // Backward compat: nip-05 -> emailAddress if emailAddress empty
                QString nip05Val = attr(a, "nip-05", "nip05");
                if (!nip05Val.isEmpty() && e.emailAddress.isEmpty())
                    e.emailAddress = nip05Val;
                // Old-style attributes -> params
                QString host = attr(a, "hostname", "path");
                if (!host.isEmpty()) e.params[QStringLiteral("hostname")] = host;
                QString pathVal = a.value(QLatin1String("path")).toString();
                if (!pathVal.isEmpty()) e.params[QStringLiteral("path")] = pathVal;
                QString u = attr(a, "username", "user-id");
                if (u.isEmpty()) u = attr(a, "userId");
                if (!u.isEmpty()) e.params[QStringLiteral("username")] = u;
                QString uid = attr(a, "user-id", "userId");
                if (!uid.isEmpty()) e.params[QStringLiteral("userId")] = uid;
                if (!attr(a, "key-path", "keyPath").isEmpty()) e.params[QStringLiteral("keyPath")] = attr(a, "key-path", "keyPath");
                if (!attr(a, "access-token", "accessToken").isEmpty()) e.params[QStringLiteral("accessToken")] = attr(a, "access-token", "accessToken");
                int p = attr(a, "port").toInt();
                if (p > 0) e.params[QStringLiteral("port")] = QString::number(p);
                QString sec = attr(a, "security");
                if (sec == QLatin1String("none")) e.params[QStringLiteral("security")] = QStringLiteral("0");
                else if (sec == QLatin1String("starttls")) e.params[QStringLiteral("security")] = QStringLiteral("1");
                else if (sec == QLatin1String("ssl")) e.params[QStringLiteral("security")] = QStringLiteral("2");
                int pollMin = attr(a, "poll-interval-minutes").toInt();
                if (pollMin == 1 || pollMin == 5 || pollMin == 10 || pollMin == 60)
                    e.params[QStringLiteral("imapPollSeconds")] = QString::number(pollMin * 60);
                if (attr(a, "deletion") == QLatin1String("move_to_trash")) e.params[QStringLiteral("imapDeletion")] = QStringLiteral("1");
                QString tf = attr(a, "trash-folder");
                if (!tf.isEmpty()) e.params[QStringLiteral("imapTrashFolder")] = tf;
                int idleSec = attr(a, "idle-seconds").toInt();
                if (idleSec == 30 || idleSec == 60 || idleSec == 300) e.params[QStringLiteral("imapIdleSeconds")] = QString::number(idleSec);
                while (!r.atEnd()) {
                    r.readNext();
                    if (r.isEndElement() && r.name() == QLatin1String("store"))
                        break;
                    if (r.isStartElement() && r.name() == QLatin1String("transport")) {
                        const QXmlStreamAttributes ta = r.attributes();
                        if (ta.value(QLatin1String("type")).toString() == QLatin1String("smtp")) {
                            QString th = ta.value(QLatin1String("hostname")).toString();
                            if (!th.isEmpty()) e.params[QStringLiteral("transportHostname")] = th;
                            int tp = ta.value(QLatin1String("port")).toString().toInt();
                            e.params[QStringLiteral("transportPort")] = QString::number(tp > 0 ? tp : 586);
                            QString tu = ta.value(QLatin1String("username")).toString();
                            if (!tu.isEmpty()) e.params[QStringLiteral("transportUsername")] = tu;
                            QString tsec = ta.value(QLatin1String("security")).toString();
                            if (tsec == QLatin1String("none")) e.params[QStringLiteral("transportSecurity")] = QStringLiteral("0");
                            else if (tsec == QLatin1String("ssl")) e.params[QStringLiteral("transportSecurity")] = QStringLiteral("2");
                            else e.params[QStringLiteral("transportSecurity")] = QStringLiteral("1");
                            QString tid = ta.value(QLatin1String("id")).toString();
                            if (!tid.isEmpty()) e.params[QStringLiteral("transportId")] = tid;
                        }
                    } else if (r.isStartElement() && r.name() == QLatin1String("param")) {
                        QString k = r.attributes().value(QLatin1String("key")).toString();
                        QString v = r.attributes().value(QLatin1String("value")).toString();
                        if (!k.isEmpty()) e.params[k] = v;
                    }
                }
                if (!e.id.isEmpty())
                    c.stores.append(e);
            } else if (r.name() == QLatin1String("security")) {
                while (!r.atEnd()) {
                    r.readNext();
                    if (r.isEndElement() && r.name() == QLatin1String("security"))
                        break;
                    if (r.isStartElement() && r.name() == QLatin1String("credentials")) {
                        QString storage = r.attributes().value(QLatin1String("storage")).toString();
                        c.useKeychain = (storage == QLatin1String("keychain"));
                    }
                }
            } else if (r.name() == QLatin1String("viewing")) {
                while (!r.atEnd()) {
                    r.readNext();
                    if (r.isEndElement() && r.name() == QLatin1String("viewing"))
                        break;
                    if (r.isStartElement() && r.name() == QLatin1String("date-format")) {
                        c.dateFormat = r.attributes().value(QLatin1String("value")).toString();
                    }
                }
            }
        }
    }
    f.close();
    return c;
}

static void saveConfig(const Config &c) {
    QSaveFile f(tagliacarteConfigPath());
    if (!f.open(QIODevice::WriteOnly))
        return;
    QXmlStreamWriter w(&f);
    w.setAutoFormatting(true);
    w.writeStartDocument(QStringLiteral("1.0"), true);
    w.writeStartElement(QStringLiteral("tagliacarte"));
    if (!c.lastSelectedStoreId.isEmpty()) {
        w.writeStartElement(QStringLiteral("selected-store"));
        w.writeAttribute(QStringLiteral("id"), c.lastSelectedStoreId);
        w.writeEndElement();
    }
    w.writeStartElement(QStringLiteral("security"));
    w.writeStartElement(QStringLiteral("credentials"));
    w.writeAttribute(QStringLiteral("storage"), c.useKeychain ? QStringLiteral("keychain") : QStringLiteral("file"));
    w.writeEndElement();
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("viewing"));
    if (!c.dateFormat.isEmpty()) {
        w.writeStartElement(QStringLiteral("date-format"));
        w.writeAttribute(QStringLiteral("value"), c.dateFormat);
        w.writeEndElement();
    }
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("stores"));
    for (const StoreEntry &e : c.stores) {
        w.writeStartElement(QStringLiteral("store"));
        w.writeAttribute(QStringLiteral("id"), e.id);
        w.writeAttribute(QStringLiteral("type"), e.type);
        w.writeAttribute(QStringLiteral("display-name"), e.displayName);
        if (!e.emailAddress.isEmpty())
            w.writeAttribute(QStringLiteral("email-address"), e.emailAddress);
        if (!e.picture.isEmpty())
            w.writeAttribute(QStringLiteral("picture"), e.picture);
        for (auto it = e.params.constBegin(); it != e.params.constEnd(); ++it) {
            w.writeStartElement(QStringLiteral("param"));
            w.writeAttribute(QStringLiteral("key"), it.key());
            w.writeAttribute(QStringLiteral("value"), it.value());
            w.writeEndElement();
        }
        w.writeEndElement();
    }
    w.writeEndElement();
    w.writeEndElement();
    w.writeEndDocument();
    f.commit();
}

// Load SVG from resource and render as QIcon with currentColor replaced by palette color.
// Renders at size and at 2*size for HiDPI. scaleFactor zooms the graphic (e.g. 1.35 to fill the frame better).
static QIcon iconFromSvgResource(const QString &path, const QColor &color, int size = 24, qreal scaleFactor = 1.0) {
    QString resolved = resolveIconPath(path);
    QFile f(resolved);
    if (!f.open(QIODevice::ReadOnly))
        return QIcon();
    QByteArray svg = f.readAll();
    f.close();
    svg.replace("currentColor", color.name(QColor::HexRgb).toLatin1());
    QSvgRenderer r(svg);
    if (!r.isValid())
        return QIcon();
    auto renderAt = [&r, scaleFactor](int w, int h) {
        QImage img(w, h, QImage::Format_ARGB32);
        img.fill(Qt::transparent);
        QPainter p(&img);
        if (scaleFactor > 1.0) {
            p.translate(w / 2.0, h / 2.0);
            p.scale(scaleFactor, scaleFactor);
            p.translate(-w / 2.0, -h / 2.0);
        }
        r.render(&p, QRectF(0, 0, w, h));
        p.end();
        return QPixmap::fromImage(img);
    };
    QIcon icon;
    QPixmap px1 = renderAt(size, size);
    icon.addPixmap(px1);
    const int size2 = qMin(size * 2, 128);
    QPixmap px2 = renderAt(size2, size2);
    px2.setDevicePixelRatio(2.0);
    icon.addPixmap(px2);
    return icon;
}

// C callbacks (run on backend thread); marshal to main thread via EventBridge.
static void on_folder_found_cb(const char *name, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString n = QString::fromUtf8(name);
    QMetaObject::invokeMethod(b, "addFolder", Qt::QueuedConnection, Q_ARG(QString, n));
}
static void on_folder_removed_cb(const char *, void *) {}
static void on_folder_list_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onFolderListComplete", Qt::QueuedConnection, Q_ARG(int, error));
}
static void on_message_summary_cb(const char *id, const char *subject, const char *from_, qint64 date_timestamp_secs, uint64_t size, void *user_data) {
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
        Q_ARG(quint64, size));
}
static void on_message_list_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onMessageListComplete", Qt::QueuedConnection, Q_ARG(int, error));
}
static void on_message_metadata_cb(const char *, const char *, const char *, const char *, void *) {}
static void on_message_content_cb(TagliacarteMessage *msg, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString subject = msg && msg->subject ? QString::fromUtf8(msg->subject) : QString();
    QString from_ = msg && msg->from_ ? QString::fromUtf8(msg->from_) : QString();
    QString to = msg && msg->to ? QString::fromUtf8(msg->to) : QString();
    QString bodyHtml = msg && msg->body_html && *msg->body_html ? QString::fromUtf8(msg->body_html) : QString();
    QString bodyPlain = msg && msg->body_plain && *msg->body_plain ? QString::fromUtf8(msg->body_plain) : QString();
    QMetaObject::invokeMethod(b, "showMessageContent", Qt::QueuedConnection,
        Q_ARG(QString, subject), Q_ARG(QString, from_), Q_ARG(QString, to),
        Q_ARG(QString, bodyHtml), Q_ARG(QString, bodyPlain));
}
static void on_message_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onMessageComplete", Qt::QueuedConnection, Q_ARG(int, error));
}
static void on_send_progress_cb(const char *status, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString s = status ? QString::fromUtf8(status) : QString();
    QMetaObject::invokeMethod(b, "onSendProgress", Qt::QueuedConnection, Q_ARG(QString, s));
}
static void on_send_complete_cb(int ok, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onSendComplete", Qt::QueuedConnection, Q_ARG(int, ok));
}

/* Async open folder: run on backend thread; marshal to main thread. */
static void on_folder_ready_cb(const char *folder_uri, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString uri = QString::fromUtf8(folder_uri);
    tagliacarte_free_string(const_cast<char *>(folder_uri));
    QMetaObject::invokeMethod(b, "onFolderReady", Qt::QueuedConnection, Q_ARG(QString, uri));
}
static void on_open_folder_error_cb(const char *message, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString msg = message ? QString::fromUtf8(message) : TR("error.unknown");
    QMetaObject::invokeMethod(b, "onOpenFolderError", Qt::QueuedConnection, Q_ARG(QString, msg));
}
static void on_open_folder_select_event_cb(int event_type, uint32_t number_value, const char *, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    if (event_type == TAGLIACARTE_OPEN_FOLDER_EXISTS && b->statusBar) {
        QMetaObject::invokeMethod(b, "showOpeningMessageCount", Qt::QueuedConnection, Q_ARG(quint32, number_value));
    }
}

static void on_credential_request_cb(const char *store_uri, int auth_type, int is_plaintext, const char *username, void *user_data) {
    (void)auth_type;
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "requestCredentialSlot", Qt::QueuedConnection,
        Q_ARG(QString, QString::fromUtf8(store_uri ? store_uri : "")),
        Q_ARG(QString, QString::fromUtf8(username ? username : "")),
        Q_ARG(int, is_plaintext));
}

// ----- Compose dialog (From/To/Subject/Body; labels vary by transport kind per plan Phase 4) -----
class ComposeDialog : public QDialog {
public:
    QLineEdit *fromEdit;
    QLineEdit *toEdit;
    QLineEdit *subjectEdit;
    QTextEdit *bodyEdit;

    ComposeDialog(QWidget *parent = nullptr, const QByteArray &transportUri = QByteArray()) : QDialog(parent) {
        setWindowTitle(TR("compose.title"));
        auto *layout = new QFormLayout(this);
        fromEdit = new QLineEdit(this);
        fromEdit->setPlaceholderText(TR("compose.placeholder.from"));
        layout->addRow(TR("compose.from") + QStringLiteral(":"), fromEdit);
        toEdit = new QLineEdit(this);
        QString toLabel = TR("compose.to");
        QString toPlaceholder = TR("compose.placeholder.to");
        if (!transportUri.isEmpty()) {
            int kind = tagliacarte_transport_kind(transportUri.constData());
            if (kind == TAGLIACARTE_TRANSPORT_KIND_NOSTR) {
                toLabel = TR("compose.to_pubkey");
                toPlaceholder = TR("compose.placeholder.to_pubkey");
            } else if (kind == TAGLIACARTE_TRANSPORT_KIND_MATRIX) {
                toLabel = TR("compose.to_room_mxid");
                toPlaceholder = TR("compose.placeholder.to_room_mxid");
            }
        }
        toEdit->setPlaceholderText(toPlaceholder);
        layout->addRow(toLabel + QStringLiteral(":"), toEdit);
        subjectEdit = new QLineEdit(this);
        layout->addRow(TR("compose.subject") + QStringLiteral(":"), subjectEdit);
        bodyEdit = new QTextEdit(this);
        bodyEdit->setMinimumHeight(120);
        layout->addRow(TR("compose.body") + QStringLiteral(":"), bodyEdit);
        auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
        connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
        connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
        layout->addRow(buttons);
    }
};

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);

    // Load L10n: .qm from Resources/translations (macOS) or translations/ next to executable
    QString l10nDir = QCoreApplication::applicationDirPath();
#if defined(Q_OS_MACOS)
    l10nDir += QStringLiteral("/../Resources/translations");
#else
    l10nDir += QStringLiteral("/translations");
#endif
    // Install English first as fallback when current locale has no translation
    QTranslator fallbackTranslator;
    if (fallbackTranslator.load(QLocale(QLocale::English), QStringLiteral("tagliacarte"), QStringLiteral("_"), l10nDir))
        app.installTranslator(&fallbackTranslator);
    QTranslator translator;
    if (translator.load(QLocale(), QStringLiteral("tagliacarte"), QStringLiteral("_"), l10nDir))
        app.installTranslator(&translator);

    const char *version = tagliacarte_version();
    if (!version) {
        QMessageBox::critical(nullptr, TR("app.name"), TR("app.failed_link"));
        return 1;
    }

    QMainWindow win;
    win.setWindowTitle(TR("app.window_title"));
    win.setMinimumSize(800, 550);

    auto *central = new QWidget(&win);
    auto *mainLayout = new QHBoxLayout(central);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);
    win.setCentralWidget(central);

    // ----- Left sidebar: store list (circle icons) + settings at bottom -----
    const int sidebarWidth = 64;  // 40px circles + inner padding (8 each side) without clipping
    auto *sidebar = new QFrame(central);
    sidebar->setObjectName("sidebar");
    sidebar->setFixedWidth(sidebarWidth);
    sidebar->setFrameShape(QFrame::NoFrame);
    sidebar->setStyleSheet("#sidebar { background-color: palette(mid); }");
    auto *sidebarLayout = new QVBoxLayout(sidebar);
    sidebarLayout->setContentsMargins(8, 8, 8, 8);
    sidebarLayout->setSpacing(4);

    auto *storeListWidget = new QWidget(sidebar);
    auto *storeListLayout = new QVBoxLayout(storeListWidget);
    storeListLayout->setContentsMargins(0, 8, 0, 0);
    storeListLayout->setSpacing(4);
    storeListLayout->setAlignment(Qt::AlignTop | Qt::AlignHCenter);
    sidebarLayout->addWidget(storeListWidget, 1);

    auto *settingsBtn = new QToolButton(sidebar);
    settingsBtn->setObjectName("settingsBtn");
    settingsBtn->setToolTip(TR("settings.tooltip"));
    static const int circleIconPx = 28;  // same as compose quill so style scales both identically
    QIcon cogIcon = iconFromSvgResource(QStringLiteral(":/icons/cog.svg"),
                                        QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!cogIcon.isNull())
        settingsBtn->setIcon(cogIcon);
    else
        settingsBtn->setText(QString::fromUtf8("⚙"));
    settingsBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    settingsBtn->setFixedSize(40, 40);
    settingsBtn->setCheckable(true);
    settingsBtn->setStyleSheet(
        "QToolButton#settingsBtn { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton#settingsBtn:hover { background-color: palette(light); }"
        "QToolButton#settingsBtn:checked { background-color: #6b6b6b; color: white; }"
    );
    sidebarLayout->addWidget(settingsBtn, 0, Qt::AlignHCenter);

    mainLayout->addWidget(sidebar);

    // ----- Right: stacked main content | settings overlay -----
    auto *rightStack = new QStackedWidget(central);
    auto *mainContentPage = new QWidget(central);
    auto *mainContentLayout = new QVBoxLayout(mainContentPage);
    mainContentLayout->setContentsMargins(0, 0, 0, 0);

    // Toolbar (top right of sidebar): round icon buttons
    auto *toolbar = new QFrame(mainContentPage);
    toolbar->setFixedHeight(48);
    toolbar->setStyleSheet("QFrame { background-color: palette(base); border-bottom: 1px solid palette(mid); }");
    auto *toolbarLayout = new QHBoxLayout(toolbar);
    toolbarLayout->setContentsMargins(12, 0, 12, 0);
    toolbarLayout->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);

    auto *composeBtn = new QToolButton(toolbar);
    composeBtn->setToolTip(TR("compose.tooltip"));
    QIcon quillIcon = iconFromSvgResource(QStringLiteral(":/icons/quill.svg"),
                                          QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!quillIcon.isNull())
        composeBtn->setIcon(quillIcon);
    composeBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    composeBtn->setFixedSize(40, 40);
    composeBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    composeBtn->setEnabled(false);
    toolbarLayout->addWidget(composeBtn);

    auto *appendMessageBtn = new QToolButton(toolbar);
    appendMessageBtn->setToolTip(TR("append_message.tooltip"));
    QIcon plusIcon = iconFromSvgResource(QStringLiteral(":/icons/plus.svg"),
                                         QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!plusIcon.isNull())
        appendMessageBtn->setIcon(plusIcon);
    else
        appendMessageBtn->setText(QStringLiteral("+"));
    appendMessageBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    appendMessageBtn->setFixedSize(40, 40);
    appendMessageBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    appendMessageBtn->setEnabled(false);
    appendMessageBtn->setVisible(false);  // shown only when no transport (file-based store)
    toolbarLayout->addWidget(appendMessageBtn);

    mainContentLayout->addWidget(toolbar);

    // Content: folder list | message list | message detail
    auto *contentSplitter = new QSplitter(Qt::Horizontal, mainContentPage);
    auto *folderListPanel = new QWidget(mainContentPage);
    auto *folderListPanelLayout = new QVBoxLayout(folderListPanel);
    folderListPanelLayout->setContentsMargins(8, 8, 0, 8);
    auto *folderList = new QListWidget(folderListPanel);
    folderList->setSelectionMode(QAbstractItemView::SingleSelection);
    folderListPanelLayout->addWidget(folderList);

    auto *rightSplitter = new QSplitter(Qt::Vertical, mainContentPage);
    auto *conversationList = new QTreeWidget(mainContentPage);
    conversationList->setColumnCount(3);
    conversationList->setHeaderLabels({ TR("message.from_column"), TR("message.subject_column"), TR("message.date_column") });
    conversationList->setSelectionMode(QAbstractItemView::SingleSelection);
    conversationList->setSortingEnabled(true);
    conversationList->setRootIsDecorated(false);  // flat for now; set true when thread hierarchy is added
    auto *messageView = new QTextBrowser(mainContentPage);
    messageView->setOpenExternalLinks(true);
    rightSplitter->addWidget(conversationList);
    rightSplitter->addWidget(messageView);
    rightSplitter->setStretchFactor(0, 0);
    rightSplitter->setStretchFactor(1, 1);

    contentSplitter->addWidget(folderListPanel);
    contentSplitter->addWidget(rightSplitter);
    contentSplitter->setStretchFactor(0, 0);
    contentSplitter->setStretchFactor(1, 1);
    contentSplitter->setSizes({ 180, 400 });
    mainContentLayout->addWidget(contentSplitter);

    rightStack->addWidget(mainContentPage);

    // Settings: tabbed pane (Accounts, Junk Mail, Viewing, Composing, Signatures, About)
    auto *settingsPage = new QWidget(central);
    auto *settingsLayout = new QVBoxLayout(settingsPage);
    settingsLayout->setContentsMargins(0, 0, 0, 0);
    auto *settingsTabs = new QTabWidget(settingsPage);
    QString editingStoreId;  // when set, save updates this account; when empty, save creates new
    // Accounts: stacked = (0) type list → (1) edit account panel
    auto *accountsStack = new QStackedWidget(settingsPage);
    // Page 0: select account to edit (if any) + create new account type buttons
    auto *accountsListPage = new QWidget(settingsPage);
    auto *accountsListLayout = new QVBoxLayout(accountsListPage);
    auto *selectLabel = new QLabel(TR("accounts.select_to_edit"), accountsListPage);
    selectLabel->setAlignment(Qt::AlignCenter);
    accountsListLayout->addWidget(selectLabel);
    auto *accountButtonsContainer = new QWidget(accountsListPage);
    auto *accountButtonsGrid = new QGridLayout(accountButtonsContainer);
    accountButtonsGrid->setAlignment(Qt::AlignCenter);
    accountButtonsContainer->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Minimum);
    accountsListLayout->addWidget(accountButtonsContainer);
    auto *createLabel = new QLabel(TR("accounts.create_new"), accountsListPage);
    createLabel->setAlignment(Qt::AlignCenter);
    accountsListLayout->addWidget(createLabel);
    struct AccountTypeBtn { QPushButton *btn; QString type; };
    QList<AccountTypeBtn> accountTypeButtons;
    struct { const char *key; bool enabled; } accountTypes[] = {
        { "accounts.type.imap", true },
        { "accounts.type.pop3", true },
        { "accounts.type.maildir", true },
        { "accounts.type.mbox", true },
        { "accounts.type.nostr", true },
        { "accounts.type.matrix", true },
        { "accounts.type.nntp", false }
    };
    const int accountButtonsPerRow = 6;
    QHBoxLayout *typeButtonsRow = new QHBoxLayout();
    typeButtonsRow->setAlignment(Qt::AlignCenter);
    for (const auto &p : accountTypes) {
        auto *b = new QPushButton(TR(p.key), accountsListPage);
        b->setEnabled(p.enabled);
        b->setMinimumWidth(120);
        accountTypeButtons.append({ b, QString::fromUtf8(p.key) });
        typeButtonsRow->addWidget(b, 0, Qt::AlignCenter);
    }
    accountsListLayout->addLayout(typeButtonsRow);
    accountsListLayout->addStretch();
    accountsStack->addWidget(accountsListPage);

    // Page 1: edit account panel (form for the selected type)
    auto *accountsEditPage = new QWidget(settingsPage);
    auto *accountsEditLayout = new QVBoxLayout(accountsEditPage);
    auto *accountFormStack = new QStackedWidget(accountsEditPage);
    // Maildir form
    auto *maildirForm = new QWidget(accountsEditPage);
    auto *maildirFormLayout = new QFormLayout(maildirForm);
    auto *maildirPathEdit = new QLineEdit(maildirForm);
    maildirPathEdit->setPlaceholderText(TR("maildir.placeholder.path"));
    auto *maildirBrowseBtn = new QPushButton(TR("common.browse"), maildirForm);
    auto *maildirPathRow = new QHBoxLayout();
    maildirPathRow->addWidget(maildirPathEdit);
    maildirPathRow->addWidget(maildirBrowseBtn);
    maildirFormLayout->addRow(TR("maildir.directory") + QStringLiteral(":"), maildirPathRow);
    auto *maildirDisplayNameEdit = new QLineEdit(maildirForm);
    maildirDisplayNameEdit->setPlaceholderText(TR("maildir.placeholder.display_name"));
    maildirFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), maildirDisplayNameEdit);
    auto *maildirSaveBtn = new QPushButton(TR("common.save"), maildirForm);
    accountFormStack->addWidget(maildirForm);
    // mbox form
    auto *mboxForm = new QWidget(accountsEditPage);
    auto *mboxFormLayout = new QFormLayout(mboxForm);
    auto *mboxPathEdit = new QLineEdit(mboxForm);
    auto *mboxBrowseBtn = new QPushButton(TR("common.browse"), mboxForm);
    auto *mboxPathRow = new QHBoxLayout();
    mboxPathRow->addWidget(mboxPathEdit);
    mboxPathRow->addWidget(mboxBrowseBtn);
    mboxFormLayout->addRow(TR("mbox.file") + QStringLiteral(":"), mboxPathRow);
    auto *mboxDisplayNameEdit = new QLineEdit(mboxForm);
    mboxFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), mboxDisplayNameEdit);
    auto *mboxSaveBtn = new QPushButton(TR("common.save"), mboxForm);
    accountFormStack->addWidget(mboxForm);
    // IMAP form (includes SMTP section for same account)
    auto *imapForm = new QWidget(accountsEditPage);
    auto *imapFormLayout = new QFormLayout(imapForm);
    auto *imapDisplayNameEdit = new QLineEdit(imapForm);
    imapDisplayNameEdit->setPlaceholderText(TR("imap.placeholder.display_name"));
    imapFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), imapDisplayNameEdit);
    auto *imapEmailEdit = new QLineEdit(imapForm);
    imapEmailEdit->setPlaceholderText(TR("imap.placeholder.email"));
    imapFormLayout->addRow(TR("imap.email") + QStringLiteral(":"), imapEmailEdit);
    auto *imapHostEdit = new QLineEdit(imapForm);
    imapHostEdit->setPlaceholderText(TR("imap.placeholder.host"));
    imapFormLayout->addRow(TR("imap.host") + QStringLiteral(":"), imapHostEdit);
    auto *imapSecurityCombo = new QComboBox(imapForm);
    imapSecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.starttls"), TR("imap.security.ssl") });
    imapSecurityCombo->setCurrentIndex(2);
    imapFormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), imapSecurityCombo);
    auto *imapPortSpin = new QSpinBox(imapForm);
    imapPortSpin->setRange(1, 65535);
    imapPortSpin->setValue(993);
    imapFormLayout->addRow(TR("imap.port") + QStringLiteral(":"), imapPortSpin);
    auto *imapUserEdit = new QLineEdit(imapForm);
    imapFormLayout->addRow(TR("imap.username") + QStringLiteral(":"), imapUserEdit);
    auto *imapPollCombo = new QComboBox(imapForm);
    imapPollCombo->addItems({ TR("imap.poll.every_minute"), TR("imap.poll.every_5_minutes"), TR("imap.poll.every_10_minutes"), TR("imap.poll.every_hour") });
    imapFormLayout->addRow(TR("imap.poll.label") + QStringLiteral(":"), imapPollCombo);
    auto *imapDeletionCombo = new QComboBox(imapForm);
    imapDeletionCombo->addItems({ TR("imap.deletion.mark_expunge"), TR("imap.deletion.move_to_trash") });
    imapFormLayout->addRow(TR("imap.deletion.label") + QStringLiteral(":"), imapDeletionCombo);
    auto *imapTrashFolderEdit = new QLineEdit(imapForm);
    imapTrashFolderEdit->setPlaceholderText(TR("imap.placeholder.trash_folder"));
    imapFormLayout->addRow(TR("imap.trash_folder") + QStringLiteral(":"), imapTrashFolderEdit);
    auto *imapIdleCombo = new QComboBox(imapForm);
    imapIdleCombo->addItems({ TR("imap.idle.30_seconds"), TR("imap.idle.1_minute"), TR("imap.idle.5_minutes") });
    imapFormLayout->addRow(TR("imap.idle.label") + QStringLiteral(":"), imapIdleCombo);
    auto *smtpLabel = new QLabel(TR("imap.smtp_section"));
    imapFormLayout->addRow(smtpLabel);
    auto *smtpHostEdit = new QLineEdit(imapForm);
    smtpHostEdit->setPlaceholderText(TR("smtp.placeholder.host"));
    imapFormLayout->addRow(TR("smtp.host") + QStringLiteral(":"), smtpHostEdit);
    auto *smtpSecurityCombo = new QComboBox(imapForm);
    smtpSecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.starttls"), TR("imap.security.ssl") });
    smtpSecurityCombo->setCurrentIndex(1);
    imapFormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), smtpSecurityCombo);
    auto *smtpPortSpin = new QSpinBox(imapForm);
    smtpPortSpin->setRange(1, 65535);
    smtpPortSpin->setValue(586);
    imapFormLayout->addRow(TR("smtp.port") + QStringLiteral(":"), smtpPortSpin);
    auto *smtpUserEdit = new QLineEdit(imapForm);
    imapFormLayout->addRow(TR("smtp.username") + QStringLiteral(":"), smtpUserEdit);
    /* SMTP password: not in form; obtained via credential callback when sending (AUTH_PLAN). */
    auto *imapSaveBtn = new QPushButton(TR("common.save"), imapForm);
    accountFormStack->addWidget(imapForm);

    // Security dropdown -> default port (IMAP: None/STARTTLS 143, SSL 993; SMTP: None/STARTTLS 586, SSL 465)
    QObject::connect(imapSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [imapPortSpin](int idx) {
        if (idx == 2) {
            imapPortSpin->setValue(993);
        } else {
            imapPortSpin->setValue(143);
        }
    });
    QObject::connect(smtpSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [smtpPortSpin](int idx) {
        if (idx == 2) {
            smtpPortSpin->setValue(465);
        } else {
            smtpPortSpin->setValue(586);
        }
    });
    // POP3 form
    auto *pop3Form = new QWidget(accountsEditPage);
    auto *pop3FormLayout = new QFormLayout(pop3Form);
    auto *pop3DisplayNameEdit = new QLineEdit(pop3Form);
    pop3DisplayNameEdit->setPlaceholderText(TR("imap.placeholder.display_name"));
    pop3FormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), pop3DisplayNameEdit);
    auto *pop3EmailEdit = new QLineEdit(pop3Form);
    pop3EmailEdit->setPlaceholderText(TR("imap.placeholder.email"));
    pop3FormLayout->addRow(TR("imap.email") + QStringLiteral(":"), pop3EmailEdit);
    auto *pop3HostEdit = new QLineEdit(pop3Form);
    pop3HostEdit->setPlaceholderText(TR("imap.placeholder.host"));
    pop3FormLayout->addRow(TR("imap.host") + QStringLiteral(":"), pop3HostEdit);
    auto *pop3SecurityCombo = new QComboBox(pop3Form);
    pop3SecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.ssl") });
    pop3SecurityCombo->setCurrentIndex(1);
    pop3FormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), pop3SecurityCombo);
    auto *pop3PortSpin = new QSpinBox(pop3Form);
    pop3PortSpin->setRange(1, 65535);
    pop3PortSpin->setValue(995);
    pop3FormLayout->addRow(TR("imap.port") + QStringLiteral(":"), pop3PortSpin);
    auto *pop3UserEdit = new QLineEdit(pop3Form);
    pop3FormLayout->addRow(TR("imap.username") + QStringLiteral(":"), pop3UserEdit);
    auto *pop3SmtpLabel = new QLabel(TR("imap.smtp_section"), pop3Form);
    pop3FormLayout->addRow(pop3SmtpLabel);
    auto *pop3SmtpHostEdit = new QLineEdit(pop3Form);
    pop3SmtpHostEdit->setPlaceholderText(TR("smtp.placeholder.host"));
    pop3FormLayout->addRow(TR("smtp.host") + QStringLiteral(":"), pop3SmtpHostEdit);
    auto *pop3SmtpSecurityCombo = new QComboBox(pop3Form);
    pop3SmtpSecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.starttls"), TR("imap.security.ssl") });
    pop3SmtpSecurityCombo->setCurrentIndex(1);
    pop3FormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), pop3SmtpSecurityCombo);
    auto *pop3SmtpPortSpin = new QSpinBox(pop3Form);
    pop3SmtpPortSpin->setRange(1, 65535);
    pop3SmtpPortSpin->setValue(586);
    pop3FormLayout->addRow(TR("smtp.port") + QStringLiteral(":"), pop3SmtpPortSpin);
    auto *pop3SmtpUserEdit = new QLineEdit(pop3Form);
    pop3FormLayout->addRow(TR("smtp.username") + QStringLiteral(":"), pop3SmtpUserEdit);
    auto *pop3SaveBtn = new QPushButton(TR("common.save"), pop3Form);
    QObject::connect(pop3SecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [pop3PortSpin](int idx) {
        if (idx == 1) {
            pop3PortSpin->setValue(995);
        } else {
            pop3PortSpin->setValue(110);
        }
    });
    QObject::connect(pop3SmtpSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [pop3SmtpPortSpin](int idx) {
        if (idx == 2) {
            pop3SmtpPortSpin->setValue(465);
        } else {
            pop3SmtpPortSpin->setValue(586);
        }
    });
    accountFormStack->addWidget(pop3Form);
    // Nostr form: display name, nip05, keys (npub/nsec or hex), key file, relay list
    auto *nostrForm = new QWidget(accountsEditPage);
    auto *nostrMainLayout = new QVBoxLayout(nostrForm);
    nostrMainLayout->setContentsMargins(0, 0, 0, 0);
    auto *nostrFormLayout = new QFormLayout();
    auto *nostrDisplayNameEdit = new QLineEdit(nostrForm);
    nostrDisplayNameEdit->setPlaceholderText(TR("nostr.placeholder.display_name"));
    nostrFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), nostrDisplayNameEdit);
    auto *nostrNip05Edit = new QLineEdit(nostrForm);
    nostrNip05Edit->setPlaceholderText(TR("nostr.placeholder.nip05"));
    nostrFormLayout->addRow(TR("nostr.nip05") + QStringLiteral(":"), nostrNip05Edit);
    auto *nostrPubkeyEdit = new QLineEdit(nostrForm);
    nostrPubkeyEdit->setPlaceholderText(TR("nostr.placeholder.pubkey"));
    nostrFormLayout->addRow(TR("nostr.pubkey") + QStringLiteral(":"), nostrPubkeyEdit);
    auto *nostrSecretKeyEdit = new QLineEdit(nostrForm);
    nostrSecretKeyEdit->setEchoMode(QLineEdit::Password);
    nostrSecretKeyEdit->setPlaceholderText(TR("nostr.placeholder.secret_key"));
    nostrFormLayout->addRow(TR("nostr.secret_key") + QStringLiteral(":"), nostrSecretKeyEdit);
    auto *nostrKeyPathEdit = new QLineEdit(nostrForm);
    auto *nostrKeyBrowseBtn = new QPushButton(TR("common.browse"), nostrForm);
    auto *nostrKeyRow = new QHBoxLayout();
    nostrKeyRow->addWidget(nostrKeyPathEdit);
    nostrKeyRow->addWidget(nostrKeyBrowseBtn);
    nostrFormLayout->addRow(TR("nostr.key_path") + QStringLiteral(":"), nostrKeyRow);
    nostrMainLayout->addLayout(nostrFormLayout);
    auto *nostrRelaysLabel = new QLabel(TR("nostr.relays") + QStringLiteral(":"), nostrForm);
    nostrMainLayout->addWidget(nostrRelaysLabel);
    auto *nostrRelayList = new QListWidget(nostrForm);
    nostrRelayList->setMinimumHeight(80);
    nostrMainLayout->addWidget(nostrRelayList);
    auto *nostrRelayAddRow = new QHBoxLayout();
    auto *nostrRelayUrlEdit = new QLineEdit(nostrForm);
    nostrRelayUrlEdit->setPlaceholderText(TR("nostr.placeholder.relay_url"));
    auto *nostrRelayAddBtn = new QPushButton(TR("nostr.add_relay"), nostrForm);
    nostrRelayAddRow->addWidget(nostrRelayUrlEdit);
    nostrRelayAddRow->addWidget(nostrRelayAddBtn);
    nostrMainLayout->addLayout(nostrRelayAddRow);
    auto *nostrRelayRemoveBtn = new QPushButton(TR("nostr.remove_relay"), nostrForm);
    nostrMainLayout->addWidget(nostrRelayRemoveBtn);
    auto *nostrSaveBtn = new QPushButton(TR("common.save"), accountsEditPage);
    nostrSaveBtn->setVisible(false);
    accountFormStack->addWidget(nostrForm);
    // Matrix form
    auto *matrixForm = new QWidget(accountsEditPage);
    auto *matrixFormLayout = new QFormLayout(matrixForm);
    auto *matrixHomeserverEdit = new QLineEdit(matrixForm);
    matrixHomeserverEdit->setPlaceholderText(TR("matrix.placeholder.homeserver"));
    matrixFormLayout->addRow(TR("matrix.homeserver") + QStringLiteral(":"), matrixHomeserverEdit);
    auto *matrixUserIdEdit = new QLineEdit(matrixForm);
    matrixUserIdEdit->setPlaceholderText(TR("matrix.placeholder.user_id"));
    matrixFormLayout->addRow(TR("matrix.user_id") + QStringLiteral(":"), matrixUserIdEdit);
    auto *matrixTokenEdit = new QLineEdit(matrixForm);
    matrixTokenEdit->setEchoMode(QLineEdit::Password);
    matrixTokenEdit->setPlaceholderText(TR("matrix.placeholder.token"));
    matrixFormLayout->addRow(TR("matrix.access_token") + QStringLiteral(":"), matrixTokenEdit);
    auto *matrixDisplayNameEdit = new QLineEdit(matrixForm);
    matrixDisplayNameEdit->setPlaceholderText(TR("matrix.placeholder.display_name"));
    matrixFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), matrixDisplayNameEdit);
    auto *matrixSaveBtn = new QPushButton(TR("common.save"), accountsEditPage);
    matrixSaveBtn->setVisible(false);  // use shared Save at bottom; this is only for programmatic click()
    accountFormStack->addWidget(matrixForm);
    // Placeholder for NNTP
    accountFormStack->addWidget(new QLabel(TR("accounts.not_implemented"), accountsEditPage));
    accountsEditLayout->addWidget(accountFormStack);
    // Bottom button row: Delete (left, when editing), Save and Cancel (right)
    auto *accountEditButtonRow = new QWidget(accountsEditPage);
    auto *accountEditButtonLayout = new QHBoxLayout(accountEditButtonRow);
    accountEditButtonLayout->setContentsMargins(0, 12, 0, 0);
    auto *accountDeleteBtn = new QPushButton(TR("accounts.delete"), accountEditButtonRow);
    accountDeleteBtn->setVisible(false);
    auto *accountSaveBtn = new QPushButton(TR("common.save"), accountEditButtonRow);
    auto *accountCancelBtn = new QPushButton(TR("common.cancel"), accountEditButtonRow);
    accountEditButtonLayout->addWidget(accountDeleteBtn, 0, Qt::AlignLeft);
    accountEditButtonLayout->addStretch(1);
    accountEditButtonLayout->addWidget(accountSaveBtn);
    accountEditButtonLayout->addWidget(accountCancelBtn);
    accountsEditLayout->addWidget(accountEditButtonRow);
    accountsStack->addWidget(accountsEditPage);

    auto refreshAccountListInSettings = [&]() {
        while (QLayoutItem *item = accountButtonsGrid->takeAt(0)) {
            if (item->widget()) {
                item->widget()->deleteLater();
            }
            delete item;
        }
        Config c = loadConfig();
        const int cols = accountButtonsPerRow;
        for (int i = 0; i < c.stores.size(); ++i) {
            const StoreEntry &entry = c.stores[i];
            QString initial = entry.displayName.left(1).toUpper();
            if (initial.isEmpty()) {
                if (entry.type == QLatin1String("maildir")) {
                    initial = QStringLiteral("M");
                } else if (entry.type == QLatin1String("imap")) {
                    initial = QStringLiteral("I");
                } else if (entry.type == QLatin1String("nostr")) {
                    initial = QStringLiteral("N");
                } else if (entry.type == QLatin1String("matrix")) {
                    initial = QStringLiteral("X");
                } else {
                    initial = QStringLiteral("?");
                }
            }
            auto *btn = new QToolButton(accountButtonsContainer);
            btn->setProperty("storeId", entry.id);
            btn->setText(initial);
            btn->setFixedSize(40, 40);
            btn->setToolTip(entry.displayName);
            QFont f = btn->font();
            f.setPointSize(20);
            f.setWeight(QFont::Bold);
            btn->setFont(f);
            btn->setStyleSheet(storeCircleStyleSheet(i));
            accountButtonsGrid->addWidget(btn, i / cols, i % cols, 1, 1, Qt::AlignCenter);
            StoreEntry entryCopy = entry;
            QObject::connect(btn, &QToolButton::clicked, [&, entryCopy, accountDeleteBtn]() {
                editingStoreId = entryCopy.id;
                accountDeleteBtn->setVisible(true);
                if (entryCopy.type == QLatin1String("maildir")) {
                    accountFormStack->setCurrentIndex(0);
                    maildirPathEdit->setText(storeHostOrPath(entryCopy));
                    maildirDisplayNameEdit->setText(entryCopy.displayName);
                } else if (entryCopy.type == QLatin1String("imap")) {
                    accountFormStack->setCurrentIndex(2);
                    imapDisplayNameEdit->setText(entryCopy.displayName);
                    imapEmailEdit->setText(entryCopy.emailAddress);
                    imapHostEdit->setText(storeHostOrPath(entryCopy));
                    imapSecurityCombo->setCurrentIndex(qBound(0, paramInt(entryCopy, "security", 2), 2));
                    imapPortSpin->setValue(paramInt(entryCopy, "port", 993));
                    imapUserEdit->setText(param(entryCopy, "username"));
                    int pollSec = paramInt(entryCopy, "imapPollSeconds", 300);
                    int pollIdx = (pollSec <= 60) ? 0 : (pollSec <= 300) ? 1 : (pollSec <= 600) ? 2 : 3;
                    imapPollCombo->setCurrentIndex(pollIdx);
                    imapDeletionCombo->setCurrentIndex(paramInt(entryCopy, "imapDeletion", 0) == 1 ? 1 : 0);
                    imapTrashFolderEdit->setText(param(entryCopy, "imapTrashFolder"));
                    int idleSec = paramInt(entryCopy, "imapIdleSeconds", 60);
                    int idleIdx = (idleSec <= 30) ? 0 : (idleSec <= 60) ? 1 : 2;
                    imapIdleCombo->setCurrentIndex(idleIdx);
                    smtpHostEdit->setText(param(entryCopy, "transportHostname"));
                    smtpSecurityCombo->setCurrentIndex(qBound(0, paramInt(entryCopy, "transportSecurity", 1), 2));
                    smtpPortSpin->setValue(paramInt(entryCopy, "transportPort", 586));
                    smtpUserEdit->setText(param(entryCopy, "transportUsername"));
                } else if (entryCopy.type == QLatin1String("pop3")) {
                    accountFormStack->setCurrentIndex(3);
                    pop3DisplayNameEdit->setText(entryCopy.displayName);
                    pop3EmailEdit->setText(entryCopy.emailAddress);
                    pop3HostEdit->setText(storeHostOrPath(entryCopy));
                    pop3PortSpin->setValue(995);
                    pop3UserEdit->setText(param(entryCopy, "username"));
                    pop3SmtpHostEdit->setText(param(entryCopy, "transportHostname"));
                    pop3SmtpPortSpin->setValue(paramInt(entryCopy, "transportPort", 586));
                    pop3SmtpUserEdit->setText(param(entryCopy, "transportUsername"));
                } else if (entryCopy.type == QLatin1String("nostr")) {
                    accountFormStack->setCurrentIndex(4);
                    nostrDisplayNameEdit->setText(entryCopy.displayName);
                    nostrNip05Edit->setText(entryCopy.emailAddress);
                    nostrPubkeyEdit->clear();
                    nostrSecretKeyEdit->clear();
                    nostrKeyPathEdit->setText(param(entryCopy, "keyPath"));
                    nostrRelayList->clear();
                    const QString pathVal = storeHostOrPath(entryCopy);
                    const QStringList relayUrls = pathVal.split(QRegularExpression(QStringLiteral("[,\\n]")), Qt::SkipEmptyParts);
                    for (const QString &u : relayUrls) {
                        QString t = u.trimmed();
                        if (!t.isEmpty())
                            nostrRelayList->addItem(t);
                    }
                } else if (entryCopy.type == QLatin1String("matrix")) {
                    accountFormStack->setCurrentIndex(5);
                    matrixHomeserverEdit->setText(storeHostOrPath(entryCopy));
                    matrixUserIdEdit->setText(param(entryCopy, "userId"));
                    matrixTokenEdit->setText(param(entryCopy, "accessToken"));
                    matrixDisplayNameEdit->setText(entryCopy.displayName);
                }
                accountsStack->setCurrentIndex(1);
            });
        }
    };

    settingsTabs->addTab(accountsStack, TR("settings.rubric.accounts"));
    const char *placeholderKeys[] = { "settings.placeholder.junk_mail", "settings.placeholder.composing", "settings.placeholder.signatures" };
    const char *tabKeys[] = { "settings.rubric.junk_mail", "settings.rubric.viewing", "settings.rubric.composing", "settings.rubric.signatures" };
    // Viewing tab: date format for message list (locale-dependent)
    auto *viewingPage = new QWidget(settingsPage);
    auto *viewingLayout = new QFormLayout(viewingPage);
    viewingLayout->setContentsMargins(24, 24, 24, 24);
    auto *dateFormatCombo = new QComboBox(viewingPage);
    const struct { const char *trKey; QString format; } dateFormatOptions[] = {
        { "viewing.date_format.locale_default", QString() },
        { "viewing.date_format.d_mmm_yyyy_hh_mm", QStringLiteral("d MMM yyyy HH:mm") },
        { "viewing.date_format.dd_mm_yy", QStringLiteral("dd/MM/yy") },
        { "viewing.date_format.iso", QStringLiteral("yyyy-MM-dd HH:mm") },
    };
    for (const auto &opt : dateFormatOptions) {
        dateFormatCombo->addItem(TR(opt.trKey), opt.format);
    }
    int dateFormatIdx = 0;
    QString savedDateFormat = loadConfig().dateFormat;
    for (int i = 0; i < dateFormatCombo->count(); ++i) {
        if (dateFormatCombo->itemData(i).toString() == savedDateFormat) {
            dateFormatIdx = i;
            break;
        }
    }
    dateFormatCombo->setCurrentIndex(dateFormatIdx);
    viewingLayout->addRow(TR("viewing.date_format") + QStringLiteral(":"), dateFormatCombo);
    settingsTabs->addTab(viewingPage, TR("settings.rubric.viewing"));
    QObject::connect(dateFormatCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [dateFormatCombo](int) {
        Config c = loadConfig();
        c.dateFormat = dateFormatCombo->currentData().toString();
        saveConfig(c);
    });
    for (int i = 0; i < 3; ++i) {
        auto *p = new QLabel(TR(placeholderKeys[i]), settingsPage);
        p->setAlignment(Qt::AlignTop | Qt::AlignLeft);
        p->setContentsMargins(24, 24, 24, 24);
        settingsTabs->addTab(p, TR(tabKeys[i]));
    }
    // About pane
    auto *aboutPage = new QWidget(settingsPage);
    auto *aboutLayout = new QVBoxLayout(aboutPage);
    aboutLayout->setSpacing(12);
    aboutLayout->addSpacing(16);
    auto *aboutNameLabel = new QLabel(TR("app.name"), aboutPage);
    QFont nameFont = aboutNameLabel->font();
    nameFont.setPointSize(nameFont.pointSize() + 4);
    nameFont.setWeight(QFont::Bold);
    aboutNameLabel->setFont(nameFont);
    aboutLayout->addWidget(aboutNameLabel, 0, Qt::AlignHCenter);
    auto *aboutVersionLabel = new QLabel(TR("about.version").arg(QString::fromUtf8(version)), aboutPage);
    aboutLayout->addWidget(aboutVersionLabel, 0, Qt::AlignHCenter);
    auto *aboutCopyrightLabel = new QLabel(TR("about.copyright"), aboutPage);
    aboutLayout->addWidget(aboutCopyrightLabel, 0, Qt::AlignHCenter);
    auto *aboutLicenceLabel = new QLabel(TR("about.licence"), aboutPage);
    aboutLicenceLabel->setWordWrap(true);
    aboutLayout->addWidget(aboutLicenceLabel, 0, Qt::AlignHCenter);
    aboutLayout->addStretch();
    settingsTabs->addTab(aboutPage, TR("settings.rubric.about"));
    QObject::connect(settingsTabs, &QTabWidget::currentChanged, [&](int index) {
        if (index == 0) {
            refreshAccountListInSettings();
        } else if (index == 2) {
            QString fmt = loadConfig().dateFormat;
            for (int i = 0; i < dateFormatCombo->count(); ++i) {
                if (dateFormatCombo->itemData(i).toString() == fmt) {
                    dateFormatCombo->setCurrentIndex(i);
                    break;
                }
            }
        }
    });

    settingsLayout->addWidget(settingsTabs);
    rightStack->addWidget(settingsPage);

    QObject::connect(settingsBtn, &QToolButton::clicked, [&]() {
        if (settingsBtn->isChecked()) {
            rightStack->setCurrentIndex(1);
            refreshAccountListInSettings();
        } else {
            rightStack->setCurrentIndex(0);
        }
    });

    mainLayout->addWidget(rightStack, 1);

    EventBridge bridge;
    bridge.folderList = folderList;
    bridge.conversationList = conversationList;
    bridge.messageView = messageView;
    bridge.statusBar = win.statusBar();
    bridge.win = &win;

    tagliacarte_set_credential_request_callback(on_credential_request_cb, &bridge);
    QObject::connect(&bridge, &EventBridge::credentialRequested, [&win](const QString &storeUriQ, const QString &username, int isPlaintext) {
        QWidget *parent = &win;
        if (isPlaintext != 0) {
            int r = QMessageBox::warning(parent, TR("auth.plaintext_title"), TR("auth.plaintext_warning"),
                QMessageBox::Ok | QMessageBox::Cancel, QMessageBox::Cancel);
            if (r != QMessageBox::Ok) {
                tagliacarte_credential_cancel(storeUriQ.toUtf8().constData());
                return;
            }
        }
        bool ok = false;
        QString password = QInputDialog::getText(parent, TR("auth.password_title"), TR("auth.password_prompt"),
            QLineEdit::Password, username, &ok);
        if (!ok || password.isEmpty()) {
            tagliacarte_credential_cancel(storeUriQ.toUtf8().constData());
            return;
        }
        int ret = tagliacarte_credential_provide(storeUriQ.toUtf8().constData(), password.toUtf8().constData());
        if (ret == 0)
            tagliacarte_store_refresh_folders(storeUriQ.toUtf8().constData());
    });

    QByteArray storeUri;
    QByteArray smtpTransportUri;  // transport for the currently selected store (used for compose)
    QMap<QByteArray, QByteArray> storeToTransport;  // store URI -> transport URI (only IMAP-with-SMTP have one)
    QList<QToolButton *> storeButtons; // circle buttons in sidebar; one per store
    QList<QByteArray> allStoreUris;    // all created store URIs (for multi-store and shutdown)

    // Security tab: credentials storage (keychain vs file)
    auto *securityPage = new QWidget(settingsPage);
    auto *securityLayout = new QVBoxLayout(securityPage);
    securityLayout->setSpacing(12);
    securityLayout->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    securityLayout->setContentsMargins(24, 24, 24, 24);
    auto *useKeychainCheck = new QCheckBox(TR("security.use_keychain"), securityPage);
    useKeychainCheck->setChecked(loadConfig().useKeychain);
    useKeychainCheck->setEnabled(tagliacarte_keychain_available() != 0);
    securityLayout->addWidget(useKeychainCheck);
    securityLayout->addStretch();
    settingsTabs->insertTab(1, securityPage, TR("settings.rubric.security"));
    QObject::connect(settingsTabs, &QTabWidget::currentChanged, [&](int index) {
        if (index == 1) {
            useKeychainCheck->setChecked(loadConfig().useKeychain);
        }
    });

    QObject::connect(useKeychainCheck, &QCheckBox::toggled, [&](bool useKeychain) {
        Config config = loadConfig();
        QString credPath = tagliacarteConfigDir() + QStringLiteral("/credentials");
        QByteArray pathBa = credPath.toUtf8();
        const char *path = pathBa.constData();
        if (useKeychain) {
            if (tagliacarte_migrate_credentials_to_keychain(path) != 0) {
                useKeychainCheck->setChecked(false);
                return;
            }
            config.useKeychain = true;
        } else {
            QList<QByteArray> uris;
            for (const QByteArray &u : allStoreUris)
                uris.append(u);
            for (const QByteArray &t : storeToTransport) {
                if (!uris.contains(t))
                    uris.append(t);
            }
            QVector<const char *> ptrs(uris.size());
            for (int i = 0; i < uris.size(); ++i)
                ptrs[i] = uris.at(i).constData();
            if (tagliacarte_migrate_credentials_to_file(path, ptrs.size(), ptrs.isEmpty() ? nullptr : ptrs.data()) != 0) {
                useKeychainCheck->setChecked(true);
                return;
            }
            config.useKeychain = false;
        }
        saveConfig(config);
        tagliacarte_set_credentials_backend(config.useKeychain ? 1 : 0);
    });

    auto updateComposeAppendButtons = [&]() {
        bool hasTransport = !smtpTransportUri.isEmpty();
        composeBtn->setVisible(hasTransport);
        composeBtn->setEnabled(hasTransport);
        appendMessageBtn->setVisible(!hasTransport);
        appendMessageBtn->setEnabled(!hasTransport && !bridge.folderUri().isEmpty());
    };

    auto addStoreCircle = [&](const QString &initial, const QByteArray &uri, int colourIndex) {
        auto *btn = new QToolButton(storeListWidget);
        btn->setProperty("storeUri", uri);
        btn->setProperty("colourIndex", colourIndex);
        btn->setText(initial);
        btn->setFixedSize(40, 40);
        btn->setToolTip(initial);
        QFont storeFont = btn->font();
        storeFont.setPointSize(20);
        storeFont.setWeight(QFont::Bold);
        btn->setFont(storeFont);
        btn->setStyleSheet(storeCircleStyleSheet(colourIndex));
        btn->setCheckable(true);
        storeListLayout->addWidget(btn, 0, Qt::AlignHCenter);
        storeButtons.append(btn);
        QObject::connect(btn, &QToolButton::clicked, [&, btn]() {
            QVariant v = btn->property("storeUri");
            if (!v.isValid()) {
                return;
            }
            QByteArray u = v.toByteArray();
            if (u.isEmpty()) {
                return;
            }
            storeUri = u;
            smtpTransportUri = storeToTransport.value(storeUri);
            updateComposeAppendButtons();
            bridge.clearFolder();
            folderList->clear();
            conversationList->clear();
            messageView->clear();
            for (auto *b : storeButtons) {
                b->setChecked(b == btn);
            }
            tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
            tagliacarte_store_refresh_folders(storeUri.constData());
            win.statusBar()->showMessage(TR("status.folders_loaded"));
        });
        if (storeButtons.size() == 1)
            btn->setChecked(true);
    };

    auto refreshStoresFromConfig = [&]() {
        for (const QByteArray &u : allStoreUris) {
            tagliacarte_store_free(u.constData());
        }
        for (const QByteArray &t : storeToTransport) {
            tagliacarte_transport_free(t.constData());
        }
        allStoreUris.clear();
        storeToTransport.clear();
        for (QToolButton *b : storeButtons) {
            b->deleteLater();
        }
        storeButtons.clear();
        bridge.clearFolder();
        folderList->clear();
        conversationList->clear();
        messageView->clear();
        Config c = loadConfig();
        for (int i = 0; i < c.stores.size(); ++i) {
            const StoreEntry &entry = c.stores[i];
            QByteArray uri;
            if (entry.type == QLatin1String("maildir") && !storeHostOrPath(entry).isEmpty()) {
                char *uriPtr = tagliacarte_store_maildir_new(storeHostOrPath(entry).toUtf8().constData());
                if (!uriPtr) {
                    continue;
                }
                uri = QByteArray(uriPtr);
                tagliacarte_free_string(uriPtr);
            } else if (entry.type == QLatin1String("imap") && !storeHostOrPath(entry).isEmpty()) {
                QString host = storeHostOrPath(entry);
                QString userAtHost = param(entry, "username");
                if (userAtHost.isEmpty()) userAtHost = host;
                if (!userAtHost.contains(QLatin1Char('@')))
                    userAtHost = userAtHost + QLatin1Char('@') + host;
                const uint16_t imapPort = static_cast<uint16_t>(qBound(1, paramInt(entry, "port", 993), 65535));
                char *uriPtr = tagliacarte_store_imap_new(userAtHost.toUtf8().constData(), host.toUtf8().constData(), imapPort);
                if (!uriPtr) {
                    continue;
                }
                uri = QByteArray(uriPtr);
                tagliacarte_free_string(uriPtr);
                if (!param(entry, "transportHostname").isEmpty()) {
                    char *tUri = tagliacarte_transport_smtp_new(param(entry, "transportHostname").toUtf8().constData(), static_cast<uint16_t>(qBound(1, paramInt(entry, "transportPort", 586), 65535)));
                    if (tUri) {
                        storeToTransport[uri] = QByteArray(tUri);
                        tagliacarte_free_string(tUri);
                    }
                }
            } else if (entry.type == QLatin1String("pop3") && !storeHostOrPath(entry).isEmpty()) {
                QString host = storeHostOrPath(entry);
                QString userAtHost = param(entry, "username");
                if (userAtHost.isEmpty()) userAtHost = host;
                if (!userAtHost.contains(QLatin1Char('@')))
                    userAtHost = userAtHost + QLatin1Char('@') + host;
                char *uriPtr = tagliacarte_store_pop3_new(userAtHost.toUtf8().constData(), host.toUtf8().constData(), 995);
                if (!uriPtr) {
                    continue;
                }
                uri = QByteArray(uriPtr);
                tagliacarte_free_string(uriPtr);
                if (!param(entry, "transportHostname").isEmpty()) {
                    char *tUri = tagliacarte_transport_smtp_new(param(entry, "transportHostname").toUtf8().constData(), static_cast<uint16_t>(qBound(1, paramInt(entry, "transportPort", 586), 65535)));
                    if (tUri) {
                        storeToTransport[uri] = QByteArray(tUri);
                        tagliacarte_free_string(tUri);
                    }
                }
            } else if (entry.type == QLatin1String("nostr") && !storeHostOrPath(entry).isEmpty()) {
                const char *kp = param(entry, "keyPath").isEmpty() ? nullptr : param(entry, "keyPath").toUtf8().constData();
                char *uriPtr = tagliacarte_store_nostr_new(storeHostOrPath(entry).toUtf8().constData(), kp);
                if (!uriPtr) {
                    continue;
                }
                uri = QByteArray(uriPtr);
                tagliacarte_free_string(uriPtr);
                char *tUri = tagliacarte_transport_nostr_new(storeHostOrPath(entry).toUtf8().constData(), kp);
                if (tUri) {
                    storeToTransport[uri] = QByteArray(tUri);
                    tagliacarte_free_string(tUri);
                }
            } else if (entry.type == QLatin1String("matrix") && !storeHostOrPath(entry).isEmpty() && !param(entry, "userId").isEmpty()) {
                const char *token = param(entry, "accessToken").isEmpty() ? nullptr : param(entry, "accessToken").toUtf8().constData();
                char *uriPtr = tagliacarte_store_matrix_new(storeHostOrPath(entry).toUtf8().constData(), param(entry, "userId").toUtf8().constData(), token);
                if (!uriPtr) {
                    continue;
                }
                uri = QByteArray(uriPtr);
                tagliacarte_free_string(uriPtr);
                char *tUri = tagliacarte_transport_matrix_new(storeHostOrPath(entry).toUtf8().constData(), param(entry, "userId").toUtf8().constData(), token);
                if (tUri) {
                    storeToTransport[uri] = QByteArray(tUri);
                    tagliacarte_free_string(tUri);
                }
            } else
                continue;
            allStoreUris.append(uri);
            QString initial = entry.displayName.left(1).toUpper();
            if (initial.isEmpty()) {
                if (entry.type == QLatin1String("maildir")) {
                    initial = QStringLiteral("M");
                } else if (entry.type == QLatin1String("imap")) {
                    initial = QStringLiteral("I");
                } else if (entry.type == QLatin1String("pop3")) {
                    initial = QStringLiteral("P");
                } else if (entry.type == QLatin1String("nostr")) {
                    initial = QStringLiteral("N");
                } else if (entry.type == QLatin1String("matrix")) {
                    initial = QStringLiteral("X");
                } else {
                    initial = QStringLiteral("?");
                }
            }
            addStoreCircle(initial, uri, i);
            tagliacarte_store_set_folder_list_callbacks(uri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
        }
        if (!allStoreUris.isEmpty()) {
            QString lastId = c.lastSelectedStoreId;
            if (!lastId.isEmpty()) {
                QByteArray lastUtf8 = lastId.toUtf8();
                for (const QByteArray &u : allStoreUris) {
                    if (u == lastUtf8) { storeUri = u; break; }
                }
            }
            if (storeUri.isEmpty()) {
                storeUri = allStoreUris.first();
            }
            smtpTransportUri = storeToTransport.value(storeUri);
            for (int i = 0; i < storeButtons.size(); ++i) {
                storeButtons[i]->setChecked(storeButtons[i]->property("storeUri").toByteArray() == storeUri);
            }
            tagliacarte_store_refresh_folders(storeUri.constData());
            updateComposeAppendButtons();
            win.statusBar()->showMessage(TR("status.folders_loaded"));
        } else
            storeUri.clear();
    };

    // Startup: restore stores from config, or open Settings if none
    {
        Config startupConfig = loadConfig();
        tagliacarte_set_credentials_backend(startupConfig.useKeychain ? 1 : 0);
        if (startupConfig.stores.isEmpty()) {
            rightStack->setCurrentIndex(1);
            settingsBtn->setChecked(true);
            win.statusBar()->showMessage(TR("status.add_store_to_start"));
        } else {
            for (int i = 0; i < startupConfig.stores.size(); ++i) {
                const StoreEntry &entry = startupConfig.stores[i];
                QByteArray uri;
                if (entry.type == QLatin1String("maildir") && !storeHostOrPath(entry).isEmpty()) {
                    char *uriPtr = tagliacarte_store_maildir_new(storeHostOrPath(entry).toUtf8().constData());
                    if (!uriPtr) {
                        continue;
                    }
                    uri = QByteArray(uriPtr);
                    tagliacarte_free_string(uriPtr);
                } else if (entry.type == QLatin1String("imap") && !storeHostOrPath(entry).isEmpty()) {
                    QString host = storeHostOrPath(entry);
                    QString userAtHost = param(entry, "username");
                    if (userAtHost.isEmpty()) userAtHost = host;
                    if (!userAtHost.contains(QLatin1Char('@')))
                        userAtHost = userAtHost + QLatin1Char('@') + host;
                    const uint16_t imapPort = static_cast<uint16_t>(qBound(1, paramInt(entry, "port", 993), 65535));
                    char *uriPtr = tagliacarte_store_imap_new(userAtHost.toUtf8().constData(), host.toUtf8().constData(), imapPort);
                    if (!uriPtr) {
                        continue;
                    }
                    uri = QByteArray(uriPtr);
                    tagliacarte_free_string(uriPtr);
                    if (!param(entry, "transportHostname").isEmpty()) {
                        char *tUri = tagliacarte_transport_smtp_new(param(entry, "transportHostname").toUtf8().constData(), static_cast<uint16_t>(qBound(1, paramInt(entry, "transportPort", 586), 65535)));
                        if (tUri) {
                            storeToTransport[uri] = QByteArray(tUri);
                            tagliacarte_free_string(tUri);
                        }
                    }
                } else if (entry.type == QLatin1String("pop3") && !storeHostOrPath(entry).isEmpty()) {
                    QString host = storeHostOrPath(entry);
                    QString userAtHost = param(entry, "username");
                    if (userAtHost.isEmpty()) userAtHost = host;
                    if (!userAtHost.contains(QLatin1Char('@')))
                        userAtHost = userAtHost + QLatin1Char('@') + host;
                    char *uriPtr = tagliacarte_store_pop3_new(userAtHost.toUtf8().constData(), host.toUtf8().constData(), 995);
                    if (!uriPtr) {
                        continue;
                    }
                    uri = QByteArray(uriPtr);
                    tagliacarte_free_string(uriPtr);
                    if (!param(entry, "transportHostname").isEmpty()) {
                        char *tUri = tagliacarte_transport_smtp_new(param(entry, "transportHostname").toUtf8().constData(), static_cast<uint16_t>(qBound(1, paramInt(entry, "transportPort", 586), 65535)));
                        if (tUri) {
                            storeToTransport[uri] = QByteArray(tUri);
                            tagliacarte_free_string(tUri);
                        }
                    }
                } else if (entry.type == QLatin1String("nostr") && !storeHostOrPath(entry).isEmpty()) {
                    const char *kp = param(entry, "keyPath").isEmpty() ? nullptr : param(entry, "keyPath").toUtf8().constData();
                    char *uriPtr = tagliacarte_store_nostr_new(storeHostOrPath(entry).toUtf8().constData(), kp);
                    if (!uriPtr) {
                        continue;
                    }
                    uri = QByteArray(uriPtr);
                    tagliacarte_free_string(uriPtr);
                    char *tUri = tagliacarte_transport_nostr_new(storeHostOrPath(entry).toUtf8().constData(), kp);
                    if (tUri) {
                        storeToTransport[uri] = QByteArray(tUri);
                        tagliacarte_free_string(tUri);
                    }
                } else if (entry.type == QLatin1String("matrix") && !storeHostOrPath(entry).isEmpty() && !param(entry, "userId").isEmpty()) {
                    const char *token = param(entry, "accessToken").isEmpty() ? nullptr : param(entry, "accessToken").toUtf8().constData();
                    char *uriPtr = tagliacarte_store_matrix_new(storeHostOrPath(entry).toUtf8().constData(), param(entry, "userId").toUtf8().constData(), token);
                    if (!uriPtr) {
                        continue;
                    }
                    uri = QByteArray(uriPtr);
                    tagliacarte_free_string(uriPtr);
                    char *tUri = tagliacarte_transport_matrix_new(storeHostOrPath(entry).toUtf8().constData(), param(entry, "userId").toUtf8().constData(), token);
                    if (tUri) {
                        storeToTransport[uri] = QByteArray(tUri);
                        tagliacarte_free_string(tUri);
                    }
                } else {
                    continue;
                }
                allStoreUris.append(uri);
                QString initial = entry.displayName.left(1).toUpper();
                if (initial.isEmpty()) {
                    if (entry.type == QLatin1String("maildir")) {
                        initial = QStringLiteral("M");
                    } else if (entry.type == QLatin1String("imap")) {
                        initial = QStringLiteral("I");
                    } else if (entry.type == QLatin1String("pop3")) {
                        initial = QStringLiteral("P");
                    } else if (entry.type == QLatin1String("nostr")) {
                        initial = QStringLiteral("N");
                    } else if (entry.type == QLatin1String("matrix")) {
                        initial = QStringLiteral("X");
                    } else {
                        initial = QStringLiteral("?");
                    }
                }
                addStoreCircle(initial, uri, i);
                tagliacarte_store_set_folder_list_callbacks(uri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
            }
            if (!allStoreUris.isEmpty()) {
                QString lastId = startupConfig.lastSelectedStoreId;
                if (!lastId.isEmpty()) {
                    QByteArray lastUtf8 = lastId.toUtf8();
                    for (const QByteArray &u : allStoreUris) {
                        if (u == lastUtf8) { storeUri = u; break; }
                    }
                }
                if (storeUri.isEmpty()) {
                    storeUri = allStoreUris.first();
                }
                smtpTransportUri = storeToTransport.value(storeUri);
                for (int i = 0; i < storeButtons.size(); ++i) {
                    storeButtons[i]->setChecked(storeButtons[i]->property("storeUri").toByteArray() == storeUri);
                }
                tagliacarte_store_refresh_folders(storeUri.constData());
                updateComposeAppendButtons();
                win.statusBar()->showMessage(TR("status.folders_loaded"));
            }
        }
    }

    // Account type list -> edit panel: switch to edit and show form for that type (create new: clear editingStoreId)
    const QVector<int> formIndexForType = { 2, 3, 0, 1, 4, 5, 6 }; // IMAP,POP3,Maildir,mbox,Nostr,Matrix,NNTP
    for (int i = 0; i < accountTypeButtons.size(); ++i) {
        QPushButton *b = accountTypeButtons[i].btn;
        int formIdx = formIndexForType[i];
        QObject::connect(b, &QPushButton::clicked, [&, accountsStack, accountFormStack, formIdx, accountDeleteBtn]() {
            editingStoreId.clear();
            accountDeleteBtn->setVisible(false);
            accountFormStack->setCurrentIndex(formIdx);
            accountsStack->setCurrentIndex(1);
        });
    }
    QObject::connect(accountCancelBtn, &QPushButton::clicked, [&]() {
        accountsStack->setCurrentIndex(0);
        refreshAccountListInSettings();
    });
    QObject::connect(accountSaveBtn, &QPushButton::clicked, [&, accountFormStack, maildirSaveBtn, mboxSaveBtn, imapSaveBtn, pop3SaveBtn, nostrSaveBtn, matrixSaveBtn]() {
        const int idx = accountFormStack->currentIndex();
        if (idx == 0) {
            maildirSaveBtn->click();
        } else if (idx == 1) {
            mboxSaveBtn->click();
        } else if (idx == 2) {
            imapSaveBtn->click();
        } else if (idx == 3) {
            pop3SaveBtn->click();
        } else if (idx == 4) {
            nostrSaveBtn->click();
        } else if (idx == 5) {
            matrixSaveBtn->click();
        }
    });
    QObject::connect(accountDeleteBtn, &QPushButton::clicked, [&]() {
        if (editingStoreId.isEmpty()) {
            return;
        }
        QMessageBox mb(&win);
        mb.setWindowTitle(TR("accounts.delete_confirm_title"));
        mb.setText(TR("accounts.delete_confirm_text"));
        QPushButton *cancelBtn = mb.addButton(TR("common.cancel"), QMessageBox::RejectRole);
        mb.addButton(TR("accounts.delete"), QMessageBox::YesRole);
        mb.setDefaultButton(cancelBtn);
        if (mb.exec() != QDialog::Accepted) {
            return;
        }
        QByteArray idToRemove = editingStoreId.toUtf8();
        Config config = loadConfig();
        config.stores.removeIf([&idToRemove](const StoreEntry &e) { return e.id.toUtf8() == idToRemove; });
        if (config.lastSelectedStoreId == editingStoreId)
            config.lastSelectedStoreId = config.stores.isEmpty() ? QString() : config.stores.first().id;
        saveConfig(config);
        QByteArray transportUri = storeToTransport.value(idToRemove);
        if (!transportUri.isEmpty()) {
            tagliacarte_transport_free(transportUri.constData());
            storeToTransport.remove(idToRemove);
        }
        allStoreUris.removeAll(idToRemove);
        tagliacarte_store_free(idToRemove.constData());
        for (int i = 0; i < storeButtons.size(); ++i) {
            if (storeButtons[i]->property("storeUri").toByteArray() == idToRemove) {
                storeButtons[i]->deleteLater();
                storeButtons.removeAt(i);
                break;
            }
        }
        if (storeUri == idToRemove) {
            storeUri.clear();
            smtpTransportUri.clear();
            bridge.clearFolder();
            folderList->clear();
            conversationList->clear();
            messageView->clear();
            if (!allStoreUris.isEmpty()) {
                storeUri = allStoreUris.first();
                smtpTransportUri = storeToTransport.value(storeUri);
                for (int i = 0; i < storeButtons.size(); ++i)
                    storeButtons[i]->setChecked(storeButtons[i]->property("storeUri").toByteArray() == storeUri);
                tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
                tagliacarte_store_refresh_folders(storeUri.constData());
            }
            updateComposeAppendButtons();
        }
        editingStoreId.clear();
        accountDeleteBtn->setVisible(false);
        accountsStack->setCurrentIndex(0);
        refreshAccountListInSettings();
        win.statusBar()->showMessage(TR("status.account_deleted"));
    });

    QObject::connect(maildirBrowseBtn, &QPushButton::clicked, [&]() {
        QString path = QFileDialog::getExistingDirectory(&win, TR("dialog.select_maildir_directory"), maildirPathEdit->text());
        if (!path.isEmpty()) {
            maildirPathEdit->setText(path);
        }
    });
    QObject::connect(mboxBrowseBtn, &QPushButton::clicked, [&]() {
        QString path = QFileDialog::getOpenFileName(&win, TR("dialog.select_mbox_file"), QString(), QStringLiteral("mbox (*)"));
        if (!path.isEmpty()) {
            mboxPathEdit->setText(path);
        }
    });

    QObject::connect(maildirSaveBtn, &QPushButton::clicked, [&]() {
        QString path = maildirPathEdit->text().trimmed();
        if (path.isEmpty()) {
            QMessageBox::warning(&win, TR("accounts.type.maildir"), TR("maildir.validation.select_directory"));
            return;
        }
        char *uriPtr = tagliacarte_store_maildir_new(path.toUtf8().constData());
        if (!uriPtr) {
            showError(&win, "error.context.maildir");
            return;
        }
        QByteArray newUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        tagliacarte_store_free(newUri.constData());  // we only needed the URI for config
        QString displayName = maildirDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = QDir(path).dirName();
        }
        if (displayName.isEmpty()) {
            displayName = TR("maildir.default_display_name");
        }

        Config config = loadConfig();
        QString oldId = editingStoreId;
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) { idx = i; break; }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newUri);
                e.type = QStringLiteral("maildir");
                e.displayName = displayName;
                e.params[QStringLiteral("path")] = path;
                if (config.lastSelectedStoreId == oldId)
                    config.lastSelectedStoreId = e.id;
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newUri);
            entry.type = QStringLiteral("maildir");
            entry.displayName = displayName;
            entry.params[QStringLiteral("path")] = path;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        refreshStoresFromConfig();
        editingStoreId.clear();
        win.statusBar()->showMessage(TR("status.added_maildir"));
        accountsStack->setCurrentIndex(0);
        rightStack->setCurrentIndex(0);
        settingsBtn->setChecked(false);
    });

    QObject::connect(imapSaveBtn, &QPushButton::clicked, [&]() {
        QString displayName = imapDisplayNameEdit->text().trimmed();
        QString imapUser = imapUserEdit->text().trimmed();
        QString imapHost = imapHostEdit->text().trimmed();
        int imapPort = imapPortSpin->value();
        QString smtpHost = smtpHostEdit->text().trimmed();
        int smtpPort = smtpPortSpin->value();
        if (imapHost.isEmpty()) {
            QMessageBox::warning(&win, TR("accounts.type.imap"), TR("imap.validation.enter_host"));
            return;
        }
        Config config = loadConfig();
        const bool isEdit = !editingStoreId.isEmpty();
        const QByteArray previousStoreUri = storeUri;
        int colourIndex = 0;
        if (isEdit) {
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == editingStoreId) { colourIndex = i; break; }
            }
        } else {
            colourIndex = config.stores.size();
        }
        QString userAtHost = imapUser;
        if (!imapUser.contains(QLatin1Char('@')) && !imapHost.isEmpty())
            userAtHost = imapUser + QLatin1Char('@') + imapHost;
        char *uriPtr = tagliacarte_store_imap_new(userAtHost.toUtf8().constData(), imapHost.toUtf8().constData(), static_cast<uint16_t>(imapPort));
        if (!uriPtr) {
            showError(&win, "error.context.imap");
            return;
        }
        storeUri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (displayName.isEmpty()) {
            displayName = imapUser;
        }
        if (displayName.isEmpty()) {
            displayName = TR("accounts.type.imap");
        }

        if (isEdit) {
            QByteArray oldUri = editingStoreId.toUtf8();
            tagliacarte_store_free(oldUri.constData());
            storeToTransport.remove(oldUri);
            allStoreUris.removeAll(oldUri);
            for (int i = 0; i < storeButtons.size(); ++i) {
                if (storeButtons[i]->property("storeUri").toByteArray() == oldUri) {
                    storeButtons[i]->deleteLater();
                    storeButtons.removeAt(i);
                    break;
                }
            }
            if (previousStoreUri == oldUri) {
                bridge.clearFolder();
                folderList->clear();
                conversationList->clear();
                messageView->clear();
            }
        } else {
            bridge.clearFolder();
            folderList->clear();
            conversationList->clear();
            messageView->clear();
        }
        allStoreUris.append(storeUri);
        tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
        tagliacarte_store_refresh_folders(storeUri.constData());
        QString initial = displayName.left(1).toUpper();
        if (initial.isEmpty()) {
            initial = QStringLiteral("I");
        }
        addStoreCircle(initial, storeUri, colourIndex);
        for (auto *b : storeButtons) {
            b->setChecked(b->property("storeUri").toByteArray() == storeUri);
        }
        smtpTransportUri.clear();
        if (!smtpHost.isEmpty()) {
            char *transportUri = tagliacarte_transport_smtp_new(smtpHost.toUtf8().constData(), static_cast<uint16_t>(smtpPort));
            if (transportUri) {
                smtpTransportUri = QByteArray(transportUri);
                tagliacarte_free_string(transportUri);
                storeToTransport[storeUri] = smtpTransportUri;
            }
        }
        updateComposeAppendButtons();

        if (isEdit) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == editingStoreId) { idx = i; break; }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(storeUri);
                e.type = QStringLiteral("imap");
                e.displayName = displayName;
                e.emailAddress = imapEmailEdit->text().trimmed();
                e.params[QStringLiteral("hostname")] = imapHost;
                e.params[QStringLiteral("username")] = userAtHost;
                e.params[QStringLiteral("port")] = QString::number(imapPortSpin->value());
                e.params[QStringLiteral("security")] = QString::number(imapSecurityCombo->currentIndex());
                const int pollSeconds[] = { 60, 300, 600, 3600 };
                e.params[QStringLiteral("imapPollSeconds")] = QString::number((imapPollCombo->currentIndex() >= 0 && imapPollCombo->currentIndex() < 4) ? pollSeconds[imapPollCombo->currentIndex()] : 300);
                e.params[QStringLiteral("imapDeletion")] = QString::number(imapDeletionCombo->currentIndex());
                e.params[QStringLiteral("imapTrashFolder")] = imapTrashFolderEdit->text().trimmed();
                const int idleSeconds[] = { 30, 60, 300 };
                e.params[QStringLiteral("imapIdleSeconds")] = QString::number((imapIdleCombo->currentIndex() >= 0 && imapIdleCombo->currentIndex() < 3) ? idleSeconds[imapIdleCombo->currentIndex()] : 60);
                if (!smtpHost.isEmpty()) {
                    e.params[QStringLiteral("transportId")] = QString::fromUtf8(smtpTransportUri);
                    e.params[QStringLiteral("transportHostname")] = smtpHost;
                    e.params[QStringLiteral("transportPort")] = QString::number(smtpPortSpin->value());
                    e.params[QStringLiteral("transportSecurity")] = QString::number(smtpSecurityCombo->currentIndex());
                    e.params[QStringLiteral("transportUsername")] = smtpUserEdit->text().trimmed();
                } else {
                    e.params.remove(QStringLiteral("transportId"));
                    e.params.remove(QStringLiteral("transportHostname"));
                    e.params.remove(QStringLiteral("transportPort"));
                    e.params.remove(QStringLiteral("transportSecurity"));
                    e.params.remove(QStringLiteral("transportUsername"));
                }
                if (config.lastSelectedStoreId == editingStoreId)
                    config.lastSelectedStoreId = e.id;
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(storeUri);
            entry.type = QStringLiteral("imap");
            entry.displayName = displayName;
            entry.emailAddress = imapEmailEdit->text().trimmed();
            entry.params[QStringLiteral("hostname")] = imapHost;
            entry.params[QStringLiteral("username")] = userAtHost;
            entry.params[QStringLiteral("port")] = QString::number(imapPortSpin->value());
            entry.params[QStringLiteral("security")] = QString::number(imapSecurityCombo->currentIndex());
            const int pollSeconds[] = { 60, 300, 600, 3600 };
            entry.params[QStringLiteral("imapPollSeconds")] = QString::number((imapPollCombo->currentIndex() >= 0 && imapPollCombo->currentIndex() < 4) ? pollSeconds[imapPollCombo->currentIndex()] : 300);
            entry.params[QStringLiteral("imapDeletion")] = QString::number(imapDeletionCombo->currentIndex());
            entry.params[QStringLiteral("imapTrashFolder")] = imapTrashFolderEdit->text().trimmed();
            const int idleSeconds[] = { 30, 60, 300 };
            entry.params[QStringLiteral("imapIdleSeconds")] = QString::number((imapIdleCombo->currentIndex() >= 0 && imapIdleCombo->currentIndex() < 3) ? idleSeconds[imapIdleCombo->currentIndex()] : 60);
            if (!smtpHost.isEmpty()) {
                entry.params[QStringLiteral("transportId")] = QString::fromUtf8(smtpTransportUri);
                entry.params[QStringLiteral("transportHostname")] = smtpHost;
                entry.params[QStringLiteral("transportPort")] = QString::number(smtpPortSpin->value());
                entry.params[QStringLiteral("transportSecurity")] = QString::number(smtpSecurityCombo->currentIndex());
                entry.params[QStringLiteral("transportUsername")] = smtpUserEdit->text().trimmed();
            }
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        editingStoreId.clear();
        win.statusBar()->showMessage(TR("status.added_imap"));
        accountsStack->setCurrentIndex(0);
        rightStack->setCurrentIndex(0);
        settingsBtn->setChecked(false);
    });

    QObject::connect(pop3SaveBtn, &QPushButton::clicked, [&]() {
        QString displayName = pop3DisplayNameEdit->text().trimmed();
        QString pop3User = pop3UserEdit->text().trimmed();
        QString pop3Host = pop3HostEdit->text().trimmed();
        int pop3Port = pop3PortSpin->value();
        if (pop3Host.isEmpty()) {
            QMessageBox::warning(&win, TR("accounts.type.pop3"), TR("imap.validation.enter_host"));
            return;
        }
        Config config = loadConfig();
        const bool isEdit = !editingStoreId.isEmpty();
        const QByteArray previousStoreUri = storeUri;
        int colourIndex = 0;
        if (isEdit) {
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == editingStoreId) { colourIndex = i; break; }
            }
        } else {
            colourIndex = config.stores.size();
        }
        QString userAtHost = pop3User;
        if (!pop3User.contains(QLatin1Char('@')) && !pop3Host.isEmpty())
            userAtHost = pop3User + QLatin1Char('@') + pop3Host;
        char *uriPtr = tagliacarte_store_pop3_new(userAtHost.toUtf8().constData(), pop3Host.toUtf8().constData(), static_cast<uint16_t>(pop3Port));
        if (!uriPtr) {
            showError(&win, "error.context.imap");
            return;
        }
        storeUri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (displayName.isEmpty()) {
            displayName = pop3User;
        }
        if (displayName.isEmpty()) {
            displayName = TR("accounts.type.pop3");
        }

        if (isEdit) {
            QByteArray oldUri = editingStoreId.toUtf8();
            tagliacarte_store_free(oldUri.constData());
            storeToTransport.remove(oldUri);
            allStoreUris.removeAll(oldUri);
            for (int i = 0; i < storeButtons.size(); ++i) {
                if (storeButtons[i]->property("storeUri").toByteArray() == oldUri) {
                    storeButtons[i]->deleteLater();
                    storeButtons.removeAt(i);
                    break;
                }
            }
            if (previousStoreUri == oldUri) {
                bridge.clearFolder();
                folderList->clear();
                conversationList->clear();
                messageView->clear();
            }
        } else {
            bridge.clearFolder();
            folderList->clear();
            conversationList->clear();
            messageView->clear();
        }
        allStoreUris.append(storeUri);
        tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
        tagliacarte_store_refresh_folders(storeUri.constData());
        QString initial = displayName.left(1).toUpper();
        if (initial.isEmpty()) {
            initial = QStringLiteral("P");
        }
        addStoreCircle(initial, storeUri, colourIndex);
        for (auto *b : storeButtons) {
            b->setChecked(b->property("storeUri").toByteArray() == storeUri);
        }
        smtpTransportUri.clear();
        QString pop3SmtpHost = pop3SmtpHostEdit->text().trimmed();
        int pop3SmtpPort = pop3SmtpPortSpin->value();
        if (!pop3SmtpHost.isEmpty()) {
            char *transportUri = tagliacarte_transport_smtp_new(pop3SmtpHost.toUtf8().constData(), static_cast<uint16_t>(pop3SmtpPort));
            if (transportUri) {
                smtpTransportUri = QByteArray(transportUri);
                tagliacarte_free_string(transportUri);
                storeToTransport[storeUri] = smtpTransportUri;
            }
        }
        updateComposeAppendButtons();

        if (isEdit) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == editingStoreId) { idx = i; break; }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(storeUri);
                e.type = QStringLiteral("pop3");
                e.displayName = displayName;
                e.emailAddress = pop3EmailEdit->text().trimmed();
                e.params[QStringLiteral("hostname")] = pop3Host;
                e.params[QStringLiteral("username")] = userAtHost;
                if (!pop3SmtpHost.isEmpty()) {
                    e.params[QStringLiteral("transportId")] = QString::fromUtf8(smtpTransportUri);
                    e.params[QStringLiteral("transportHostname")] = pop3SmtpHost;
                    e.params[QStringLiteral("transportPort")] = QString::number(pop3SmtpPortSpin->value());
                    e.params[QStringLiteral("transportUsername")] = pop3SmtpUserEdit->text().trimmed();
                } else {
                    e.params.remove(QStringLiteral("transportId"));
                    e.params.remove(QStringLiteral("transportHostname"));
                    e.params.remove(QStringLiteral("transportPort"));
                    e.params.remove(QStringLiteral("transportUsername"));
                }
                if (config.lastSelectedStoreId == editingStoreId)
                    config.lastSelectedStoreId = e.id;
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(storeUri);
            entry.type = QStringLiteral("pop3");
            entry.displayName = displayName;
            entry.emailAddress = pop3EmailEdit->text().trimmed();
            entry.params[QStringLiteral("hostname")] = pop3Host;
            entry.params[QStringLiteral("username")] = userAtHost;
            if (!pop3SmtpHost.isEmpty()) {
                entry.params[QStringLiteral("transportId")] = QString::fromUtf8(smtpTransportUri);
                entry.params[QStringLiteral("transportHostname")] = pop3SmtpHost;
                entry.params[QStringLiteral("transportPort")] = QString::number(pop3SmtpPortSpin->value());
                entry.params[QStringLiteral("transportUsername")] = pop3SmtpUserEdit->text().trimmed();
            }
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        editingStoreId.clear();
        win.statusBar()->showMessage(TR("status.added_pop3"));
        accountsStack->setCurrentIndex(0);
        rightStack->setCurrentIndex(0);
        settingsBtn->setChecked(false);
    });

    QObject::connect(mboxSaveBtn, &QPushButton::clicked, [&]() {
        QMessageBox::information(&win, TR("accounts.type.mbox"), TR("mbox.not_implemented"));
    });

    QObject::connect(nostrKeyBrowseBtn, &QPushButton::clicked, [&]() {
        QString path = QFileDialog::getOpenFileName(&win, TR("nostr.dialog.select_key"), nostrKeyPathEdit->text(), QStringLiteral("*"));
        if (!path.isEmpty()) {
            nostrKeyPathEdit->setText(path);
        }
    });

    QObject::connect(nostrRelayAddBtn, &QPushButton::clicked, [&]() {
        QString u = nostrRelayUrlEdit->text().trimmed();
        if (!u.isEmpty()) {
            nostrRelayList->addItem(u);
            nostrRelayUrlEdit->clear();
        }
    });
    QObject::connect(nostrRelayRemoveBtn, &QPushButton::clicked, [&]() {
        QList<QListWidgetItem *> sel = nostrRelayList->selectedItems();
        for (QListWidgetItem *item : sel) {
            delete nostrRelayList->takeItem(nostrRelayList->row(item));
        }
    });

    QObject::connect(nostrSaveBtn, &QPushButton::clicked, [&]() {
        QStringList relayList;
        for (int i = 0; i < nostrRelayList->count(); ++i) {
            QString u = nostrRelayList->item(i)->text().trimmed();
            if (!u.isEmpty())
                relayList.append(u);
        }
        if (relayList.isEmpty()) {
            QMessageBox::warning(&win, TR("accounts.type.nostr"), TR("nostr.validation.relays"));
            return;
        }
        const QString relays = relayList.join(QLatin1Char(','));
        QString keyPath = nostrKeyPathEdit->text().trimmed();
        QString secretText = nostrSecretKeyEdit->text().trimmed();
        if (!secretText.isEmpty()) {
            QString fileName = editingStoreId.isEmpty()
                ? QStringLiteral("nostr_secret_new_%1.key").arg(QDateTime::currentMSecsSinceEpoch())
                : QStringLiteral("nostr_secret_%1.key").arg(QString(editingStoreId).replace(QLatin1Char(':'), QLatin1Char('_')));
            QString path = tagliacarteConfigDir() + QLatin1Char('/') + fileName;
            QFile f(path);
            if (!f.open(QIODevice::WriteOnly | QIODevice::Text)) {
                QMessageBox::warning(&win, TR("accounts.type.nostr"), TR("nostr.validation.key_file_write"));
                return;
            }
            f.write(secretText.toUtf8());
            f.close();
            keyPath = path;
        }
        const char *keyPathPtr = keyPath.isEmpty() ? nullptr : keyPath.toUtf8().constData();
        char *uriPtr = tagliacarte_store_nostr_new(relays.toUtf8().constData(), keyPathPtr);
        if (!uriPtr) {
            showError(&win, "error.context.nostr");
            return;
        }
        QByteArray newStoreUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        char *transportUriPtr = tagliacarte_transport_nostr_new(relays.toUtf8().constData(), keyPathPtr);
        QByteArray newTransportUri;
        if (transportUriPtr) {
            newTransportUri = QByteArray(transportUriPtr);
            tagliacarte_free_string(transportUriPtr);
        }
        QString displayName = nostrDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = TR("accounts.type.nostr");
        }
        QString nip05 = nostrNip05Edit->text().trimmed();

        Config config = loadConfig();
        QString oldId = editingStoreId;
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) { idx = i; break; }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newStoreUri);
                e.type = QStringLiteral("nostr");
                e.displayName = displayName;
                e.emailAddress = nip05;
                e.params[QStringLiteral("path")] = relays;
                e.params[QStringLiteral("keyPath")] = keyPath;
                if (config.lastSelectedStoreId == oldId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newStoreUri);
            entry.type = QStringLiteral("nostr");
            entry.displayName = displayName;
            entry.emailAddress = nip05;
            entry.params[QStringLiteral("path")] = relays;
            entry.params[QStringLiteral("keyPath")] = keyPath;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        refreshStoresFromConfig();
        editingStoreId.clear();
        win.statusBar()->showMessage(TR("status.added_nostr"));
        accountsStack->setCurrentIndex(0);
        rightStack->setCurrentIndex(0);
        settingsBtn->setChecked(false);
    });

    QObject::connect(matrixSaveBtn, &QPushButton::clicked, [&]() {
        QString homeserver = matrixHomeserverEdit->text().trimmed();
        QString userId = matrixUserIdEdit->text().trimmed();
        QString token = matrixTokenEdit->text().trimmed();
        if (homeserver.isEmpty() || userId.isEmpty()) {
            QMessageBox::warning(&win, TR("accounts.type.matrix"), TR("matrix.validation.homeserver_user"));
            return;
        }
        const char *tokenPtr = token.isEmpty() ? nullptr : token.toUtf8().constData();
        char *uriPtr = tagliacarte_store_matrix_new(homeserver.toUtf8().constData(), userId.toUtf8().constData(), tokenPtr);
        if (!uriPtr) {
            showError(&win, "error.context.matrix");
            return;
        }
        QByteArray newStoreUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        char *transportUriPtr = tagliacarte_transport_matrix_new(homeserver.toUtf8().constData(), userId.toUtf8().constData(), tokenPtr);
        QByteArray newTransportUri;
        if (transportUriPtr) {
            newTransportUri = QByteArray(transportUriPtr);
            tagliacarte_free_string(transportUriPtr);
        }
        QString displayName = matrixDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = userId;
        }

        Config config = loadConfig();
        QString oldId = editingStoreId;
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) { idx = i; break; }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newStoreUri);
                e.type = QStringLiteral("matrix");
                e.displayName = displayName;
                e.params[QStringLiteral("path")] = homeserver;
                e.params[QStringLiteral("userId")] = userId;
                e.params[QStringLiteral("accessToken")] = token;
                if (config.lastSelectedStoreId == oldId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newStoreUri);
            entry.type = QStringLiteral("matrix");
            entry.displayName = displayName;
            entry.params[QStringLiteral("path")] = homeserver;
            entry.params[QStringLiteral("userId")] = userId;
            entry.params[QStringLiteral("accessToken")] = token;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        refreshStoresFromConfig();
        editingStoreId.clear();
        win.statusBar()->showMessage(TR("status.added_matrix"));
        accountsStack->setCurrentIndex(0);
        rightStack->setCurrentIndex(0);
        settingsBtn->setChecked(false);
    });

    QObject::connect(&bridge, &EventBridge::folderReadyForMessages, [&](quint64 total) {
        QByteArray uri = bridge.folderUri();
        if (uri.isEmpty()) {
            return;
        }
        updateComposeAppendButtons();
        auto *item = folderList->currentItem();
        tagliacarte_folder_set_message_list_callbacks(uri.constData(), on_message_summary_cb, on_message_list_complete_cb, &bridge);
        tagliacarte_folder_request_message_list(uri.constData(), 0, total > 100 ? 100 : total);
        if (item) {
            win.statusBar()->showMessage(TR("status.folder_loading").arg(item->text()));
        }
    });

    QObject::connect(folderList, &QListWidget::itemSelectionChanged, [&]() {
        bridge.clearFolder();
        conversationList->clear();
        messageView->clear();

        auto *item = folderList->currentItem();
        if (!item || storeUri.isEmpty()) {
            return;
        }

        bridge.setFolderNameOpening(item->text());
        QByteArray name = item->text().toUtf8();
        tagliacarte_store_start_open_folder(
            storeUri.constData(),
            name.constData(),
            on_open_folder_select_event_cb,
            on_folder_ready_cb,
            on_open_folder_error_cb,
            &bridge
        );
        win.statusBar()->showMessage(TR("status.opening").arg(item->text()));
    });

    QObject::connect(conversationList, &QTreeWidget::itemSelectionChanged, [&]() {
        messageView->clear();
        auto *item = conversationList->currentItem();
        QByteArray uri = bridge.folderUri();
        if (!item || uri.isEmpty()) {
            return;
        }

        QVariant idVar = item->data(0, MessageIdRole);
        if (!idVar.isValid()) {
            return;
        }
        QByteArray id = idVar.toString().toUtf8();

        tagliacarte_folder_set_message_callbacks(uri.constData(), on_message_metadata_cb, on_message_content_cb, on_message_complete_cb, &bridge);
        tagliacarte_folder_request_message(uri.constData(), id.constData());
        messageView->setPlainText(TR("status.loading"));
    });

    QObject::connect(appendMessageBtn, &QToolButton::clicked, [&]() {
        QByteArray folderUri = bridge.folderUri();
        if (folderUri.isEmpty()) {
            return;
        }
        QString path = QFileDialog::getOpenFileName(&win, TR("append_message.dialog_title"),
            QString(), TR("append_message.file_filter"));
        if (path.isEmpty()) {
            return;
        }
        QFile f(path);
        if (!f.open(QIODevice::ReadOnly)) {
            QMessageBox::warning(&win, TR("common.error"), TR("append_message.read_error"));
            return;
        }
        QByteArray data = f.readAll();
        f.close();
        if (data.isEmpty()) {
            return;
        }
        int r = tagliacarte_folder_append_message(folderUri.constData(),
            reinterpret_cast<const unsigned char *>(data.constData()), data.size());
        if (r != 0) {
            showError(&win, "error.context.append_message");
            return;
        }
        conversationList->clear();
        quint64 total = tagliacarte_folder_message_count(folderUri.constData());
        tagliacarte_folder_set_message_list_callbacks(folderUri.constData(), on_message_summary_cb, on_message_list_complete_cb, &bridge);
        tagliacarte_folder_request_message_list(folderUri.constData(), 0, total > 100 ? 100 : total);
        win.statusBar()->showMessage(TR("status.message_appended"));
    });

    QObject::connect(composeBtn, &QPushButton::clicked, [&]() {
        if (smtpTransportUri.isEmpty()) {
            return;  // no transport for current store (button should be disabled)
        }
        ComposeDialog dlg(&win, smtpTransportUri);
        if (dlg.exec() != QDialog::Accepted) {
            return;
        }

        QString from = dlg.fromEdit->text().trimmed();
        QString to = dlg.toEdit->text().trimmed();
        QString subject = dlg.subjectEdit->text().trimmed();
        QString body = dlg.bodyEdit->toPlainText();

        if (from.isEmpty() || to.isEmpty()) {
            QMessageBox::warning(&win, TR("compose.title"), TR("compose.validation.from_to"));
            return;
        }

        QByteArray fromUtf8 = from.toUtf8();
        QByteArray toUtf8 = to.toUtf8();
        QByteArray subjUtf8 = subject.toUtf8();
        QByteArray bodyUtf8 = body.toUtf8();

        win.statusBar()->showMessage(TR("status.sending"));
        tagliacarte_transport_send_async(
            smtpTransportUri.constData(),
            fromUtf8.constData(),
            toUtf8.constData(),
            nullptr,           /* cc */
            subjUtf8.constData(),
            bodyUtf8.constData(),
            nullptr,           /* body_html */
            0,
            nullptr,           /* attachments */
            on_send_progress_cb,
            on_send_complete_cb,
            &bridge
        );
    });

    win.statusBar()->showMessage(TR("status.open_maildir_to_start"));
    win.show();

    int ret = app.exec();

    bridge.clearFolder();
    for (const QByteArray &u : allStoreUris) {
        tagliacarte_store_free(u.constData());
    }
    for (const QByteArray &t : storeToTransport) {
        tagliacarte_transport_free(t.constData());
    }

    return ret;
}
