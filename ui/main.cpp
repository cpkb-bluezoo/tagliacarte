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
#include <QPushButton>
#include <QToolButton>
#include <QListWidget>
#include <QSplitter>
#include <QFileDialog>
#include <QMessageBox>
#include <QStatusBar>
#include <QLabel>
#include <QTextBrowser>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QLineEdit>
#include <QTextEdit>
#include <QDateTime>
#include <QMetaObject>
#include <QTimer>
#include <QFrame>
#include <QStackedWidget>
#include <QTabWidget>
#include <QScrollArea>
#include <QSizePolicy>
#include <QSpinBox>
#include <QCheckBox>
#include <QComboBox>
#include <QDir>
#include <QStandardPaths>
#include <QFile>
#include <QSaveFile>
#include <QXmlStreamReader>
#include <QXmlStreamWriter>
#include <QSvgRenderer>
#include <QPainter>
#include <QPaintEvent>
#include <QColor>
#include <QTranslator>
#include <QLocale>
#include <QMap>

#include "tagliacarte.h"
#include "EventBridge.h"
#include "Tr.h"

#include <cstring>
#include <cstdint>

void showError(QWidget *parent, const char *contextKey) {
    const char *msg = tagliacarte_last_error();
    QString msgStr = msg ? QString::fromUtf8(msg) : TR("error.unknown");
    QMessageBox::critical(parent, TR("common.error"), QString("%1: %2").arg(TR(contextKey)).arg(msgStr));
}

// Resolve icon path: try Qt resource first, then filesystem (app dir, bundle Resources, build dir).
static QString resolveIconPath(const QString &resourcePath) {
    QFile f(resourcePath);
    if (f.open(QIODevice::ReadOnly)) {
        f.close();
        return resourcePath;
    }
    QString base = QCoreApplication::applicationDirPath();
    int lastSlash = resourcePath.lastIndexOf(QLatin1Char('/'));
    QString name = lastSlash >= 0 ? resourcePath.mid(lastSlash + 1) : resourcePath;
    QStringList tries;
    tries << (base + QLatin1String("/icons/") + name);
#if defined(Q_OS_MACOS) || defined(Q_OS_DARWIN)
    tries << (base + QLatin1String("/../Resources/icons/") + name);
#endif
    tries << (base + QLatin1String("/../ui/icons/") + name);
    tries << (base + QLatin1String("/../../ui/icons/") + name);
    for (const QString &p : tries) {
        if (QFile::exists(p)) return p;
    }
    return resourcePath;
}

// Render SVG from resource to a QPixmap at exact size (no QIcon; we control pixels).
static QPixmap renderSvgToPixmap(const QString &path, const QColor &color, int w, int h, qreal scaleFactor = 1.0) {
    QString resolved = resolveIconPath(path);
    QFile f(resolved);
    if (!f.open(QIODevice::ReadOnly)) return QPixmap();
    QByteArray svg = f.readAll();
    f.close();
    svg.replace("currentColor", color.name(QColor::HexRgb).toLatin1());
    QSvgRenderer r(svg);
    if (!r.isValid()) return QPixmap();
    QImage img(w, h, QImage::Format_ARGB32);
    img.fill(Qt::transparent);
    QPainter p(&img);
    if (scaleFactor > 1.0) {
        p.translate(w / 2.0, h / 2.0);
        p.scale(scaleFactor, scaleFactor);
        p.translate(-w / 2.0, -h / 2.0);
    }
    r.render(&p, QRectF(0, 0, w, h));
    p.end();
    return QPixmap::fromImage(img);
}

// Web-safe colours for store/account circles (unselected = border, selected = background). Order determines assignment.
static const char *const storeCircleColours[] = {
    "#6699CC", "#996633", "#339966", "#993366", "#666699", "#CC9933", "#33CC99", "#CC6699"
};
static const int storeCircleColourCount = int(sizeof(storeCircleColours) / sizeof(storeCircleColours[0]));

// Stylesheet for a store/account circle: unselected = thin border of colour, selected = full background of colour.
static QString storeCircleStyleSheet(int colourIndex) {
    QString hex(storeCircleColours[colourIndex % storeCircleColourCount]);
    return QStringLiteral("QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); font-weight: bold; padding: 0; min-width: 40px; min-height: 40px; border: 2px solid %1; }"
                         "QToolButton:hover { background-color: palette(light); }"
                         "QToolButton:checked { background-color: %1; color: #fff; border-color: %1; }"
                         "QToolButton:checked:hover { background-color: %1; color: #fff; }").arg(hex);
}

// Config under ~/.tagliacarte/config.xml (XML per specification)
struct StoreEntry {
    QString id;
    QString type;
    QString displayName;
    QString path;
};
struct Config {
    QList<StoreEntry> stores;
    QString lastSelectedStoreId;
};

static QString tagliacarteConfigDir() {
    QString path = QStandardPaths::writableLocation(QStandardPaths::HomeLocation) + QStringLiteral("/.tagliacarte");
    QDir().mkpath(path);
    return path;
}
static QString tagliacarteConfigPath() { return tagliacarteConfigDir() + QStringLiteral("/config.xml"); }

static Config loadConfig() {
    Config c;
    QFile f(tagliacarteConfigPath());
    if (!f.open(QIODevice::ReadOnly))
        return c;
    QXmlStreamReader r(&f);
    while (!r.atEnd()) {
        r.readNext();
        if (r.isStartElement()) {
            if (r.name() == QLatin1String("lastSelectedStoreId")) {
                c.lastSelectedStoreId = r.readElementText().trimmed();
            } else if (r.name() == QLatin1String("store")) {
                StoreEntry e;
                e.id = r.attributes().value(QLatin1String("id")).toString();
                e.type = r.attributes().value(QLatin1String("type")).toString();
                e.displayName = r.attributes().value(QLatin1String("displayName")).toString();
                e.path = r.attributes().value(QLatin1String("path")).toString();
                if (!e.id.isEmpty())
                    c.stores.append(e);
            }
        }
    }
    f.close();
    return c;
}

