#include "ComposeDialog.h"
#include "FlowLayout.h"
#include "IconUtils.h"
#include "Tr.h"
#include "tagliacarte.h"

#include <QFormLayout>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QToolButton>
#include <QLabel>
#include <QFileDialog>
#include <QFileInfo>
#include <QFontDatabase>
#include <QTextCursor>
#include <QFrame>
#include <QMenu>
#include <QMessageBox>
#include <QMainWindow>
#include <QPalette>
#include <QStatusBar>
#include <QApplication>

// FFI callback: media upload complete. user_data is a ComposeDialog*.
static QStatusBar *parentStatusBar(QWidget *w) {
    if (auto *mw = qobject_cast<QMainWindow *>(w->parentWidget()))
        return mw->statusBar();
    return nullptr;
}

static void on_media_upload_complete(const char *url, const char *file_hash, void *user_data) {
    auto *dlg = static_cast<ComposeDialog *>(user_data);
    if (!url || !file_hash) {
        QMetaObject::invokeMethod(dlg, [dlg]() {
            if (auto *sb = parentStatusBar(dlg))
                sb->showMessage(TR("compose.nostr_upload_failed"), 5000);
        }, Qt::QueuedConnection);
        return;
    }
    QString urlStr = QString::fromUtf8(url);
    QByteArray hashBa = QByteArray(file_hash);
    QMetaObject::invokeMethod(dlg, [dlg, urlStr, hashBa]() {
        dlg->onMediaUploadComplete(urlStr, hashBa);
        if (auto *sb = parentStatusBar(dlg))
            sb->showMessage(TR("status.upload_complete"), 3000);
    }, Qt::QueuedConnection);
}

// FFI callback: media delete complete (fire-and-forget, ignore result).
static void on_media_delete_complete(int /*ok*/, void * /*user_data*/) {}

