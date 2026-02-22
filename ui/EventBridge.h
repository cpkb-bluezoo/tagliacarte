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
#include <QSet>
#include <QStringList>
#include <QImage>

void showError(QWidget *parent, const char *context);

static const int MessageIdRole = Qt::UserRole;

// Custom data roles for folder tree items
static const int FolderNameRole  = Qt::UserRole + 1;  // real protocol folder name (e.g. "INBOX/Subfolder")
static const int FolderAttrsRole = Qt::UserRole + 2;  // space-separated attribute string
static const int FolderDelimRole = Qt::UserRole + 3;  // delimiter character (QChar, null if none)

// Custom data role for message flags bitmask
static const int MessageFlagsRole = Qt::UserRole + 10;

struct ChatMessage {
    QString content;
    QString authorId;    // hex pubkey (Nostr) or user ID (Matrix)
    qint64 timestampSecs;
};

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

    /** Set the current store kind (TAGLIACARTE_STORE_KIND_*). */
    void setStoreKind(int kind) { m_storeKind = kind; }
    int storeKind() const { return m_storeKind; }
    bool isConversationMode() const;

    /** Relay list for the current Nostr store (used for profile fetches). */
    void setNostrRelays(const QString &relaysCsv) { m_nostrRelaysCsv = relaysCsv; }

    /** Secret key (hex) for NIP-42 relay auth during profile fetches. */
    void setNostrSecretKey(const QString &hex) { m_nostrSecretKey = hex; }

    /** Set / get the self user's pubkey (hex) for the current Nostr store. */
    void setSelfPubkey(const QString &hex) { m_selfPubkey = hex.toLower(); }
    QString selfPubkey() const { return m_selfPubkey; }

    /** Compose bar widgets (created in main.cpp, shown only in conversation mode). */
    QWidget *composeBar = nullptr;

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
    void addMessageSummary(const QString &id, const QString &subject, const QString &from, const QString &dateFormatted, qint64 timestampSecs, quint64 size, quint32 flags = 0);
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
    /** Update a folder tree item's display text after an async profile fetch. */
    void updateFolderDisplayName(const QString &realName, const QString &displayName);
    /** Update a folder tree item's icon after an avatar has been downloaded. */
    void updateFolderAvatar(const QString &realName, const QString &filePath);

Q_SIGNALS:
    void folderReadyForMessages(quint64 total);
    void messageSent();
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

    int m_storeKind = 0;
    QString m_nostrRelaysCsv;
    QString m_nostrSecretKey;                      // hex secret key for NIP-42 auth
    QString m_selfPubkey;                          // hex pubkey of the current Nostr user
    QMap<QString, QString> m_nostrNameCache;       // hex pubkey -> resolved display name
    QMap<QString, QString> m_nostrPictureCache;    // hex pubkey -> local file path for avatar
    QSet<QString> m_profileFetchPending;           // pubkeys currently being fetched
    QList<ChatMessage> m_chatMessages;             // accumulated for conversation rendering

    /** Get cached display name for an author ID, or a short fallback. */
    QString authorDisplayName(const QString &authorId) const;
    /** Get cached avatar file path for an author ID, or empty. */
    QString authorAvatarPath(const QString &authorId) const;
    /** Generate a deterministic hue (0-360) from a string for placeholder avatar colors. */
    static int avatarHue(const QString &id);
    /** Format a timestamp for the conversation view (smart: today/this week/older). */
    static QString formatChatTimestamp(qint64 secs);

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

    static bool isHexPubkey(const QString &s);
    void fetchNostrProfile(const QString &hexPubkey);
    void ensureProfilesFetched();
    void renderChatMessages();
};

#endif // EVENTBRIDGE_H