static void saveConfig(const Config &c) {
    QSaveFile f(tagliacarteConfigPath());
    if (!f.open(QIODevice::WriteOnly))
        return;
    QXmlStreamWriter w(&f);
    w.setAutoFormatting(true);
    w.writeStartDocument(QStringLiteral("1.0"), true);
    w.writeStartElement(QStringLiteral("tagliacarte"));
    if (!c.lastSelectedStoreId.isEmpty())
        w.writeTextElement(QStringLiteral("lastSelectedStoreId"), c.lastSelectedStoreId);
    w.writeStartElement(QStringLiteral("stores"));
    for (const StoreEntry &e : c.stores) {
        w.writeStartElement(QStringLiteral("store"));
        w.writeAttribute(QStringLiteral("id"), e.id);
        w.writeAttribute(QStringLiteral("type"), e.type);
        w.writeAttribute(QStringLiteral("displayName"), e.displayName);
        w.writeAttribute(QStringLiteral("path"), e.path);
        w.writeEndElement();
    }
    w.writeEndElement();
    w.writeEndElement();
    w.writeEndDocument();
    f.commit();
}

// Load SVG from resource and render as QIcon with currentColor replaced by palette color.
// Renders at size and at 2*size for HiDPI. scaleFactor zooms the graphic (e.g. 1.35 to fill the frame better).
static QIcon iconFromSvgResource(const QString &path, const QColor &color, int size = 24, qreal scaleFactor = 1.0) {
    QString resolved = resolveIconPath(path);
    QFile f(resolved);
    if (!f.open(QIODevice::ReadOnly))
        return QIcon();
    QByteArray svg = f.readAll();
    f.close();
    svg.replace("currentColor", color.name(QColor::HexRgb).toLatin1());
    QSvgRenderer r(svg);
    if (!r.isValid())
        return QIcon();
    auto renderAt = [&r, scaleFactor](int w, int h) {
        QImage img(w, h, QImage::Format_ARGB32);
        img.fill(Qt::transparent);
        QPainter p(&img);
        if (scaleFactor > 1.0) {
            p.translate(w / 2.0, h / 2.0);
            p.scale(scaleFactor, scaleFactor);
            p.translate(-w / 2.0, -h / 2.0);
        }
        r.render(&p, QRectF(0, 0, w, h));
        p.end();
        return QPixmap::fromImage(img);
    };
    QIcon icon;
    QPixmap px1 = renderAt(size, size);
    icon.addPixmap(px1);
    const int size2 = qMin(size * 2, 128);
    QPixmap px2 = renderAt(size2, size2);
    px2.setDevicePixelRatio(2.0);
    icon.addPixmap(px2);
    return icon;
}

// C callbacks (run on backend thread); marshal to main thread via EventBridge.
static void on_folder_found_cb(const char *name, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString n = QString::fromUtf8(name);
    QMetaObject::invokeMethod(b, "addFolder", Qt::QueuedConnection, Q_ARG(QString, n));
}
static void on_folder_removed_cb(const char *, void *) {}
static void on_folder_list_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onFolderListComplete", Qt::QueuedConnection, Q_ARG(int, error));
}
static void on_message_summary_cb(const char *id, const char *subject, const char *from_, uint64_t size, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "addMessageSummary", Qt::QueuedConnection,
        Q_ARG(QString, QString::fromUtf8(id)),
        Q_ARG(QString, subject ? QString::fromUtf8(subject) : QString()),
        Q_ARG(QString, from_ ? QString::fromUtf8(from_) : QString()),
        Q_ARG(quint64, size));
}
static void on_message_list_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onMessageListComplete", Qt::QueuedConnection, Q_ARG(int, error));
}
static void on_message_metadata_cb(const char *, const char *, const char *, const char *, void *) {}
static void on_message_content_cb(TagliacarteMessage *msg, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString subject = msg && msg->subject ? QString::fromUtf8(msg->subject) : QString();
    QString from_ = msg && msg->from_ ? QString::fromUtf8(msg->from_) : QString();
    QString to = msg && msg->to ? QString::fromUtf8(msg->to) : QString();
    QString bodyHtml = msg && msg->body_html && *msg->body_html ? QString::fromUtf8(msg->body_html) : QString();
    QString bodyPlain = msg && msg->body_plain && *msg->body_plain ? QString::fromUtf8(msg->body_plain) : QString();
    QMetaObject::invokeMethod(b, "showMessageContent", Qt::QueuedConnection,
        Q_ARG(QString, subject), Q_ARG(QString, from_), Q_ARG(QString, to),
        Q_ARG(QString, bodyHtml), Q_ARG(QString, bodyPlain));
}
static void on_message_complete_cb(int error, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onMessageComplete", Qt::QueuedConnection, Q_ARG(int, error));
}
static void on_send_progress_cb(const char *status, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString s = status ? QString::fromUtf8(status) : QString();
    QMetaObject::invokeMethod(b, "onSendProgress", Qt::QueuedConnection, Q_ARG(QString, s));
}
static void on_send_complete_cb(int ok, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QMetaObject::invokeMethod(b, "onSendComplete", Qt::QueuedConnection, Q_ARG(int, ok));
}

/* Async open folder: run on backend thread; marshal to main thread. */
static void on_folder_ready_cb(const char *folder_uri, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString uri = QString::fromUtf8(folder_uri);
    tagliacarte_free_string(const_cast<char *>(folder_uri));
    QMetaObject::invokeMethod(b, "onFolderReady", Qt::QueuedConnection, Q_ARG(QString, uri));
}
static void on_open_folder_error_cb(const char *message, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    QString msg = message ? QString::fromUtf8(message) : TR("error.unknown");
    QMetaObject::invokeMethod(b, "onOpenFolderError", Qt::QueuedConnection, Q_ARG(QString, msg));
}
static void on_open_folder_select_event_cb(int event_type, uint32_t number_value, const char *, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    if (event_type == TAGLIACARTE_OPEN_FOLDER_EXISTS && b->statusBar) {
        QMetaObject::invokeMethod(b, "showOpeningMessageCount", Qt::QueuedConnection, Q_ARG(quint32, number_value));
    }
}

