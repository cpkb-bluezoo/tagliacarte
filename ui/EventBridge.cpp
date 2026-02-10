#include "EventBridge.h"
#include "Tr.h"
#include "tagliacarte.h"
#include <QTreeWidgetItem>
#include <QMessageBox>

void EventBridge::setFolderUri(const QByteArray &uri) {
    if (!m_folderUri.isEmpty()) {
        tagliacarte_folder_free(m_folderUri.constData());
    }
    m_folderUri = uri;
}

void EventBridge::clearFolder() {
    if (!m_folderUri.isEmpty()) {
        tagliacarte_folder_free(m_folderUri.constData());
        m_folderUri.clear();
    }
}

void EventBridge::addFolder(const QString &name) {
    if (folderList) {
        if (name.compare(QStringLiteral("INBOX"), Qt::CaseInsensitive) == 0) {
            folderList->insertItem(0, name);
        } else {
            folderList->addItem(name);
        }
    }
}

void EventBridge::onFolderListComplete(int error) {
    if (statusBar && win) {
        if (error == TAGLIACARTE_NEEDS_CREDENTIAL) {
            /* Credential request already triggered via callback; don't show generic error */
            return;
        }
        if (error != 0) {
            showError(win, "error.context.list_folders");
        } else if (folderList) {
            statusBar->showMessage(TR_N("status.folders_count", folderList->count()));
        }
    }
}

void EventBridge::requestCredentialSlot(const QString &storeUri, const QString &username, int isPlaintext) {
    emit credentialRequested(storeUri, username, isPlaintext);
}

void EventBridge::onFolderReady(const QString &folderUri) {
    auto *current = folderList ? folderList->currentItem() : nullptr;
    if (!current || current->text() != m_folderNameOpening) {
        return; /* stale: user selected a different folder */
    }
    setFolderUri(folderUri.toUtf8());
    uint64_t total = tagliacarte_folder_message_count(m_folderUri.constData());
    emit folderReadyForMessages(total);
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

void EventBridge::addMessageSummary(const QString &id, const QString &subject, const QString &from, const QString &dateFormatted, quint64 size) {
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
    conversationList->addTopLevelItem(item);
}

void EventBridge::onMessageListComplete(int error) {
    if (statusBar && win) {
        if (error != 0) {
            showError(win, "error.context.list_conversations");
        } else if (conversationList) {
            statusBar->showMessage(m_folderNameOpening + QStringLiteral(" — ") + TR_N("status.folder_messages_count", conversationList->topLevelItemCount()));
        }
    }
}

void EventBridge::showMessageContent(const QString &subject, const QString &from, const QString &to, const QString &bodyHtml, const QString &bodyPlain) {
    if (!messageView) {
        return;
    }
    QString html;
    if (!bodyHtml.isEmpty()) {
        html = bodyHtml;
    } else if (!bodyPlain.isEmpty()) {
        // Keep using the HTML widget; feed processed HTML: wrap text/plain in <pre> with explicit charset.
        html = QStringLiteral("<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body><pre>")
            + bodyPlain.toHtmlEscaped()
            + QStringLiteral("</pre></body></html>");
    } else {
        html = TR("message.no_body_html");
    }
    QString header = QString("<p><b>%1</b> %2<br><b>%3</b> %4<br><b>%5</b> %6</p><hr>")
        .arg(TR("message.from_label"), from.toHtmlEscaped(),
             TR("message.to_label"), to.toHtmlEscaped(),
             TR("message.subject_label"), subject.toHtmlEscaped());
    messageView->setHtml(header + html);
}

void EventBridge::onMessageComplete(int error) {
    if (win && error != 0) {
        showError(win, "error.context.load_message");
    }
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
        }
    }
}
