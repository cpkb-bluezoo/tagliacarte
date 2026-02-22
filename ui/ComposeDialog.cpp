#include "ComposeDialog.h"
#include "Tr.h"
#include "tagliacarte.h"

#include <QFormLayout>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPushButton>
#include <QLabel>
#include <QFileDialog>
#include <QDialogButtonBox>
#include <QFontDatabase>
#include <QTextCursor>

ComposeDialog::ComposeDialog(QWidget *parent, const QByteArray &transportUri,
             const QString &from, const QString &to, const QString &cc,
             const QString &subject, const QString &body, bool replyCursorBefore,
             bool conversationMode) : QDialog(parent) {
    setWindowTitle(conversationMode ? TR("compose.new_conversation") : TR("compose.title"));
    auto *layout = new QFormLayout(this);

    fromEdit = new QLineEdit(this);
    fromEdit->setPlaceholderText(TR("compose.placeholder.from"));
    fromEdit->setText(from);
    auto *fromLabel = new QLabel(TR("compose.from") + QStringLiteral(":"), this);
    layout->addRow(fromLabel, fromEdit);

    toEdit = new QLineEdit(this);
    QString toLabelText = TR("compose.to");
    QString toPlaceholder = TR("compose.placeholder.to");
    if (!transportUri.isEmpty()) {
        int kind = tagliacarte_transport_kind(transportUri.constData());
        if (kind == TAGLIACARTE_TRANSPORT_KIND_NOSTR) {
            toLabelText = TR("compose.to_pubkey");
            toPlaceholder = TR("compose.placeholder.to_pubkey");
        } else if (kind == TAGLIACARTE_TRANSPORT_KIND_MATRIX) {
            toLabelText = TR("compose.to_room_mxid");
            toPlaceholder = TR("compose.placeholder.to_room_mxid");
        } else if (kind == TAGLIACARTE_TRANSPORT_KIND_NNTP) {
            toLabelText = TR("compose.to_newsgroups");
            toPlaceholder = TR("compose.placeholder.to_newsgroups");
        }
    }
    toEdit->setPlaceholderText(toPlaceholder);
    toEdit->setText(to);
    layout->addRow(toLabelText + QStringLiteral(":"), toEdit);

    ccEdit = new QLineEdit(this);
    ccEdit->setPlaceholderText(TR("compose.placeholder.cc"));
    ccEdit->setText(cc);
    bool isEmail = transportUri.isEmpty() || tagliacarte_transport_kind(transportUri.constData()) == TAGLIACARTE_TRANSPORT_KIND_EMAIL;
    auto *ccLabel = new QLabel(TR("compose.cc") + QStringLiteral(":"), this);
    layout->addRow(ccLabel, ccEdit);

    subjectEdit = new QLineEdit(this);
    subjectEdit->setText(subject);
    auto *subjectLabel = new QLabel(TR("compose.subject") + QStringLiteral(":"), this);
    layout->addRow(subjectLabel, subjectEdit);

    auto *partsLabel = new QLabel(TR("compose.parts") + QStringLiteral(":"), this);
    auto *partsContainer = new QWidget(this);
    auto *partsLayout = new QVBoxLayout(partsContainer);
    partsLayout->setContentsMargins(0, 0, 0, 0);
    partsList = new QListWidget(this);
    partsList->setMaximumHeight(80);
    partsList->setFont(QFontDatabase::systemFont(QFontDatabase::FixedFont));
    partsLayout->addWidget(partsList);
    auto *partsButtons = new QHBoxLayout();
    auto *addFileBtn = new QPushButton(TR("compose.attach_file"), this);
    auto *removePartBtn = new QPushButton(TR("compose.remove_part"), this);
    removePartBtn->setEnabled(false);
    partsButtons->addWidget(addFileBtn);
    partsButtons->addWidget(removePartBtn);
    partsButtons->addStretch();
    partsLayout->addLayout(partsButtons);
    layout->addRow(partsLabel, partsContainer);

    bodyEdit = new QTextEdit(this);
    bodyEdit->setMinimumHeight(120);
    bodyEdit->setPlainText(body);
    if (replyCursorBefore) {
        bodyEdit->moveCursor(QTextCursor::Start);
    }
    layout->addRow(TR("compose.body") + QStringLiteral(":"), bodyEdit);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
    layout->addRow(buttons);

    if (conversationMode) {
        fromEdit->hide();
        fromLabel->hide();
        ccEdit->hide();
        ccLabel->hide();
        subjectEdit->hide();
        subjectLabel->hide();
        partsLabel->hide();
        partsContainer->hide();
    } else {
        ccEdit->setVisible(isEmail);
        ccLabel->setVisible(isEmail);
    }

    connect(partsList, &QListWidget::itemSelectionChanged, [removePartBtn, this]() {
        removePartBtn->setEnabled(partsList->currentItem() != nullptr);
    });
    connect(addFileBtn, &QPushButton::clicked, [this]() {
        QString path = QFileDialog::getOpenFileName(this, TR("compose.attach_file_dialog"));
        if (!path.isEmpty()) {
            addPartFile(path);
        }
    });
    connect(removePartBtn, &QPushButton::clicked, [this]() {
        auto *item = partsList->currentItem();
        if (item) {
            delete partsList->takeItem(partsList->row(item));
        }
    });

    resize(conversationMode ? 400 : 600, conversationMode ? 300 : 500);
}

void ComposeDialog::addPartFile(const QString &path) {
    auto *item = new QListWidgetItem(TR("compose.part_file") + QLatin1String(": ") + path);
    ComposePart p;
    p.type = ComposePartFile;
    p.pathOrDisplay = path;
    item->setData(Qt::UserRole, QVariant::fromValue(p));
    partsList->addItem(item);
}

void ComposeDialog::addPartMessage(const QByteArray &folderUri, const QByteArray &messageId, const QString &display, bool asAttachment) {
    auto *item = new QListWidgetItem((asAttachment ? TR("compose.part_attachment") : TR("compose.part_embedded")) + QLatin1String(": ") + display);
    ComposePart p;
    p.type = ComposePartMessage;
    p.pathOrDisplay = display;
    p.folderUri = folderUri;
    p.messageId = messageId;
    p.asAttachment = asAttachment;
    item->setData(Qt::UserRole, QVariant::fromValue(p));
    partsList->addItem(item);
}

QVector<ComposePart> ComposeDialog::parts() const {
    QVector<ComposePart> out;
    for (int i = 0; i < partsList->count(); ++i) {
        QVariant v = partsList->item(i)->data(Qt::UserRole);
        if (v.canConvert<ComposePart>()) {
            out.append(v.value<ComposePart>());
        }
    }
    return out;
}
