#ifndef EVENTBRIDGE_H
#define EVENTBRIDGE_H

#include <QObject>
#include <QListWidget>
#include <QTextBrowser>
#include <QStatusBar>
#include <QWidget>
#include <QString>

void showError(QWidget *parent, const char *context);

static const int MessageIdRole = Qt::UserRole;

class EventBridge : public QObject {
    Q_OBJECT
public:
    QListWidget *folderList = nullptr;
    QListWidget *conversationList = nullptr;
    QTextBrowser *messageView = nullptr;
    QStatusBar *statusBar = nullptr;
    QWidget *win = nullptr;

    QByteArray folderUri() const { return m_folderUri; }
    void setFolderUri(const QByteArray &uri);
    void clearFolder();
    void setFolderNameOpening(const QString &name) { m_folderNameOpening = name; }
    /** Call with the pointer returned from tagliacarte_transport_smtp_new; bridge frees it in onSendComplete. */
    void setPendingSendTransport(char *uri) { m_pendingSendTransportUri = uri; }

public Q_SLOTS:
    void addFolder(const QString &name);
    void onFolderListComplete(int error);
    void onFolderReady(const QString &folderUri);
    void onOpenFolderError(const QString &message);
    void showOpeningMessageCount(quint32 count);
    void addMessageSummary(const QString &id, const QString &subject, const QString &from, quint64 size);
    void onMessageListComplete(int error);
    void showMessageContent(const QString &subject, const QString &from, const QString &to, const QString &bodyHtml, const QString &bodyPlain);
    void onMessageComplete(int error);
    void onSendProgress(const QString &status);
    void onSendComplete(int ok);

Q_SIGNALS:
    void folderReadyForMessages(quint64 total);

private:
    QByteArray m_folderUri;
    QString m_folderNameOpening;
    char *m_pendingSendTransportUri = nullptr;
};

#endif // EVENTBRIDGE_H
