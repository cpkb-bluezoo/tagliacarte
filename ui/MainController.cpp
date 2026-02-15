#include "MainController.h"
#include "Config.h"
#include "IconUtils.h"
#include "Callbacks.h"
#include "EventBridge.h"
#include "CidTextBrowser.h"
#include "ComposeDialog.h"
#include "Tr.h"
#include "tagliacarte.h"

#include <QMainWindow>
#include <QToolButton>
#include <QTreeWidget>
#include <QTreeWidgetItem>
#include <QTextBrowser>
#include <QStackedWidget>
#include <QVBoxLayout>
#include <QStatusBar>
#include <QFont>
#include <QMessageBox>
#include <QFile>
#include <QFileInfo>

MainController::MainController(QObject *parent)
    : QObject(parent)
{
}

void MainController::updateComposeAppendButtons()
{
    bool hasTransport = !smtpTransportUri.isEmpty();
    composeBtn->setVisible(hasTransport);
    composeBtn->setEnabled(hasTransport);
    appendMessageBtn->setVisible(!hasTransport);
    appendMessageBtn->setEnabled(!hasTransport && !bridge->folderUri().isEmpty());
}

void MainController::updateMessageActionButtons()
{
    bool hasMessage = !bridge->folderUri().isEmpty() && conversationList->currentItem() != nullptr;
    bool hasTransport = !smtpTransportUri.isEmpty();
    replyBtn->setEnabled(hasMessage && hasTransport);
    replyAllBtn->setEnabled(hasMessage && hasTransport);
    forwardBtn->setEnabled(hasMessage && hasTransport);
    junkBtn->setEnabled(hasMessage);
    moveBtn->setEnabled(hasMessage);
    deleteBtn->setEnabled(hasMessage);
}

void MainController::addStoreCircle(const QString &initial, const QByteArray &uri, int colourIndex)
{
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
    QObject::connect(btn, &QToolButton::clicked, this, [this, btn]() {
        QVariant v = btn->property("storeUri");
        if (!v.isValid()) {
            return;
        }
        QByteArray u = v.toByteArray();
        if (u.isEmpty()) {
            return;
        }
        selectStore(u);
    });
    if (storeButtons.size() == 1) {
        btn->setChecked(true);
    }
}

