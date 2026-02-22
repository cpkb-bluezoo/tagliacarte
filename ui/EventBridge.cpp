#include "EventBridge.h"
#include "MessageDragTreeWidget.h"
#include "Tr.h"
#include "tagliacarte.h"
#include <QTreeWidgetItem>
#include <QFont>
#include <QMessageBox>
#include <QPushButton>
#include <QHBoxLayout>
#include <QLabel>
#include <QFileDialog>
#include <QTextCursor>
#include <QTextDocument>
#include <QUrl>
#include <QRegularExpression>
#include <QDateTime>
#include <QLocale>
#include <QPalette>
#include <QApplication>
#include <QPainter>
#include <QPixmap>
#include <QDir>
#include <QFile>
#include <QStandardPaths>
#include <QNetworkAccessManager>
#include <QNetworkRequest>
#include <QNetworkReply>
#include <QEventLoop>
#include <QBuffer>
#include <QtConcurrent/QtConcurrentRun>

void showError(QWidget *parent, const char *contextKey) {
    const char *msg = tagliacarte_last_error();
    QString msgStr = msg ? QString::fromUtf8(msg) : TR("error.unknown");
    QMessageBox::critical(parent, TR("common.error"), QString("%1: %2").arg(TR(contextKey)).arg(msgStr));
}

// --- System folder display name mapping ---

QString EventBridge::displayNameForFolder(const QString &realName) {
    QString lower = realName.trimmed().toLower();
    if (lower == QLatin1String("inbox")) {
        return TR("folder.inbox");
    }
    if (lower == QLatin1String("outbox")) {
        return TR("folder.outbox");
    }
    if (lower == QLatin1String("sent") || lower == QLatin1String("sent messages") || lower == QLatin1String("sent items")) {
        return TR("folder.sent");
    }
    if (lower == QLatin1String("drafts")) {
        return TR("folder.drafts");
    }
    if (lower == QLatin1String("trash") || lower == QLatin1String("deleted") || lower == QLatin1String("deleted items") || lower == QLatin1String("deleted messages")) {
        return TR("folder.trash");
    }
    if (lower == QLatin1String("junk") || lower == QLatin1String("spam") || lower == QLatin1String("bulk mail")) {
        return TR("folder.junk");
    }
    if (lower == QLatin1String("archive") || lower == QLatin1String("archives")) {
        return TR("folder.archive");
    }
    return realName;
}

bool EventBridge::isSystemFolder(const QString &realName, const QString &attributes) {
    QString lower = realName.trimmed().toLower();
    if (lower == QLatin1String("inbox")) {
        return true;
    }
    // Check IMAP special-use attributes
    QString attrsLower = attributes.toLower();
    if (attrsLower.contains(QLatin1String("\\sent")) ||
        attrsLower.contains(QLatin1String("\\drafts")) ||
        attrsLower.contains(QLatin1String("\\trash")) ||
        attrsLower.contains(QLatin1String("\\junk")) ||
        attrsLower.contains(QLatin1String("\\archive")) ||
        attrsLower.contains(QLatin1String("\\all")) ||
        attrsLower.contains(QLatin1String("\\flagged"))) {
        return true;
    }
    // Check well-known names
    static const QStringList systemNames = {
        QStringLiteral("sent"), QStringLiteral("sent messages"), QStringLiteral("sent items"),
        QStringLiteral("drafts"),
        QStringLiteral("trash"), QStringLiteral("deleted items"), QStringLiteral("deleted messages"), QStringLiteral("deleted"),
        QStringLiteral("junk"), QStringLiteral("spam"), QStringLiteral("bulk mail"),
        QStringLiteral("archive"), QStringLiteral("archives"),
        QStringLiteral("outbox")
    };
    return systemNames.contains(lower);
}

// --- Helper functions ---

QTreeWidgetItem *EventBridge::findFolderItem(const QString &realName) const {
    if (!folderTree) {
        return nullptr;
    }
    QTreeWidgetItemIterator it(folderTree);
    while (*it) {
        if ((*it)->data(0, FolderNameRole).toString() == realName) {
            return *it;
        }
        ++it;
    }
    return nullptr;
}

int EventBridge::countAllItems(QTreeWidget *tree) {
    int count = 0;
    QTreeWidgetItemIterator it(tree);
    while (*it) {
        ++count;
        ++it;
    }
    return count;
}

bool EventBridge::isConversationMode() const {
    return m_storeKind == TAGLIACARTE_STORE_KIND_NOSTR
        || m_storeKind == TAGLIACARTE_STORE_KIND_MATRIX;
}

bool EventBridge::isHexPubkey(const QString &s) {
    if (s.length() != 64) return false;
    for (QChar c : s) {
        if (!c.isDigit() && (c < QLatin1Char('a') || c > QLatin1Char('f'))
                         && (c < QLatin1Char('A') || c > QLatin1Char('F')))
            return false;
    }
    return true;
}

