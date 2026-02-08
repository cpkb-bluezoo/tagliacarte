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
#include <QScrollArea>
#include <QSizePolicy>
#include <QSpinBox>
#include <QCheckBox>
#include <QDir>
#include <QStandardPaths>
#include <QFile>
#include <QSaveFile>
#include <QXmlStreamReader>
#include <QXmlStreamWriter>
#include <QSvgRenderer>
#include <QPainter>
#include <QColor>

#include "tagliacarte.h"
#include "EventBridge.h"

#include <cstring>
#include <cstdint>

void showError(QWidget *parent, const char *context) {
    const char *msg = tagliacarte_last_error();
    if (!msg) msg = "Unknown error";
    QMessageBox::critical(parent, "Error", QString("%1: %2").arg(context).arg(msg));
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

// Load SVG from resource and render as QIcon with currentColor replaced by palette color
static QIcon iconFromSvgResource(const QString &path, const QColor &color, int size = 24) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly))
        return QIcon();
    QByteArray svg = f.readAll();
    f.close();
    svg.replace("currentColor", color.name(QColor::HexRgb).toLatin1());
    QSvgRenderer r(svg);
    if (!r.isValid())
        return QIcon();
    QImage img(size, size, QImage::Format_ARGB32);
    img.fill(Qt::transparent);
    QPainter p(&img);
    r.render(&p);
    p.end();
    QIcon icon;
    icon.addPixmap(QPixmap::fromImage(img));
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
    QString msg = message ? QString::fromUtf8(message) : QStringLiteral("Unknown error");
    QMetaObject::invokeMethod(b, "onOpenFolderError", Qt::QueuedConnection, Q_ARG(QString, msg));
}
static void on_open_folder_select_event_cb(int event_type, uint32_t number_value, const char *, void *user_data) {
    EventBridge *b = static_cast<EventBridge*>(user_data);
    if (event_type == TAGLIACARTE_OPEN_FOLDER_EXISTS && b->statusBar) {
        QMetaObject::invokeMethod(b, "showOpeningMessageCount", Qt::QueuedConnection, Q_ARG(quint32, number_value));
    }
}

// ----- Compose dialog -----
class ComposeDialog : public QDialog {
public:
    QLineEdit *fromEdit;
    QLineEdit *toEdit;
    QLineEdit *subjectEdit;
    QTextEdit *bodyEdit;
    QLineEdit *smtpHostEdit;
    QLineEdit *smtpPortEdit;