ComposeDialog::ComposeDialog(QWidget *parent, const QByteArray &transportUri,
             const QString &from, const QString &to, const QString &cc,
             const QString &subject, const QString &body, bool replyCursorBefore,
             bool conversationMode, const QString &mediaServerUrl) : QDialog(parent) {
    setWindowTitle(conversationMode ? TR("compose.new_conversation") : TR("compose.title"));

    m_transportUri = transportUri;
    m_mediaServerUrl = mediaServerUrl;

    int transportKind = -1;
    if (!transportUri.isEmpty()) {
        transportKind = tagliacarte_transport_kind(transportUri.constData());
    }
    m_isNostr = (transportKind == TAGLIACARTE_TRANSPORT_KIND_NOSTR);
    bool isEmail = transportUri.isEmpty() || transportKind == TAGLIACARTE_TRANSPORT_KIND_EMAIL;

    auto *outerLayout = new QVBoxLayout(this);

    // --- Header fields (form) ---
    auto *formLayout = new QFormLayout();
    formLayout->setFieldGrowthPolicy(QFormLayout::ExpandingFieldsGrow);

    fromEdit = new QLineEdit(this);
    fromEdit->setPlaceholderText(TR("compose.placeholder.from"));
    fromEdit->setText(from);
    auto *fromLabel = new QLabel(TR("compose.from") + QStringLiteral(":"), this);
    formLayout->addRow(fromLabel, fromEdit);

    toEdit = new QLineEdit(this);
    QString toLabelText = TR("compose.to");
    QString toPlaceholder = TR("compose.placeholder.to");
    if (transportKind == TAGLIACARTE_TRANSPORT_KIND_NOSTR) {
        toLabelText = TR("compose.to_pubkey");
        toPlaceholder = TR("compose.placeholder.to_pubkey");
    } else if (transportKind == TAGLIACARTE_TRANSPORT_KIND_MATRIX) {
        toLabelText = TR("compose.to_room_mxid");
        toPlaceholder = TR("compose.placeholder.to_room_mxid");
    } else if (transportKind == TAGLIACARTE_TRANSPORT_KIND_NNTP) {
        toLabelText = TR("compose.to_newsgroups");
        toPlaceholder = TR("compose.placeholder.to_newsgroups");
    }
    toEdit->setPlaceholderText(toPlaceholder);
    toEdit->setText(to);
    formLayout->addRow(toLabelText + QStringLiteral(":"), toEdit);

    ccEdit = new QLineEdit(this);
    ccEdit->setPlaceholderText(TR("compose.placeholder.cc"));
    ccEdit->setText(cc);
    auto *ccLabel = new QLabel(TR("compose.cc") + QStringLiteral(":"), this);
    formLayout->addRow(ccLabel, ccEdit);

    bccEdit = new QLineEdit(this);
    bccEdit->setPlaceholderText(TR("compose.placeholder.bcc"));
    auto *bccLabel = new QLabel(TR("compose.bcc") + QStringLiteral(":"), this);
    formLayout->addRow(bccLabel, bccEdit);

    subjectEdit = new QLineEdit(this);
    subjectEdit->setText(subject);
    auto *subjectLabel = new QLabel(TR("compose.subject") + QStringLiteral(":"), this);
    formLayout->addRow(subjectLabel, subjectEdit);

    outerLayout->addLayout(formLayout);

    // --- Body (message) ---
    auto *messageLabel = new QLabel(TR("compose.message") + QStringLiteral(":"), this);
    outerLayout->addWidget(messageLabel);

    bodyEdit = new QTextEdit(this);
    bodyEdit->setPlainText(body);
    if (replyCursorBefore) {
        bodyEdit->moveCursor(QTextCursor::Start);
    }
    outerLayout->addWidget(bodyEdit, 1);

    // --- Attachments pane (hidden until populated; always hidden for Nostr) ---
    attachmentsPane = new QWidget(this);
    attachmentsPane->setLayout(new FlowLayout(attachmentsPane, 0, 6, 4));
    attachmentsPane->setVisible(false);
    outerLayout->addWidget(attachmentsPane);

    // --- Bottom bar ---
    auto *bottomBar = new QHBoxLayout();

    QColor btnColor = QApplication::palette().color(QPalette::ButtonText);

    auto *attachBtn = new QToolButton(this);
    attachBtn->setIcon(iconFromSvgResource(QStringLiteral(":/icons/paperclip.svg"), btnColor, 20));
    attachBtn->setToolTip(TR("compose.attach_file"));
    attachBtn->setAutoRaise(true);
    attachBtn->setIconSize(QSize(20, 20));
    if (m_isNostr && m_mediaServerUrl.isEmpty()) {
        attachBtn->setEnabled(false);
    }
    bottomBar->addWidget(attachBtn);
    bottomBar->addStretch();

    auto *cancelBtn = new QToolButton(this);
    cancelBtn->setIcon(iconFromSvgResource(QStringLiteral(":/icons/x.svg"), btnColor, 20));
    cancelBtn->setToolTip(TR("compose.cancel"));
    cancelBtn->setAutoRaise(true);
    cancelBtn->setIconSize(QSize(20, 20));
    bottomBar->addWidget(cancelBtn);

    auto *sendBtn = new QToolButton(this);
    sendBtn->setIcon(iconFromSvgResource(QStringLiteral(":/icons/send.svg"), btnColor, 20));
    sendBtn->setToolTip(TR("compose.send"));
    sendBtn->setAutoRaise(true);
    sendBtn->setIconSize(QSize(20, 20));
    bottomBar->addWidget(sendBtn);

    outerLayout->addLayout(bottomBar);

    // --- Visibility ---
    if (conversationMode) {
        fromEdit->hide(); fromLabel->hide();
        ccEdit->hide(); ccLabel->hide();
        bccEdit->hide(); bccLabel->hide();
        subjectEdit->hide(); subjectLabel->hide();
        messageLabel->hide();
    } else if (m_isNostr) {
        fromEdit->hide(); fromLabel->hide();
        ccEdit->hide(); ccLabel->hide();
        bccEdit->hide(); bccLabel->hide();
        subjectEdit->hide(); subjectLabel->hide();
    } else {
        ccEdit->setVisible(isEmail);
        ccLabel->setVisible(isEmail);
        bccEdit->setVisible(isEmail);
        bccLabel->setVisible(isEmail);
    }

    // --- Connections ---
    connect(attachBtn, &QToolButton::clicked, this, [this]() {
        QString path = QFileDialog::getOpenFileName(this, TR("compose.attach_file_dialog"));
        if (!path.isEmpty()) {
            if (m_isNostr) {
                nostrUploadFile(path);
            } else {
                addPartFile(path);
            }
        }
    });
    connect(cancelBtn, &QToolButton::clicked, this, &QDialog::reject);
    connect(sendBtn, &QToolButton::clicked, this, &QDialog::accept);

    // --- Focus: body for replies (To pre-filled), To for new/forward ---
    if (to.isEmpty()) {
        toEdit->setFocus();
    } else {
        bodyEdit->setFocus();
    }

    resize(conversationMode ? 400 : 600, conversationMode ? 300 : 500);
}

void ComposeDialog::reject() {
    deleteUploadedMedia();
    QDialog::reject();
}

void ComposeDialog::onMediaUploadComplete(const QString &url, const QByteArray &fileHash) {
    m_uploadedHashes.append(fileHash);
    QTextCursor cursor = bodyEdit->textCursor();
    cursor.insertText(url);
    bodyEdit->setTextCursor(cursor);
}