void EventBridge::fetchNostrProfile(const QString &hexPubkey) {
    if (m_nostrRelaysCsv.isEmpty()) return;
    QString lower = hexPubkey.toLower();
    if (m_profileFetchPending.contains(lower)) return;
    if (m_nostrNameCache.contains(lower)) return;
    m_profileFetchPending.insert(lower);
    QString relays = m_nostrRelaysCsv;
    QString secretKey = m_nostrSecretKey;
    QString pk = lower;
    (void)QtConcurrent::run([this, pk, relays, secretKey]() {
        QByteArray pkBa = pk.toUtf8();
        QByteArray relaysBa = relays.toUtf8();
        QByteArray skBa = secretKey.toUtf8();
        const char *skPtr = skBa.isEmpty() ? nullptr : skBa.constData();
        auto *profile = tagliacarte_nostr_fetch_profile(pkBa.constData(), relaysBa.constData(), skPtr);
        if (!profile) {
            fprintf(stderr, "[avatar] profile fetch failed for %s\n", pkBa.constData());
            return;
        }
        QString displayName;
        if (profile->display_name)
            displayName = QString::fromUtf8(profile->display_name);
        QString nip05;
        if (profile->nip05)
            nip05 = QString::fromUtf8(profile->nip05);
        QString pictureUrl;
        if (profile->picture)
            pictureUrl = QString::fromUtf8(profile->picture);
        tagliacarte_nostr_profile_free(profile);

        fprintf(stderr, "[avatar] %s: name=%s picture=%s\n",
            pkBa.constData(),
            displayName.toUtf8().constData(),
            pictureUrl.isEmpty() ? "(none)" : pictureUrl.toUtf8().constData());

        QString best;
        if (!displayName.isEmpty())
            best = displayName;
        else if (!nip05.isEmpty())
            best = nip05;
        if (!best.isEmpty()) {
            QMetaObject::invokeMethod(this, "updateFolderDisplayName",
                Qt::QueuedConnection, Q_ARG(QString, pk), Q_ARG(QString, best));
        }

        // Download and cache the profile picture
        if (!pictureUrl.isEmpty()) {
            QString cacheDir = QStandardPaths::writableLocation(QStandardPaths::CacheLocation)
                + QStringLiteral("/avatars");
            QDir().mkpath(cacheDir);
            QString filePath = cacheDir + QStringLiteral("/") + pk + QStringLiteral(".img");

            // Validate existing cached file; remove if corrupt
            if (QFile::exists(filePath)) {
                QImage cached(filePath);
                if (cached.isNull()) {
                    fprintf(stderr, "[avatar] cached file corrupt, removing: %s\n", filePath.toUtf8().constData());
                    QFile::remove(filePath);
                }
            }

            if (!QFile::exists(filePath)) {
                QNetworkAccessManager nam;
                QUrl avatarUrl(pictureUrl);
                QNetworkRequest req{avatarUrl};
                req.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                                 QNetworkRequest::NoLessSafeRedirectPolicy);
                req.setHeader(QNetworkRequest::UserAgentHeader,
                              QStringLiteral("Tagliacarte/1.0"));
                QNetworkReply *reply = nam.get(req);
                QEventLoop loop;
                QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
                loop.exec();
                if (reply->error() == QNetworkReply::NoError) {
                    QByteArray body = reply->readAll();
                    fprintf(stderr, "[avatar] downloaded %lld bytes for %s\n",
                        (long long)body.size(), pkBa.constData());
                    QImage testImg;
                    if (!body.isEmpty() && testImg.loadFromData(body)) {
                        QFile f(filePath);
                        if (f.open(QIODevice::WriteOnly)) {
                            f.write(body);
                            f.close();
                        }
                    } else {
                        QString ct = reply->header(QNetworkRequest::ContentTypeHeader).toString();
                        fprintf(stderr, "[avatar] response is not a valid image for %s (content-type: %s, %lld bytes)\n",
                            pkBa.constData(), ct.toUtf8().constData(), (long long)body.size());
                    }
                } else {
                    fprintf(stderr, "[avatar] download failed for %s: %s\n",
                        pkBa.constData(), reply->errorString().toUtf8().constData());
                }
                reply->deleteLater();
            }
            if (QFile::exists(filePath)) {
                fprintf(stderr, "[avatar] cached at %s\n", filePath.toUtf8().constData());
                QMetaObject::invokeMethod(this, "updateFolderAvatar",
                    Qt::QueuedConnection, Q_ARG(QString, pk), Q_ARG(QString, filePath));
            }
        }
    });
}

void EventBridge::updateFolderDisplayName(const QString &realName, const QString &displayName) {
    m_nostrNameCache.insert(realName.toLower(), displayName);
    m_profileFetchPending.remove(realName.toLower());
    QTreeWidgetItem *item = findFolderItem(realName);
    if (item) {
        item->setText(0, displayName);
    }
    if (isConversationMode() && !m_chatMessages.isEmpty())
        renderChatMessages();
}

static QPixmap circularAvatar(const QPixmap &src, int size) {
    QPixmap scaled = src.scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    if (scaled.width() != scaled.height()) {
        int sz = qMin(scaled.width(), scaled.height());
        scaled = scaled.copy((scaled.width() - sz) / 2, (scaled.height() - sz) / 2, sz, sz);
    }
    QPixmap rounded(size, size);
    rounded.fill(Qt::transparent);
    QPainter p(&rounded);
    p.setRenderHint(QPainter::Antialiasing);
    p.setBrush(QBrush(scaled));
    p.setPen(Qt::NoPen);
    p.drawEllipse(0, 0, size, size);
    p.end();
    return rounded;
}

