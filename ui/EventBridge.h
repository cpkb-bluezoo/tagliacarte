#ifndef EVENTBRIDGE_H
#define EVENTBRIDGE_H

#include <QObject>
#include <QTreeWidget>
#include <QTextBrowser>
#include <QStatusBar>
#include <QProgressBar>
#include <QWidget>
#include <QLabel>
#include <QString>
#include <QVariantList>
#include <QMap>
#include <QStringList>

void showError(QWidget *parent, const char *context);

static const int MessageIdRole = Qt::UserRole;

// Custom data roles for folder tree items
static const int FolderNameRole  = Qt::UserRole + 1;  // real protocol folder name (e.g. "INBOX/Subfolder")
static const int FolderAttrsRole = Qt::UserRole + 2;  // space-separated attribute string
static const int FolderDelimRole = Qt::UserRole + 3;  // delimiter character (QChar, null if none)

// Custom data role for message flags bitmask
static const int MessageFlagsRole = Qt::UserRole + 10;

class EventBridge : public QObject {
    Q_OBJECT
public:
    QTreeWidget *folderTree = nullptr;
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

    /** Map a real folder name to a user-friendly display name (localised for system folders). */
    static QString displayNameForFolder(const QString &realName);
    /** Check if a folder is a system folder that should not be deleted. */
    static bool isSystemFolder(const QString &realName, const QString &attributes);

public Q_SLOTS:
    void startMessageLoading(quint64 total);
    void addFolder(const QString &name, const QString &delimiter, const QString &attributes);
    void removeFolder(const QString &name);
    void onFolderListComplete(int error, const QString &errorMessage);
    /** Called from C credential callback (marshal to main thread). Emits credentialRequested. */
    void requestCredentialSlot(const QString &storeUri, const QString &username, int isPlaintext, int authType);
    void onFolderReady(const QString &folderUri);
    void onOpenFolderError(const QString &message);
    void showOpeningMessageCount(quint32 count);
    void addMessageSummary(const QString &id, const QString &subject, const QString &from, const QString &dateFormatted, quint64 size, quint32 flags = 0);
    void onMessageListComplete(int error);
    void onBulkComplete(int ok, const QString &errorMessage);
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
    void onFolderOpError(const QString &message);

Q_SIGNALS:
    void folderReadyForMessages(quint64 total);
    /** Core needs a credential; show password dialog then call tagliacarte_credential_provide or tagliacarte_credential_cancel.
     *  authType: TAGLIACARTE_AUTH_TYPE_AUTO (0) for password, TAGLIACARTE_AUTH_TYPE_OAUTH2 (1) for OAuth re-auth. */
    void credentialRequested(const QString &storeUri, const QString &username, int isPlaintext, int authType);
    /** OAuth flow completed. provider: "google" or "microsoft". error: 0 = success, non-zero = failure. */
    void oauthComplete(const QString &provider, int error, const QString &errorMessage);

private:
    QByteArray m_folderUri;
    QString m_folderNameOpening;
    quint64 m_messageLoadTotal = 0;
    quint64 m_messageLoadCount = 0;
    QProgressBar *m_loadProgressBar = nullptr;
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

    /** Find a tree item by its real folder name (FolderNameRole). */
    QTreeWidgetItem *findFolderItem(const QString &realName) const;
    /** Recursively count all items in the tree. */
    static int countAllItems(QTreeWidget *tree);
};

#endif // EVENTBRIDGE_H