void ComposeDialog::nostrUploadFile(const QString &path) {
    if (m_transportUri.isEmpty() || m_mediaServerUrl.isEmpty()) return;
    if (auto *sb = parentStatusBar(this))
        sb->showMessage(TR("status.uploading"));
    tagliacarte_nostr_media_upload_async(
        m_transportUri.constData(),
        path.toUtf8().constData(),
        m_mediaServerUrl.toUtf8().constData(),
        on_media_upload_complete,
        this
    );
}

void ComposeDialog::deleteUploadedMedia() {
    if (m_uploadedHashes.isEmpty() || m_transportUri.isEmpty() || m_mediaServerUrl.isEmpty()) return;
    for (const QByteArray &hash : m_uploadedHashes) {
        tagliacarte_nostr_media_delete_async(
            m_transportUri.constData(),
            hash.constData(),
            m_mediaServerUrl.toUtf8().constData(),
            on_media_delete_complete,
            nullptr
        );
    }
    m_uploadedHashes.clear();
}

void ComposeDialog::addPartFile(const QString &path) {
    ComposePart p;
    p.type = ComposePartFile;
    p.pathOrDisplay = path;
    QFileInfo fi(path);
    p.fileSize = fi.exists() ? fi.size() : 0;
    m_parts.append(p);
    rebuildAttachmentCards();
}

void ComposeDialog::addPartMessage(const QByteArray &folderUri, const QByteArray &messageId, const QString &display, bool asAttachment) {
    ComposePart p;
    p.type = ComposePartMessage;
    p.pathOrDisplay = display;
    p.folderUri = folderUri;
    p.messageId = messageId;
    p.asAttachment = asAttachment;
    m_parts.append(p);
    rebuildAttachmentCards();
}

QVector<ComposePart> ComposeDialog::parts() const {
    return m_parts;
}

void ComposeDialog::rebuildAttachmentCards() {
    if (m_isNostr) return;

    auto *flow = static_cast<FlowLayout *>(attachmentsPane->layout());

    while (QLayoutItem *child = flow->takeAt(0)) {
        delete child->widget();
        delete child;
    }

    QFont sizeFont = QApplication::font();
    sizeFont.setPointSizeF(sizeFont.pointSizeF() * 0.85);
    QColor mutedColor = QApplication::palette().color(QPalette::PlaceholderText);

    for (int i = 0; i < m_parts.size(); ++i) {
        const ComposePart &p = m_parts[i];
        auto *card = new QFrame(attachmentsPane);
        card->setFrameShape(QFrame::StyledPanel);
        card->setStyleSheet(QStringLiteral(
            "QFrame { border: 1px solid palette(mid); border-radius: 4px; padding: 3px 8px; }"));

        auto *cardLayout = new QHBoxLayout(card);
        cardLayout->setContentsMargins(0, 0, 0, 0);
        cardLayout->setSpacing(6);

        QString name = (p.type == ComposePartFile) ? QFileInfo(p.pathOrDisplay).fileName() : p.pathOrDisplay;
        auto *nameLabel = new QLabel(name, card);
        cardLayout->addWidget(nameLabel);

        if (p.fileSize > 0) {
            auto *sizeLabel = new QLabel(humanFileSize(p.fileSize), card);
            sizeLabel->setFont(sizeFont);
            QPalette pal = sizeLabel->palette();
            pal.setColor(QPalette::WindowText, mutedColor);
            sizeLabel->setPalette(pal);
            cardLayout->addWidget(sizeLabel);
        }

        card->setContextMenuPolicy(Qt::CustomContextMenu);
        card->setProperty("partIndex", i);
        connect(card, &QWidget::customContextMenuRequested, this, [this, card](const QPoint &pos) {
            int idx = card->property("partIndex").toInt();
            if (idx < 0 || idx >= m_parts.size()) return;
            QMenu menu;
            menu.addAction(TR("compose.remove_part"), this, [this, idx]() {
                m_parts.removeAt(idx);
                rebuildAttachmentCards();
            });
            menu.exec(card->mapToGlobal(pos));
        });

        flow->addWidget(card);
    }

    attachmentsPane->setVisible(!m_parts.isEmpty());
}

QString ComposeDialog::humanFileSize(qint64 bytes) {
    if (bytes < 1024) return QString::number(bytes) + QStringLiteral(" B");
    double kb = bytes / 1024.0;
    if (kb < 1024.0) return QString::number(kb, 'f', 1) + QStringLiteral(" KB");
    double mb = kb / 1024.0;
    if (mb < 1024.0) return QString::number(mb, 'f', 1) + QStringLiteral(" MB");
    double gb = mb / 1024.0;
    return QString::number(gb, 'f', 2) + QStringLiteral(" GB");
}