void EventBridge::updateFolderAvatar(const QString &realName, const QString &filePath) {
    QString lower = realName.toLower();
    fprintf(stderr, "[avatar] updateFolderAvatar: pk=%s file=%s\n",
        lower.toUtf8().constData(), filePath.toUtf8().constData());
    m_nostrPictureCache.insert(lower, filePath);
    QTreeWidgetItem *item = findFolderItem(realName);
    if (item) {
        QPixmap pix(filePath);
        if (!pix.isNull()) {
            item->setIcon(0, QIcon(circularAvatar(pix, 24)));
            fprintf(stderr, "[avatar] folder icon set for %s\n", lower.toUtf8().constData());
        } else {
            fprintf(stderr, "[avatar] pixmap load FAILED for %s\n", filePath.toUtf8().constData());
        }
    } else {
        fprintf(stderr, "[avatar] folder item NOT FOUND for %s\n", lower.toUtf8().constData());
    }
    if (isConversationMode() && !m_chatMessages.isEmpty())
        renderChatMessages();
}

void EventBridge::ensureProfilesFetched() {
    QSet<QString> needed;
    for (const ChatMessage &msg : m_chatMessages) {
        QString lower = msg.authorId.toLower();
        if (!m_nostrNameCache.contains(lower) && !m_profileFetchPending.contains(lower) && isHexPubkey(lower))
            needed.insert(lower);
    }
    if (!m_selfPubkey.isEmpty() && !m_nostrNameCache.contains(m_selfPubkey)
        && !m_profileFetchPending.contains(m_selfPubkey))
        needed.insert(m_selfPubkey);
    for (const QString &pk : needed)
        fetchNostrProfile(pk);
}

QString EventBridge::authorDisplayName(const QString &authorId) const {
    QString lower = authorId.toLower();
    if (m_nostrNameCache.contains(lower))
        return m_nostrNameCache.value(lower);
    if (isHexPubkey(lower)) {
        QByteArray hexBa = lower.toUtf8();
        char *npub = tagliacarte_nostr_hex_to_npub(hexBa.constData());
        if (npub) {
            QString s = QString::fromUtf8(npub);
            tagliacarte_free_string(npub);
            return s.left(12) + QStringLiteral("…");
        }
    }
    return authorId.left(12) + QStringLiteral("…");
}

QString EventBridge::authorAvatarPath(const QString &authorId) const {
    QString lower = authorId.toLower();
    QString cached = m_nostrPictureCache.value(lower);
    if (!cached.isEmpty()) return cached;
    // Fallback: check disk cache directly (handles timing gaps)
    QString diskPath = QStandardPaths::writableLocation(QStandardPaths::CacheLocation)
        + QStringLiteral("/avatars/") + lower + QStringLiteral(".img");
    if (QFile::exists(diskPath)) {
        QImage test(diskPath);
        if (!test.isNull()) {
            const_cast<QMap<QString, QString> &>(m_nostrPictureCache).insert(lower, diskPath);
            return diskPath;
        }
    }
    return QString();
}

int EventBridge::avatarHue(const QString &id) {
    uint hash = 0;
    for (QChar c : id) {
        hash = hash * 31 + c.unicode();
    }
    return static_cast<int>(hash % 360);
}

QString EventBridge::formatChatTimestamp(qint64 secs) {
    if (secs <= 0) return QString();
    QLocale locale;
    QDateTime dt = QDateTime::fromSecsSinceEpoch(secs);
    QDateTime now = QDateTime::currentDateTime();
    QDate today = now.date();
    if (dt.date() == today) {
        return locale.toString(dt.time(), QLocale::ShortFormat);
    }
    int daysDiff = dt.date().daysTo(today);
    if (daysDiff > 0 && daysDiff < 7) {
        return locale.dayName(dt.date().dayOfWeek(), QLocale::ShortFormat)
            + QStringLiteral(" ")
            + locale.toString(dt.time(), QLocale::ShortFormat);
    }
    return locale.toString(dt, QLocale::ShortFormat);
}

