#ifndef EVENTBRIDGE_H
#define EVENTBRIDGE_H

#include <QObject>
#include <QListWidget>
#include <QTreeWidget>
#include <QTextBrowser>
#include <QStatusBar>
#include <QWidget>
#include <QLabel>
#include <QString>
#include <QVariantList>
#include <QMap>
#include <QStringList>

void showError(QWidget *parent, const char *context);

static const int MessageIdRole = Qt::UserRole;

class EventBridge : public QObject {
    Q_OBJECT
public:
    QListWidget *folderList = nullptr;
    QTreeWidget *conversationList = nullptr;  // columns: From, Subject, Date (sortable; supports hierarchy for thread view later)
    QTextBrowser *messageView = nullptr;
    QWidget *attachmentsPane = nullptr;  // optional; child widgets are attachment buttons, cleared/repopulated per message
    QStatusBar *statusBar = nullptr;
    QWidget *win = nullptr;
    // Message header labels (outside QTextBrowser, not affected by HTML backgrounds)
    QWidget *messageHeaderPane = nullptr;
    QLabel *headerFromLabel = nullptr;
    QLabel *headerToLabel = nullptr;
    QLabel *headerSubjectLabel = nullptr;

    QByteArray folderUri() const { return m_folderUri; }
    void setFolderUri(const QByteArray &uri);
    /** Pointer to the CID resource registry for CidTextBrowser to look up cid: URLs. */
    const QMap<QString, QByteArray> *cidRegistryPtr() const { return &m_cidRegistry; }
    void clearFolder();
    void setFolderNameOpening(const QString &name) { m_folderNameOpening = name; }
    /** Last displayed message (for Reply/Forward). Cleared when selection changes. */
    QString lastMessageFrom() const { return m_lastMessageFrom; }
    QString lastMessageTo() const { return m_lastMessageTo; }
    QString lastMessageSubject() const { return m_lastMessageSubject; }
    QString lastMessageBodyPlain() const { return m_lastMessageBodyPlain; }
    void setLastMessage(const QString &from, const QString &to, const QString &subject, const QString &bodyPlain);
    void clearLastMessage();
    /** Call with the pointer returned from tagliacarte_transport_smtp_new; bridge frees it in onSendComplete. */
    void setPendingSendTransport(char *uri) { m_pendingSendTransportUri = uri; }

public Q_SLOTS:
    void addFolder(const QString &name);
    void onFolderListComplete(int error);
    /** Called from C credential callback (marshal to main thread). Emits credentialRequested. */
    void requestCredentialSlot(const QString &storeUri, const QString &username, int isPlaintext);
    void onFolderReady(const QString &folderUri);
    void onOpenFolderError(const QString &message);
    void showOpeningMessageCount(quint32 count);
    void addMessageSummary(const QString &id, const QString &subject, const QString &from, const QString &dateFormatted, quint64 size);
    void onMessageListComplete(int error);
    void showMessageMetadata(const QString &subject, const QString &from, const QString &to, const QString &date);
    void onStartEntity();
    void onContentType(const QString &value);
    void onContentDisposition(const QString &value);
    void onContentId(const QString &value);
    void onEndHeaders();
    void onBodyContent(const QByteArray &data);
    void onEndEntity();
    void onMessageComplete(int error);
    void onSendProgress(const QString &status);
    void onSendComplete(int ok);

Q_SIGNALS:
    void folderReadyForMessages(quint64 total);
    /** Core needs a credential; show password dialog then call tagliacarte_credential_provide or tagliacarte_credential_cancel. */
    void credentialRequested(const QString &storeUri, const QString &username, int isPlaintext);

private:
    QByteArray m_folderUri;
    QString m_folderNameOpening;
    char *m_pendingSendTransportUri = nullptr;
    QString m_lastMessageFrom;
    QString m_lastMessageTo;
    QString m_lastMessageSubject;
    QString m_lastMessageBodyPlain;
    QString m_messageBody;     // body HTML, built from content parts

    // Per-entity state for streaming MIME events
    QString m_entityContentType;         // content-type of current entity
    QString m_entityContentDisposition;  // content-disposition of current entity
    QString m_entityFilename;            // filename extracted from disposition
    QString m_entityContentId;           // bare Content-ID (angle brackets stripped)
    bool    m_entityIsMultipart = false;  // true for multipart/* containers
    bool    m_entityIsAttachment = false; // true if disposition=attachment or has filename
    bool    m_entityIsHtml = false;       // true for text/html
    bool    m_entityIsPlain = false;      // true for text/plain
    QByteArray m_entityBuffer;           // accumulated body for HTML/attachments

    // Per-message state (cleared in showMessageMetadata)
    QMap<QString, QByteArray> m_cidRegistry;  // Content-ID -> raw bytes (for cid: resolution)
    QStringList m_inlineHtmlParts;            // accumulated HTML fragments for composite display

    // multipart/alternative tracking
    int  m_alternativeGroupStart = -1;   // index into m_inlineHtmlParts at start of alternative group
    bool m_inMultipartAlternative = false;

    static QString sanitizeHtml(const QString &html);
};

#endif // EVENTBRIDGE_H
