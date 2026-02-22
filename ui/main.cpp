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
#include <QToolButton>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QSplitter>
#include <QFileDialog>
#include <QMessageBox>
#include <QStatusBar>
#include <QLabel>
#include <QTextBrowser>
#include <QLineEdit>
#include <QFrame>
#include <QStackedWidget>
#include <QFile>
#include <QTranslator>
#include <QLocale>
#include <QInputDialog>
#include <QMenu>
#include <QAction>
#include <QHeaderView>
#include <QUrl>
#include <QDesktopServices>
#include <QTimer>

#include <memory>

#include "tagliacarte.h"
#include "EventBridge.h"
#include "Tr.h"
#include "Config.h"
#include "CidTextBrowser.h"
#include "IconUtils.h"
#include "ComposeDialog.h"
#include "Callbacks.h"
#include "MainController.h"
#include "SettingsPage.h"
#include "MessageDragTreeWidget.h"
#include "FolderDropTreeWidget.h"


int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    qRegisterMetaType<ComposePart>();

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

    toolbarLayout->addSpacing(12);

    auto *replyBtn = new QToolButton(toolbar);
    replyBtn->setObjectName("replyBtn");
    replyBtn->setToolTip(TR("message.reply.tooltip"));
    QIcon replyIcon = iconFromSvgResource(QStringLiteral(":/icons/reply.svg"), QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!replyIcon.isNull())
        replyBtn->setIcon(replyIcon);
    replyBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    replyBtn->setFixedSize(40, 40);
    replyBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    replyBtn->setEnabled(false);
    toolbarLayout->addWidget(replyBtn);

    auto *replyAllBtn = new QToolButton(toolbar);
    replyAllBtn->setObjectName("replyAllBtn");
    replyAllBtn->setToolTip(TR("message.reply_all.tooltip"));
    QIcon replyAllIcon = iconFromSvgResource(QStringLiteral(":/icons/reply-all.svg"), QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!replyAllIcon.isNull())
        replyAllBtn->setIcon(replyAllIcon);
    replyAllBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    replyAllBtn->setFixedSize(40, 40);
    replyAllBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    replyAllBtn->setEnabled(false);
    toolbarLayout->addWidget(replyAllBtn);

    auto *forwardBtn = new QToolButton(toolbar);
    forwardBtn->setObjectName("forwardBtn");
    forwardBtn->setToolTip(TR("message.forward.tooltip"));
    QIcon forwardIcon = iconFromSvgResource(QStringLiteral(":/icons/forward.svg"), QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!forwardIcon.isNull())
        forwardBtn->setIcon(forwardIcon);
    forwardBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    forwardBtn->setFixedSize(40, 40);
    forwardBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    forwardBtn->setEnabled(false);
    toolbarLayout->addWidget(forwardBtn);

    auto *junkBtn = new QToolButton(toolbar);
    junkBtn->setObjectName("junkBtn");
    junkBtn->setToolTip(TR("message.junk.tooltip"));
    QIcon junkIcon = iconFromSvgResource(QStringLiteral(":/icons/junk.svg"), QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!junkIcon.isNull())
        junkBtn->setIcon(junkIcon);
    junkBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    junkBtn->setFixedSize(40, 40);
    junkBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    junkBtn->setEnabled(false);
    toolbarLayout->addWidget(junkBtn);

    auto *moveBtn = new QToolButton(toolbar);
    moveBtn->setObjectName("moveBtn");
    moveBtn->setToolTip(TR("message.move.tooltip"));
    QIcon moveIcon = iconFromSvgResource(QStringLiteral(":/icons/move.svg"), QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!moveIcon.isNull())
        moveBtn->setIcon(moveIcon);
    moveBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    moveBtn->setFixedSize(40, 40);
    moveBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    moveBtn->setEnabled(false);
    moveBtn->setVisible(false);  // hidden for now; move via drag-drop from message list to folder pane
    toolbarLayout->addWidget(moveBtn);

    auto *deleteBtn = new QToolButton(toolbar);
    deleteBtn->setObjectName("deleteBtn");
    deleteBtn->setToolTip(TR("message.delete.tooltip"));
    QIcon trashIcon = iconFromSvgResource(QStringLiteral(":/icons/trash.svg"), QApplication::palette().color(QPalette::ButtonText), circleIconPx);
    if (!trashIcon.isNull())
        deleteBtn->setIcon(trashIcon);
    deleteBtn->setIconSize(QSize(circleIconPx, circleIconPx));
    deleteBtn->setFixedSize(40, 40);
    deleteBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); padding: 0; border: none; min-width: 40px; min-height: 40px; }"
        "QToolButton:hover:enabled { background-color: palette(light); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    deleteBtn->setEnabled(false);
    toolbarLayout->addWidget(deleteBtn);

    mainContentLayout->addWidget(toolbar);

    // Content: folder list | message list | message detail
    auto *contentSplitter = new QSplitter(Qt::Horizontal, mainContentPage);
    auto *folderListPanel = new QWidget(mainContentPage);
    auto *folderListPanelLayout = new QVBoxLayout(folderListPanel);
    folderListPanelLayout->setContentsMargins(8, 8, 0, 8);
    auto *folderTree = new FolderDropTreeWidget(folderListPanel);
    folderTree->setColumnCount(1);
    folderTree->setHeaderHidden(true);
    folderTree->setSelectionMode(QAbstractItemView::SingleSelection);
    folderTree->setIndentation(16);
    folderTree->setContextMenuPolicy(Qt::CustomContextMenu);
    folderTree->setAcceptDrops(true);
    folderTree->setDragDropMode(QAbstractItemView::DropOnly);
    folderListPanelLayout->addWidget(folderTree);

    auto *rightSplitter = new QSplitter(Qt::Vertical, mainContentPage);
    auto *conversationList = new MessageDragTreeWidget(mainContentPage);
    conversationList->setColumnCount(3);
    conversationList->setHeaderLabels({ TR("message.from_column"), TR("message.subject_column"), TR("message.date_column") });
    conversationList->setSelectionMode(QAbstractItemView::ExtendedSelection);
    conversationList->setDragEnabled(true);
    conversationList->setDragDropMode(QAbstractItemView::DragOnly);
    conversationList->setDefaultDropAction(Qt::CopyAction);
    conversationList->setSortingEnabled(true);
    conversationList->setRootIsDecorated(false);  // flat for now; set true when thread hierarchy is added
    auto *msgListHeader = conversationList->header();
    msgListHeader->setSortIndicatorShown(true);
    msgListHeader->setStretchLastSection(false);
    msgListHeader->setSectionResizeMode(0, QHeaderView::Interactive);
    msgListHeader->setSectionResizeMode(1, QHeaderView::Stretch);  // subject fills space
    msgListHeader->setSectionResizeMode(2, QHeaderView::Interactive);
    msgListHeader->setDefaultAlignment(Qt::AlignLeft);
    {
        Config c = loadConfig();
        int sortCol = (c.messageListSortColumn >= 0 && c.messageListSortColumn < 3) ? c.messageListSortColumn : 2;
        Qt::SortOrder sortOrd = c.messageListSortOrder;
        conversationList->sortByColumn(sortCol, sortOrd);
        msgListHeader->setSortIndicator(sortCol, sortOrd);
        if (!c.messageListColumnWidths.isEmpty()) {
            QStringList w = c.messageListColumnWidths.split(QLatin1Char(','));
            for (int i = 0; i < 3 && i < w.size(); ++i) {
                int wval = w[i].toInt();
                if (wval > 0) {
                    conversationList->setColumnWidth(i, wval);
                }
            }
        } else {
            conversationList->setColumnWidth(0, 150);
            conversationList->setColumnWidth(2, 100);
        }
    }
    conversationList->headerItem()->setTextAlignment(2, Qt::AlignRight | Qt::AlignVCenter);
    auto saveMessageListConfig = [conversationList]() {
        Config c = loadConfig();
        c.messageListSortColumn = conversationList->header()->sortIndicatorSection();
        c.messageListSortOrder = conversationList->header()->sortIndicatorOrder();
        QStringList widths;
        for (int i = 0; i < 3; ++i) {
            int w = conversationList->columnWidth(i);
            widths << QString::number(w);
        }
        c.messageListColumnWidths = widths.join(QLatin1String(","));
        QStringList order;
        for (int i = 0; i < 3; ++i) {
            order << QString::number(conversationList->header()->visualIndex(i));
        }
        c.messageListColumnOrder = order.join(QLatin1String(","));
        saveConfig(c);
    };
    QObject::connect(msgListHeader, &QHeaderView::sortIndicatorChanged, saveMessageListConfig);
    QObject::connect(msgListHeader, &QHeaderView::sectionResized, saveMessageListConfig);
    QObject::connect(msgListHeader, &QHeaderView::sectionMoved, saveMessageListConfig);
    auto *messageArea = new QWidget(mainContentPage);
    auto *messageAreaLayout = new QVBoxLayout(messageArea);
    messageAreaLayout->setContentsMargins(0, 0, 0, 0);
    messageAreaLayout->setSpacing(0);
    // --- Message header pane (outside QTextBrowser, unaffected by HTML backgrounds) ---
    auto *messageHeaderPane = new QWidget(messageArea);
    auto *headerLayout = new QVBoxLayout(messageHeaderPane);
    headerLayout->setContentsMargins(6, 4, 6, 0);
    headerLayout->setSpacing(1);
    auto *headerFromLabel = new QLabel(messageHeaderPane);
    headerFromLabel->setTextFormat(Qt::RichText);
    headerFromLabel->setWordWrap(true);
    headerLayout->addWidget(headerFromLabel);
    auto *headerToLabel = new QLabel(messageHeaderPane);
    headerToLabel->setTextFormat(Qt::RichText);
    headerToLabel->setWordWrap(true);
    headerLayout->addWidget(headerToLabel);
    auto *headerSubjectLabel = new QLabel(messageHeaderPane);
    headerSubjectLabel->setTextFormat(Qt::RichText);
    headerSubjectLabel->setWordWrap(true);
    headerLayout->addWidget(headerSubjectLabel);
    auto *headerSeparator = new QFrame(messageHeaderPane);
    headerSeparator->setFrameShape(QFrame::HLine);
    headerSeparator->setFrameShadow(QFrame::Sunken);
    headerLayout->addWidget(headerSeparator);
    messageHeaderPane->hide();
    messageAreaLayout->addWidget(messageHeaderPane);
    // --- Message body browser ---
    auto *messageView = new CidTextBrowser(mainContentPage);
    messageView->setOpenExternalLinks(true);
    messageView->setResourceLoadPolicy(loadConfig().resourceLoadPolicy);
    messageAreaLayout->addWidget(messageView);
    auto *attachmentsPane = new QWidget(messageArea);
    auto *attachmentsLayout = new QHBoxLayout(attachmentsPane);
    attachmentsLayout->setContentsMargins(0, 4, 0, 0);
    attachmentsPane->hide();
    messageAreaLayout->addWidget(attachmentsPane);
    rightSplitter->addWidget(conversationList);
    rightSplitter->addWidget(messageArea);
    rightSplitter->setStretchFactor(0, 0);
    rightSplitter->setStretchFactor(1, 1);

    contentSplitter->addWidget(folderListPanel);
    contentSplitter->addWidget(rightSplitter);
    contentSplitter->setStretchFactor(0, 0);
    contentSplitter->setStretchFactor(1, 1);
    contentSplitter->setSizes({ 180, 400 });
    mainContentLayout->addWidget(contentSplitter);

    rightStack->addWidget(mainContentPage);

    // --- MainController: central shared state for store management ---
    // Widget pointers that depend on later declarations (bridge) are set below.
    MainController ctrl;
    ctrl.win = &win;
    ctrl.folderTree = folderTree;
    ctrl.conversationList = conversationList;
    ctrl.messageView = messageView;
    ctrl.messageHeaderPane = messageHeaderPane;
    ctrl.composeBtn = composeBtn;
    ctrl.appendMessageBtn = appendMessageBtn;
    ctrl.replyBtn = replyBtn;
    ctrl.replyAllBtn = replyAllBtn;
    ctrl.forwardBtn = forwardBtn;
    ctrl.junkBtn = junkBtn;
    ctrl.moveBtn = moveBtn;
    ctrl.deleteBtn = deleteBtn;
    ctrl.storeListWidget = storeListWidget;
    ctrl.storeListLayout = storeListLayout;
    ctrl.rightStack = rightStack;
    ctrl.settingsBtn = settingsBtn;

    EventBridge bridge;
    ctrl.bridge = &bridge;

    buildSettingsPage(&ctrl, &win, version);

    mainLayout->addWidget(rightStack, 1);
    bridge.folderTree = folderTree;
    bridge.conversationList = conversationList;
    bridge.messageView = messageView;
    messageView->setCidRegistry(bridge.cidRegistryPtr());
    bridge.attachmentsPane = attachmentsPane;
    bridge.messageHeaderPane = messageHeaderPane;
    bridge.headerFromLabel = headerFromLabel;
    bridge.headerToLabel = headerToLabel;
    bridge.headerSubjectLabel = headerSubjectLabel;
    bridge.statusBar = win.statusBar();
    bridge.win = &win;

    // Wire drag-and-drop: folder tree drop -> MainController
    QObject::connect(folderTree, &FolderDropTreeWidget::messagesDropped,
        &ctrl, &MainController::handleMessageDrop);

    struct OAuthReauthContext {
        QByteArray storeUri;
        EventBridge *bridge;
    };

    tagliacarte_set_credential_request_callback(on_credential_request_cb, &bridge);
    QObject::connect(&bridge, &EventBridge::credentialRequested, [&win, &bridge](const QString &storeUriQ, const QString &username, int isPlaintext, int authType) {
        QWidget *parent = &win;

        // OAuth re-auth: trigger browser-based re-authorization instead of password dialog.
        if (authType == TAGLIACARTE_AUTH_TYPE_OAUTH2) {
            // Determine provider from URI scheme.
            const char *provider = nullptr;
            if (storeUriQ.startsWith(QLatin1String("gmail://")) || storeUriQ.startsWith(QLatin1String("gmail+smtp://"))) {
                provider = "google";
            } else if (storeUriQ.startsWith(QLatin1String("graph://")) || storeUriQ.startsWith(QLatin1String("graph+send://"))) {
                provider = "microsoft";
            }
            if (provider) {
                auto *ctx = new OAuthReauthContext{ storeUriQ.toUtf8(), &bridge };
                tagliacarte_oauth_start(
                    provider,
                    username.toUtf8().constData(),
                    [](const char *url, void *user_data) {
                        (void)user_data;
                        QString urlStr = QString::fromUtf8(url);
                        QDesktopServices::openUrl(QUrl(urlStr));
                    },
                    [](int error, const char * /*errorMessage*/, void *user_data) {
                        auto *ctx = static_cast<OAuthReauthContext *>(user_data);
                        if (error == 0) {
                            QByteArray uri = ctx->storeUri;
                            EventBridge *b = ctx->bridge;
                            QMetaObject::invokeMethod(b, [uri]() {
                                tagliacarte_store_reload_oauth_token(uri.constData());
                                tagliacarte_store_refresh_folders(uri.constData());
                            }, Qt::QueuedConnection);
                        }
                        delete ctx;
                    },
                    ctx
                );
                return;
            }
            // Fallback: cancel if we can't determine the provider.
            tagliacarte_credential_cancel(storeUriQ.toUtf8().constData());
            return;
        }

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
        if (ret == 0) {
            tagliacarte_store_refresh_folders(storeUriQ.toUtf8().constData());
        }
    });

    // Startup: show window first, then restore stores from config asynchronously.
    // This ensures the UI is visible even if store connections are slow or fail.
    {
        Config startupConfig = loadConfig();
        tagliacarte_set_credentials_backend(startupConfig.useKeychain ? 1 : 0);
        if (startupConfig.stores.isEmpty()) {
            rightStack->setCurrentIndex(1);
            settingsBtn->setChecked(true);
            win.statusBar()->showMessage(TR("status.add_store_to_start"));
        } else {
            QTimer::singleShot(0, &ctrl, [&ctrl]() {
                ctrl.refreshStoresFromConfig();
            });
        }
    }

    QObject::connect(&bridge, &EventBridge::folderReadyForMessages, [&](quint64 total) {
        QByteArray uri = bridge.folderUri();
        if (uri.isEmpty()) {
            return;
        }
        ctrl.updateComposeAppendButtons();
        ctrl.updateMessageActionButtons();
        auto *item = folderTree->currentItem();
        tagliacarte_folder_set_message_list_callbacks(uri.constData(), on_message_summary_cb, on_message_list_complete_cb, &bridge);
        bridge.startMessageLoading(total);
        tagliacarte_folder_request_message_list(uri.constData(), 0, total);
        if (item) {
            win.statusBar()->showMessage(TR("status.folder_loading").arg(item->text(0)));
        }
    });

    QObject::connect(folderTree, &QTreeWidget::itemSelectionChanged, [&]() {
        bridge.clearFolder();
        conversationList->clear();
        messageView->clear();
        messageHeaderPane->hide();
        ctrl.updateMessageActionButtons();

        auto *item = folderTree->currentItem();
        if (!item || ctrl.storeUri.isEmpty()) {
            return;
        }

        QString realName = item->data(0, FolderNameRole).toString();
        if (realName.isEmpty()) {
            return;  // placeholder item during inline editing – nothing to open
        }
        bridge.setFolderNameOpening(realName);
        QByteArray name = realName.toUtf8();
        tagliacarte_store_start_open_folder(
            ctrl.storeUri.constData(),
            name.constData(),
            on_open_folder_select_event_cb,
            on_folder_ready_cb,
            on_open_folder_error_cb,
            &bridge
        );
        win.statusBar()->showMessage(TR("status.opening").arg(item->text(0)));
    });

    // --- Folder tree context menu ---
    QObject::connect(folderTree, &QTreeWidget::customContextMenuRequested, [&](const QPoint &pos) {
        if (ctrl.storeUri.isEmpty()) {
            return;
        }
        // Determine store kind to check capability
        int kind = tagliacarte_store_kind(ctrl.storeUri.constData());
        bool canManageFolders = (kind == TAGLIACARTE_STORE_KIND_EMAIL);  // IMAP and Maildir

        QTreeWidgetItem *item = folderTree->itemAt(pos);
        QMenu menu;

        if (item) {
            // Right-click on an existing folder
            QString realName = item->data(0, FolderNameRole).toString();
            QString attrs = item->data(0, FolderAttrsRole).toString();
            bool isInbox = (realName.compare(QLatin1String("INBOX"), Qt::CaseInsensitive) == 0);
            bool isSystem = EventBridge::isSystemFolder(realName, attrs);
            bool hasNoinferiors = attrs.toLower().contains(QLatin1String("\\noinferiors"));

            if (canManageFolders && !isInbox) {
                QAction *renameAct = menu.addAction(TR("folder.rename"));
                QObject::connect(renameAct, &QAction::triggered, [&, item, realName]() {
                    // Get delimiter for this store
                    char delimC = tagliacarte_store_hierarchy_delimiter(ctrl.storeUri.constData());
                    QChar delimChar = delimC ? QChar(delimC) : QChar();

                    // Extract the leaf name (last component)
                    QString leafName = item->text(0);

                    // Create inline editor
                    auto *editor = new QLineEdit(folderTree);
                    editor->setText(leafName);
                    editor->selectAll();
                    folderTree->setItemWidget(item, 0, editor);
                    editor->setFocus();

                    auto done = std::make_shared<bool>(false);

                    auto commit = [&, editor, item, realName, delimChar, done]() {
                        if (*done) {
                            return;
                        }
                        *done = true;
                        QString newLeaf = editor->text().trimmed();
                        folderTree->removeItemWidget(item, 0);
                        editor->deleteLater();

                        if (newLeaf.isEmpty() || newLeaf == item->text(0)) {
                            return;  // no change or empty
                        }

                        // Sanitize: remove delimiter and control chars
                        QString sanitized;
                        for (const QChar &ch : newLeaf) {
                            if (ch < QChar(0x20)) {
                                continue;
                            }
                            if (!delimChar.isNull() && ch == delimChar) {
                                continue;
                            }
                            sanitized += ch;
                        }
                        if (sanitized.isEmpty()) {
                            return;
                        }

                        // Build full new name: replace leaf in realName
                        QString newRealName;
                        if (!delimChar.isNull()) {
                            int lastDelim = realName.lastIndexOf(delimChar);
                            if (lastDelim >= 0) {
                                newRealName = realName.left(lastDelim + 1) + sanitized;
                            } else {
                                newRealName = sanitized;
                            }
                        } else {
                            newRealName = sanitized;
                        }

                        QByteArray oldUtf8 = realName.toUtf8();
                        QByteArray newUtf8 = newRealName.toUtf8();
                        tagliacarte_store_rename_folder(
                            ctrl.storeUri.constData(),
                            oldUtf8.constData(),
                            newUtf8.constData(),
                            on_folder_op_error_cb,
                            &bridge
                        );
                    };

                    QObject::connect(editor, &QLineEdit::returnPressed, commit);
                    QObject::connect(editor, &QLineEdit::editingFinished, [&, editor, item, done]() {
                        if (*done) {
                            return;
                        }
                        *done = true;
                        folderTree->removeItemWidget(item, 0);
                        editor->deleteLater();
                    });
                });
            }

            if (canManageFolders && !hasNoinferiors) {
                QAction *addSubAct = menu.addAction(TR("folder.add_subfolder"));
                QObject::connect(addSubAct, &QAction::triggered, [&, item, realName]() {
                    char delimC = tagliacarte_store_hierarchy_delimiter(ctrl.storeUri.constData());
                    QChar delimChar = delimC ? QChar(delimC) : QChar('/');

                    // Generate unique "New folder" name
                    QString baseName = TR("folder.new_folder");
                    QString candidateLeaf = baseName;
                    int suffix = 2;
                    while (true) {
                        QString candidateFullName = realName + delimChar + candidateLeaf;
                        bool exists = false;
                        QTreeWidgetItemIterator it(folderTree);
                        while (*it) {
                            if ((*it)->data(0, FolderNameRole).toString() == candidateFullName) {
                                exists = true;
                                break;
                            }
                            ++it;
                        }
                        if (!exists) {
                            break;
                        }
                        candidateLeaf = baseName + QStringLiteral(" %1").arg(suffix++);
                    }

                    // Create a temporary placeholder for editing
                    auto *placeholder = new QTreeWidgetItem(item);
                    placeholder->setText(0, candidateLeaf);
                    item->setExpanded(true);

                    auto *editor = new QLineEdit(folderTree);
                    editor->setText(candidateLeaf);
                    editor->selectAll();
                    folderTree->setItemWidget(placeholder, 0, editor);
                    editor->setFocus();

                    auto done = std::make_shared<bool>(false);

                    auto commit = [&, editor, placeholder, item, realName, delimChar, done]() {
                        if (*done) {
                            return;
                        }
                        *done = true;
                        QString newLeaf = editor->text().trimmed();
                        // Remove placeholder
                        folderTree->removeItemWidget(placeholder, 0);
                        item->removeChild(placeholder);
                        delete placeholder;
                        editor->deleteLater();

                        if (newLeaf.isEmpty()) {
                            return;
                        }

                        // Sanitize
                        QString sanitized;
                        for (const QChar &ch : newLeaf) {
                            if (ch < QChar(0x20)) {
                                continue;
                            }
                            if (ch == delimChar) {
                                continue;
                            }
                            sanitized += ch;
                        }
                        if (sanitized.isEmpty()) {
                            return;
                        }

                        QString fullName = realName + delimChar + sanitized;
                        QByteArray nameUtf8 = fullName.toUtf8();
                        tagliacarte_store_create_folder(
                            ctrl.storeUri.constData(),
                            nameUtf8.constData(),
                            on_folder_op_error_cb,
                            &bridge
                        );
                    };

                    QObject::connect(editor, &QLineEdit::returnPressed, commit);
                    QObject::connect(editor, &QLineEdit::editingFinished, [&, editor, placeholder, item, done]() {
                        if (*done) {
                            return;
                        }
                        *done = true;
                        folderTree->removeItemWidget(placeholder, 0);
                        item->removeChild(placeholder);
                        delete placeholder;
                        editor->deleteLater();
                    });
                });
            }

            if (canManageFolders && !isSystem) {
                if (!menu.isEmpty()) {
                    menu.addSeparator();
                }
                QAction *deleteAct = menu.addAction(TR("folder.delete"));
                QObject::connect(deleteAct, &QAction::triggered, [&, realName, item]() {
                    QString displayName = item->text(0);
                    int ret = QMessageBox::question(
                        &win,
                        TR("folder.delete_confirm_title"),
                        TR("folder.delete_confirm_text").arg(displayName),
                        QMessageBox::Yes | QMessageBox::No,
                        QMessageBox::No
                    );
                    if (ret == QMessageBox::Yes) {
                        QByteArray nameUtf8 = realName.toUtf8();
                        tagliacarte_store_delete_folder(
                            ctrl.storeUri.constData(),
                            nameUtf8.constData(),
                            on_folder_op_error_cb,
                            &bridge
                        );
                    }
                });
            }

            // Expunge action: available for Email stores (IMAP).
            // Permanently removes all \Deleted messages from the selected folder.
            if (canManageFolders && !realName.isEmpty()) {
                bool noselect = attrs.toLower().contains(QLatin1String("\\noselect"));
                if (!noselect) {
                    if (!menu.isEmpty()) {
                        menu.addSeparator();
                    }
                    QAction *expungeAct = menu.addAction(TR("folder.expunge"));
                    QObject::connect(expungeAct, &QAction::triggered, [&, realName]() {
                        QByteArray folderUri = bridge.folderUri();
                        if (folderUri.isEmpty()) {
                            return;
                        }
                        // Count messages marked \Deleted in the current list
                        int deletedCount = 0;
                        if (conversationList) {
                            for (int i = 0; i < conversationList->topLevelItemCount(); ++i) {
                                auto *it = conversationList->topLevelItem(i);
                                quint32 flags = it->data(0, MessageFlagsRole).toUInt();
                                if (flags & TAGLIACARTE_FLAG_DELETED) {
                                    ++deletedCount;
                                }
                            }
                        }
                        if (deletedCount == 0) {
                            return;
                        }
                        int ret = QMessageBox::warning(
                            &win,
                            TR("folder.expunge"),
                            TR("folder.expunge_confirm").arg(deletedCount),
                            QMessageBox::Ok | QMessageBox::Cancel,
                            QMessageBox::Cancel
                        );
                        if (ret != QMessageBox::Ok) {
                            return;
                        }
                        tagliacarte_folder_expunge_async(
                            folderUri.constData(),
                            on_bulk_complete_cb,
                            &bridge
                        );
                    });
                }
            }
        } else {
            // Right-click on empty area
            if (canManageFolders) {
                QAction *addAct = menu.addAction(TR("folder.add_folder"));
                QObject::connect(addAct, &QAction::triggered, [&]() {
                    char delimC = tagliacarte_store_hierarchy_delimiter(ctrl.storeUri.constData());
                    QChar delimChar = delimC ? QChar(delimC) : QChar();

                    // Generate unique "New folder" name
                    QString baseName = TR("folder.new_folder");
                    QString candidate = baseName;
                    int suffix = 2;
                    while (true) {
                        bool exists = false;
                        QTreeWidgetItemIterator it(folderTree);
                        while (*it) {
                            if ((*it)->data(0, FolderNameRole).toString() == candidate) {
                                exists = true;
                                break;
                            }
                            ++it;
                        }
                        if (!exists) {
                            break;
                        }
                        candidate = baseName + QStringLiteral(" %1").arg(suffix++);
                    }

                    // Create a temporary top-level placeholder for editing
                    auto *placeholder = new QTreeWidgetItem();
                    placeholder->setText(0, candidate);
                    folderTree->addTopLevelItem(placeholder);

                    auto *editor = new QLineEdit(folderTree);
                    editor->setText(candidate);
                    editor->selectAll();
                    folderTree->setItemWidget(placeholder, 0, editor);
                    editor->setFocus();

                    auto done = std::make_shared<bool>(false);

                    auto commit = [&, editor, placeholder, delimChar, done]() {
                        if (*done) {
                            return;
                        }
                        *done = true;
                        QString newName = editor->text().trimmed();
                        // Remove placeholder
                        folderTree->removeItemWidget(placeholder, 0);
                        int idx = folderTree->indexOfTopLevelItem(placeholder);
                        if (idx >= 0) {
                            folderTree->takeTopLevelItem(idx);
                        }
                        delete placeholder;
                        editor->deleteLater();

                        if (newName.isEmpty()) {
                            return;
                        }

                        // Sanitize
                        QString sanitized;
                        for (const QChar &ch : newName) {
                            if (ch < QChar(0x20)) {
                                continue;
                            }
                            if (!delimChar.isNull() && ch == delimChar) {
                                continue;
                            }
                            sanitized += ch;
                        }
                        if (sanitized.isEmpty()) {
                            return;
                        }

                        QByteArray nameUtf8 = sanitized.toUtf8();
                        tagliacarte_store_create_folder(
                            ctrl.storeUri.constData(),
                            nameUtf8.constData(),
                            on_folder_op_error_cb,
                            &bridge
                        );
                    };

                    QObject::connect(editor, &QLineEdit::returnPressed, commit);
                    QObject::connect(editor, &QLineEdit::editingFinished, [&, editor, placeholder, done]() {
                        if (*done) {
                            return;
                        }
                        *done = true;
                        folderTree->removeItemWidget(placeholder, 0);
                        int idx = folderTree->indexOfTopLevelItem(placeholder);
                        if (idx >= 0) {
                            folderTree->takeTopLevelItem(idx);
                        }
                        delete placeholder;
                        editor->deleteLater();
                    });
                });
            }
        }

        if (!menu.isEmpty()) {
            menu.exec(folderTree->viewport()->mapToGlobal(pos));
        }
    });

    QObject::connect(conversationList, &QTreeWidget::itemSelectionChanged, [&]() {
        ctrl.updateMessageActionButtons();
        messageView->clear();
        messageHeaderPane->hide();
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

        tagliacarte_folder_set_message_callbacks(uri.constData(),
            on_message_metadata_cb,
            on_start_entity_cb,
            on_content_type_cb,
            on_content_disposition_cb,
            on_content_id_cb,
            on_end_headers_cb,
            on_body_content_cb,
            on_end_entity_cb,
            on_message_complete_cb,
            &bridge);
        tagliacarte_folder_request_message(uri.constData(), id.constData());
        win.statusBar()->showMessage(TR("status.loading"));
    });

    // Show link URL in status bar on mouse hover
    QObject::connect(messageView, &QTextBrowser::highlighted, [&](const QUrl &url) {
        if (url.isEmpty()) {
            win.statusBar()->clearMessage();
        } else {
            win.statusBar()->showMessage(url.toString());
        }
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
        // After append, request message count asynchronously, then load list
        tagliacarte_folder_message_count(folderUri.constData(),
            [](uint64_t count, int error, void *user_data) {
                auto *b = static_cast<EventBridge *>(user_data);
                if (error == 0) {
                    QMetaObject::invokeMethod(b, "folderReadyForMessages", Qt::QueuedConnection,
                        Q_ARG(quint64, static_cast<quint64>(count)));
                }
            }, &bridge);
        win.statusBar()->showMessage(TR("status.message_appended"));
    });

    ctrl.connectComposeActions();

    win.show();

    int ret = app.exec();

    ctrl.shutdown();

    return ret;
}