QByteArray MainController::createStoreFromEntry(const StoreEntry &entry)
{
    QByteArray uri;
    if (entry.type == QLatin1String("maildir") && !storeHostOrPath(entry).isEmpty()) {
        char *uriPtr = tagliacarte_store_maildir_new(storeHostOrPath(entry).toUtf8().constData());
        if (!uriPtr) {
            return {};
        }
        uri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
    } else if (entry.type == QLatin1String("imap") && !storeHostOrPath(entry).isEmpty()) {
        QString host = storeHostOrPath(entry);
        QString userAtHost = param(entry, "username");
        if (userAtHost.isEmpty()) {
            userAtHost = host;
        }
        if (!userAtHost.contains(QLatin1Char('@'))) {
            userAtHost = userAtHost + QLatin1Char('@') + host;
        }
        const uint16_t imapPort = static_cast<uint16_t>(qBound(1, paramInt(entry, "port", 993), 65535));
        char *uriPtr = tagliacarte_store_imap_new(userAtHost.toUtf8().constData(), host.toUtf8().constData(), imapPort);
        if (!uriPtr) {
            return {};
        }
        uri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (!param(entry, "transportHostname").isEmpty()) {
            char *tUri = tagliacarte_transport_smtp_new(
                param(entry, "transportHostname").toUtf8().constData(),
                static_cast<uint16_t>(qBound(1, paramInt(entry, "transportPort", 586), 65535)));
            if (tUri) {
                storeToTransport[uri] = QByteArray(tUri);
                tagliacarte_free_string(tUri);
            }
        }
    } else if (entry.type == QLatin1String("pop3") && !storeHostOrPath(entry).isEmpty()) {
        QString host = storeHostOrPath(entry);
        QString userAtHost = param(entry, "username");
        if (userAtHost.isEmpty()) {
            userAtHost = host;
        }
        if (!userAtHost.contains(QLatin1Char('@'))) {
            userAtHost = userAtHost + QLatin1Char('@') + host;
        }
        char *uriPtr = tagliacarte_store_pop3_new(userAtHost.toUtf8().constData(), host.toUtf8().constData(), 995);
        if (!uriPtr) {
            return {};
        }
        uri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (!param(entry, "transportHostname").isEmpty()) {
            char *tUri = tagliacarte_transport_smtp_new(
                param(entry, "transportHostname").toUtf8().constData(),
                static_cast<uint16_t>(qBound(1, paramInt(entry, "transportPort", 586), 65535)));
            if (tUri) {
                storeToTransport[uri] = QByteArray(tUri);
                tagliacarte_free_string(tUri);
            }
        }
    } else if (entry.type == QLatin1String("nostr") && !storeHostOrPath(entry).isEmpty()) {
        const char *kp = param(entry, "keyPath").isEmpty() ? nullptr : param(entry, "keyPath").toUtf8().constData();
        char *uriPtr = tagliacarte_store_nostr_new(storeHostOrPath(entry).toUtf8().constData(), kp);
        if (!uriPtr) {
            return {};
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
        char *uriPtr = tagliacarte_store_matrix_new(
            storeHostOrPath(entry).toUtf8().constData(),
            param(entry, "userId").toUtf8().constData(), token);
        if (!uriPtr) {
            return {};
        }
        uri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        char *tUri = tagliacarte_transport_matrix_new(
            storeHostOrPath(entry).toUtf8().constData(),
            param(entry, "userId").toUtf8().constData(), token);
        if (tUri) {
            storeToTransport[uri] = QByteArray(tUri);
            tagliacarte_free_string(tUri);
        }
    }
    return uri;
}

void MainController::refreshStoresFromConfig()
{
    // Tear down existing stores
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
    bridge->clearFolder();
    folderTree->clear();
    conversationList->clear();
    messageView->clear();
    messageHeaderPane->hide();

    // Re-create from config
    Config c = loadConfig();
    for (int i = 0; i < c.stores.size(); ++i) {
        const StoreEntry &entry = c.stores[i];
        QByteArray uri = createStoreFromEntry(entry);
        if (uri.isEmpty()) {
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
        tagliacarte_store_set_folder_list_callbacks(uri.constData(),
            on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, bridge);
    }
    if (!allStoreUris.isEmpty()) {
        QString lastId = c.lastSelectedStoreId;
        if (!lastId.isEmpty()) {
            QByteArray lastUtf8 = lastId.toUtf8();
            for (const QByteArray &u : allStoreUris) {
                if (u == lastUtf8) {
                    storeUri = u;
                    break;
                }
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
        win->statusBar()->showMessage(TR("status.folders_loaded"));
    } else {
        storeUri.clear();
    }
}

void MainController::selectStore(const QByteArray &uri)
{
    storeUri = uri;
    smtpTransportUri = storeToTransport.value(storeUri);
    updateComposeAppendButtons();
    bridge->clearFolder();
    folderTree->clear();
    conversationList->clear();
    messageView->clear();
    messageHeaderPane->hide();
    updateMessageActionButtons();
    for (auto *b : storeButtons) {
        b->setChecked(b->property("storeUri").toByteArray() == storeUri);
    }
    tagliacarte_store_set_folder_list_callbacks(storeUri.constData(),
        on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, bridge);
    tagliacarte_store_refresh_folders(storeUri.constData());
    win->statusBar()->showMessage(TR("status.folders_loaded"));
}

void MainController::shutdown()
{
    bridge->clearFolder();
    for (const QByteArray &u : allStoreUris) {
        tagliacarte_store_free(u.constData());
    }
    for (const QByteArray &t : storeToTransport) {
        tagliacarte_transport_free(t.constData());
    }
    allStoreUris.clear();
    storeToTransport.clear();
}

// --- Compose / message action methods ---

QString MainController::buildQuotedBody(const QString &original, const QString &header, const Config &c)
{
    QString quoted = original;
    if (c.quoteUsePrefix && !c.quotePrefix.isEmpty()) {
        QStringList lines = quoted.split(QLatin1Char('\n'));
        for (QString &line : lines) {
            line = c.quotePrefix + line;
        }
        quoted = lines.join(QLatin1Char('\n'));
    }
    return QStringLiteral("\n\n") + header + QLatin1String("\n\n") + quoted;
}

void MainController::sendFromComposeDialog(ComposeDialog &dlg)
{
    QString from = dlg.fromEdit->text().trimmed();
    QString to = dlg.toEdit->text().trimmed();
    QString cc = dlg.ccEdit->text().trimmed();
    QString subject = dlg.subjectEdit->text().trimmed();
    QString body = dlg.bodyEdit->toPlainText();

    if (from.isEmpty() || to.isEmpty()) {
        QMessageBox::warning(win, TR("compose.title"), TR("compose.validation.from_to"));
        return;
    }

    QVector<ComposePart> parts = dlg.parts();
    bool hasMessageParts = false;
    for (const ComposePart &p : parts) {
        if (p.type == ComposePartMessage) {
            hasMessageParts = true;
            break;
        }
    }
    if (hasMessageParts) {
        QMessageBox::information(win, TR("compose.title"), TR("compose.parts.message_not_implemented"));
        return;
    }

    QVector<TagliacarteAttachment> fileAttachments;
    QVector<QByteArray> fileDataHolder;
    QVector<QByteArray> fileNamesHolder;
    for (const ComposePart &p : parts) {
        if (p.type == ComposePartFile) {
            QFile f(p.pathOrDisplay);
            if (!f.open(QIODevice::ReadOnly)) {
                QMessageBox::warning(win, TR("compose.title"), TR("compose.attach_file_read_error"));
                return;
            }
            QByteArray data = f.readAll();
            f.close();
            if (data.isEmpty()) {
                continue;
            }
            fileDataHolder.append(data);
            fileNamesHolder.append(QFileInfo(p.pathOrDisplay).fileName().toUtf8());
            TagliacarteAttachment att;
            att.filename = fileNamesHolder.last().constData();
            att.mime_type = "application/octet-stream";
            att.data = reinterpret_cast<const uint8_t *>(fileDataHolder.last().constData());
            att.data_len = static_cast<size_t>(fileDataHolder.last().size());
            fileAttachments.append(att);
        }
    }

    QByteArray fromUtf8 = from.toUtf8();
    QByteArray toUtf8 = to.toUtf8();
    const char *ccPtr = cc.isEmpty() ? nullptr : cc.toUtf8().constData();
    QByteArray subjUtf8 = subject.toUtf8();
    QByteArray bodyUtf8 = body.toUtf8();

    size_t attachmentCount = fileAttachments.size();
    const TagliacarteAttachment *attachmentsPtr = fileAttachments.isEmpty() ? nullptr : fileAttachments.constData();

    win->statusBar()->showMessage(TR("status.sending"));
    tagliacarte_transport_send_async(
        smtpTransportUri.constData(),
        fromUtf8.constData(),
        toUtf8.constData(),
        ccPtr,
        subjUtf8.constData(),
        bodyUtf8.constData(),
        nullptr,
        attachmentCount,
        attachmentsPtr,
        on_send_progress_cb,
        on_send_complete_cb,
        bridge
    );
}

void MainController::connectComposeActions()
{
    QObject::connect(replyBtn, &QToolButton::clicked, this, [this]() {
        if (smtpTransportUri.isEmpty()) {
            return;
        }
        Config c = loadConfig();
        QString to = bridge->lastMessageFrom();
        QString subject = bridge->lastMessageSubject();
        QString reSubject = subject.startsWith(QLatin1String("Re:")) ? subject : (QStringLiteral("Re: ") + subject);
        QString body = bridge->lastMessageBodyPlain();
        QString header = TR("message.quoted_on").arg(bridge->lastMessageFrom());
        QString quotedBody = body.isEmpty() ? QString() : buildQuotedBody(body, header, c);
        bool cursorBefore = (c.replyPosition == QLatin1String("before"));
        ComposeDialog dlg(win, smtpTransportUri, QString(), to, QString(), reSubject, quotedBody, cursorBefore);
        if (dlg.exec() != QDialog::Accepted) {
            return;
        }
        sendFromComposeDialog(dlg);
    });

    QObject::connect(replyAllBtn, &QToolButton::clicked, this, [this]() {
        if (smtpTransportUri.isEmpty()) {
            return;
        }
        Config c = loadConfig();
        QString to = bridge->lastMessageFrom();
        QString cc = bridge->lastMessageTo();
        QString subject = bridge->lastMessageSubject();
        QString reSubject = subject.startsWith(QLatin1String("Re:")) ? subject : (QStringLiteral("Re: ") + subject);
        QString body = bridge->lastMessageBodyPlain();
        QString header = TR("message.quoted_on").arg(bridge->lastMessageFrom());
        QString quotedBody = body.isEmpty() ? QString() : buildQuotedBody(body, header, c);
        bool cursorBefore = (c.replyPosition == QLatin1String("before"));
        ComposeDialog dlg(win, smtpTransportUri, QString(), to, cc, reSubject, quotedBody, cursorBefore);
        if (dlg.exec() != QDialog::Accepted) {
            return;
        }
        sendFromComposeDialog(dlg);
    });

    QObject::connect(forwardBtn, &QToolButton::clicked, this, [this]() {
        if (smtpTransportUri.isEmpty()) {
            return;
        }
        Config c = loadConfig();
        QString subject = bridge->lastMessageSubject();
        QString fwdSubject = subject.startsWith(QLatin1String("Fwd:")) ? subject : (QStringLiteral("Fwd: ") + subject);
        QString body = bridge->lastMessageBodyPlain();
        QString forwardMode = c.forwardMode.isEmpty() ? QStringLiteral("inline") : c.forwardMode;

        if (forwardMode == QLatin1String("embedded") || forwardMode == QLatin1String("attachment")) {
            auto *item = conversationList->currentItem();
            QByteArray folderUri = bridge->folderUri();
            if (!item || folderUri.isEmpty()) {
                return;
            }
            QVariant idVar = item->data(0, MessageIdRole);
            if (!idVar.isValid()) {
                return;
            }
            QByteArray id = idVar.toString().toUtf8();
            QString display = bridge->lastMessageSubject().isEmpty() ? TR("message.no_subject") : bridge->lastMessageSubject();
            ComposeDialog dlg(win, smtpTransportUri, QString(), QString(), QString(), fwdSubject, QString());
            dlg.addPartMessage(folderUri, id, display, forwardMode == QLatin1String("attachment"));
            if (dlg.exec() != QDialog::Accepted) {
                return;
            }
            sendFromComposeDialog(dlg);
        } else {
            QString header = TR("message.quoted_forward");
            QString quotedBody = body.isEmpty() ? QString() : buildQuotedBody(body, header, c);
            ComposeDialog dlg(win, smtpTransportUri, QString(), QString(), QString(), fwdSubject, quotedBody);
            if (dlg.exec() != QDialog::Accepted) {
                return;
            }
            sendFromComposeDialog(dlg);
        }
    });

    QObject::connect(junkBtn, &QToolButton::clicked, this, [this]() {
        QMessageBox::information(win, TR("message.junk.tooltip"), TR("message.junk.not_implemented"));
    });

    QObject::connect(moveBtn, &QToolButton::clicked, this, [this]() {
        QMessageBox::information(win, TR("message.move.tooltip"), TR("message.move.not_implemented"));
    });

    QObject::connect(deleteBtn, &QToolButton::clicked, this, [this]() {
        auto *item = conversationList->currentItem();
        QByteArray folderUri = bridge->folderUri();
        if (!item || folderUri.isEmpty()) {
            return;
        }
        QVariant idVar = item->data(0, MessageIdRole);
        if (!idVar.isValid()) {
            return;
        }
        QByteArray id = idVar.toString().toUtf8();
        int r = tagliacarte_folder_delete_message(folderUri.constData(), id.constData());
        if (r != 0) {
            showError(win, "error.context.delete_message");
            return;
        }
        delete conversationList->takeTopLevelItem(conversationList->indexOfTopLevelItem(item));
        messageView->clear();
        messageHeaderPane->hide();
        bridge->clearLastMessage();
        updateMessageActionButtons();
        win->statusBar()->showMessage(TR("status.message_deleted"));
    });

    QObject::connect(composeBtn, &QToolButton::clicked, this, [this]() {
        if (smtpTransportUri.isEmpty()) {
            return;  // no transport for current store (button should be disabled)
        }
        ComposeDialog dlg(win, smtpTransportUri);
        if (dlg.exec() != QDialog::Accepted) {
            return;
        }
        sendFromComposeDialog(dlg);
    });
}