// ----- Compose dialog (From/To/Subject/Body only; transport is the current store's transport) -----
class ComposeDialog : public QDialog {
public:
    QLineEdit *fromEdit;
    QLineEdit *toEdit;
    QLineEdit *subjectEdit;
    QTextEdit *bodyEdit;

    ComposeDialog(QWidget *parent = nullptr) : QDialog(parent) {
        setWindowTitle(TR("compose.title"));
        auto *layout = new QFormLayout(this);
        fromEdit = new QLineEdit(this);
        fromEdit->setPlaceholderText(TR("compose.placeholder.from"));
        layout->addRow(TR("compose.from") + QStringLiteral(":"), fromEdit);
        toEdit = new QLineEdit(this);
        toEdit->setPlaceholderText(TR("compose.placeholder.to"));
        layout->addRow(TR("compose.to") + QStringLiteral(":"), toEdit);
        subjectEdit = new QLineEdit(this);
        layout->addRow(TR("compose.subject") + QStringLiteral(":"), subjectEdit);
        bodyEdit = new QTextEdit(this);
        bodyEdit->setMinimumHeight(120);
        layout->addRow(TR("compose.body") + QStringLiteral(":"), bodyEdit);
        auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
        connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
        connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
        layout->addRow(buttons);
    }
};

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);

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
    const int sidebarWidth = 56;  // 40px circles + margins
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

    mainContentLayout->addWidget(toolbar);

    // Content: folder list | message list | message detail
    auto *contentSplitter = new QSplitter(Qt::Horizontal, mainContentPage);
    auto *folderListPanel = new QWidget(mainContentPage);
    auto *folderListPanelLayout = new QVBoxLayout(folderListPanel);
    folderListPanelLayout->setContentsMargins(8, 8, 0, 8);
    auto *folderList = new QListWidget(folderListPanel);
    folderList->setSelectionMode(QAbstractItemView::SingleSelection);
    folderListPanelLayout->addWidget(folderList);

    auto *rightSplitter = new QSplitter(Qt::Vertical, mainContentPage);
    auto *conversationList = new QListWidget(mainContentPage);
    conversationList->setSelectionMode(QAbstractItemView::SingleSelection);
    auto *messageView = new QTextBrowser(mainContentPage);
    messageView->setOpenExternalLinks(true);
    rightSplitter->addWidget(conversationList);
    rightSplitter->addWidget(messageView);
    rightSplitter->setStretchFactor(0, 0);
    rightSplitter->setStretchFactor(1, 1);

    contentSplitter->addWidget(folderListPanel);
    contentSplitter->addWidget(rightSplitter);
    contentSplitter->setStretchFactor(0, 0);
    contentSplitter->setStretchFactor(1, 1);
    contentSplitter->setSizes({ 180, 400 });
    mainContentLayout->addWidget(contentSplitter);

    rightStack->addWidget(mainContentPage);

    // Settings: tabbed pane (Accounts, Junk Mail, Viewing, Composing, Signatures, About)
    auto *settingsPage = new QWidget(central);
    auto *settingsLayout = new QVBoxLayout(settingsPage);
    settingsLayout->setContentsMargins(0, 0, 0, 0);
    auto *settingsTabs = new QTabWidget(settingsPage);
    QString editingStoreId;  // when set, save updates this account; when empty, save creates new
    // Accounts: stacked = (0) type list → (1) edit account panel
    auto *accountsStack = new QStackedWidget(settingsPage);
    // Page 0: select account to edit (if any) + create new account type buttons
    auto *accountsListPage = new QWidget(settingsPage);
    auto *accountsListLayout = new QVBoxLayout(accountsListPage);
    auto *selectLabel = new QLabel(TR("accounts.select_to_edit"), accountsListPage);
    selectLabel->setAlignment(Qt::AlignCenter);
    accountsListLayout->addWidget(selectLabel);
    auto *accountButtonsContainer = new QWidget(accountsListPage);
    auto *accountButtonsGrid = new QGridLayout(accountButtonsContainer);
    accountButtonsGrid->setAlignment(Qt::AlignCenter);
    accountButtonsContainer->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Minimum);
    accountsListLayout->addWidget(accountButtonsContainer);
    auto *createLabel = new QLabel(TR("accounts.create_new"), accountsListPage);
    createLabel->setAlignment(Qt::AlignCenter);
    accountsListLayout->addWidget(createLabel);
    struct AccountTypeBtn { QPushButton *btn; QString type; };
    QList<AccountTypeBtn> accountTypeButtons;
    struct { const char *key; bool enabled; } accountTypes[] = {
        { "accounts.type.imap", true },
        { "accounts.type.pop3", false },
        { "accounts.type.maildir", true },
        { "accounts.type.mbox", true },
        { "accounts.type.nostr", false },
        { "accounts.type.matrix", false },
        { "accounts.type.nntp", false }
    };
    const int accountButtonsPerRow = 6;
    QHBoxLayout *typeButtonsRow = new QHBoxLayout();
    typeButtonsRow->setAlignment(Qt::AlignCenter);
    for (const auto &p : accountTypes) {
        auto *b = new QPushButton(TR(p.key), accountsListPage);
        b->setEnabled(p.enabled);
        b->setMinimumWidth(120);
        accountTypeButtons.append({ b, QString::fromUtf8(p.key) });
        typeButtonsRow->addWidget(b, 0, Qt::AlignCenter);
    }
    accountsListLayout->addLayout(typeButtonsRow);
    accountsListLayout->addStretch();
    accountsStack->addWidget(accountsListPage);

    // Page 1: edit account panel (form for the selected type)
    auto *accountsEditPage = new QWidget(settingsPage);
    auto *accountsEditLayout = new QVBoxLayout(accountsEditPage);
    auto *accountsEditBackBtn = new QPushButton(TR("accounts.back_to_list"), accountsEditPage);
    accountsEditLayout->addWidget(accountsEditBackBtn);
    auto *accountFormStack = new QStackedWidget(accountsEditPage);
    // Maildir form
    auto *maildirForm = new QWidget(accountsEditPage);
    auto *maildirFormLayout = new QFormLayout(maildirForm);
    auto *maildirPathEdit = new QLineEdit(maildirForm);
    maildirPathEdit->setPlaceholderText(TR("maildir.placeholder.path"));
    auto *maildirBrowseBtn = new QPushButton(TR("common.browse"), maildirForm);
    auto *maildirPathRow = new QHBoxLayout();
    maildirPathRow->addWidget(maildirPathEdit);
    maildirPathRow->addWidget(maildirBrowseBtn);
    maildirFormLayout->addRow(TR("maildir.directory") + QStringLiteral(":"), maildirPathRow);
    auto *maildirDisplayNameEdit = new QLineEdit(maildirForm);
    maildirDisplayNameEdit->setPlaceholderText(TR("maildir.placeholder.display_name"));
    maildirFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), maildirDisplayNameEdit);
    auto *maildirSaveBtn = new QPushButton(TR("common.save"), maildirForm);
    maildirFormLayout->addRow(maildirSaveBtn);
    accountFormStack->addWidget(maildirForm);
    // mbox form
    auto *mboxForm = new QWidget(accountsEditPage);
    auto *mboxFormLayout = new QFormLayout(mboxForm);
    auto *mboxPathEdit = new QLineEdit(mboxForm);
    auto *mboxBrowseBtn = new QPushButton(TR("common.browse"), mboxForm);
    auto *mboxPathRow = new QHBoxLayout();
    mboxPathRow->addWidget(mboxPathEdit);
    mboxPathRow->addWidget(mboxBrowseBtn);
    mboxFormLayout->addRow(TR("mbox.file") + QStringLiteral(":"), mboxPathRow);
    auto *mboxDisplayNameEdit = new QLineEdit(mboxForm);
    mboxFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), mboxDisplayNameEdit);
    auto *mboxSaveBtn = new QPushButton(TR("common.save"), mboxForm);
    mboxFormLayout->addRow(mboxSaveBtn);
    accountFormStack->addWidget(mboxForm);
    // IMAP form (includes SMTP section for same account)
    auto *imapForm = new QWidget(accountsEditPage);
    auto *imapFormLayout = new QFormLayout(imapForm);
    auto *imapDisplayNameEdit = new QLineEdit(imapForm);
    imapDisplayNameEdit->setPlaceholderText(TR("imap.placeholder.display_name"));
    imapFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), imapDisplayNameEdit);
    auto *imapHostEdit = new QLineEdit(imapForm);
    imapHostEdit->setPlaceholderText(TR("imap.placeholder.host"));
    imapFormLayout->addRow(TR("imap.host") + QStringLiteral(":"), imapHostEdit);
    auto *imapSecurityCombo = new QComboBox(imapForm);
    imapSecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.starttls"), TR("imap.security.ssl") });
    imapSecurityCombo->setCurrentIndex(2);
    imapFormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), imapSecurityCombo);
    auto *imapPortSpin = new QSpinBox(imapForm);
    imapPortSpin->setRange(1, 65535);
    imapPortSpin->setValue(993);
    imapFormLayout->addRow(TR("imap.port") + QStringLiteral(":"), imapPortSpin);
    auto *imapUserEdit = new QLineEdit(imapForm);
    imapFormLayout->addRow(TR("imap.username") + QStringLiteral(":"), imapUserEdit);
    auto *imapPassEdit = new QLineEdit(imapForm);
    imapPassEdit->setEchoMode(QLineEdit::Password);
    imapFormLayout->addRow(TR("imap.password") + QStringLiteral(":"), imapPassEdit);
    auto *imapPollCombo = new QComboBox(imapForm);
    imapPollCombo->addItems({ TR("imap.poll.every_minute"), TR("imap.poll.every_5_minutes"), TR("imap.poll.every_10_minutes"), TR("imap.poll.every_hour") });
    imapFormLayout->addRow(TR("imap.poll.label") + QStringLiteral(":"), imapPollCombo);
    auto *imapDeletionCombo = new QComboBox(imapForm);
    imapDeletionCombo->addItems({ TR("imap.deletion.mark_expunge"), TR("imap.deletion.move_to_trash") });
    imapFormLayout->addRow(TR("imap.deletion.label") + QStringLiteral(":"), imapDeletionCombo);
    auto *imapTrashFolderEdit = new QLineEdit(imapForm);
    imapTrashFolderEdit->setPlaceholderText(TR("imap.placeholder.trash_folder"));
    imapFormLayout->addRow(TR("imap.trash_folder") + QStringLiteral(":"), imapTrashFolderEdit);
    auto *imapIdleCombo = new QComboBox(imapForm);
    imapIdleCombo->addItems({ TR("imap.idle.30_seconds"), TR("imap.idle.1_minute"), TR("imap.idle.5_minutes") });
    imapFormLayout->addRow(TR("imap.idle.label") + QStringLiteral(":"), imapIdleCombo);
    auto *smtpLabel = new QLabel(TR("imap.smtp_section"));
    imapFormLayout->addRow(smtpLabel);
    auto *smtpHostEdit = new QLineEdit(imapForm);
    smtpHostEdit->setPlaceholderText(TR("smtp.placeholder.host"));
    imapFormLayout->addRow(TR("smtp.host") + QStringLiteral(":"), smtpHostEdit);
    auto *smtpSecurityCombo = new QComboBox(imapForm);
    smtpSecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.starttls"), TR("imap.security.ssl") });
    smtpSecurityCombo->setCurrentIndex(1);
    imapFormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), smtpSecurityCombo);
    auto *smtpPortSpin = new QSpinBox(imapForm);
    smtpPortSpin->setRange(1, 65535);
    smtpPortSpin->setValue(586);
    imapFormLayout->addRow(TR("smtp.port") + QStringLiteral(":"), smtpPortSpin);
    auto *smtpUserEdit = new QLineEdit(imapForm);
    imapFormLayout->addRow(TR("smtp.username") + QStringLiteral(":"), smtpUserEdit);
    auto *smtpPassEdit = new QLineEdit(imapForm);
    smtpPassEdit->setEchoMode(QLineEdit::Password);
    imapFormLayout->addRow(TR("smtp.password") + QStringLiteral(":"), smtpPassEdit);
    auto *imapSaveBtn = new QPushButton(TR("common.save"), imapForm);
    imapFormLayout->addRow(imapSaveBtn);
    accountFormStack->addWidget(imapForm);

    // Security dropdown -> default port (IMAP: None/STARTTLS 143, SSL 993; SMTP: None/STARTTLS 586, SSL 465)
    QObject::connect(imapSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [imapPortSpin](int idx) {
        if (idx == 2) imapPortSpin->setValue(993);
        else imapPortSpin->setValue(143);
    });
    QObject::connect(smtpSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [smtpPortSpin](int idx) {
        if (idx == 2) smtpPortSpin->setValue(465);
        else smtpPortSpin->setValue(586);
    });
    // Placeholder forms for disabled types (POP3, Nostr, Matrix, NNTP)
    for (int i = 0; i < 4; ++i) {
        auto *ph = new QLabel(TR("accounts.not_implemented"), accountsEditPage);
        accountFormStack->addWidget(ph);
    }
    accountsEditLayout->addWidget(accountFormStack);
    accountsStack->addWidget(accountsEditPage);

    auto refreshAccountListInSettings = [&]() {
        while (QLayoutItem *item = accountButtonsGrid->takeAt(0)) {
            if (item->widget()) item->widget()->deleteLater();
            delete item;
        }
        Config c = loadConfig();
        const int cols = accountButtonsPerRow;
        for (int i = 0; i < c.stores.size(); ++i) {
            const StoreEntry &entry = c.stores[i];
            QString initial = entry.displayName.left(1).toUpper();
            if (initial.isEmpty()) initial = (entry.type == QLatin1String("maildir")) ? QStringLiteral("M") : QStringLiteral("I");
            auto *btn = new QToolButton(accountButtonsContainer);
            btn->setProperty("storeId", entry.id);
            btn->setText(initial);
            btn->setFixedSize(40, 40);
            btn->setToolTip(entry.displayName);
            QFont f = btn->font();
            f.setPointSize(20);
            f.setWeight(QFont::Bold);
            btn->setFont(f);
            btn->setStyleSheet(storeCircleStyleSheet(i));
            accountButtonsGrid->addWidget(btn, i / cols, i % cols, 1, 1, Qt::AlignCenter);
            StoreEntry entryCopy = entry;
            QObject::connect(btn, &QToolButton::clicked, [&, entryCopy]() {
                editingStoreId = entryCopy.id;
                if (entryCopy.type == QLatin1String("maildir")) {
                    accountFormStack->setCurrentIndex(0);
                    maildirPathEdit->setText(entryCopy.path);
                    maildirDisplayNameEdit->setText(entryCopy.displayName);
                } else if (entryCopy.type == QLatin1String("imap")) {
                    accountFormStack->setCurrentIndex(2);
                    imapDisplayNameEdit->setText(entryCopy.displayName);
                    imapHostEdit->setText(entryCopy.path);
                }
                accountsStack->setCurrentIndex(1);
            });
        }
    };

    settingsTabs->addTab(accountsStack, TR("settings.rubric.accounts"));
    const char *placeholderKeys[] = { "settings.placeholder.junk_mail", "settings.placeholder.viewing", "settings.placeholder.composing", "settings.placeholder.signatures" };
    const char *tabKeys[] = { "settings.rubric.junk_mail", "settings.rubric.viewing", "settings.rubric.composing", "settings.rubric.signatures" };
    for (int i = 0; i < 4; ++i) {
        auto *p = new QLabel(TR(placeholderKeys[i]), settingsPage);
        p->setAlignment(Qt::AlignTop | Qt::AlignLeft);
        p->setMargin(24);
        settingsTabs->addTab(p, TR(tabKeys[i]));
    }
    // About pane
    auto *aboutPage = new QWidget(settingsPage);
    auto *aboutLayout = new QVBoxLayout(aboutPage);
    aboutLayout->setSpacing(12);
    aboutLayout->addSpacing(16);
    auto *aboutNameLabel = new QLabel(TR("app.name"), aboutPage);
    QFont nameFont = aboutNameLabel->font();
    nameFont.setPointSize(nameFont.pointSize() + 4);
    nameFont.setWeight(QFont::Bold);
    aboutNameLabel->setFont(nameFont);
    aboutLayout->addWidget(aboutNameLabel);
    auto *aboutVersionLabel = new QLabel(TR("about.version").arg(QString::fromUtf8(version)), aboutPage);
    aboutLayout->addWidget(aboutVersionLabel);
    auto *aboutCopyrightLabel = new QLabel(TR("about.copyright"), aboutPage);
    aboutLayout->addWidget(aboutCopyrightLabel);
    auto *aboutLicenceLabel = new QLabel(TR("about.licence"), aboutPage);
    aboutLicenceLabel->setWordWrap(true);
    aboutLayout->addWidget(aboutLicenceLabel);
    aboutLayout->addStretch();
    settingsTabs->addTab(aboutPage, TR("settings.rubric.about"));
    QObject::connect(settingsTabs, &QTabWidget::currentChanged, [&](int index) {
        if (index == 0) refreshAccountListInSettings();
    });

    settingsLayout->addWidget(settingsTabs);
    rightStack->addWidget(settingsPage);

    QObject::connect(settingsBtn, &QToolButton::clicked, [&]() {
        if (settingsBtn->isChecked()) {
            rightStack->setCurrentIndex(1);
            refreshAccountListInSettings();
        } else {
            rightStack->setCurrentIndex(0);
        }
    });

    mainLayout->addWidget(rightStack, 1);

    EventBridge bridge;
    bridge.folderList = folderList;
    bridge.conversationList = conversationList;
    bridge.messageView = messageView;
    bridge.statusBar = win.statusBar();
    bridge.win = &win;

    QByteArray storeUri;
    QByteArray smtpTransportUri;  // transport for the currently selected store (used for compose)
    QMap<QByteArray, QByteArray> storeToTransport;  // store URI -> transport URI (only IMAP-with-SMTP have one)
    QList<QToolButton *> storeButtons; // circle buttons in sidebar; one per store
    QList<QByteArray> allStoreUris;    // all created store URIs (for multi-store and shutdown)

    auto updateComposeAppendButtons = [&]() {
        bool hasTransport = !smtpTransportUri.isEmpty();
        composeBtn->setVisible(hasTransport);
        composeBtn->setEnabled(hasTransport);
        appendMessageBtn->setVisible(!hasTransport);
        appendMessageBtn->setEnabled(!hasTransport && !bridge.folderUri().isEmpty());
    };

    auto addStoreCircle = [&](const QString &initial, const QByteArray &uri, int colourIndex) {
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
        QObject::connect(btn, &QToolButton::clicked, [&, btn]() {
            QVariant v = btn->property("storeUri");
            if (!v.isValid()) return;
            QByteArray u = v.toByteArray();
            if (u.isEmpty()) return;
            storeUri = u;
            smtpTransportUri = storeToTransport.value(storeUri);
            updateComposeAppendButtons();
            bridge.clearFolder();
            folderList->clear();
            conversationList->clear();
            messageView->clear();
            for (auto *b : storeButtons) b->setChecked(b == btn);
            tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
            tagliacarte_store_refresh_folders(storeUri.constData());
            win.statusBar()->showMessage(TR("status.folders_loaded"));
        });
        if (storeButtons.size() == 1)
            btn->setChecked(true);
    };

    auto refreshStoresFromConfig = [&]() {
        for (const QByteArray &u : allStoreUris) tagliacarte_store_free(u.constData());
        for (const QByteArray &t : storeToTransport) tagliacarte_transport_free(t.constData());
        allStoreUris.clear();
        storeToTransport.clear();
        for (QToolButton *b : storeButtons) b->deleteLater();
        storeButtons.clear();
        bridge.clearFolder();
        folderList->clear();
        conversationList->clear();
        messageView->clear();
        Config c = loadConfig();
        for (int i = 0; i < c.stores.size(); ++i) {
            const StoreEntry &entry = c.stores[i];
            if (entry.type != QLatin1String("maildir") || entry.path.isEmpty())
                continue;
            char *uriPtr = tagliacarte_store_maildir_new(entry.path.toUtf8().constData());
            if (!uriPtr) continue;
            QByteArray uri(uriPtr);
            tagliacarte_free_string(uriPtr);
            allStoreUris.append(uri);
            QString initial = entry.displayName.left(1).toUpper();
            if (initial.isEmpty()) initial = QStringLiteral("M");
            addStoreCircle(initial, uri, i);
            tagliacarte_store_set_folder_list_callbacks(uri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
        }
        if (!allStoreUris.isEmpty()) {
            QString lastId = c.lastSelectedStoreId;
            if (!lastId.isEmpty()) {
                QByteArray lastUtf8 = lastId.toUtf8();
                for (const QByteArray &u : allStoreUris) {
                    if (u == lastUtf8) { storeUri = u; break; }
                }
            }
            if (storeUri.isEmpty()) storeUri = allStoreUris.first();
            smtpTransportUri = storeToTransport.value(storeUri);
            for (int i = 0; i < storeButtons.size(); ++i) {
                if (storeButtons[i]->property("storeUri").toByteArray() == storeUri) {
                    storeButtons[i]->setChecked(true);
                    break;
                }
            }
            tagliacarte_store_refresh_folders(storeUri.constData());
            updateComposeAppendButtons();
            win.statusBar()->showMessage(TR("status.folders_loaded"));
        } else
            storeUri.clear();
    };

    // Startup: restore stores from config, or open Settings if none
    {
        Config startupConfig = loadConfig();
        if (startupConfig.stores.isEmpty()) {
            rightStack->setCurrentIndex(1);
            settingsBtn->setChecked(true);
            win.statusBar()->showMessage(TR("status.add_store_to_start"));
        } else {
            for (const StoreEntry &entry : startupConfig.stores) {
                if (entry.type != QLatin1String("maildir") || entry.path.isEmpty())
                    continue;
                char *uriPtr = tagliacarte_store_maildir_new(entry.path.toUtf8().constData());
                if (!uriPtr) continue;
                QByteArray uri(uriPtr);
                tagliacarte_free_string(uriPtr);
                allStoreUris.append(uri);
                QString initial = entry.displayName.left(1).toUpper();
                if (initial.isEmpty()) initial = QStringLiteral("M");
                addStoreCircle(initial, uri, int(allStoreUris.size()) - 1);
                tagliacarte_store_set_folder_list_callbacks(uri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
            }
            if (!allStoreUris.isEmpty()) {
                QString lastId = startupConfig.lastSelectedStoreId;
                if (!lastId.isEmpty()) {
                    QByteArray lastUtf8 = lastId.toUtf8();
                    for (const QByteArray &u : allStoreUris) {
                        if (u == lastUtf8) { storeUri = u; break; }
                    }
                }
                if (storeUri.isEmpty()) storeUri = allStoreUris.first();
                smtpTransportUri = storeToTransport.value(storeUri);
                for (int i = 0; i < storeButtons.size() && i < allStoreUris.size(); ++i) {
                    if (storeButtons[i]->property("storeUri").toByteArray() == storeUri) {
                        storeButtons[i]->setChecked(true);
                        break;
                    }
                }
                tagliacarte_store_refresh_folders(storeUri.constData());
                updateComposeAppendButtons();
                win.statusBar()->showMessage(TR("status.folders_loaded"));
            }
        }
    }

    // Account type list -> edit panel: switch to edit and show form for that type (create new: clear editingStoreId)
    const QVector<int> formIndexForType = { 2, 3, 0, 1, 4, 5, 6 }; // IMAP,POP3,Maildir,mbox,Nostr,Matrix,NNTP
    for (int i = 0; i < accountTypeButtons.size(); ++i) {
        QPushButton *b = accountTypeButtons[i].btn;
        int formIdx = formIndexForType[i];
        QObject::connect(b, &QPushButton::clicked, [&, accountsStack, accountFormStack, formIdx]() {
            editingStoreId.clear();
            accountFormStack->setCurrentIndex(formIdx);
            accountsStack->setCurrentIndex(1);
        });
    }
    QObject::connect(accountsEditBackBtn, &QPushButton::clicked, [&]() {
        accountsStack->setCurrentIndex(0);
        refreshAccountListInSettings();
    });

    QObject::connect(maildirBrowseBtn, &QPushButton::clicked, [&]() {
        QString path = QFileDialog::getExistingDirectory(&win, TR("dialog.select_maildir_directory"), maildirPathEdit->text());
        if (!path.isEmpty()) maildirPathEdit->setText(path);
    });
    QObject::connect(mboxBrowseBtn, &QPushButton::clicked, [&]() {
        QString path = QFileDialog::getOpenFileName(&win, TR("dialog.select_mbox_file"), QString(), QStringLiteral("mbox (*)"));
        if (!path.isEmpty()) mboxPathEdit->setText(path);
    });

    QObject::connect(maildirSaveBtn, &QPushButton::clicked, [&]() {
        QString path = maildirPathEdit->text().trimmed();
        if (path.isEmpty()) {
            QMessageBox::warning(&win, TR("accounts.type.maildir"), TR("maildir.validation.select_directory"));
            return;
        }
        char *uriPtr = tagliacarte_store_maildir_new(path.toUtf8().constData());
        if (!uriPtr) {
            showError(&win, "error.context.maildir");
            return;
        }
        QByteArray newUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        tagliacarte_store_free(newUri.constData());  // we only needed the URI for config
        QString displayName = maildirDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) displayName = QDir(path).dirName();
        if (displayName.isEmpty()) displayName = TR("maildir.default_display_name");

        Config config = loadConfig();
        QString oldId = editingStoreId;
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) { idx = i; break; }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newUri);
                e.type = QStringLiteral("maildir");
                e.displayName = displayName;
                e.path = path;
                if (config.lastSelectedStoreId == oldId)
                    config.lastSelectedStoreId = e.id;
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newUri);
            entry.type = QStringLiteral("maildir");
            entry.displayName = displayName;
            entry.path = path;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        refreshStoresFromConfig();
        editingStoreId.clear();
        win.statusBar()->showMessage(TR("status.added_maildir"));
        accountsStack->setCurrentIndex(0);
        rightStack->setCurrentIndex(0);
        settingsBtn->setChecked(false);
    });

    QObject::connect(imapSaveBtn, &QPushButton::clicked, [&]() {
        QString displayName = imapDisplayNameEdit->text().trimmed();
        QString imapUser = imapUserEdit->text().trimmed();
        QString imapHost = imapHostEdit->text().trimmed();
        int imapPort = imapPortSpin->value();
        QString smtpHost = smtpHostEdit->text().trimmed();
        int smtpPort = smtpPortSpin->value();
        if (imapHost.isEmpty()) {
            QMessageBox::warning(&win, TR("accounts.type.imap"), TR("imap.validation.enter_host"));
            return;
        }
        Config config = loadConfig();
        const bool isEdit = !editingStoreId.isEmpty();
        const QByteArray previousStoreUri = storeUri;
        int colourIndex = 0;
        if (isEdit) {
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == editingStoreId) { colourIndex = i; break; }
            }
        } else {
            colourIndex = config.stores.size();
        }
        QString userAtHost = imapUser;
        if (!imapUser.contains(QLatin1Char('@')) && !imapHost.isEmpty())
            userAtHost = imapUser + QLatin1Char('@') + imapHost;
        char *uriPtr = tagliacarte_store_imap_new(userAtHost.toUtf8().constData(), imapHost.toUtf8().constData(), static_cast<uint16_t>(imapPort));
        if (!uriPtr) {
            showError(&win, "error.context.imap");
            return;
        }
        storeUri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (displayName.isEmpty()) displayName = imapUser;
        if (displayName.isEmpty()) displayName = TR("accounts.type.imap");

        if (isEdit) {
            QByteArray oldUri = editingStoreId.toUtf8();
            tagliacarte_store_free(oldUri.constData());
            storeToTransport.remove(oldUri);
            allStoreUris.removeAll(oldUri);
            for (int i = 0; i < storeButtons.size(); ++i) {
                if (storeButtons[i]->property("storeUri").toByteArray() == oldUri) {
                    storeButtons[i]->deleteLater();
                    storeButtons.removeAt(i);
                    break;
                }
            }
            if (previousStoreUri == oldUri) {
                bridge.clearFolder();
                folderList->clear();
                conversationList->clear();
                messageView->clear();
            }
        } else {
            for (const QByteArray &t : storeToTransport) tagliacarte_transport_free(t.constData());
            storeToTransport.clear();
            smtpTransportUri.clear();
            for (const QByteArray &u : allStoreUris) tagliacarte_store_free(u.constData());
            allStoreUris.clear();
            bridge.clearFolder();
            for (auto *b : storeButtons) b->deleteLater();
            storeButtons.clear();
            folderList->clear();
            conversationList->clear();
            messageView->clear();
        }
        allStoreUris.append(storeUri);
        tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
        tagliacarte_store_refresh_folders(storeUri.constData());
        QString initial = displayName.left(1).toUpper();
        if (initial.isEmpty()) initial = QStringLiteral("I");
        addStoreCircle(initial, storeUri, colourIndex);
        smtpTransportUri.clear();
        if (!smtpHost.isEmpty()) {
            char *transportUri = tagliacarte_transport_smtp_new(smtpHost.toUtf8().constData(), static_cast<uint16_t>(smtpPort));
            if (transportUri) {
                smtpTransportUri = QByteArray(transportUri);
                tagliacarte_free_string(transportUri);
                storeToTransport[storeUri] = smtpTransportUri;
            }
        }
        updateComposeAppendButtons();

        if (isEdit) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == editingStoreId) { idx = i; break; }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(storeUri);
                e.type = QStringLiteral("imap");
                e.displayName = displayName;
                e.path = imapHost;
                if (config.lastSelectedStoreId == editingStoreId)
                    config.lastSelectedStoreId = e.id;
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(storeUri);
            entry.type = QStringLiteral("imap");
            entry.displayName = displayName;
            entry.path = imapHost;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        editingStoreId.clear();
        win.statusBar()->showMessage(TR("status.added_imap"));
        accountsStack->setCurrentIndex(0);
        rightStack->setCurrentIndex(0);
        settingsBtn->setChecked(false);
    });

    QObject::connect(mboxSaveBtn, &QPushButton::clicked, [&]() {
        QMessageBox::information(&win, TR("accounts.type.mbox"), TR("mbox.not_implemented"));
    });

    QObject::connect(&bridge, &EventBridge::folderReadyForMessages, [&](quint64 total) {
        QByteArray uri = bridge.folderUri();
        if (uri.isEmpty()) return;
        updateComposeAppendButtons();
        auto *item = folderList->currentItem();
        tagliacarte_folder_set_message_list_callbacks(uri.constData(), on_message_summary_cb, on_message_list_complete_cb, &bridge);
        tagliacarte_folder_request_message_list(uri.constData(), 0, total > 100 ? 100 : total);
        if (item) win.statusBar()->showMessage(TR("status.folder_loading").arg(item->text()));
    });

    QObject::connect(folderList, &QListWidget::itemSelectionChanged, [&]() {
        bridge.clearFolder();
        conversationList->clear();
        messageView->clear();

        auto *item = folderList->currentItem();
        if (!item || storeUri.isEmpty()) return;

        bridge.setFolderNameOpening(item->text());
        QByteArray name = item->text().toUtf8();
        tagliacarte_store_start_open_folder(
            storeUri.constData(),
            name.constData(),
            on_open_folder_select_event_cb,
            on_folder_ready_cb,
            on_open_folder_error_cb,
            &bridge
        );
        win.statusBar()->showMessage(TR("status.opening").arg(item->text()));
    });

    QObject::connect(conversationList, &QListWidget::itemSelectionChanged, [&]() {
        messageView->clear();
        auto *item = conversationList->currentItem();
        QByteArray uri = bridge.folderUri();
        if (!item || uri.isEmpty()) return;

        QVariant idVar = item->data(MessageIdRole);
        if (!idVar.isValid()) return;
        QByteArray id = idVar.toString().toUtf8();

        tagliacarte_folder_set_message_callbacks(uri.constData(), on_message_metadata_cb, on_message_content_cb, on_message_complete_cb, &bridge);
        tagliacarte_folder_request_message(uri.constData(), id.constData());
        messageView->setPlainText(TR("status.loading"));
    });

    QObject::connect(appendMessageBtn, &QToolButton::clicked, [&]() {
        QByteArray folderUri = bridge.folderUri();
        if (folderUri.isEmpty()) return;
        QString path = QFileDialog::getOpenFileName(&win, TR("append_message.dialog_title"),
            QString(), TR("append_message.file_filter"));
        if (path.isEmpty()) return;
        QFile f(path);
        if (!f.open(QIODevice::ReadOnly)) {
            QMessageBox::warning(&win, TR("common.error"), TR("append_message.read_error"));
            return;
        }
        QByteArray data = f.readAll();
        f.close();
        if (data.isEmpty()) return;
        int r = tagliacarte_folder_append_message(folderUri.constData(),
            reinterpret_cast<const unsigned char *>(data.constData()), data.size());
        if (r != 0) {
            showError(&win, "error.context.append_message");
            return;
        }
        conversationList->clear();
        quint64 total = tagliacarte_folder_message_count(folderUri.constData());
        tagliacarte_folder_set_message_list_callbacks(folderUri.constData(), on_message_summary_cb, on_message_list_complete_cb, &bridge);
        tagliacarte_folder_request_message_list(folderUri.constData(), 0, total > 100 ? 100 : total);
        win.statusBar()->showMessage(TR("status.message_appended"));
    });

    QObject::connect(composeBtn, &QPushButton::clicked, [&]() {
        if (smtpTransportUri.isEmpty()) return;  // no transport for current store (button should be disabled)
        ComposeDialog dlg(&win);
        if (dlg.exec() != QDialog::Accepted) return;

        QString from = dlg.fromEdit->text().trimmed();
        QString to = dlg.toEdit->text().trimmed();
        QString subject = dlg.subjectEdit->text().trimmed();
        QString body = dlg.bodyEdit->toPlainText();

        if (from.isEmpty() || to.isEmpty()) {
            QMessageBox::warning(&win, TR("compose.title"), TR("compose.validation.from_to"));
            return;
        }

        QByteArray fromUtf8 = from.toUtf8();
        QByteArray toUtf8 = to.toUtf8();
        QByteArray subjUtf8 = subject.toUtf8();
        QByteArray bodyUtf8 = body.toUtf8();

        win.statusBar()->showMessage(TR("status.sending"));
        tagliacarte_transport_send_async(
            smtpTransportUri.constData(),
            fromUtf8.constData(),
            toUtf8.constData(),
            nullptr,           /* cc */
            subjUtf8.constData(),
            bodyUtf8.constData(),
            nullptr,           /* body_html */
            0,
            nullptr,           /* attachments */
            on_send_progress_cb,
            on_send_complete_cb,
            &bridge
        );
    });

    win.statusBar()->showMessage(TR("status.open_maildir_to_start"));
    win.show();

    int ret = app.exec();

    bridge.clearFolder();
    for (const QByteArray &u : allStoreUris) tagliacarte_store_free(u.constData());
    for (const QByteArray &t : storeToTransport) tagliacarte_transport_free(t.constData());

    return ret;
}
