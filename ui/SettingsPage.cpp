/*
 * SettingsPage.cpp
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

#include "SettingsPage.h"
#include "MainController.h"
#include "Config.h"
#include "IconUtils.h"
#include "CidTextBrowser.h"
#include "Callbacks.h"
#include "Tr.h"
#include "tagliacarte.h"
#include "EventBridge.h"

#include <QWidget>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QFormLayout>
#include <QPushButton>
#include <QToolButton>
#include <QLineEdit>
#include <QTextEdit>
#include <QSpinBox>
#include <QCheckBox>
#include <QComboBox>
#include <QListWidget>
#include <QTabWidget>
#include <QStackedWidget>
#include <QLabel>
#include <QMessageBox>
#include <QFileDialog>
#include <QDir>
#include <QFile>
#include <QDateTime>
#include <QRegularExpression>
#include <QMainWindow>
#include <QDialog>
#include <QDesktopServices>
#include <QUrl>
#include <memory>

QWidget *buildSettingsPage(MainController *ctrl, QMainWindow *win, const char *version) {
    auto *settingsPage = new QWidget(nullptr);
    auto *settingsLayout = new QVBoxLayout(settingsPage);
    settingsLayout->setContentsMargins(0, 0, 0, 0);
    auto *settingsTabs = new QTabWidget(settingsPage);

    // Accounts: stacked = (0) type list → (1) edit account panel
    auto *accountsStack = new QStackedWidget(settingsPage);
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
        { "accounts.type.pop3", true },
        { "accounts.type.maildir", true },
        { "accounts.type.mbox", true },
        { "accounts.type.nostr", true },
        { "accounts.type.matrix", true },
        { "accounts.type.nntp", true },
        { "accounts.type.gmail", true },
        { "accounts.type.exchange", true }
    };
    const int accountButtonsPerRow = 6;
    const int typeButtonsPerRow = 4;
    auto *typeButtonsContainer = new QWidget(accountsListPage);
    auto *typeButtonsGrid = new QGridLayout(typeButtonsContainer);
    typeButtonsGrid->setAlignment(Qt::AlignCenter);
    int typeIdx = 0;
    for (const auto &p : accountTypes) {
        auto *b = new QPushButton(TR(p.key), typeButtonsContainer);
        b->setEnabled(p.enabled);
        b->setMinimumWidth(120);
        accountTypeButtons.append({ b, QString::fromUtf8(p.key) });
        typeButtonsGrid->addWidget(b, typeIdx / typeButtonsPerRow, typeIdx % typeButtonsPerRow, 1, 1, Qt::AlignCenter);
        ++typeIdx;
    }
    accountsListLayout->addWidget(typeButtonsContainer);
    accountsListLayout->addStretch();
    accountsStack->addWidget(accountsListPage);

    // Page 1: edit account panel
    auto *accountsEditPage = new QWidget(settingsPage);
    auto *accountsEditLayout = new QVBoxLayout(accountsEditPage);
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
    accountFormStack->addWidget(mboxForm);
    // IMAP form
    auto *imapForm = new QWidget(accountsEditPage);
    auto *imapFormLayout = new QFormLayout(imapForm);
    auto *imapDisplayNameEdit = new QLineEdit(imapForm);
    imapDisplayNameEdit->setPlaceholderText(TR("imap.placeholder.display_name"));
    imapFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), imapDisplayNameEdit);
    auto *imapEmailEdit = new QLineEdit(imapForm);
    imapEmailEdit->setPlaceholderText(TR("imap.placeholder.email"));
    imapFormLayout->addRow(TR("imap.email") + QStringLiteral(":"), imapEmailEdit);
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
    auto *smtpLabel = new QLabel(TR("imap.smtp_section"), imapForm);
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
    auto *imapSaveBtn = new QPushButton(TR("common.save"), imapForm);
    accountFormStack->addWidget(imapForm);

    QObject::connect(imapSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [imapPortSpin](int idx) {
        if (idx == 2) {
            imapPortSpin->setValue(993);
        } else {
            imapPortSpin->setValue(143);
        }
    });
    QObject::connect(smtpSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [smtpPortSpin](int idx) {
        if (idx == 2) {
            smtpPortSpin->setValue(465);
        } else {
            smtpPortSpin->setValue(586);
        }
    });
    // POP3 form
    auto *pop3Form = new QWidget(accountsEditPage);
    auto *pop3FormLayout = new QFormLayout(pop3Form);
    auto *pop3DisplayNameEdit = new QLineEdit(pop3Form);
    pop3DisplayNameEdit->setPlaceholderText(TR("imap.placeholder.display_name"));
    pop3FormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), pop3DisplayNameEdit);
    auto *pop3EmailEdit = new QLineEdit(pop3Form);
    pop3EmailEdit->setPlaceholderText(TR("imap.placeholder.email"));
    pop3FormLayout->addRow(TR("imap.email") + QStringLiteral(":"), pop3EmailEdit);
    auto *pop3HostEdit = new QLineEdit(pop3Form);
    pop3HostEdit->setPlaceholderText(TR("imap.placeholder.host"));
    pop3FormLayout->addRow(TR("imap.host") + QStringLiteral(":"), pop3HostEdit);
    auto *pop3SecurityCombo = new QComboBox(pop3Form);
    pop3SecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.ssl") });
    pop3SecurityCombo->setCurrentIndex(1);
    pop3FormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), pop3SecurityCombo);
    auto *pop3PortSpin = new QSpinBox(pop3Form);
    pop3PortSpin->setRange(1, 65535);
    pop3PortSpin->setValue(995);
    pop3FormLayout->addRow(TR("imap.port") + QStringLiteral(":"), pop3PortSpin);
    auto *pop3UserEdit = new QLineEdit(pop3Form);
    pop3FormLayout->addRow(TR("imap.username") + QStringLiteral(":"), pop3UserEdit);
    auto *pop3SmtpLabel = new QLabel(TR("imap.smtp_section"), pop3Form);
    pop3FormLayout->addRow(pop3SmtpLabel);
    auto *pop3SmtpHostEdit = new QLineEdit(pop3Form);
    pop3SmtpHostEdit->setPlaceholderText(TR("smtp.placeholder.host"));
    pop3FormLayout->addRow(TR("smtp.host") + QStringLiteral(":"), pop3SmtpHostEdit);
    auto *pop3SmtpSecurityCombo = new QComboBox(pop3Form);
    pop3SmtpSecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.starttls"), TR("imap.security.ssl") });
    pop3SmtpSecurityCombo->setCurrentIndex(1);
    pop3FormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), pop3SmtpSecurityCombo);
    auto *pop3SmtpPortSpin = new QSpinBox(pop3Form);
    pop3SmtpPortSpin->setRange(1, 65535);
    pop3SmtpPortSpin->setValue(586);
    pop3FormLayout->addRow(TR("smtp.port") + QStringLiteral(":"), pop3SmtpPortSpin);
    auto *pop3SmtpUserEdit = new QLineEdit(pop3Form);
    pop3FormLayout->addRow(TR("smtp.username") + QStringLiteral(":"), pop3SmtpUserEdit);
    auto *pop3SaveBtn = new QPushButton(TR("common.save"), pop3Form);
    QObject::connect(pop3SecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [pop3PortSpin](int idx) {
        if (idx == 1) {
            pop3PortSpin->setValue(995);
        } else {
            pop3PortSpin->setValue(110);
        }
    });
    QObject::connect(pop3SmtpSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [pop3SmtpPortSpin](int idx) {
        if (idx == 2) {
            pop3SmtpPortSpin->setValue(465);
        } else {
            pop3SmtpPortSpin->setValue(586);
        }
    });
    accountFormStack->addWidget(pop3Form);
    // Nostr form
    auto *nostrForm = new QWidget(accountsEditPage);
    auto *nostrMainLayout = new QVBoxLayout(nostrForm);
    nostrMainLayout->setContentsMargins(0, 0, 0, 0);
    auto *nostrFormLayout = new QFormLayout();
    nostrFormLayout->setFieldGrowthPolicy(QFormLayout::ExpandingFieldsGrow);
    auto *nostrSecretKeyEdit = new QLineEdit(nostrForm);
    nostrSecretKeyEdit->setEchoMode(QLineEdit::Password);
    nostrSecretKeyEdit->setPlaceholderText(TR("nostr.placeholder.secret_key"));
    nostrFormLayout->addRow(TR("nostr.secret_key") + QStringLiteral(":"), nostrSecretKeyEdit);
    auto *nostrPubkeyLabel = new QLineEdit(nostrForm);
    nostrPubkeyLabel->setReadOnly(true);
    nostrPubkeyLabel->setPlaceholderText(TR("nostr.placeholder.pubkey_derived"));
    nostrFormLayout->addRow(TR("nostr.pubkey") + QStringLiteral(":"), nostrPubkeyLabel);
    auto *nostrDisplayNameEdit = new QLineEdit(nostrForm);
    nostrDisplayNameEdit->setPlaceholderText(TR("nostr.placeholder.display_name"));
    nostrFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), nostrDisplayNameEdit);
    auto *nostrNip05Label = new QLineEdit(nostrForm);
    nostrNip05Label->setReadOnly(true);
    nostrNip05Label->setPlaceholderText(TR("nostr.placeholder.nip05_derived"));
    nostrFormLayout->addRow(TR("nostr.nip05") + QStringLiteral(":"), nostrNip05Label);
    auto *nostrMediaServerEdit = new QLineEdit(nostrForm);
    nostrMediaServerEdit->setPlaceholderText(TR("nostr.placeholder.media_server"));
    nostrFormLayout->addRow(TR("nostr.media_server") + QStringLiteral(":"), nostrMediaServerEdit);
    auto *nostrProfileStatus = new QLabel(nostrForm);
    nostrProfileStatus->setVisible(false);
    nostrFormLayout->addRow(nostrProfileStatus);
    nostrMainLayout->addLayout(nostrFormLayout);
    auto *nostrRelaysLabel = new QLabel(TR("nostr.relays") + QStringLiteral(":"), nostrForm);
    nostrMainLayout->addWidget(nostrRelaysLabel);
    auto *nostrRelayList = new QListWidget(nostrForm);
    nostrRelayList->setMinimumHeight(80);
    nostrMainLayout->addWidget(nostrRelayList);
    auto *nostrRelayAddRow = new QHBoxLayout();
    auto *nostrRelayUrlEdit = new QLineEdit(nostrForm);
    nostrRelayUrlEdit->setPlaceholderText(TR("nostr.placeholder.relay_url"));
    auto *nostrRelayAddBtn = new QPushButton(TR("nostr.add_relay"), nostrForm);
    nostrRelayAddRow->addWidget(nostrRelayUrlEdit);
    nostrRelayAddRow->addWidget(nostrRelayAddBtn);
    nostrMainLayout->addLayout(nostrRelayAddRow);
    auto *nostrRelayRemoveBtn = new QPushButton(TR("nostr.remove_relay"), nostrForm);
    nostrMainLayout->addWidget(nostrRelayRemoveBtn);
    auto *nostrSaveBtn = new QPushButton(TR("common.save"), accountsEditPage);
    nostrSaveBtn->setVisible(false);
    auto nostrDerivedPubkeyHex = std::make_shared<QString>();
    accountFormStack->addWidget(nostrForm);
    // Matrix form
    auto *matrixForm = new QWidget(accountsEditPage);
    auto *matrixFormLayout = new QFormLayout(matrixForm);
    auto *matrixHomeserverEdit = new QLineEdit(matrixForm);
    matrixHomeserverEdit->setPlaceholderText(TR("matrix.placeholder.homeserver"));
    matrixFormLayout->addRow(TR("matrix.homeserver") + QStringLiteral(":"), matrixHomeserverEdit);
    auto *matrixUserIdEdit = new QLineEdit(matrixForm);
    matrixUserIdEdit->setPlaceholderText(TR("matrix.placeholder.user_id"));
    matrixFormLayout->addRow(TR("matrix.user_id") + QStringLiteral(":"), matrixUserIdEdit);
    auto *matrixTokenEdit = new QLineEdit(matrixForm);
    matrixTokenEdit->setEchoMode(QLineEdit::Password);
    matrixTokenEdit->setPlaceholderText(TR("matrix.placeholder.token"));
    matrixFormLayout->addRow(TR("matrix.access_token") + QStringLiteral(":"), matrixTokenEdit);
    auto *matrixDisplayNameEdit = new QLineEdit(matrixForm);
    matrixDisplayNameEdit->setPlaceholderText(TR("matrix.placeholder.display_name"));
    matrixFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), matrixDisplayNameEdit);
    auto *matrixSaveBtn = new QPushButton(TR("common.save"), accountsEditPage);
    matrixSaveBtn->setVisible(false);
    accountFormStack->addWidget(matrixForm);
    // NNTP form (index 6)
    auto *nntpForm = new QWidget(accountsEditPage);
    auto *nntpFormLayout = new QFormLayout(nntpForm);
    auto *nntpDisplayNameEdit = new QLineEdit(nntpForm);
    nntpDisplayNameEdit->setPlaceholderText(TR("imap.placeholder.display_name"));
    nntpFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), nntpDisplayNameEdit);
    auto *nntpHostEdit = new QLineEdit(nntpForm);
    nntpHostEdit->setPlaceholderText(QStringLiteral("news.example.com"));
    nntpFormLayout->addRow(TR("imap.host") + QStringLiteral(":"), nntpHostEdit);
    auto *nntpSecurityCombo = new QComboBox(nntpForm);
    nntpSecurityCombo->addItems({ TR("imap.security.none"), TR("imap.security.starttls"), TR("imap.security.ssl") });
    nntpSecurityCombo->setCurrentIndex(2);
    nntpFormLayout->addRow(TR("imap.security.label") + QStringLiteral(":"), nntpSecurityCombo);
    auto *nntpPortSpin = new QSpinBox(nntpForm);
    nntpPortSpin->setRange(1, 65535);
    nntpPortSpin->setValue(563);
    nntpFormLayout->addRow(TR("imap.port") + QStringLiteral(":"), nntpPortSpin);
    auto *nntpUserEdit = new QLineEdit(nntpForm);
    nntpFormLayout->addRow(TR("imap.username") + QStringLiteral(":"), nntpUserEdit);
    auto *nntpSaveBtn = new QPushButton(TR("common.save"), accountsEditPage);
    nntpSaveBtn->setVisible(false);
    QObject::connect(nntpSecurityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [=](int idx) {
        if (idx == 0) nntpPortSpin->setValue(119);
        else if (idx == 1) nntpPortSpin->setValue(119);
        else nntpPortSpin->setValue(563);
    });
    accountFormStack->addWidget(nntpForm);
    // Gmail OAuth form (index 7)
    auto *gmailForm = new QWidget(accountsEditPage);
    auto *gmailFormLayout = new QFormLayout(gmailForm);
    auto *gmailInfoLabel = new QLabel(TR("gmail.info"), gmailForm);
    gmailInfoLabel->setWordWrap(true);
    gmailFormLayout->addRow(gmailInfoLabel);
    auto *gmailEmailEdit = new QLineEdit(gmailForm);
    gmailEmailEdit->setPlaceholderText(TR("gmail.placeholder.email"));
    gmailFormLayout->addRow(TR("gmail.email") + QStringLiteral(":"), gmailEmailEdit);
    auto *gmailDisplayNameEdit = new QLineEdit(gmailForm);
    gmailDisplayNameEdit->setPlaceholderText(TR("gmail.placeholder.display_name"));
    gmailFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), gmailDisplayNameEdit);
    auto *gmailSignInBtn = new QPushButton(TR("gmail.sign_in"), gmailForm);
    gmailFormLayout->addRow(gmailSignInBtn);
    auto *gmailStatusLabel = new QLabel(gmailForm);
    gmailStatusLabel->setVisible(false);
    gmailFormLayout->addRow(gmailStatusLabel);
    auto *gmailSaveBtn = new QPushButton(TR("common.save"), gmailForm);
    gmailSaveBtn->setVisible(false);
    accountFormStack->addWidget(gmailForm);
    // Exchange/Outlook OAuth form (index 8)
    auto *exchangeForm = new QWidget(accountsEditPage);
    auto *exchangeFormLayout = new QFormLayout(exchangeForm);
    auto *exchangeInfoLabel = new QLabel(TR("exchange.info"), exchangeForm);
    exchangeInfoLabel->setWordWrap(true);
    exchangeFormLayout->addRow(exchangeInfoLabel);
    auto *exchangeEmailEdit = new QLineEdit(exchangeForm);
    exchangeEmailEdit->setPlaceholderText(TR("exchange.placeholder.email"));
    exchangeFormLayout->addRow(TR("exchange.email") + QStringLiteral(":"), exchangeEmailEdit);
    auto *exchangeDisplayNameEdit = new QLineEdit(exchangeForm);
    exchangeDisplayNameEdit->setPlaceholderText(TR("exchange.placeholder.display_name"));
    exchangeFormLayout->addRow(TR("common.display_name") + QStringLiteral(":"), exchangeDisplayNameEdit);
    auto *exchangeSignInBtn = new QPushButton(TR("exchange.sign_in"), exchangeForm);
    exchangeFormLayout->addRow(exchangeSignInBtn);
    auto *exchangeStatusLabel = new QLabel(exchangeForm);
    exchangeStatusLabel->setVisible(false);
    exchangeFormLayout->addRow(exchangeStatusLabel);
    auto *exchangeSaveBtn = new QPushButton(TR("common.save"), exchangeForm);
    exchangeSaveBtn->setVisible(false);
    accountFormStack->addWidget(exchangeForm);
    accountsEditLayout->addWidget(accountFormStack);
    auto *accountEditButtonRow = new QWidget(accountsEditPage);
    auto *accountEditButtonLayout = new QHBoxLayout(accountEditButtonRow);
    accountEditButtonLayout->setContentsMargins(0, 12, 0, 0);
    auto *accountDeleteBtn = new QPushButton(TR("accounts.delete"), accountEditButtonRow);
    accountDeleteBtn->setVisible(false);
    auto *accountSaveBtn = new QPushButton(TR("common.save"), accountEditButtonRow);
    auto *accountCancelBtn = new QPushButton(TR("common.cancel"), accountEditButtonRow);
    accountEditButtonLayout->addWidget(accountDeleteBtn, 0, Qt::AlignLeft);
    accountEditButtonLayout->addStretch(1);
    accountEditButtonLayout->addWidget(accountSaveBtn);
    accountEditButtonLayout->addWidget(accountCancelBtn);
    accountsEditLayout->addWidget(accountEditButtonRow);
    accountsStack->addWidget(accountsEditPage);

    auto refreshAccountListInSettings = [=]() {
        while (QLayoutItem *item = accountButtonsGrid->takeAt(0)) {
            if (item->widget()) {
                item->widget()->deleteLater();
            }
            delete item;
        }
        Config c = loadConfig();
        const int cols = accountButtonsPerRow;
        for (int i = 0; i < c.stores.size(); ++i) {
            const StoreEntry &entry = c.stores[i];
            QString initial = entry.displayName.left(1).toUpper();
            if (initial.isEmpty()) {
                if (entry.type == QLatin1String("maildir")) {
                    initial = QStringLiteral("M");
                } else if (entry.type == QLatin1String("imap")) {
                    initial = QStringLiteral("I");
                } else if (entry.type == QLatin1String("nostr")) {
                    initial = QStringLiteral("N");
                } else if (entry.type == QLatin1String("matrix")) {
                    initial = QStringLiteral("X");
                } else if (entry.type == QLatin1String("nntp")) {
                    initial = QStringLiteral("U");
                } else if (entry.type == QLatin1String("gmail")) {
                    initial = QStringLiteral("G");
                } else if (entry.type == QLatin1String("exchange")) {
                    initial = QStringLiteral("E");
                } else {
                    initial = QStringLiteral("?");
                }
            }
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
            QObject::connect(btn, &QToolButton::clicked, [=]() {
                ctrl->editingStoreId = entryCopy.id;
                accountDeleteBtn->setVisible(true);
                if (entryCopy.type == QLatin1String("maildir")) {
                    accountFormStack->setCurrentIndex(0);
                    maildirPathEdit->setText(storeHostOrPath(entryCopy));
                    maildirDisplayNameEdit->setText(entryCopy.displayName);
                } else if (entryCopy.type == QLatin1String("imap")) {
                    accountFormStack->setCurrentIndex(2);
                    imapDisplayNameEdit->setText(entryCopy.displayName);
                    imapEmailEdit->setText(entryCopy.emailAddress);
                    imapHostEdit->setText(storeHostOrPath(entryCopy));
                    imapSecurityCombo->setCurrentIndex(qBound(0, paramInt(entryCopy, "security", 2), 2));
                    imapPortSpin->setValue(paramInt(entryCopy, "port", 993));
                    imapUserEdit->setText(param(entryCopy, "username"));
                    int pollSec = paramInt(entryCopy, "imapPollSeconds", 300);
                    int pollIdx = (pollSec <= 60) ? 0 : (pollSec <= 300) ? 1 : (pollSec <= 600) ? 2 : 3;
                    imapPollCombo->setCurrentIndex(pollIdx);
                    imapDeletionCombo->setCurrentIndex(paramInt(entryCopy, "imapDeletion", 0) == 1 ? 1 : 0);
                    imapTrashFolderEdit->setText(param(entryCopy, "imapTrashFolder"));
                    int idleSec = paramInt(entryCopy, "imapIdleSeconds", 60);
                    int idleIdx = (idleSec <= 30) ? 0 : (idleSec <= 60) ? 1 : 2;
                    imapIdleCombo->setCurrentIndex(idleIdx);
                    smtpHostEdit->setText(param(entryCopy, "transportHostname"));
                    smtpSecurityCombo->setCurrentIndex(qBound(0, paramInt(entryCopy, "transportSecurity", 1), 2));
                    smtpPortSpin->setValue(paramInt(entryCopy, "transportPort", 586));
                    smtpUserEdit->setText(param(entryCopy, "transportUsername"));
                } else if (entryCopy.type == QLatin1String("pop3")) {
                    accountFormStack->setCurrentIndex(3);
                    pop3DisplayNameEdit->setText(entryCopy.displayName);
                    pop3EmailEdit->setText(entryCopy.emailAddress);
                    pop3HostEdit->setText(storeHostOrPath(entryCopy));
                    pop3PortSpin->setValue(995);
                    pop3UserEdit->setText(param(entryCopy, "username"));
                    pop3SmtpHostEdit->setText(param(entryCopy, "transportHostname"));
                    pop3SmtpPortSpin->setValue(paramInt(entryCopy, "transportPort", 586));
                    pop3SmtpUserEdit->setText(param(entryCopy, "transportUsername"));
                } else if (entryCopy.type == QLatin1String("nostr")) {
                    accountFormStack->setCurrentIndex(4);
                    nostrDisplayNameEdit->setText(entryCopy.displayName);
                    nostrNip05Label->setText(entryCopy.emailAddress);
                    nostrPubkeyLabel->setText(param(entryCopy, "pubkey"));
                    *nostrDerivedPubkeyHex = param(entryCopy, "pubkey");
                    nostrSecretKeyEdit->clear();
                    QString mediaServer = param(entryCopy, "mediaServer");
                    nostrMediaServerEdit->setText(mediaServer.isEmpty()
                        ? QStringLiteral("https://blossom.primal.net") : mediaServer);
                    nostrProfileStatus->setVisible(false);
                    nostrRelayList->clear();
                    const QString pathVal = storeHostOrPath(entryCopy);
                    const QStringList relayUrls = pathVal.split(QRegularExpression(QStringLiteral("[,\\n]")), Qt::SkipEmptyParts);
                    for (const QString &u : relayUrls) {
                        QString t = u.trimmed();
                        if (!t.isEmpty()) {
                            nostrRelayList->addItem(t);
                        }
                    }
                } else if (entryCopy.type == QLatin1String("matrix")) {
                    accountFormStack->setCurrentIndex(5);
                    matrixHomeserverEdit->setText(storeHostOrPath(entryCopy));
                    matrixUserIdEdit->setText(param(entryCopy, "userId"));
                    matrixTokenEdit->setText(param(entryCopy, "accessToken"));
                    matrixDisplayNameEdit->setText(entryCopy.displayName);
                } else if (entryCopy.type == QLatin1String("nntp")) {
                    accountFormStack->setCurrentIndex(6);
                    nntpDisplayNameEdit->setText(entryCopy.displayName);
                    nntpHostEdit->setText(storeHostOrPath(entryCopy));
                    nntpSecurityCombo->setCurrentIndex(qBound(0, paramInt(entryCopy, "security", 2), 2));
                    nntpPortSpin->setValue(paramInt(entryCopy, "port", 563));
                    nntpUserEdit->setText(param(entryCopy, "username"));
                } else if (entryCopy.type == QLatin1String("gmail")) {
                    accountFormStack->setCurrentIndex(7);
                    gmailEmailEdit->setText(entryCopy.emailAddress);
                    gmailDisplayNameEdit->setText(entryCopy.displayName);
                } else if (entryCopy.type == QLatin1String("exchange")) {
                    accountFormStack->setCurrentIndex(8);
                    exchangeEmailEdit->setText(entryCopy.emailAddress);
                    exchangeDisplayNameEdit->setText(entryCopy.displayName);
                }
                accountsStack->setCurrentIndex(1);
            });
        }
    };

    settingsTabs->addTab(accountsStack, TR("settings.rubric.accounts"));

    // Security tab
    auto *securityPage = new QWidget(settingsPage);
    auto *securityLayout = new QVBoxLayout(securityPage);
    securityLayout->setSpacing(12);
    securityLayout->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    securityLayout->setContentsMargins(24, 24, 24, 24);
    auto *useKeychainCheck = new QCheckBox(TR("security.use_keychain"), securityPage);
    useKeychainCheck->setChecked(loadConfig().useKeychain);
    useKeychainCheck->setEnabled(tagliacarte_keychain_available() != 0);
    securityLayout->addWidget(useKeychainCheck);
    securityLayout->addStretch();
    settingsTabs->addTab(securityPage, TR("settings.rubric.security"));

    const char *placeholderKeys[] = { "settings.placeholder.junk_mail", "settings.placeholder.signatures" };
    const char *tabKeys[] = { "settings.rubric.junk_mail", "settings.rubric.viewing", "settings.rubric.composing", "settings.rubric.signatures" };
    // Viewing tab
    auto *viewingPage = new QWidget(settingsPage);
    auto *viewingLayout = new QFormLayout(viewingPage);
    viewingLayout->setContentsMargins(24, 24, 24, 24);
    auto *dateFormatCombo = new QComboBox(viewingPage);
    const struct { const char *trKey; QString format; } dateFormatOptions[] = {
        { "viewing.date_format.locale_default", QString() },
        { "viewing.date_format.d_mmm_yyyy_hh_mm", QStringLiteral("d MMM yyyy HH:mm") },
        { "viewing.date_format.dd_mm_yy", QStringLiteral("dd/MM/yy") },
        { "viewing.date_format.iso", QStringLiteral("yyyy-MM-dd HH:mm") },
    };
    for (const auto &opt : dateFormatOptions) {
        dateFormatCombo->addItem(TR(opt.trKey), opt.format);
    }
    int dateFormatIdx = 0;
    QString savedDateFormat = loadConfig().dateFormat;
    for (int i = 0; i < dateFormatCombo->count(); ++i) {
        if (dateFormatCombo->itemData(i).toString() == savedDateFormat) {
            dateFormatIdx = i;
            break;
        }
    }
    dateFormatCombo->setCurrentIndex(dateFormatIdx);
    viewingLayout->addRow(TR("viewing.date_format") + QStringLiteral(":"), dateFormatCombo);
    auto *resourceLoadCombo = new QComboBox(viewingPage);
    resourceLoadCombo->addItem(TR("viewing.resource_load.none"), 0);
    resourceLoadCombo->addItem(TR("viewing.resource_load.cid_only"), 1);
    resourceLoadCombo->addItem(TR("viewing.resource_load.external"), 2);
    int savedResourcePolicy = loadConfig().resourceLoadPolicy;
    for (int i = 0; i < resourceLoadCombo->count(); ++i) {
        if (resourceLoadCombo->itemData(i).toInt() == savedResourcePolicy) {
            resourceLoadCombo->setCurrentIndex(i);
            break;
        }
    }
    viewingLayout->addRow(TR("viewing.resource_load.label") + QStringLiteral(":"), resourceLoadCombo);
    settingsTabs->addTab(viewingPage, TR("settings.rubric.viewing"));
    QObject::connect(dateFormatCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [dateFormatCombo](int) {
        Config c = loadConfig();
        c.dateFormat = dateFormatCombo->currentData().toString();
        saveConfig(c);
    });
    QObject::connect(resourceLoadCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), [resourceLoadCombo, messageView = ctrl->messageView](int) {
        int policy = resourceLoadCombo->currentData().toInt();
        Config c = loadConfig();
        c.resourceLoadPolicy = policy;
        saveConfig(c);
        messageView->setResourceLoadPolicy(policy);
    });

    // Composing tab
    auto *composingPage = new QWidget(settingsPage);
    auto *composingLayout = new QFormLayout(composingPage);
    composingLayout->setContentsMargins(24, 24, 24, 24);
    auto *forwardModeCombo = new QComboBox(composingPage);
    forwardModeCombo->addItem(TR("composing.forward.inline"), QStringLiteral("inline"));
    forwardModeCombo->addItem(TR("composing.forward.embedded"), QStringLiteral("embedded"));
    forwardModeCombo->addItem(TR("composing.forward.attachment"), QStringLiteral("attachment"));
    QString savedForwardMode = loadConfig().forwardMode;
    int forwardModeIdx = 0;
    for (int i = 0; i < forwardModeCombo->count(); ++i) {
        if (forwardModeCombo->itemData(i).toString() == savedForwardMode) {
            forwardModeIdx = i;
            break;
        }
    }
    forwardModeCombo->setCurrentIndex(forwardModeIdx);
    composingLayout->addRow(TR("composing.forward.label") + QStringLiteral(":"), forwardModeCombo);
    auto *quoteUsePrefixCheck = new QCheckBox(composingPage);
    quoteUsePrefixCheck->setChecked(loadConfig().quoteUsePrefix);
    composingLayout->addRow(TR("composing.quote_use_prefix") + QStringLiteral(":"), quoteUsePrefixCheck);
    auto *quotePrefixEdit = new QLineEdit(composingPage);
    quotePrefixEdit->setPlaceholderText(TR("composing.quote_prefix.placeholder"));
    quotePrefixEdit->setText(loadConfig().quotePrefix);
    composingLayout->addRow(TR("composing.quote_prefix") + QStringLiteral(":"), quotePrefixEdit);
    auto *replyPositionCombo = new QComboBox(composingPage);
    replyPositionCombo->addItem(TR("composing.reply_position.before"), QStringLiteral("before"));
    replyPositionCombo->addItem(TR("composing.reply_position.after"), QStringLiteral("after"));
    QString savedReplyPosition = loadConfig().replyPosition;
    int replyPositionIdx = 0;
    for (int i = 0; i < replyPositionCombo->count(); ++i) {
        if (replyPositionCombo->itemData(i).toString() == savedReplyPosition) {
            replyPositionIdx = i;
            break;
        }
    }
    replyPositionCombo->setCurrentIndex(replyPositionIdx);
    composingLayout->addRow(TR("composing.reply_position.label") + QStringLiteral(":"), replyPositionCombo);
    auto *composingSaveBtn = new QPushButton(TR("common.save"), composingPage);
    composingLayout->addRow(composingSaveBtn);
    QObject::connect(composingSaveBtn, &QPushButton::clicked, [=]() {
        Config c = loadConfig();
        c.forwardMode = forwardModeCombo->currentData().toString();
        c.quoteUsePrefix = quoteUsePrefixCheck->isChecked();
        c.quotePrefix = quotePrefixEdit->text().trimmed();
        if (c.quotePrefix.isEmpty()) {
            c.quotePrefix = QStringLiteral("> ");
        }
        c.replyPosition = replyPositionCombo->currentData().toString();
        saveConfig(c);
        QMessageBox::information(composingPage, TR("settings.rubric.composing"), TR("composing.saved"));
    });
    settingsTabs->addTab(composingPage, TR("settings.rubric.composing"));

    auto *junkPlaceholder = new QLabel(TR(placeholderKeys[0]), settingsPage);
    junkPlaceholder->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    junkPlaceholder->setContentsMargins(24, 24, 24, 24);
    settingsTabs->addTab(junkPlaceholder, TR(tabKeys[0]));
    auto *signaturesPlaceholder = new QLabel(TR(placeholderKeys[1]), settingsPage);
    signaturesPlaceholder->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    signaturesPlaceholder->setContentsMargins(24, 24, 24, 24);
    settingsTabs->addTab(signaturesPlaceholder, TR(tabKeys[3]));
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
    aboutLayout->addWidget(aboutNameLabel, 0, Qt::AlignHCenter);
    auto *aboutVersionLabel = new QLabel(TR("about.version").arg(QString::fromUtf8(version)), aboutPage);
    aboutLayout->addWidget(aboutVersionLabel, 0, Qt::AlignHCenter);
    auto *aboutCopyrightLabel = new QLabel(TR("about.copyright"), aboutPage);
    aboutLayout->addWidget(aboutCopyrightLabel, 0, Qt::AlignHCenter);
    auto *aboutLicenceLabel = new QLabel(TR("about.licence"), aboutPage);
    aboutLicenceLabel->setWordWrap(true);
    aboutLayout->addWidget(aboutLicenceLabel, 0, Qt::AlignHCenter);
    aboutLayout->addStretch();
    settingsTabs->addTab(aboutPage, TR("settings.rubric.about"));

    QObject::connect(settingsTabs, &QTabWidget::currentChanged, [=](int index) {
        if (index == 0) {
            refreshAccountListInSettings();
        } else if (index == 1) {
            useKeychainCheck->setChecked(loadConfig().useKeychain);
        } else if (index == 2) {
            Config vc = loadConfig();
            for (int i = 0; i < dateFormatCombo->count(); ++i) {
                if (dateFormatCombo->itemData(i).toString() == vc.dateFormat) {
                    dateFormatCombo->setCurrentIndex(i);
                    break;
                }
            }
            for (int i = 0; i < resourceLoadCombo->count(); ++i) {
                if (resourceLoadCombo->itemData(i).toInt() == vc.resourceLoadPolicy) {
                    resourceLoadCombo->setCurrentIndex(i);
                    break;
                }
            }
        } else if (index == 3) {
            Config c = loadConfig();
            for (int i = 0; i < forwardModeCombo->count(); ++i) {
                if (forwardModeCombo->itemData(i).toString() == c.forwardMode) {
                    forwardModeCombo->setCurrentIndex(i);
                    break;
                }
            }
            quoteUsePrefixCheck->setChecked(c.quoteUsePrefix);
            quotePrefixEdit->setText(c.quotePrefix);
            for (int i = 0; i < replyPositionCombo->count(); ++i) {
                if (replyPositionCombo->itemData(i).toString() == c.replyPosition) {
                    replyPositionCombo->setCurrentIndex(i);
                    break;
                }
            }
        }
    });

    QObject::connect(useKeychainCheck, &QCheckBox::toggled, [ctrl, useKeychainCheck](bool useKeychain) {
        Config config = loadConfig();
        QString credPath = tagliacarteConfigDir() + QStringLiteral("/credentials");
        QByteArray pathBa = credPath.toUtf8();
        const char *path = pathBa.constData();
        if (useKeychain) {
            if (tagliacarte_migrate_credentials_to_keychain(path) != 0) {
                useKeychainCheck->setChecked(false);
                return;
            }
            config.useKeychain = true;
        } else {
            QList<QByteArray> uris;
            for (const QByteArray &u : ctrl->allStoreUris) {
                uris.append(u);
            }
            for (const QByteArray &t : ctrl->storeToTransport) {
                if (!uris.contains(t)) {
                    uris.append(t);
                }
            }
            QVector<const char *> ptrs(uris.size());
            for (int i = 0; i < uris.size(); ++i) {
                ptrs[i] = uris.at(i).constData();
            }
            if (tagliacarte_migrate_credentials_to_file(path, ptrs.size(), ptrs.isEmpty() ? nullptr : ptrs.data()) != 0) {
                useKeychainCheck->setChecked(true);
                return;
            }
            config.useKeychain = false;
        }
        saveConfig(config);
        tagliacarte_set_credentials_backend(config.useKeychain ? 1 : 0);
    });

    // Account type buttons and save/delete handlers
    const QVector<int> formIndexForType = { 2, 3, 0, 1, 4, 5, 6, 7, 8 };
    for (int i = 0; i < accountTypeButtons.size(); ++i) {
        QPushButton *b = accountTypeButtons[i].btn;
        int formIdx = formIndexForType[i];
        QObject::connect(b, &QPushButton::clicked, [=]() {
            ctrl->editingStoreId.clear();
            accountDeleteBtn->setVisible(false);
            accountFormStack->setCurrentIndex(formIdx);
            accountsStack->setCurrentIndex(1);
        });
    }
    QObject::connect(accountCancelBtn, &QPushButton::clicked, [=]() {
        accountsStack->setCurrentIndex(0);
        refreshAccountListInSettings();
    });
    QObject::connect(accountSaveBtn, &QPushButton::clicked, [=]() {
        const int idx = accountFormStack->currentIndex();
        if (idx == 0) {
            maildirSaveBtn->click();
        } else if (idx == 1) {
            mboxSaveBtn->click();
        } else if (idx == 2) {
            imapSaveBtn->click();
        } else if (idx == 3) {
            pop3SaveBtn->click();
        } else if (idx == 4) {
            nostrSaveBtn->click();
        } else if (idx == 5) {
            matrixSaveBtn->click();
        } else if (idx == 6) {
            nntpSaveBtn->click();
        } else if (idx == 7) {
            gmailSaveBtn->click();
        } else if (idx == 8) {
            exchangeSaveBtn->click();
        }
    });
    QObject::connect(accountDeleteBtn, &QPushButton::clicked, [=]() {
        if (ctrl->editingStoreId.isEmpty()) {
            return;
        }
        Config currentCfg = loadConfig();
        QString acctDesc;
        for (const StoreEntry &e : currentCfg.stores) {
            if (e.id == ctrl->editingStoreId) {
                acctDesc = e.type + QStringLiteral(": ") + e.emailAddress;
                break;
            }
        }
        QMessageBox mb(win);
        mb.setWindowTitle(TR("accounts.delete_confirm_title"));
        mb.setText(TR("accounts.delete_confirm_text").arg(acctDesc));
        QPushButton *cancelBtn = mb.addButton(TR("common.cancel"), QMessageBox::RejectRole);
        QPushButton *deleteConfirmBtn = mb.addButton(TR("accounts.delete"), QMessageBox::DestructiveRole);
        mb.setDefaultButton(cancelBtn);
        mb.exec();
        if (mb.clickedButton() != deleteConfirmBtn) {
            return;
        }
        QByteArray idToRemove = ctrl->editingStoreId.toUtf8();
        Config config = loadConfig();
        config.stores.removeIf([&idToRemove](const StoreEntry &e) {
            return e.id.toUtf8() == idToRemove;
        });
        if (config.lastSelectedStoreId == ctrl->editingStoreId) {
            config.lastSelectedStoreId = config.stores.isEmpty() ? QString() : config.stores.first().id;
        }
        saveConfig(config);
        QByteArray transportUri = ctrl->storeToTransport.value(idToRemove);
        if (!transportUri.isEmpty()) {
            tagliacarte_transport_free(transportUri.constData());
            ctrl->storeToTransport.remove(idToRemove);
        }
        ctrl->allStoreUris.removeAll(idToRemove);
        tagliacarte_store_free(idToRemove.constData());
        for (int i = 0; i < ctrl->storeButtons.size(); ++i) {
            if (ctrl->storeButtons[i]->property("storeUri").toByteArray() == idToRemove) {
                ctrl->storeButtons[i]->deleteLater();
                ctrl->storeButtons.removeAt(i);
                break;
            }
        }
        if (ctrl->storeUri == idToRemove) {
            ctrl->storeUri.clear();
            ctrl->smtpTransportUri.clear();
            ctrl->bridge->clearFolder();
            ctrl->folderTree->clear();
            ctrl->conversationList->clear();
            ctrl->messageView->clear();
            ctrl->messageHeaderPane->hide();
            if (!ctrl->allStoreUris.isEmpty()) {
                ctrl->storeUri = ctrl->allStoreUris.first();
                ctrl->smtpTransportUri = ctrl->storeToTransport.value(ctrl->storeUri);
                for (int i = 0; i < ctrl->storeButtons.size(); ++i) {
                    ctrl->storeButtons[i]->setChecked(ctrl->storeButtons[i]->property("storeUri").toByteArray() == ctrl->storeUri);
                }
                tagliacarte_store_set_folder_list_callbacks(ctrl->storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, ctrl->bridge);
                tagliacarte_store_refresh_folders(ctrl->storeUri.constData());
            }
            ctrl->updateComposeAppendButtons();
        }
        ctrl->editingStoreId.clear();
        accountDeleteBtn->setVisible(false);
        accountsStack->setCurrentIndex(0);
        refreshAccountListInSettings();
        win->statusBar()->showMessage(TR("status.account_deleted"));
    });

    QObject::connect(maildirBrowseBtn, &QPushButton::clicked, [=]() {
        QString path = QFileDialog::getExistingDirectory(win, TR("dialog.select_maildir_directory"), maildirPathEdit->text());
        if (!path.isEmpty()) {
            maildirPathEdit->setText(path);
        }
    });
    QObject::connect(mboxBrowseBtn, &QPushButton::clicked, [=]() {
        QString path = QFileDialog::getOpenFileName(win, TR("dialog.select_mbox_file"), QString(), QStringLiteral("mbox (*)"));
        if (!path.isEmpty()) {
            mboxPathEdit->setText(path);
        }
    });

    QObject::connect(maildirSaveBtn, &QPushButton::clicked, [=]() {
        QString path = maildirPathEdit->text().trimmed();
        if (path.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.maildir"), TR("maildir.validation.select_directory"));
            return;
        }
        char *uriPtr = tagliacarte_store_maildir_new(path.toUtf8().constData());
        if (!uriPtr) {
            showError(win, "error.context.maildir");
            return;
        }
        QByteArray newUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        tagliacarte_store_free(newUri.constData());
        QString displayName = maildirDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = QDir(path).dirName();
        }
        if (displayName.isEmpty()) {
            displayName = TR("maildir.default_display_name");
        }
        Config config = loadConfig();
        QString oldId = ctrl->editingStoreId;
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) {
                    idx = i;
                    break;
                }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newUri);
                e.type = QStringLiteral("maildir");
                e.displayName = displayName;
                e.params[QStringLiteral("path")] = path;
                if (config.lastSelectedStoreId == oldId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newUri);
            entry.type = QStringLiteral("maildir");
            entry.displayName = displayName;
            entry.params[QStringLiteral("path")] = path;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        ctrl->refreshStoresFromConfig();
        ctrl->editingStoreId.clear();
        win->statusBar()->showMessage(TR("status.added_maildir"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    QObject::connect(imapSaveBtn, &QPushButton::clicked, [=]() {
        QString displayName = imapDisplayNameEdit->text().trimmed();
        QString imapUser = imapUserEdit->text().trimmed();
        QString imapHost = imapHostEdit->text().trimmed();
        int imapPort = imapPortSpin->value();
        QString smtpHost = smtpHostEdit->text().trimmed();
        int smtpPort = smtpPortSpin->value();
        if (imapHost.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.imap"), TR("imap.validation.enter_host"));
            return;
        }
        Config config = loadConfig();
        const bool isEdit = !ctrl->editingStoreId.isEmpty();
        const QByteArray previousStoreUri = ctrl->storeUri;
        int colourIndex = 0;
        if (isEdit) {
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == ctrl->editingStoreId) {
                    colourIndex = i;
                    break;
                }
            }
        } else {
            colourIndex = config.stores.size();
        }
        QString userAtHost = imapUser;
        if (!imapUser.contains(QLatin1Char('@')) && !imapHost.isEmpty()) {
            userAtHost = imapUser + QLatin1Char('@') + imapHost;
        }
        char *uriPtr = tagliacarte_store_imap_new(userAtHost.toUtf8().constData(), imapHost.toUtf8().constData(), static_cast<uint16_t>(imapPort));
        if (!uriPtr) {
            showError(win, "error.context.imap");
            return;
        }
        ctrl->storeUri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (displayName.isEmpty()) {
            displayName = imapUser;
        }
        if (displayName.isEmpty()) {
            displayName = TR("accounts.type.imap");
        }
        if (isEdit) {
            QByteArray oldUri = ctrl->editingStoreId.toUtf8();
            tagliacarte_store_free(oldUri.constData());
            ctrl->storeToTransport.remove(oldUri);
            ctrl->allStoreUris.removeAll(oldUri);
            for (int i = 0; i < ctrl->storeButtons.size(); ++i) {
                if (ctrl->storeButtons[i]->property("storeUri").toByteArray() == oldUri) {
                    ctrl->storeButtons[i]->deleteLater();
                    ctrl->storeButtons.removeAt(i);
                    break;
                }
            }
            if (previousStoreUri == oldUri) {
                ctrl->bridge->clearFolder();
                ctrl->folderTree->clear();
                ctrl->conversationList->clear();
                ctrl->messageView->clear();
                ctrl->messageHeaderPane->hide();
            }
        } else {
            ctrl->bridge->clearFolder();
            ctrl->folderTree->clear();
            ctrl->conversationList->clear();
            ctrl->messageView->clear();
            ctrl->messageHeaderPane->hide();
        }
        ctrl->allStoreUris.append(ctrl->storeUri);
        tagliacarte_store_set_folder_list_callbacks(ctrl->storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, ctrl->bridge);
        tagliacarte_store_refresh_folders(ctrl->storeUri.constData());
        QString initial = displayName.left(1).toUpper();
        if (initial.isEmpty()) {
            initial = QStringLiteral("I");
        }
        ctrl->addStoreCircle(initial, ctrl->storeUri, colourIndex);
        for (auto *b : ctrl->storeButtons) {
            b->setChecked(b->property("storeUri").toByteArray() == ctrl->storeUri);
        }
        ctrl->smtpTransportUri.clear();
        if (!smtpHost.isEmpty()) {
            char *transportUri = tagliacarte_transport_smtp_new(smtpHost.toUtf8().constData(), static_cast<uint16_t>(smtpPort));
            if (transportUri) {
                ctrl->smtpTransportUri = QByteArray(transportUri);
                tagliacarte_free_string(transportUri);
                ctrl->storeToTransport[ctrl->storeUri] = ctrl->smtpTransportUri;
            }
        }
        ctrl->updateComposeAppendButtons();
        if (isEdit) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == ctrl->editingStoreId) {
                    idx = i;
                    break;
                }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(ctrl->storeUri);
                e.type = QStringLiteral("imap");
                e.displayName = displayName;
                e.emailAddress = imapEmailEdit->text().trimmed();
                e.params[QStringLiteral("hostname")] = imapHost;
                e.params[QStringLiteral("username")] = userAtHost;
                e.params[QStringLiteral("port")] = QString::number(imapPortSpin->value());
                e.params[QStringLiteral("security")] = QString::number(imapSecurityCombo->currentIndex());
                const int pollSeconds[] = { 60, 300, 600, 3600 };
                e.params[QStringLiteral("imapPollSeconds")] = QString::number((imapPollCombo->currentIndex() >= 0 && imapPollCombo->currentIndex() < 4) ? pollSeconds[imapPollCombo->currentIndex()] : 300);
                e.params[QStringLiteral("imapDeletion")] = QString::number(imapDeletionCombo->currentIndex());
                e.params[QStringLiteral("imapTrashFolder")] = imapTrashFolderEdit->text().trimmed();
                const int idleSeconds[] = { 30, 60, 300 };
                e.params[QStringLiteral("imapIdleSeconds")] = QString::number((imapIdleCombo->currentIndex() >= 0 && imapIdleCombo->currentIndex() < 3) ? idleSeconds[imapIdleCombo->currentIndex()] : 60);
                if (!smtpHost.isEmpty()) {
                    e.params[QStringLiteral("transportId")] = QString::fromUtf8(ctrl->smtpTransportUri);
                    e.params[QStringLiteral("transportHostname")] = smtpHost;
                    e.params[QStringLiteral("transportPort")] = QString::number(smtpPortSpin->value());
                    e.params[QStringLiteral("transportSecurity")] = QString::number(smtpSecurityCombo->currentIndex());
                    e.params[QStringLiteral("transportUsername")] = smtpUserEdit->text().trimmed();
                } else {
                    e.params.remove(QStringLiteral("transportId"));
                    e.params.remove(QStringLiteral("transportHostname"));
                    e.params.remove(QStringLiteral("transportPort"));
                    e.params.remove(QStringLiteral("transportSecurity"));
                    e.params.remove(QStringLiteral("transportUsername"));
                }
                if (config.lastSelectedStoreId == ctrl->editingStoreId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(ctrl->storeUri);
            entry.type = QStringLiteral("imap");
            entry.displayName = displayName;
            entry.emailAddress = imapEmailEdit->text().trimmed();
            entry.params[QStringLiteral("hostname")] = imapHost;
            entry.params[QStringLiteral("username")] = userAtHost;
            entry.params[QStringLiteral("port")] = QString::number(imapPortSpin->value());
            entry.params[QStringLiteral("security")] = QString::number(imapSecurityCombo->currentIndex());
            const int pollSeconds[] = { 60, 300, 600, 3600 };
            entry.params[QStringLiteral("imapPollSeconds")] = QString::number((imapPollCombo->currentIndex() >= 0 && imapPollCombo->currentIndex() < 4) ? pollSeconds[imapPollCombo->currentIndex()] : 300);
            entry.params[QStringLiteral("imapDeletion")] = QString::number(imapDeletionCombo->currentIndex());
            entry.params[QStringLiteral("imapTrashFolder")] = imapTrashFolderEdit->text().trimmed();
            const int idleSeconds[] = { 30, 60, 300 };
            entry.params[QStringLiteral("imapIdleSeconds")] = QString::number((imapIdleCombo->currentIndex() >= 0 && imapIdleCombo->currentIndex() < 3) ? idleSeconds[imapIdleCombo->currentIndex()] : 60);
            if (!smtpHost.isEmpty()) {
                entry.params[QStringLiteral("transportId")] = QString::fromUtf8(ctrl->smtpTransportUri);
                entry.params[QStringLiteral("transportHostname")] = smtpHost;
                entry.params[QStringLiteral("transportPort")] = QString::number(smtpPortSpin->value());
                entry.params[QStringLiteral("transportSecurity")] = QString::number(smtpSecurityCombo->currentIndex());
                entry.params[QStringLiteral("transportUsername")] = smtpUserEdit->text().trimmed();
            }
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        ctrl->editingStoreId.clear();
        win->statusBar()->showMessage(TR("status.added_imap"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    QObject::connect(pop3SaveBtn, &QPushButton::clicked, [=]() {
        QString displayName = pop3DisplayNameEdit->text().trimmed();
        QString pop3User = pop3UserEdit->text().trimmed();
        QString pop3Host = pop3HostEdit->text().trimmed();
        int pop3Port = pop3PortSpin->value();
        if (pop3Host.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.pop3"), TR("imap.validation.enter_host"));
            return;
        }
        Config config = loadConfig();
        const bool isEdit = !ctrl->editingStoreId.isEmpty();
        const QByteArray previousStoreUri = ctrl->storeUri;
        int colourIndex = 0;
        if (isEdit) {
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == ctrl->editingStoreId) {
                    colourIndex = i;
                    break;
                }
            }
        } else {
            colourIndex = config.stores.size();
        }
        QString userAtHost = pop3User;
        if (!pop3User.contains(QLatin1Char('@')) && !pop3Host.isEmpty()) {
            userAtHost = pop3User + QLatin1Char('@') + pop3Host;
        }
        char *uriPtr = tagliacarte_store_pop3_new(userAtHost.toUtf8().constData(), pop3Host.toUtf8().constData(), static_cast<uint16_t>(pop3Port));
        if (!uriPtr) {
            showError(win, "error.context.imap");
            return;
        }
        ctrl->storeUri = QByteArray(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (displayName.isEmpty()) {
            displayName = pop3User;
        }
        if (displayName.isEmpty()) {
            displayName = TR("accounts.type.pop3");
        }
        if (isEdit) {
            QByteArray oldUri = ctrl->editingStoreId.toUtf8();
            tagliacarte_store_free(oldUri.constData());
            ctrl->storeToTransport.remove(oldUri);
            ctrl->allStoreUris.removeAll(oldUri);
            for (int i = 0; i < ctrl->storeButtons.size(); ++i) {
                if (ctrl->storeButtons[i]->property("storeUri").toByteArray() == oldUri) {
                    ctrl->storeButtons[i]->deleteLater();
                    ctrl->storeButtons.removeAt(i);
                    break;
                }
            }
            if (previousStoreUri == oldUri) {
                ctrl->bridge->clearFolder();
                ctrl->folderTree->clear();
                ctrl->conversationList->clear();
                ctrl->messageView->clear();
                ctrl->messageHeaderPane->hide();
            }
        } else {
            ctrl->bridge->clearFolder();
            ctrl->folderTree->clear();
            ctrl->conversationList->clear();
            ctrl->messageView->clear();
            ctrl->messageHeaderPane->hide();
        }
        ctrl->allStoreUris.append(ctrl->storeUri);
        tagliacarte_store_set_folder_list_callbacks(ctrl->storeUri.constData(), on_folder_found_cb, on_folder_removed_cb, on_folder_list_complete_cb, ctrl->bridge);
        tagliacarte_store_refresh_folders(ctrl->storeUri.constData());
        QString initial = displayName.left(1).toUpper();
        if (initial.isEmpty()) {
            initial = QStringLiteral("P");
        }
        ctrl->addStoreCircle(initial, ctrl->storeUri, colourIndex);
        for (auto *b : ctrl->storeButtons) {
            b->setChecked(b->property("storeUri").toByteArray() == ctrl->storeUri);
        }
        ctrl->smtpTransportUri.clear();
        QString pop3SmtpHost = pop3SmtpHostEdit->text().trimmed();
        int pop3SmtpPort = pop3SmtpPortSpin->value();
        if (!pop3SmtpHost.isEmpty()) {
            char *transportUri = tagliacarte_transport_smtp_new(pop3SmtpHost.toUtf8().constData(), static_cast<uint16_t>(pop3SmtpPort));
            if (transportUri) {
                ctrl->smtpTransportUri = QByteArray(transportUri);
                tagliacarte_free_string(transportUri);
                ctrl->storeToTransport[ctrl->storeUri] = ctrl->smtpTransportUri;
            }
        }
        ctrl->updateComposeAppendButtons();
        if (isEdit) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == ctrl->editingStoreId) {
                    idx = i;
                    break;
                }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(ctrl->storeUri);
                e.type = QStringLiteral("pop3");
                e.displayName = displayName;
                e.emailAddress = pop3EmailEdit->text().trimmed();
                e.params[QStringLiteral("hostname")] = pop3Host;
                e.params[QStringLiteral("username")] = userAtHost;
                if (!pop3SmtpHost.isEmpty()) {
                    e.params[QStringLiteral("transportId")] = QString::fromUtf8(ctrl->smtpTransportUri);
                    e.params[QStringLiteral("transportHostname")] = pop3SmtpHost;
                    e.params[QStringLiteral("transportPort")] = QString::number(pop3SmtpPortSpin->value());
                    e.params[QStringLiteral("transportUsername")] = pop3SmtpUserEdit->text().trimmed();
                } else {
                    e.params.remove(QStringLiteral("transportId"));
                    e.params.remove(QStringLiteral("transportHostname"));
                    e.params.remove(QStringLiteral("transportPort"));
                    e.params.remove(QStringLiteral("transportUsername"));
                }
                if (config.lastSelectedStoreId == ctrl->editingStoreId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(ctrl->storeUri);
            entry.type = QStringLiteral("pop3");
            entry.displayName = displayName;
            entry.emailAddress = pop3EmailEdit->text().trimmed();
            entry.params[QStringLiteral("hostname")] = pop3Host;
            entry.params[QStringLiteral("username")] = userAtHost;
            if (!pop3SmtpHost.isEmpty()) {
                entry.params[QStringLiteral("transportId")] = QString::fromUtf8(ctrl->smtpTransportUri);
                entry.params[QStringLiteral("transportHostname")] = pop3SmtpHost;
                entry.params[QStringLiteral("transportPort")] = QString::number(pop3SmtpPortSpin->value());
                entry.params[QStringLiteral("transportUsername")] = pop3SmtpUserEdit->text().trimmed();
            }
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        ctrl->editingStoreId.clear();
        win->statusBar()->showMessage(TR("status.added_pop3"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    QObject::connect(mboxSaveBtn, &QPushButton::clicked, [=]() {
        QMessageBox::information(win, TR("accounts.type.mbox"), TR("mbox.not_implemented"));
    });

    QObject::connect(nostrRelayAddBtn, &QPushButton::clicked, [=]() {
        QString u = nostrRelayUrlEdit->text().trimmed();
        if (!u.isEmpty()) {
            nostrRelayList->addItem(u);
            nostrRelayUrlEdit->clear();
        }
    });
    QObject::connect(nostrRelayRemoveBtn, &QPushButton::clicked, [=]() {
        QList<QListWidgetItem *> sel = nostrRelayList->selectedItems();
        for (QListWidgetItem *item : sel) {
            delete nostrRelayList->takeItem(nostrRelayList->row(item));
        }
    });

    QObject::connect(nostrSecretKeyEdit, &QLineEdit::editingFinished, [=]() {
        QString secretText = nostrSecretKeyEdit->text().trimmed();
        if (secretText.isEmpty()) {
            return;
        }
        char *pkHex = tagliacarte_nostr_derive_pubkey(secretText.toUtf8().constData());
        if (!pkHex) {
            nostrPubkeyLabel->clear();
            nostrDerivedPubkeyHex->clear();
            nostrProfileStatus->setText(TR("nostr.status.invalid_key"));
            nostrProfileStatus->setVisible(true);
            return;
        }
        *nostrDerivedPubkeyHex = QString::fromUtf8(pkHex);
        nostrPubkeyLabel->setText(*nostrDerivedPubkeyHex);
        tagliacarte_free_string(pkHex);

        if (nostrRelayList->count() == 0) {
            Config bootstrapCfg = loadConfig();
            if (!bootstrapCfg.nostrBootstrapRelays.isEmpty()) {
                for (const QString &r : bootstrapCfg.nostrBootstrapRelays) {
                    QString t = r.trimmed();
                    if (!t.isEmpty()) {
                        nostrRelayList->addItem(t);
                    }
                }
            } else {
                char *defaults = tagliacarte_nostr_default_relays();
                if (defaults) {
                    QString defaultsStr = QString::fromUtf8(defaults);
                    tagliacarte_free_string(defaults);
                    for (const QString &r : defaultsStr.split(QLatin1Char(','))) {
                        QString t = r.trimmed();
                        if (!t.isEmpty()) {
                            nostrRelayList->addItem(t);
                        }
                    }
                }
            }
        }

        QStringList currentRelays;
        for (int i = 0; i < nostrRelayList->count(); ++i) {
            QString u = nostrRelayList->item(i)->text().trimmed();
            if (!u.isEmpty()) {
                currentRelays.append(u);
            }
        }
        if (currentRelays.isEmpty()) {
            return;
        }
        nostrProfileStatus->setText(TR("nostr.status.fetching_profile"));
        nostrProfileStatus->setVisible(true);

        QString relaysCsv = currentRelays.join(QLatin1Char(','));
        TagliacarteNostrProfile *profile = tagliacarte_nostr_fetch_profile(
            nostrDerivedPubkeyHex->toUtf8().constData(),
            relaysCsv.toUtf8().constData(),
            nullptr
        );
        if (profile) {
            if (profile->display_name) {
                QString name = QString::fromUtf8(profile->display_name);
                if (!name.isEmpty() && nostrDisplayNameEdit->text().trimmed().isEmpty()) {
                    nostrDisplayNameEdit->setText(name);
                }
            }
            if (profile->nip05) {
                nostrNip05Label->setText(QString::fromUtf8(profile->nip05));
            }
            if (profile->relays) {
                QString relaysFromProfile = QString::fromUtf8(profile->relays);
                if (!relaysFromProfile.isEmpty()) {
                    nostrRelayList->clear();
                    for (const QString &r : relaysFromProfile.split(QLatin1Char(','))) {
                        QString t = r.trimmed();
                        if (!t.isEmpty()) {
                            nostrRelayList->addItem(t);
                        }
                    }
                }
            }
            tagliacarte_nostr_profile_free(profile);
            nostrProfileStatus->setText(TR("nostr.status.profile_loaded"));
        } else {
            nostrProfileStatus->setText(TR("nostr.status.profile_not_found"));
        }
    });

    QObject::connect(nostrSaveBtn, &QPushButton::clicked, [=]() {
        QStringList relayList;
        for (int i = 0; i < nostrRelayList->count(); ++i) {
            QString u = nostrRelayList->item(i)->text().trimmed();
            if (!u.isEmpty()) {
                relayList.append(u);
            }
        }
        if (relayList.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.nostr"), TR("nostr.validation.relays"));
            return;
        }
        const QString relays = relayList.join(QLatin1Char(','));
        QString pubkeyHex = *nostrDerivedPubkeyHex;
        QString secretText = nostrSecretKeyEdit->text().trimmed();
        if (pubkeyHex.isEmpty() && !secretText.isEmpty()) {
            char *pkHex = tagliacarte_nostr_derive_pubkey(secretText.toUtf8().constData());
            if (pkHex) {
                pubkeyHex = QString::fromUtf8(pkHex);
                tagliacarte_free_string(pkHex);
            }
        }
        if (pubkeyHex.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.nostr"), TR("nostr.validation.key_required"));
            return;
        }
        QByteArray relaysUtf8 = relays.toUtf8();
        QByteArray pubkeyUtf8 = pubkeyHex.toUtf8();
        char *uriPtr = tagliacarte_store_nostr_new(relaysUtf8.constData(), pubkeyUtf8.constData());
        if (!uriPtr) {
            showError(win, "error.context.nostr");
            return;
        }
        QByteArray newStoreUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        if (!secretText.isEmpty()) {
            tagliacarte_credential_provide(newStoreUri.constData(), secretText.toUtf8().constData());
        }
        char *transportUriPtr = tagliacarte_transport_nostr_new(relaysUtf8.constData(), pubkeyUtf8.constData());
        QByteArray newTransportUri;
        if (transportUriPtr) {
            newTransportUri = QByteArray(transportUriPtr);
            tagliacarte_free_string(transportUriPtr);
        }
        QString displayName = nostrDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = TR("accounts.type.nostr");
        }
        QString nip05 = nostrNip05Label->text().trimmed();
        Config config = loadConfig();
        QString oldId = ctrl->editingStoreId;
        QString mediaServer = nostrMediaServerEdit->text().trimmed();
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) {
                    idx = i;
                    break;
                }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newStoreUri);
                e.type = QStringLiteral("nostr");
                e.displayName = displayName;
                e.emailAddress = nip05;
                e.params[QStringLiteral("path")] = relays;
                e.params[QStringLiteral("pubkey")] = pubkeyHex;
                e.params[QStringLiteral("mediaServer")] = mediaServer;
                if (config.lastSelectedStoreId == oldId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newStoreUri);
            entry.type = QStringLiteral("nostr");
            entry.displayName = displayName;
            entry.emailAddress = nip05;
            entry.params[QStringLiteral("path")] = relays;
            entry.params[QStringLiteral("pubkey")] = pubkeyHex;
            entry.params[QStringLiteral("mediaServer")] = mediaServer;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        ctrl->refreshStoresFromConfig();
        ctrl->editingStoreId.clear();
        win->statusBar()->showMessage(TR("status.added_nostr"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    QObject::connect(matrixSaveBtn, &QPushButton::clicked, [=]() {
        QString homeserver = matrixHomeserverEdit->text().trimmed();
        QString userId = matrixUserIdEdit->text().trimmed();
        QString token = matrixTokenEdit->text().trimmed();
        if (homeserver.isEmpty() || userId.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.matrix"), TR("matrix.validation.homeserver_user"));
            return;
        }
        const char *tokenPtr = token.isEmpty() ? nullptr : token.toUtf8().constData();
        char *uriPtr = tagliacarte_store_matrix_new(homeserver.toUtf8().constData(), userId.toUtf8().constData(), tokenPtr);
        if (!uriPtr) {
            showError(win, "error.context.matrix");
            return;
        }
        QByteArray newStoreUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        char *transportUriPtr = tagliacarte_transport_matrix_new(homeserver.toUtf8().constData(), userId.toUtf8().constData(), tokenPtr);
        QByteArray newTransportUri;
        if (transportUriPtr) {
            newTransportUri = QByteArray(transportUriPtr);
            tagliacarte_free_string(transportUriPtr);
        }
        QString displayName = matrixDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = userId;
        }
        Config config = loadConfig();
        QString oldId = ctrl->editingStoreId;
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) {
                    idx = i;
                    break;
                }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newStoreUri);
                e.type = QStringLiteral("matrix");
                e.displayName = displayName;
                e.params[QStringLiteral("path")] = homeserver;
                e.params[QStringLiteral("userId")] = userId;
                e.params[QStringLiteral("accessToken")] = token;
                if (config.lastSelectedStoreId == oldId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newStoreUri);
            entry.type = QStringLiteral("matrix");
            entry.displayName = displayName;
            entry.params[QStringLiteral("path")] = homeserver;
            entry.params[QStringLiteral("userId")] = userId;
            entry.params[QStringLiteral("accessToken")] = token;
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        ctrl->refreshStoresFromConfig();
        ctrl->editingStoreId.clear();
        win->statusBar()->showMessage(TR("status.added_matrix"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    // ---- NNTP save ----
    QObject::connect(nntpSaveBtn, &QPushButton::clicked, [=]() {
        QString displayName = nntpDisplayNameEdit->text().trimmed();
        QString nntpHost = nntpHostEdit->text().trimmed();
        int nntpPort = nntpPortSpin->value();
        QString nntpUser = nntpUserEdit->text().trimmed();
        if (nntpHost.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.nntp"), TR("imap.validation.enter_host"));
            return;
        }
        QString userAtHost = nntpUser;
        if (!nntpUser.isEmpty() && !nntpUser.contains(QLatin1Char('@'))) {
            userAtHost = nntpUser + QLatin1Char('@') + nntpHost;
        }
        if (userAtHost.isEmpty()) {
            userAtHost = nntpHost;
        }
        char *uriPtr = tagliacarte_store_nntp_new(userAtHost.toUtf8().constData(), nntpHost.toUtf8().constData(), static_cast<uint16_t>(nntpPort));
        if (!uriPtr) {
            showError(win, "error.context.nntp");
            return;
        }
        QByteArray newStoreUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        char *tUri = tagliacarte_transport_nntp_new(userAtHost.toUtf8().constData(), nntpHost.toUtf8().constData(), static_cast<uint16_t>(nntpPort));
        QByteArray newTransportUri;
        if (tUri) {
            newTransportUri = QByteArray(tUri);
            tagliacarte_free_string(tUri);
        }
        if (displayName.isEmpty()) {
            displayName = nntpHost;
        }
        Config config = loadConfig();
        QString oldId = ctrl->editingStoreId;
        if (!oldId.isEmpty()) {
            int idx = -1;
            for (int i = 0; i < config.stores.size(); ++i) {
                if (config.stores[i].id == oldId) {
                    idx = i;
                    break;
                }
            }
            if (idx >= 0) {
                StoreEntry &e = config.stores[idx];
                e.id = QString::fromUtf8(newStoreUri);
                e.type = QStringLiteral("nntp");
                e.displayName = displayName;
                e.params[QStringLiteral("hostname")] = nntpHost;
                e.params[QStringLiteral("username")] = userAtHost;
                e.params[QStringLiteral("port")] = QString::number(nntpPort);
                e.params[QStringLiteral("security")] = QString::number(nntpSecurityCombo->currentIndex());
                if (!newTransportUri.isEmpty()) {
                    e.params[QStringLiteral("transportId")] = QString::fromUtf8(newTransportUri);
                }
                if (config.lastSelectedStoreId == oldId) {
                    config.lastSelectedStoreId = e.id;
                }
            }
        } else {
            StoreEntry entry;
            entry.id = QString::fromUtf8(newStoreUri);
            entry.type = QStringLiteral("nntp");
            entry.displayName = displayName;
            entry.params[QStringLiteral("hostname")] = nntpHost;
            entry.params[QStringLiteral("username")] = userAtHost;
            entry.params[QStringLiteral("port")] = QString::number(nntpPort);
            entry.params[QStringLiteral("security")] = QString::number(nntpSecurityCombo->currentIndex());
            if (!newTransportUri.isEmpty()) {
                entry.params[QStringLiteral("transportId")] = QString::fromUtf8(newTransportUri);
            }
            config.stores.append(entry);
            config.lastSelectedStoreId = entry.id;
        }
        saveConfig(config);
        ctrl->refreshStoresFromConfig();
        ctrl->editingStoreId.clear();
        win->statusBar()->showMessage(TR("status.added_nntp"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    // ---- Gmail OAuth sign-in ----
    QObject::connect(gmailSignInBtn, &QPushButton::clicked, [=]() {
        QString email = gmailEmailEdit->text().trimmed();
        if (email.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.gmail"), TR("gmail.validation.enter_email"));
            return;
        }
        gmailStatusLabel->setText(TR("gmail.status.waiting"));
        gmailStatusLabel->setVisible(true);
        gmailSignInBtn->setEnabled(false);

        tagliacarte_oauth_start(
            "google",
            email.toUtf8().constData(),
            // on_url: open browser
            [](const char *url, void *user_data) {
                auto *bridge = static_cast<EventBridge *>(user_data);
                QString urlStr = QString::fromUtf8(url);
                QMetaObject::invokeMethod(bridge, [urlStr]() {
                    QDesktopServices::openUrl(QUrl(urlStr));
                }, Qt::QueuedConnection);
            },
            // on_complete: emit oauthComplete signal on the bridge
            [](int error, const char *errorMessage, void *user_data) {
                auto *bridge = static_cast<EventBridge *>(user_data);
                int err = error;
                QString errMsg = errorMessage ? QString::fromUtf8(errorMessage) : QString();
                QMetaObject::invokeMethod(bridge, [bridge, err, errMsg]() {
                    emit bridge->oauthComplete(QStringLiteral("google"), err, errMsg);
                }, Qt::QueuedConnection);
            },
            ctrl->bridge
        );
    });

    // When Google OAuth completes successfully, auto-save the Gmail account.
    QObject::connect(ctrl->bridge, &EventBridge::oauthComplete, [=](const QString &provider, int error, const QString &errorMessage) {
        if (provider != QLatin1String("google")) {
            return;
        }
        if (error == 0) {
            gmailSaveBtn->click();
        } else {
            gmailStatusLabel->setText(TR("gmail.status.error") + QStringLiteral(" ") + errorMessage);
            gmailSignInBtn->setEnabled(true);
        }
    });

    QObject::connect(gmailSaveBtn, &QPushButton::clicked, [=]() {
        QString email = gmailEmailEdit->text().trimmed();
        if (email.isEmpty()) {
            return;
        }
        char *uriPtr = tagliacarte_store_gmail_new(email.toUtf8().constData());
        if (!uriPtr) {
            showError(win, "error.context.gmail");
            gmailStatusLabel->setText(TR("gmail.status.error"));
            gmailSignInBtn->setEnabled(true);
            return;
        }
        QByteArray newStoreUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        char *transportUriPtr = tagliacarte_transport_gmail_smtp_new(email.toUtf8().constData());
        QByteArray newTransportUri;
        if (transportUriPtr) {
            newTransportUri = QByteArray(transportUriPtr);
            tagliacarte_free_string(transportUriPtr);
        }
        QString displayName = gmailDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = email;
        }
        Config config = loadConfig();
        StoreEntry entry;
        entry.id = QString::fromUtf8(newStoreUri);
        entry.type = QStringLiteral("gmail");
        entry.displayName = displayName;
        entry.emailAddress = email;
        config.stores.append(entry);
        config.lastSelectedStoreId = entry.id;
        saveConfig(config);
        ctrl->refreshStoresFromConfig();
        ctrl->editingStoreId.clear();
        gmailStatusLabel->setVisible(false);
        gmailSignInBtn->setEnabled(true);
        win->statusBar()->showMessage(TR("status.added_gmail"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    // ---- Exchange/Outlook OAuth sign-in ----
    QObject::connect(exchangeSignInBtn, &QPushButton::clicked, [=]() {
        QString email = exchangeEmailEdit->text().trimmed();
        if (email.isEmpty()) {
            QMessageBox::warning(win, TR("accounts.type.exchange"), TR("exchange.validation.enter_email"));
            return;
        }
        exchangeStatusLabel->setText(TR("exchange.status.waiting"));
        exchangeStatusLabel->setVisible(true);
        exchangeSignInBtn->setEnabled(false);

        tagliacarte_oauth_start(
            "microsoft",
            email.toUtf8().constData(),
            // on_url: open browser
            [](const char *url, void *user_data) {
                auto *bridge = static_cast<EventBridge *>(user_data);
                QString urlStr = QString::fromUtf8(url);
                QMetaObject::invokeMethod(bridge, [urlStr]() {
                    QDesktopServices::openUrl(QUrl(urlStr));
                }, Qt::QueuedConnection);
            },
            // on_complete: emit oauthComplete signal on the bridge
            [](int error, const char *errorMessage, void *user_data) {
                auto *bridge = static_cast<EventBridge *>(user_data);
                int err = error;
                QString errMsg = errorMessage ? QString::fromUtf8(errorMessage) : QString();
                QMetaObject::invokeMethod(bridge, [bridge, err, errMsg]() {
                    emit bridge->oauthComplete(QStringLiteral("microsoft"), err, errMsg);
                }, Qt::QueuedConnection);
            },
            ctrl->bridge
        );
    });

    // When Microsoft OAuth completes successfully, auto-save the Exchange account.
    QObject::connect(ctrl->bridge, &EventBridge::oauthComplete, [=](const QString &provider, int error, const QString &errorMessage) {
        if (provider != QLatin1String("microsoft")) {
            return;
        }
        if (error == 0) {
            exchangeSaveBtn->click();
        } else {
            exchangeStatusLabel->setText(TR("exchange.status.error") + QStringLiteral(" ") + errorMessage);
            exchangeSignInBtn->setEnabled(true);
        }
    });

    QObject::connect(exchangeSaveBtn, &QPushButton::clicked, [=]() {
        QString email = exchangeEmailEdit->text().trimmed();
        if (email.isEmpty()) {
            return;
        }
        char *uriPtr = tagliacarte_store_graph_new(email.toUtf8().constData());
        if (!uriPtr) {
            showError(win, "error.context.exchange");
            exchangeStatusLabel->setText(TR("exchange.status.error"));
            exchangeSignInBtn->setEnabled(true);
            return;
        }
        QByteArray newStoreUri(uriPtr);
        tagliacarte_free_string(uriPtr);
        char *transportUriPtr = tagliacarte_transport_graph_new(email.toUtf8().constData());
        QByteArray newTransportUri;
        if (transportUriPtr) {
            newTransportUri = QByteArray(transportUriPtr);
            tagliacarte_free_string(transportUriPtr);
        }
        QString displayName = exchangeDisplayNameEdit->text().trimmed();
        if (displayName.isEmpty()) {
            displayName = email;
        }
        Config config = loadConfig();
        StoreEntry entry;
        entry.id = QString::fromUtf8(newStoreUri);
        entry.type = QStringLiteral("exchange");
        entry.displayName = displayName;
        entry.emailAddress = email;
        config.stores.append(entry);
        config.lastSelectedStoreId = entry.id;
        saveConfig(config);
        ctrl->refreshStoresFromConfig();
        ctrl->editingStoreId.clear();
        exchangeStatusLabel->setVisible(false);
        exchangeSignInBtn->setEnabled(true);
        win->statusBar()->showMessage(TR("status.added_exchange"));
        accountsStack->setCurrentIndex(0);
        ctrl->rightStack->setCurrentIndex(0);
        ctrl->settingsBtn->setChecked(false);
    });

    settingsLayout->addWidget(settingsTabs);
    ctrl->rightStack->addWidget(settingsPage);

    QObject::connect(ctrl->settingsBtn, &QToolButton::clicked, [ctrl, refreshAccountListInSettings]() {
        if (ctrl->settingsBtn->isChecked()) {
            ctrl->rightStack->setCurrentIndex(1);
            refreshAccountListInSettings();
        } else {
            ctrl->rightStack->setCurrentIndex(0);
        }
    });

    return settingsPage;
}