void EventBridge::renderChatMessages() {
    if (!messageView) return;
    if (m_chatMessages.isEmpty()) {
        QPalette pal = QApplication::palette();
        QString mutedColor = pal.color(QPalette::Disabled, QPalette::WindowText).name();
        messageView->setHtml(QStringLiteral(
            "<p style='color:%1; text-align:center; padding-top:40px;'>No messages</p>").arg(mutedColor));
        return;
    }

    static const int AVATAR_PX = 40;

    QPalette pal = QApplication::palette();
    bool isDark = pal.color(QPalette::Window).lightness() < 128;
    QString bgColor = pal.color(QPalette::Base).name();
    QString textColor = pal.color(QPalette::Text).name();
    QString mutedColor = isDark ? QStringLiteral("#888888") : QStringLiteral("#999999");
    QString nameColor = isDark ? QStringLiteral("#dddddd") : QStringLiteral("#1d1c1d");

    // Pre-render circular avatars as base64 data URIs (decoded in CidTextBrowser::loadResource)
    fprintf(stderr, "[render] picture cache has %lld entries\n", (long long)m_nostrPictureCache.size());
    QMap<QString, QString> avatarDataUris;
    for (const ChatMessage &msg : m_chatMessages) {
        QString lower = msg.authorId.toLower();
        if (avatarDataUris.contains(lower)) continue;
        QString avatarPath = authorAvatarPath(lower);
        fprintf(stderr, "[render] author %s avatar=%s\n",
            lower.left(8).toUtf8().constData(),
            avatarPath.isEmpty() ? "(none)" : avatarPath.toUtf8().constData());
        if (!avatarPath.isEmpty() && QFile::exists(avatarPath)) {
            QPixmap pix(avatarPath);
            if (!pix.isNull()) {
                QPixmap circ = circularAvatar(pix, AVATAR_PX * 2);
                QByteArray ba;
                QBuffer buf(&ba);
                buf.open(QIODevice::WriteOnly);
                circ.save(&buf, "PNG");
                buf.close();
                if (!ba.isEmpty()) {
                    avatarDataUris.insert(lower,
                        QStringLiteral("data:image/png;base64,")
                            + QString::fromLatin1(ba.toBase64()));
                }
            }
        }
    }

    QString html;
    html.reserve(m_chatMessages.size() * 500);
    html += QStringLiteral(
        "<html><body style='margin:0; padding:0; background-color:%1; color:%2;'>"
    ).arg(bgColor, textColor);

    for (const ChatMessage &msg : m_chatMessages) {
        QString lower = msg.authorId.toLower();
        QString name = authorDisplayName(lower).toHtmlEscaped();
        QString timeStr = formatChatTimestamp(msg.timestampSecs).toHtmlEscaped();
        QString content = msg.content.toHtmlEscaped().replace(QLatin1Char('\n'), QStringLiteral("<br>"));

        QString avatarHtml;
        if (avatarDataUris.contains(lower)) {
            avatarHtml = QStringLiteral("<img src='%1' width='%2' height='%2' />")
                .arg(avatarDataUris.value(lower), QString::number(AVATAR_PX));
        } else {
            int hue = avatarHue(lower);
            int sat = isDark ? 50 : 60;
            int light = isDark ? 40 : 65;
            QString circleColor = QColor::fromHsl(hue, sat * 255 / 100, light * 255 / 100).name();
            QString initial = name.isEmpty() ? QStringLiteral("?") : name.left(1).toUpper();
            avatarHtml = QStringLiteral(
                "<table cellspacing='0' cellpadding='0'><tr><td style='"
                "width:%3px; height:%3px; background-color:%1;"
                "text-align:center; vertical-align:middle; color:#ffffff; font-weight:bold;"
                "font-size:18px;'>%2</td></tr></table>"
            ).arg(circleColor, initial.toHtmlEscaped(), QString::number(AVATAR_PX));
        }

        html += QStringLiteral(
            "<table width='100%%' cellspacing='0' cellpadding='0' style='padding:6px 16px;'><tr>"
            "<td width='%5' style='vertical-align:top; padding-top:4px;'>%1</td>"
            "<td style='vertical-align:top; padding:2px 4px 8px 6px;'>"
            "<b style='font-size:14px; color:%6;'>%2</b>"
            "&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;"
            "<span style='font-size:11px; color:%7;'>%3</span>"
            "<br/>%4"
            "</td></tr></table>"
        ).arg(avatarHtml, name, timeStr, content,
              QString::number(AVATAR_PX + 8), nameColor, mutedColor);
    }

    html += QStringLiteral("</body></html>");
    messageView->setHtml(html);

    QTextCursor cursor = messageView->textCursor();
    cursor.movePosition(QTextCursor::End);
    messageView->setTextCursor(cursor);
    messageView->ensureCursorVisible();
}

// --- EventBridge implementation ---

void EventBridge::setFolderUri(const QByteArray &uri) {
    if (!m_folderUri.isEmpty()) {
        tagliacarte_folder_free(m_folderUri.constData());
    }
    m_folderUri = uri;
    // Keep the drag source widget in sync
    if (auto *dragWidget = qobject_cast<MessageDragTreeWidget *>(conversationList)) {
        dragWidget->setSourceFolderUri(uri);
    }
}

void EventBridge::setLastMessage(const QString &from, const QString &to, const QString &subject, const QString &bodyPlain) {
    m_lastMessageFrom = from;
    m_lastMessageTo = to;
    m_lastMessageSubject = subject;
    m_lastMessageBodyPlain = bodyPlain;
}

void EventBridge::clearLastMessage() {
    m_lastMessageFrom.clear();
    m_lastMessageTo.clear();
    m_lastMessageSubject.clear();
    m_lastMessageBodyPlain.clear();
}

void EventBridge::clearFolder() {
    if (!m_folderUri.isEmpty()) {
        tagliacarte_folder_free(m_folderUri.constData());
        m_folderUri.clear();
    }
}