    ComposeDialog(QWidget *parent = nullptr) : QDialog(parent) {
        setWindowTitle("Compose");
        auto *layout = new QFormLayout(this);
        fromEdit = new QLineEdit(this);
        fromEdit->setPlaceholderText("sender@example.com");
        layout->addRow("From:", fromEdit);
        toEdit = new QLineEdit(this);
        toEdit->setPlaceholderText("recipient@example.com");
        layout->addRow("To:", toEdit);
        subjectEdit = new QLineEdit(this);
        layout->addRow("Subject:", subjectEdit);
        bodyEdit = new QTextEdit(this);
        bodyEdit->setMinimumHeight(120);
        layout->addRow("Body:", bodyEdit);
        smtpHostEdit = new QLineEdit(this);
        smtpHostEdit->setPlaceholderText("smtp.example.com");
        layout->addRow("SMTP host:", smtpHostEdit);
        smtpPortEdit = new QLineEdit(this);
        smtpPortEdit->setText("587");
        layout->addRow("SMTP port:", smtpPortEdit);
        auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
        connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
        connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
        layout->addRow(buttons);
    }
};

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);

    const char *version = tagliacarte_version();
    if (!version) {
        QMessageBox::critical(nullptr, "Tagliacarte", "Failed to link tagliacarte_ffi (version is null).");
        return 1;
    }

    QMainWindow win;
    win.setWindowTitle(QString("Tagliacarte — %1").arg(version));
    win.setMinimumSize(800, 550);

    auto *central = new QWidget(&win);
    auto *mainLayout = new QHBoxLayout(central);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);
    win.setCentralWidget(central);

    // ----- Left sidebar: store list (circle icons) + settings at bottom -----
    const int sidebarWidth = 56;
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
    settingsBtn->setToolTip("Settings");
    QIcon cogIcon = iconFromSvgResource(QStringLiteral(":/icons/cog.svg"),
                                        QApplication::palette().color(QPalette::ButtonText), 24);
    if (!cogIcon.isNull())
        settingsBtn->setIcon(cogIcon);
    else
        settingsBtn->setText(QString::fromUtf8("⚙"));
    settingsBtn->setIconSize(QSize(24, 24));
    settingsBtn->setFixedSize(40, 40);
    settingsBtn->setStyleSheet(
        "QToolButton#settingsBtn { border-radius: 20px; background-color: palette(button); color: palette(button-text); }"
        "QToolButton#settingsBtn:hover { background-color: palette(light); }"
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
    composeBtn->setToolTip("Compose");
    QIcon quillIcon = iconFromSvgResource(QStringLiteral(":/icons/quill.svg"),
                                          QApplication::palette().color(QPalette::WindowText), 22);
    if (!quillIcon.isNull())
        composeBtn->setIcon(quillIcon);
    composeBtn->setIconSize(QSize(22, 22));
    composeBtn->setFixedSize(40, 40);
    composeBtn->setStyleSheet(
        "QToolButton { border-radius: 20px; background-color: transparent; }"
        "QToolButton:hover:enabled { background-color: palette(mid); }"
        "QToolButton:disabled { opacity: 0.5; }"
    );
    composeBtn->setEnabled(false);
    toolbarLayout->addWidget(composeBtn);

    mainContentLayout->addWidget(toolbar);

    // Content: folder list | message list | message detail
    auto *contentSplitter = new QSplitter(Qt::Horizontal, mainContentPage);
    auto *folderListPanel = new QWidget(mainContentPage);
    auto *folderListPanelLayout = new QVBoxLayout(folderListPanel);
    folderListPanelLayout->setContentsMargins(8, 8, 0, 8);
    folderListPanelLayout->addWidget(new QLabel("Folders"));
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

    // Settings overlay: rubric list + stacked content panes
    auto *settingsPage = new QWidget(central);
    auto *settingsLayout = new QHBoxLayout(settingsPage);
    settingsLayout->setContentsMargins(0, 0, 0, 0);
    auto *settingsSidebar = new QFrame(settingsPage);
    settingsSidebar->setFixedWidth(180);
    settingsSidebar->setStyleSheet("QFrame { background-color: palette(mid); }");
    auto *settingsSidebarLayout = new QVBoxLayout(settingsSidebar);
    settingsSidebarLayout->setContentsMargins(8, 16, 8, 8);
    auto *rubricList = new QListWidget(settingsPage);
    rubricList->addItems({ "Accounts", "Junk Mail", "Viewing", "Composing", "Signatures" });
    rubricList->setCurrentRow(0);
    settingsSidebarLayout->addWidget(rubricList);

    auto *settingsStack = new QStackedWidget(settingsPage);
    // Accounts: stacked = (0) type list → (1) edit account panel
    auto *accountsStack = new QStackedWidget(settingsPage);
    // Page 0: list of store-type buttons (create new store of specified type)
    auto *accountsListPage = new QWidget(settingsPage);
    auto *accountsListLayout = new QVBoxLayout(accountsListPage);
    auto *accountsListLabel = new QLabel(
        QStringLiteral("Create a new store of the specified type. "
                       "IMAP and Maildir include an associated SMTP transport where applicable."),
        accountsListPage);
    accountsListLabel->setWordWrap(true);
    accountsListLabel->setAlignment(Qt::AlignCenter);
    accountsListLayout->addWidget(accountsListLabel);
    struct AccountTypeBtn { QPushButton *btn; QString type; };
    QList<AccountTypeBtn> accountTypeButtons;
    for (const auto &p : {
        std::make_pair(QStringLiteral("IMAP"), true),
        std::make_pair(QStringLiteral("POP3"), false),
        std::make_pair(QStringLiteral("Maildir"), true),
        std::make_pair(QStringLiteral("mbox"), true),
        std::make_pair(QStringLiteral("Nostr"), false),
        std::make_pair(QStringLiteral("Matrix"), false),
        std::make_pair(QStringLiteral("NNTP"), false)
    }) {
        auto *b = new QPushButton(p.first + QStringLiteral("…"), accountsListPage);
        b->setEnabled(p.second);
        b->setMinimumWidth(160);
        accountTypeButtons.append({ b, p.first });
        accountsListLayout->addWidget(b, 0, Qt::AlignHCenter);
    }
    accountsListLayout->addStretch();
    accountsStack->addWidget(accountsListPage);

    // Page 1: edit account panel (form for the selected type)
    auto *accountsEditPage = new QWidget(settingsPage);
    auto *accountsEditLayout = new QVBoxLayout(accountsEditPage);
    auto *accountsEditBackBtn = new QPushButton(QStringLiteral("← Account list"), accountsEditPage);
    accountsEditLayout->addWidget(accountsEditBackBtn);
    auto *accountFormStack = new QStackedWidget(accountsEditPage);
    // Maildir form
    auto *maildirForm = new QWidget(accountsEditPage);
    auto *maildirFormLayout = new QFormLayout(maildirForm);
    auto *maildirPathEdit = new QLineEdit(maildirForm);
    maildirPathEdit->setPlaceholderText(QStringLiteral("/path/to/maildir"));
    auto *maildirBrowseBtn = new QPushButton(QStringLiteral("Browse…"), maildirForm);
    auto *maildirPathRow = new QHBoxLayout();
    maildirPathRow->addWidget(maildirPathEdit);
    maildirPathRow->addWidget(maildirBrowseBtn);
    maildirFormLayout->addRow(QStringLiteral("Directory:"), maildirPathRow);
    auto *maildirDisplayNameEdit = new QLineEdit(maildirForm);
    maildirDisplayNameEdit->setPlaceholderText(QStringLiteral("Optional display name"));
    maildirFormLayout->addRow(QStringLiteral("Display name:"), maildirDisplayNameEdit);
    auto *maildirSaveBtn = new QPushButton(QStringLiteral("Save"), maildirForm);
    auto *maildirCancelBtn = new QPushButton(QStringLiteral("Cancel"), maildirForm);
    auto *maildirBtnRow = new QHBoxLayout();
    maildirBtnRow->addWidget(maildirSaveBtn);
    maildirBtnRow->addWidget(maildirCancelBtn);
    maildirFormLayout->addRow(maildirBtnRow);
    accountFormStack->addWidget(maildirForm);
    // mbox form
    auto *mboxForm = new QWidget(accountsEditPage);
    auto *mboxFormLayout = new QFormLayout(mboxForm);
    auto *mboxPathEdit = new QLineEdit(mboxForm);
    auto *mboxBrowseBtn = new QPushButton(QStringLiteral("Browse…"), mboxForm);
    auto *mboxPathRow = new QHBoxLayout();
    mboxPathRow->addWidget(mboxPathEdit);
    mboxPathRow->addWidget(mboxBrowseBtn);
    mboxFormLayout->addRow(QStringLiteral("File:"), mboxPathRow);
    auto *mboxDisplayNameEdit = new QLineEdit(mboxForm);
    mboxFormLayout->addRow(QStringLiteral("Display name:"), mboxDisplayNameEdit);
    auto *mboxSaveBtn = new QPushButton(QStringLiteral("Save"), mboxForm);
    auto *mboxCancelBtn = new QPushButton(QStringLiteral("Cancel"), mboxForm);
    auto *mboxBtnRow = new QHBoxLayout();
    mboxBtnRow->addWidget(mboxSaveBtn);
    mboxBtnRow->addWidget(mboxCancelBtn);
    mboxFormLayout->addRow(mboxBtnRow);
    accountFormStack->addWidget(mboxForm);
    // IMAP form (includes SMTP section for same account)
    auto *imapForm = new QWidget(accountsEditPage);
    auto *imapFormLayout = new QFormLayout(imapForm);
    auto *imapDisplayNameEdit = new QLineEdit(imapForm);
    imapDisplayNameEdit->setPlaceholderText(QStringLiteral("e.g. Work"));
    imapFormLayout->addRow(QStringLiteral("Display name:"), imapDisplayNameEdit);
    auto *imapUserEdit = new QLineEdit(imapForm);
    imapFormLayout->addRow(QStringLiteral("IMAP username:"), imapUserEdit);
    auto *imapPassEdit = new QLineEdit(imapForm);
    imapPassEdit->setEchoMode(QLineEdit::Password);
    imapFormLayout->addRow(QStringLiteral("IMAP password:"), imapPassEdit);
    auto *imapHostEdit = new QLineEdit(imapForm);
    imapHostEdit->setPlaceholderText(QStringLiteral("imap.example.com"));
    imapFormLayout->addRow(QStringLiteral("IMAP host:"), imapHostEdit);
    auto *imapPortSpin = new QSpinBox(imapForm);
    imapPortSpin->setRange(1, 65535);
    imapPortSpin->setValue(993);
    imapFormLayout->addRow(QStringLiteral("IMAP port:"), imapPortSpin);
    auto *imapSslCheck = new QCheckBox(imapForm);
    imapSslCheck->setChecked(true);
    imapSslCheck->setText(QStringLiteral("Implicit SSL (imaps)"));
    imapFormLayout->addRow(imapSslCheck);
    auto *smtpLabel = new QLabel(QStringLiteral("<b>SMTP (sending)</b>"));
    imapFormLayout->addRow(smtpLabel);
    auto *smtpUserEdit = new QLineEdit(imapForm);
    imapFormLayout->addRow(QStringLiteral("SMTP username:"), smtpUserEdit);
    auto *smtpPassEdit = new QLineEdit(imapForm);
    smtpPassEdit->setEchoMode(QLineEdit::Password);
    imapFormLayout->addRow(QStringLiteral("SMTP password:"), smtpPassEdit);
    auto *smtpHostEdit = new QLineEdit(imapForm);
    smtpHostEdit->setPlaceholderText(QStringLiteral("smtp.example.com"));
    imapFormLayout->addRow(QStringLiteral("SMTP host:"), smtpHostEdit);
    auto *smtpPortSpin = new QSpinBox(imapForm);
    smtpPortSpin->setRange(1, 65535);
    smtpPortSpin->setValue(587);
    imapFormLayout->addRow(QStringLiteral("SMTP port:"), smtpPortSpin);
    auto *smtpSslCheck = new QCheckBox(imapForm);
    smtpSslCheck->setText(QStringLiteral("Use SSL/TLS"));
    imapFormLayout->addRow(smtpSslCheck);
    auto *imapSaveBtn = new QPushButton(QStringLiteral("Save"), imapForm);
    auto *imapCancelBtn = new QPushButton(QStringLiteral("Cancel"), imapForm);
    auto *imapBtnRow = new QHBoxLayout();
    imapBtnRow->addWidget(imapSaveBtn);
    imapBtnRow->addWidget(imapCancelBtn);
    imapFormLayout->addRow(imapBtnRow);
    accountFormStack->addWidget(imapForm);
    // Placeholder forms for disabled types (POP3, Nostr, Matrix, NNTP)
    for (int i = 0; i < 4; ++i) {
        auto *ph = new QLabel(QStringLiteral("Not yet implemented."), accountsEditPage);
        accountFormStack->addWidget(ph);
    }
    accountsEditLayout->addWidget(accountFormStack);
    accountsStack->addWidget(accountsEditPage);

    settingsStack->addWidget(accountsStack);
    // Other rubrics: placeholders
    const char *rubricPlaceholders[] = { "Junk Mail — to be configured", "Viewing — layout, fonts, read marking", "Composing — text/HTML, quoting", "Signatures" };
    for (int i = 0; i < 4; ++i) {
        auto *p = new QLabel(QString::fromUtf8(rubricPlaceholders[i]), settingsPage);
        p->setAlignment(Qt::AlignTop | Qt::AlignLeft);
        p->setMargin(24);
        settingsStack->addWidget(p);
    }
    QObject::connect(rubricList, &QListWidget::currentRowChanged, settingsStack, &QStackedWidget::setCurrentIndex);

    auto *settingsBackBtn = new QPushButton("← Back", settingsSidebar);
    settingsBackBtn->setFixedHeight(32);
    settingsSidebarLayout->addStretch();
    settingsSidebarLayout->addWidget(settingsBackBtn);

    settingsLayout->addWidget(settingsSidebar);
    settingsLayout->addWidget(settingsStack, 1);

    rightStack->addWidget(settingsPage);

    QObject::connect(settingsBtn, &QToolButton::clicked, [&]() { rightStack->setCurrentIndex(1); });
    QObject::connect(settingsBackBtn, &QPushButton::clicked, [&]() { rightStack->setCurrentIndex(0); });

    mainLayout->addWidget(rightStack, 1);

    // Startup: if no configured stores, open Settings so user can add accounts
    {
        Config startupConfig = loadConfig();
        if (startupConfig.stores.isEmpty()) {
            rightStack->setCurrentIndex(1);
            win.statusBar()->showMessage(QStringLiteral("Add a store in Accounts to get started."));
        }
    }

    EventBridge bridge;
    bridge.folderList = folderList;
    bridge.conversationList = conversationList;
    bridge.messageView = messageView;
    bridge.statusBar = win.statusBar();
    bridge.win = &win;

    QByteArray storeUri;
    QByteArray smtpTransportUri; // when set (e.g. IMAP account with SMTP), use for compose
    QList<QToolButton *> storeButtons; // circle buttons in sidebar; one per store

    auto addStoreCircle = [&](const QString &initial) {
        auto *btn = new QToolButton(storeListWidget);
        btn->setText(initial);
        btn->setFixedSize(40, 40);
        btn->setToolTip(initial);
        btn->setStyleSheet(
            "QToolButton { border-radius: 20px; background-color: palette(button); font-weight: bold; }"
            "QToolButton:hover { background-color: palette(light); }"
            "QToolButton:checked { background-color: palette(highlight); color: palette(highlighted-text); }"
        );
        btn->setCheckable(true);
        storeListLayout->addWidget(btn, 0, Qt::AlignHCenter);
        storeButtons.append(btn);
        if (storeButtons.size() == 1)
            btn->setChecked(true);
    };

    // Account type list -> edit panel: switch to edit and show form for that type
    const QVector<int> formIndexForType = { 2, 3, 0, 1, 4, 5, 6 }; // IMAP,POP3,Maildir,mbox,Nostr,Matrix,NNTP
    for (int i = 0; i < accountTypeButtons.size(); ++i) {
        QPushButton *b = accountTypeButtons[i].btn;
        int formIdx = formIndexForType[i];
        QObject::connect(b, &QPushButton::clicked, [accountsStack, accountFormStack, formIdx]() {
            accountFormStack->setCurrentIndex(formIdx);
            accountsStack->setCurrentIndex(1);
        });
    }
    QObject::connect(accountsEditBackBtn, &QPushButton::clicked, [accountsStack]() { accountsStack->setCurrentIndex(0); });

    QObject::connect(maildirBrowseBtn, &QPushButton::clicked, [&]() {
        QString path = QFileDialog::getExistingDirectory(&win, QStringLiteral("Select Maildir directory"), maildirPathEdit->text());
        if (!path.isEmpty()) maildirPathEdit->setText(path);
    });
    QObject::connect(mboxBrowseBtn, &QPushButton::clicked, [&]() {
        QString path = QFileDialog::getOpenFileName(&win, QStringLiteral("Select mbox file"), QString(), QStringLiteral("mbox (*)"));
        if (!path.isEmpty()) mboxPathEdit->setText(path);
    });

    QObject::connect(maildirCancelBtn, &QPushButton::clicked, [accountsStack]() { accountsStack->setCurrentIndex(0); });
    QObject::connect(imapCancelBtn, &QPushButton::clicked, [accountsStack]() { accountsStack->setCurrentIndex(0); });
    QObject::connect(mboxCancelBtn, &QPushButton::clicked, [accountsStack]() { accountsStack->setCurrentIndex(0); });

    QObject::connect(maildirSaveBtn, &QPushButton::clicked, [&]() {
        smtpTransportUri.clear();
        QString path = maildirPathEdit->text().trimmed();
        if (path.isEmpty()) {
            QMessageBox::warning(&win, QStringLiteral("Maildir"), QStringLiteral("Please select a directory."));
            return;
        }
        char *uriPtr = tagliacarte_store_maildir_new(path.toUtf8().constData());
        if (!uriPtr) {
            showError(&win, "Maildir");
            return;
        }
        storeUri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        bridge.clearFolder();
        for (auto *b : storeButtons) b->deleteLater();
        storeButtons.clear();
        folderList->clear();
        conversationList->clear();
        messageView->clear();

        tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
        tagliacarte_store_refresh_folders(storeUri.constData());
        QString displayName = maildirDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) displayName = QDir(path).dirName();
        if (displayName.isEmpty()) displayName = QStringLiteral("Maildir");
        QString initial = displayName.left(1).toUpper();
        if (initial.isEmpty()) initial = QStringLiteral("M");
        addStoreCircle(initial);
        composeBtn->setEnabled(true);
        win.statusBar()->showMessage(QStringLiteral("Added Maildir — loading folders…"));

        Config config = loadConfig();
        StoreEntry entry;
        entry.id = QString::fromUtf8(storeUri);
        entry.type = QStringLiteral("maildir");
        entry.displayName = displayName;
        entry.path = path;
        config.stores.append(entry);
        config.lastSelectedStoreId = entry.id;
        saveConfig(config);
    });

    QObject::connect(imapSaveBtn, &QPushButton::clicked, [&]() {
        QString displayName = imapDisplayNameEdit->text().trimmed();
        QString imapUser = imapUserEdit->text().trimmed();
        QString imapPass = imapPassEdit->text();
        QString imapHost = imapHostEdit->text().trimmed();
        int imapPort = imapPortSpin->value();
        bool imapSsl = imapSslCheck->isChecked();
        QString smtpUser = smtpUserEdit->text().trimmed();
        QString smtpPass = smtpPassEdit->text();
        QString smtpHost = smtpHostEdit->text().trimmed();
        int smtpPort = smtpPortSpin->value();
        if (imapHost.isEmpty()) {
            QMessageBox::warning(&win, QStringLiteral("IMAP"), QStringLiteral("Please enter IMAP host."));
            return;
        }
        QString userAtHost = imapUser;
        if (!imapUser.contains(QLatin1Char('@')) && !imapHost.isEmpty())
            userAtHost = imapUser + QLatin1Char('@') + imapHost;
        char *uriPtr = tagliacarte_store_imap_new(userAtHost.toUtf8().constData(), imapHost.toUtf8().constData(), static_cast<uint16_t>(imapPort));
        if (!uriPtr) {
            showError(&win, "IMAP");
            return;
        }
        storeUri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        bridge.clearFolder();
        for (auto *b : storeButtons) b->deleteLater();
        storeButtons.clear();
        folderList->clear();
        conversationList->clear();
        messageView->clear();

        tagliacarte_store_set_folder_list_callbacks(storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, &bridge);
        tagliacarte_store_refresh_folders(storeUri.constData());
        if (displayName.isEmpty()) displayName = imapUser;
        if (displayName.isEmpty()) displayName = QStringLiteral("IMAP");
        QString initial = displayName.left(1).toUpper();
        if (initial.isEmpty()) initial = QStringLiteral("I");
        addStoreCircle(initial);
        composeBtn->setEnabled(true);
        win.statusBar()->showMessage(QStringLiteral("Added IMAP account — loading folders…"));

        Config config = loadConfig();
        StoreEntry entry;
        entry.id = QString::fromUtf8(storeUri);
        entry.type = QStringLiteral("imap");
        entry.displayName = displayName;
        entry.path = imapHost;
        config.stores.append(entry);
        config.lastSelectedStoreId = entry.id;
        saveConfig(config);

        if (!smtpHost.isEmpty()) {
            char *transportUri = tagliacarte_transport_smtp_new(smtpHost.toUtf8().constData(), static_cast<uint16_t>(smtpPort));
            if (transportUri) {
                smtpTransportUri = QByteArray(transportUri);
                tagliacarte_free_string(transportUri);
            }
        }
    });

    QObject::connect(mboxSaveBtn, &QPushButton::clicked, [&]() {
        QMessageBox::information(&win, QStringLiteral("mbox"), QStringLiteral("mbox store support is not yet implemented in the backend."));
    });

    QObject::connect(&bridge, &EventBridge::folderReadyForMessages, [&](quint64 total) {
        QByteArray uri = bridge.folderUri();
        if (uri.isEmpty()) return;
        auto *item = folderList->currentItem();
        tagliacarte_folder_set_message_list_callbacks(uri.constData(), on_message_summary_cb, on_message_list_complete_cb, &bridge);
        tagliacarte_folder_request_message_list(uri.constData(), 0, total > 100 ? 100 : total);
        if (item) win.statusBar()->showMessage(QString("Folder: %1 — loading…").arg(item->text()));
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
        win.statusBar()->showMessage(QString("Opening… %1").arg(item->text()));
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
        messageView->setPlainText("Loading…");
    });

    QObject::connect(composeBtn, &QPushButton::clicked, [&]() {
        ComposeDialog dlg(&win);
        if (dlg.exec() != QDialog::Accepted) return;

        QString from = dlg.fromEdit->text().trimmed();
        QString to = dlg.toEdit->text().trimmed();
        QString subject = dlg.subjectEdit->text().trimmed();
        QString body = dlg.bodyEdit->toPlainText();
        QString smtpHost = dlg.smtpHostEdit->text().trimmed();
        bool ok = false;
        int smtpPort = dlg.smtpPortEdit->text().trimmed().toInt(&ok);
        if (!ok || smtpPort <= 0) smtpPort = 587;

        if (from.isEmpty() || to.isEmpty()) {
            QMessageBox::warning(&win, "Compose", "Please fill in From and To.");
            return;
        }
        if (smtpHost.isEmpty()) {
            QMessageBox::warning(&win, "Compose", "Please fill in SMTP host.");
            return;
        }

        QByteArray fromUtf8 = from.toUtf8();
        QByteArray toUtf8 = to.toUtf8();
        QByteArray subjUtf8 = subject.toUtf8();
        QByteArray bodyUtf8 = body.toUtf8();

        char *transport_uri = tagliacarte_transport_smtp_new(smtpHost.toUtf8().constData(), static_cast<uint16_t>(smtpPort));
        if (!transport_uri) {
            showError(&win, "SMTP transport");
            return;
        }
        bridge.setPendingSendTransport(transport_uri);
        win.statusBar()->showMessage(QStringLiteral("Sending…"));
        tagliacarte_transport_send_async(
            transport_uri,
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

    win.statusBar()->showMessage("Open a Maildir to start.");
    win.show();

    int ret = app.exec();

    bridge.clearFolder();
    if (!storeUri.isEmpty()) tagliacarte_store_free(storeUri.constData());
    if (!smtpTransportUri.isEmpty()) {
        tagliacarte_transport_free(smtpTransportUri.constData());
    }

    return ret;
}
