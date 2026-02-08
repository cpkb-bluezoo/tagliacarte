#include "EventBridge.h"
#include "tagliacarte.h"
#include <QListWidgetItem>
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
    if (folderList) folderList->addItem(name);
}

void EventBridge::onFolderListComplete(int error) {
    if (statusBar && win) {
        if (error != 0) showError(win, "List folders");
        else if (folderList) statusBar->showMessage(QString("%1 folder(s)").arg(folderList->count()));
    }
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
        QMessageBox::critical(win, "Open folder", message);
    }
}

void EventBridge::showOpeningMessageCount(quint32 count) {
    if (statusBar) {
        statusBar->showMessage(QString("Opening… %1 messages").arg(count));
    }
}

void EventBridge::addMessageSummary(const QString &id, const QString &subject, const QString &from, quint64 size) {
    if (!conversationList) return;
    QString fromStr = from.isEmpty() ? QStringLiteral("(unknown)") : from;
    QString subj = subject.isEmpty() ? QStringLiteral("(no subject)") : subject;
    auto *item = new QListWidgetItem(QString("%1 — %2").arg(fromStr, subj));
    item->setData(MessageIdRole, id);
    conversationList->addItem(item);
}

void EventBridge::onMessageListComplete(int error) {
    if (statusBar && win && error != 0) showError(win, "List conversations");
}

void EventBridge::showMessageContent(const QString &subject, const QString &from, const QString &to, const QString &bodyHtml, const QString &bodyPlain) {
    if (!messageView) return;
    QString html;
    if (!bodyHtml.isEmpty()) html = bodyHtml;
    else if (!bodyPlain.isEmpty()) html = bodyPlain.toHtmlEscaped().replace("\n", "<br>\n");
    else html = "<em>No body</em>";
    QString header = QString("<p><b>From:</b> %1<br><b>To:</b> %2<br><b>Subject:</b> %3</p><hr>")
        .arg(from.toHtmlEscaped(), to.toHtmlEscaped(), subject.toHtmlEscaped());
    messageView->setHtml(header + html);
}

void EventBridge::onMessageComplete(int error) {
    if (win && error != 0) showError(win, "Load message");
}

void EventBridge::onSendProgress(const QString &status) {
    if (statusBar && !status.isEmpty())
        statusBar->showMessage(status.left(1).toUpper() + status.mid(1) + QStringLiteral("…"));
}

void EventBridge::onSendComplete(int ok) {
    if (m_pendingSendTransportUri) {
        tagliacarte_transport_free(m_pendingSendTransportUri);
        tagliacarte_free_string(m_pendingSendTransportUri);
        m_pendingSendTransportUri = nullptr;
    }
    if (statusBar && win) {
        if (ok != 0) showError(win, "Send");
        else statusBar->showMessage(QStringLiteral("Message sent."));
    }
}