void EventBridge::addFolder(const QString &name, const QString &delimiter, const QString &attributes) {
    if (!folderTree) {
        return;
    }

    // Split by delimiter to build hierarchy
    QStringList parts;
    QChar delimChar;
    if (!delimiter.isEmpty() && delimiter[0] != QChar(0)) {
        delimChar = delimiter[0];
        parts = name.split(delimChar, Qt::SkipEmptyParts);
    } else {
        parts = QStringList{ name };
    }

    // Walk/create the hierarchy in the tree
    QTreeWidgetItem *parent = nullptr;
    QString pathSoFar;
    for (int i = 0; i < parts.size(); ++i) {
        if (i > 0 && !delimChar.isNull()) {
            pathSoFar += delimChar;
        }
        pathSoFar += parts[i];

        bool isLeaf = (i == parts.size() - 1);

        // Try to find existing item at this level
        QTreeWidgetItem *existing = nullptr;
        int childCount = parent ? parent->childCount() : folderTree->topLevelItemCount();
        for (int c = 0; c < childCount; ++c) {
            QTreeWidgetItem *child = parent ? parent->child(c) : folderTree->topLevelItem(c);
            if (child->data(0, FolderNameRole).toString() == pathSoFar) {
                existing = child;
                break;
            }
        }

        if (existing) {
            if (isLeaf) {
                // Update attributes on the leaf
                existing->setData(0, FolderAttrsRole, attributes);
                existing->setData(0, FolderDelimRole, delimiter);
            }
            parent = existing;
        } else {
            // Create new item
            auto *item = new QTreeWidgetItem();
            QString displayText;
            bool needsProfileFetch = false;

            if (isConversationMode() && isHexPubkey(parts[i])) {
                QString lower = parts[i].toLower();
                if (m_nostrNameCache.contains(lower)) {
                    displayText = m_nostrNameCache.value(lower);
                } else {
                    QByteArray hexBa = lower.toUtf8();
                    char *npub = tagliacarte_nostr_hex_to_npub(hexBa.constData());
                    if (npub) {
                        displayText = QString::fromUtf8(npub);
                        tagliacarte_free_string(npub);
                    } else {
                        displayText = lower;
                    }
                    needsProfileFetch = true;
                }
            } else {
                displayText = displayNameForFolder(parts[i]);
            }

            item->setText(0, displayText);
            item->setData(0, FolderNameRole, pathSoFar);
            item->setData(0, FolderDelimRole, delimiter);
            if (isLeaf) {
                item->setData(0, FolderAttrsRole, attributes);
            } else {
                item->setData(0, FolderAttrsRole, QString());
            }

            // INBOX always first at top level
            bool isInbox = (pathSoFar.compare(QLatin1String("INBOX"), Qt::CaseInsensitive) == 0);
            if (parent) {
                parent->addChild(item);
            } else if (isInbox) {
                folderTree->insertTopLevelItem(0, item);
            } else {
                folderTree->addTopLevelItem(item);
            }
            item->setExpanded(true);
            parent = item;

            if (needsProfileFetch) {
                fetchNostrProfile(parts[i].toLower());
            }
        }
    }
}

void EventBridge::removeFolder(const QString &name) {
    QTreeWidgetItem *item = findFolderItem(name);
    if (!item) {
        return;
    }
    QTreeWidgetItem *parent = item->parent();
    if (parent) {
        parent->removeChild(item);
    } else if (folderTree) {
        int idx = folderTree->indexOfTopLevelItem(item);
        if (idx >= 0) {
            folderTree->takeTopLevelItem(idx);
        }
    }
    delete item;
}

void EventBridge::onFolderListComplete(int error, const QString &errorMessage) {
    if (statusBar && win) {
        if (error == TAGLIACARTE_NEEDS_CREDENTIAL) {
            return;
        }
        if (error != 0) {
            QString detail = errorMessage.isEmpty() ? TR("error.unknown") : errorMessage;
            QMessageBox::warning(win, TR("common.error"),
                TR("error.context.store_connect") + QStringLiteral("\n\n") + detail);
        } else if (folderTree) {
            statusBar->showMessage(TR_N("status.folders_count", countAllItems(folderTree)));
        }
    }
    if (conversationList && error == 0) {
        int n = conversationList->topLevelItemCount();
        if (n > 0) {
            conversationList->scrollToItem(conversationList->topLevelItem(n - 1));
        }
    }
}

void EventBridge::requestCredentialSlot(const QString &storeUri, const QString &username, int isPlaintext, int authType) {
    emit credentialRequested(storeUri, username, isPlaintext, authType);
}

static void on_message_count_complete_cb(uint64_t count, int error, void *user_data) {
    auto *bridge = static_cast<EventBridge *>(user_data);
    if (error == 0) {
        QMetaObject::invokeMethod(bridge, "folderReadyForMessages", Qt::QueuedConnection,
            Q_ARG(quint64, static_cast<quint64>(count)));
    }
}

void EventBridge::onFolderReady(const QString &folderUri) {
    auto *current = folderTree ? folderTree->currentItem() : nullptr;
    if (!current) {
        return;
    }
    QString realName = current->data(0, FolderNameRole).toString();
    if (realName != m_folderNameOpening) {
        return; /* stale: user selected a different folder */
    }
    setFolderUri(folderUri.toUtf8());
    tagliacarte_folder_message_count(m_folderUri.constData(), on_message_count_complete_cb, this);
}

