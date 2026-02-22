#ifndef MAINCONTROLLER_H
#define MAINCONTROLLER_H

#include <QObject>
#include <QByteArray>
#include <QMap>
#include <QList>
#include <QString>

class QMainWindow;
class QToolButton;
class QLineEdit;
class QPlainTextEdit;
class QTreeWidget;
class QTextBrowser;
class QWidget;
class QVBoxLayout;
class QStackedWidget;

class EventBridge;
class CidTextBrowser;
class ComposeDialog;
struct StoreEntry;
struct Config;

/**
 * Central controller holding shared mutable state that was formerly
 * a set of local variables in main() captured by [&] lambdas.
 *
 * Widget pointers are non-owning (Qt parent hierarchy owns the widgets).
 * Call init() after all widgets have been created.
 */
class MainController : public QObject {
    Q_OBJECT
public:
    explicit MainController(QObject *parent = nullptr);
    bool eventFilter(QObject *obj, QEvent *event) override;

    // --- Shared state (was locals in main()) ---
    QByteArray storeUri;
    QByteArray smtpTransportUri;
    QMap<QByteArray, QByteArray> storeToTransport;
    QList<QToolButton *> storeButtons;
    QList<QByteArray> allStoreUris;
    QString editingStoreId;

    // --- Widget refs (set once during setup, not owned) ---
    EventBridge *bridge = nullptr;
    QMainWindow *win = nullptr;
    QTreeWidget *folderTree = nullptr;
    QTreeWidget *conversationList = nullptr;
    CidTextBrowser *messageView = nullptr;
    QWidget *messageHeaderPane = nullptr;
    QToolButton *composeBtn = nullptr;
    QToolButton *appendMessageBtn = nullptr;
    QToolButton *replyBtn = nullptr;
    QToolButton *replyAllBtn = nullptr;
    QToolButton *forwardBtn = nullptr;
    QToolButton *junkBtn = nullptr;
    QToolButton *moveBtn = nullptr;
    QToolButton *deleteBtn = nullptr;
    QWidget *storeListWidget = nullptr;
    QVBoxLayout *storeListLayout = nullptr;
    QStackedWidget *rightStack = nullptr;
    QToolButton *settingsBtn = nullptr;

    // --- Methods extracted from lambdas ---
    void updateComposeAppendButtons();
    void updateMessageActionButtons();
    void addStoreCircle(const QString &initial, const QByteArray &uri, int colourIndex);

    /** Create a store URI from a config entry.  Returns the URI (empty on failure).
     *  Adds any associated transport to storeToTransport. */
    QByteArray createStoreFromEntry(const StoreEntry &entry);

    /** Tear down all current stores and re-create them from config. */
    void refreshStoresFromConfig();

    /** Select a store by URI: update state, load folders, check correct circle. */
    void selectStore(const QByteArray &uri);

    /** Free all stores and transports. Called during shutdown. */
    void shutdown();

    // --- Compose / message action methods ---

    /** Build a quoted body string for reply/forward. */
    static QString buildQuotedBody(const QString &original, const QString &header, const Config &c);

    /** Send the contents of a filled-in ComposeDialog. */
    void sendFromComposeDialog(ComposeDialog &dlg);

    /** Wire compose/message button click handlers. Call after bridge is set. */
    void connectComposeActions();

    /** Send a plain-text message to the current conversation partner. */
    void sendChatMessage(const QString &text);

    // --- Compose bar widgets (conversation mode) ---
    QPlainTextEdit *chatInput = nullptr;
    QToolButton *chatAttachBtn = nullptr;
    QToolButton *chatSendBtn = nullptr;

    /** Handle a drag-and-drop of messages from the message list to a folder.
     *  Called from FolderDropTreeWidget::messagesDropped signal.
     *  sourceFolderUri: URI of the source folder.
     *  messageIds: list of message IDs being dragged.
     *  destFolderName: real protocol name of the destination folder.
     *  isMove: true = Shift+drop (move), false = copy. */
    void handleMessageDrop(const QByteArray &sourceFolderUri,
                           const QStringList &messageIds,
                           const QString &destFolderName,
                           bool isMove);
};

#endif // MAINCONTROLLER_H