void EventBridge::onOpenFolderError(const QString &message) {
    if (win) {
        QMessageBox::critical(win, TR("error.context.open_folder"), message);
    }
}

void EventBridge::showOpeningMessageCount(quint32 count) {
    if (statusBar) {
        statusBar->showMessage(TR_N("status.opening_messages", count));
    }
}

void EventBridge::startMessageLoading(quint64 total) {
    m_messageLoadTotal = total;
    m_messageLoadCount = 0;
    m_chatMessages.clear();
    if (m_loadProgressBar) {
        statusBar->removeWidget(m_loadProgressBar);
        delete m_loadProgressBar;
        m_loadProgressBar = nullptr;
    }
    if (statusBar && total > 0) {
        m_loadProgressBar = new QProgressBar();
        m_loadProgressBar->setRange(0, static_cast<int>(total));
        m_loadProgressBar->setValue(0);
        m_loadProgressBar->setMaximumWidth(200);
        m_loadProgressBar->setMaximumHeight(16);
        statusBar->addPermanentWidget(m_loadProgressBar);
    }
}

void EventBridge::addMessageSummary(const QString &id, const QString &subject, const QString &from, const QString &dateFormatted, qint64 timestampSecs, quint64 size, quint32 flags) {
    if (isConversationMode()) {
        m_chatMessages.append({subject, from.toLower(), timestampSecs});
        m_messageLoadCount++;
        if (m_loadProgressBar)
            m_loadProgressBar->setValue(static_cast<int>(m_messageLoadCount));
        return;
    }

    if (!conversationList) {
        return;
    }
    QString fromStr = from.trimmed();
    if (fromStr.isEmpty() || fromStr.compare(QLatin1String("(unknown)"), Qt::CaseInsensitive) == 0) {
        fromStr = TR("message.unknown_sender");
    }
    QString subj = subject.isEmpty() ? TR("message.no_subject") : subject;
    auto *item = new QTreeWidgetItem(QStringList() << fromStr << subj << dateFormatted);
    item->setData(0, MessageIdRole, id);
    item->setData(0, MessageFlagsRole, flags);
    item->setTextAlignment(2, Qt::AlignRight | Qt::AlignVCenter);  // date column right-aligned

    // Apply flag-based visual styling
    bool seen = (flags & TAGLIACARTE_FLAG_SEEN) != 0;
    bool deleted = (flags & TAGLIACARTE_FLAG_DELETED) != 0;
    for (int col = 0; col < 3; ++col) {
        QFont f = item->font(col);
        if (!seen) {
            f.setBold(true);  // unread messages are bold
        }
        if (deleted) {
            f.setStrikeOut(true);  // deleted messages have strikethrough
        }
        item->setFont(col, f);
    }

    conversationList->addTopLevelItem(item);

    m_messageLoadCount++;
    if (m_loadProgressBar) {
        m_loadProgressBar->setValue(static_cast<int>(m_messageLoadCount));
    }
}

void EventBridge::onMessageListComplete(int error) {
    if (m_loadProgressBar && statusBar) {
        statusBar->removeWidget(m_loadProgressBar);
        delete m_loadProgressBar;
        m_loadProgressBar = nullptr;
    }

    if (isConversationMode() && error == 0) {
        ensureProfilesFetched();
        renderChatMessages();
        if (statusBar) {
            statusBar->showMessage(TR_N("status.folder_messages_count", m_chatMessages.size()));
        }
        return;
    }

    if (statusBar && win) {
        if (error != 0) {
            showError(win, "error.context.list_conversations");
        } else if (conversationList) {
            statusBar->showMessage(m_folderNameOpening + QStringLiteral(" — ") + TR_N("status.folder_messages_count", conversationList->topLevelItemCount()));
        }
    }
    if (conversationList && error == 0) {
        int n = conversationList->topLevelItemCount();
        if (n > 0) {
            conversationList->scrollToItem(conversationList->topLevelItem(n - 1));
        }
    }
}

void EventBridge::showMessageMetadata(const QString &subject, const QString &from, const QString &to, const QString &date) {
    setLastMessage(from, to, subject, QString());
    // Populate header labels (outside QTextBrowser, unaffected by HTML backgrounds)
    if (headerFromLabel) {
        headerFromLabel->setText(QStringLiteral("<b>%1</b> %2").arg(TR("message.from_label"), from.toHtmlEscaped()));
    }
    if (headerToLabel) {
        headerToLabel->setText(QStringLiteral("<b>%1</b> %2").arg(TR("message.to_label"), to.toHtmlEscaped()));
    }
    if (headerSubjectLabel) {
        headerSubjectLabel->setText(QStringLiteral("<b>%1</b> %2").arg(TR("message.subject_label"), subject.toHtmlEscaped()));
    }
    if (messageHeaderPane) {
        messageHeaderPane->setVisible(true);
    }
    if (statusBar) {
        statusBar->showMessage(TR("status.receiving_message"));
    }
    m_messageBody.clear();
    m_lastMessageBodyPlain.clear();
    m_cidRegistry.clear();
    m_inlineHtmlParts.clear();
    m_alternativeGroupStart = -1;
    m_inMultipartAlternative = false;
    if (messageView) {
        messageView->setHtml(TR("status.loading"));
    }
    if (attachmentsPane) {
        QLayout *layout = attachmentsPane->layout();
        if (layout) {
            QLayoutItem *item;
            while ((item = layout->takeAt(0)) != nullptr) {
                delete item->widget();
                delete item;
            }
        }
        attachmentsPane->hide();
    }
}

// --- Streaming MIME entity events (mirror MimeHandler) ---

void EventBridge::onStartEntity() {
    m_entityContentType.clear();
    m_entityContentDisposition.clear();
    m_entityFilename.clear();
    m_entityContentId.clear();
    m_entityIsMultipart = false;
    m_entityIsAttachment = false;
    m_entityIsHtml = false;
    m_entityIsPlain = false;
    m_entityBuffer.clear();
}

void EventBridge::onContentType(const QString &value) {
    m_entityContentType = value.trimmed().toLower();
    m_entityIsMultipart = m_entityContentType.startsWith(QLatin1String("multipart/"));
    m_entityIsHtml = m_entityContentType.startsWith(QLatin1String("text/html"));
    m_entityIsPlain = m_entityContentType.startsWith(QLatin1String("text/plain"));
    // Track multipart/alternative groups for text/html replacing text/plain
    if (m_entityContentType.startsWith(QLatin1String("multipart/alternative"))) {
        m_inMultipartAlternative = true;
        m_alternativeGroupStart = m_inlineHtmlParts.size();
    }
}

void EventBridge::onContentDisposition(const QString &value) {
    m_entityContentDisposition = value.trimmed().toLower();
    m_entityIsAttachment = m_entityContentDisposition.startsWith(QLatin1String("attachment"));
    // Extract filename (useful for save-as label, but does NOT imply attachment —
    // inline CID images often have filenames like "image001.png")
    static QRegularExpression filenameRe(QStringLiteral("filename\\*?=\"?([^\";\r\n]+)\"?"),
                                         QRegularExpression::CaseInsensitiveOption);
    auto match = filenameRe.match(value);
    if (match.hasMatch()) {
        m_entityFilename = match.captured(1).trimmed();
    }
}

void EventBridge::onContentId(const QString &value) {
    // Angle brackets already stripped by the FFI layer
    m_entityContentId = value.trimmed();
}

void EventBridge::onEndHeaders() {
    if (m_entityIsMultipart) {
        return;  // multipart containers have no displayable body
    }
    // For text/plain not in alternative: set up progressive display
    if (m_entityIsPlain && !m_entityIsAttachment && !m_inMultipartAlternative) {
        if (messageView) {
            // Show existing composite + empty <pre> for progressive text append
            messageView->setHtml(m_inlineHtmlParts.join(QString()) + QStringLiteral("<pre></pre>"));
        }
    }
}

void EventBridge::onBodyContent(const QByteArray &data) {
    if (m_entityIsMultipart) {
        return;
    }
    // Always buffer for onEndEntity processing
    m_entityBuffer.append(data);
    // Progressive display for text/plain (not in alternative, not attachment)
    if (m_entityIsPlain && !m_entityIsAttachment && !m_inMultipartAlternative && messageView) {
        QTextCursor cursor = messageView->textCursor();
        cursor.movePosition(QTextCursor::End);
        cursor.insertText(QString::fromUtf8(data));
        messageView->setTextCursor(cursor);
    }
}

void EventBridge::onEndEntity() {
    if (m_entityIsMultipart) {
        // multipart container ending
        if (m_entityContentType.startsWith(QLatin1String("multipart/alternative"))) {
            m_inMultipartAlternative = false;
        }
        return;
    }
    if (!messageView) {
        return;
    }

    // Register CID resource if Content-ID is present.
    // CidTextBrowser::loadResource looks up directly from the registry on demand,
    // so we do NOT use QTextDocument::addResource (which gets cleared by setHtml).
    if (!m_entityContentId.isEmpty() && !m_entityBuffer.isEmpty()) {
        m_cidRegistry.insert(m_entityContentId, m_entityBuffer);
    }

    if (m_entityIsAttachment) {
        // Attachment: create save button in attachments pane
        if (attachmentsPane && !m_entityBuffer.isEmpty()) {
            QLayout *layout = attachmentsPane->layout();
            if (!layout) {
                layout = new QHBoxLayout(attachmentsPane);
                layout->setContentsMargins(0, 4, 0, 0);
            }
            if (layout->count() == 0) {
                layout->addWidget(new QLabel(TR("message.attachments") + QStringLiteral(":"), attachmentsPane));
            }
            QString label = m_entityFilename.isEmpty() ? QStringLiteral("unnamed") : m_entityFilename;
            auto *btn = new QPushButton(label, attachmentsPane);
            QByteArray data = m_entityBuffer;  // copy for lambda capture
            QObject::connect(btn, &QPushButton::clicked, [label, data]() {
                QString path = QFileDialog::getSaveFileName(nullptr, QString(), label);
                if (!path.isEmpty()) {
                    QFile f(path);
                    if (f.open(QIODevice::WriteOnly)) {
                        f.write(data);
                        f.close();
                    }
                }
            });
            layout->addWidget(btn);
            attachmentsPane->setVisible(true);
        }
    } else if (m_entityIsHtml) {
        // Inline HTML: sanitize and add to composite display
        QString html = sanitizeHtml(QString::fromUtf8(m_entityBuffer));
        if (!html.isEmpty()) {
            if (m_inMultipartAlternative && m_alternativeGroupStart >= 0) {
                // Replace text/plain fallback with HTML within this alternative group
                while (m_inlineHtmlParts.size() > m_alternativeGroupStart) {
                    m_inlineHtmlParts.removeLast();
                }
            }
            m_inlineHtmlParts.append(html);
            m_messageBody = m_inlineHtmlParts.join(QString());
            messageView->setHtml(m_messageBody);
        }
    } else if (m_entityIsPlain && !m_entityIsAttachment) {
        // Inline text/plain: finalize into composite display
        QString plainText = QString::fromUtf8(m_entityBuffer);
        m_lastMessageBodyPlain.append(plainText);
        QString htmlFragment = QStringLiteral("<pre>") + plainText.toHtmlEscaped() + QStringLiteral("</pre>");
        m_inlineHtmlParts.append(htmlFragment);
        m_messageBody = m_inlineHtmlParts.join(QString());
        messageView->setHtml(m_messageBody);
        setLastMessage(m_lastMessageFrom, m_lastMessageTo, m_lastMessageSubject, m_lastMessageBodyPlain);
    } else if (!m_entityContentId.isEmpty()) {
        // Non-text CID resource (image, etc.) — already registered above
        // Re-render to pick up the new resource in existing HTML
        if (!m_messageBody.isEmpty()) {
            messageView->setHtml(m_messageBody);
        }
    }

    m_entityBuffer.clear();
}

void EventBridge::onMessageComplete(int error) {
    if (win && error != 0) {
        showError(win, "error.context.load_message");
    }
    if (messageView && error == 0 && m_inlineHtmlParts.isEmpty() && m_messageBody.isEmpty()) {
        messageView->setHtml(TR("message.no_body_html"));
    }
    if (statusBar) {
        if (error != 0) {
            statusBar->showMessage(TR("status.message_load_error"));
        } else {
            statusBar->showMessage(TR("status.message_loaded"), 3000);
        }
    }
    if (error == 0 && conversationList) {
        auto *item = conversationList->currentItem();
        if (item) {
            quint32 flags = item->data(0, MessageFlagsRole).toUInt();
            if (!(flags & TAGLIACARTE_FLAG_SEEN)) {
                flags |= TAGLIACARTE_FLAG_SEEN;
                item->setData(0, MessageFlagsRole, flags);
                for (int col = 0; col < 3; ++col) {
                    QFont f = item->font(col);
                    f.setBold(false);
                    item->setFont(col, f);
                }
            }
        }
    }
}

// --- HTML sanitization ---

QString EventBridge::sanitizeHtml(const QString &html) {
    QString out = html;
    // Strip <script> tags and their content
    out.remove(QRegularExpression(QStringLiteral("<script[^>]*>.*?</script>"),
        QRegularExpression::CaseInsensitiveOption | QRegularExpression::DotMatchesEverythingOption));
    out.remove(QRegularExpression(QStringLiteral("<script[^>]*/?>"),
        QRegularExpression::CaseInsensitiveOption));
    // Strip <form> tags and their content
    out.remove(QRegularExpression(QStringLiteral("<form[^>]*>.*?</form>"),
        QRegularExpression::CaseInsensitiveOption | QRegularExpression::DotMatchesEverythingOption));
    out.remove(QRegularExpression(QStringLiteral("<form[^>]*/?>"),
        QRegularExpression::CaseInsensitiveOption));
    return out;
}

void EventBridge::onSendProgress(const QString &status) {
    if (statusBar && !status.isEmpty()) {
        statusBar->showMessage(status.left(1).toUpper() + status.mid(1) + QStringLiteral("…"));
    }
}

void EventBridge::onSendComplete(int ok) {
    if (m_pendingSendTransportUri) {
        tagliacarte_transport_free(m_pendingSendTransportUri);
        tagliacarte_free_string(m_pendingSendTransportUri);
        m_pendingSendTransportUri = nullptr;
    }
    if (statusBar && win) {
        if (ok != 0) {
            showError(win, "error.context.send");
        } else {
            statusBar->showMessage(TR("status.message_sent"));
            emit messageSent();
        }
    }
}

void EventBridge::onFolderOpError(const QString &message) {
    if (win) {
        QMessageBox::warning(win, TR("error.context.create_folder"), message);
    }
}

void EventBridge::onBulkComplete(int ok, const QString &errorMessage) {
    if (ok != 0 && win) {
        QMessageBox::warning(win, TR("error.context.bulk_operation"),
            errorMessage.isEmpty() ? TR("error.unknown") : errorMessage);
    } else if (statusBar) {
        statusBar->showMessage(TR("status.operation_complete"), 3000);
    }
}
