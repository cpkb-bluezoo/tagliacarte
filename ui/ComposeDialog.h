#ifndef COMPOSEDIALOG_H
#define COMPOSEDIALOG_H

#include <QDialog>
#include <QLineEdit>
#include <QTextEdit>
#include <QListWidget>
#include <QVector>
#include <QByteArray>
#include <QString>
#include <QMetaType>

// Compose part: file path or message reference (folder_uri + message_id). Stored as user data on list items.
enum ComposePartType { ComposePartFile, ComposePartMessage };
struct ComposePart {
    ComposePartType type = ComposePartFile;
    QString pathOrDisplay;
    QByteArray folderUri;
    QByteArray messageId;
    bool asAttachment = false;
};
Q_DECLARE_METATYPE(ComposePart)

// Compose dialog (From/To/Subject/Body; labels vary by transport kind).
class ComposeDialog : public QDialog {
public:
    QLineEdit *fromEdit;
    QLineEdit *toEdit;
    QLineEdit *ccEdit;
    QLineEdit *subjectEdit;
    QListWidget *partsList;   // list of attachments / embedded messages (files by path, messages by ref)
    QTextEdit *bodyEdit;

    ComposeDialog(QWidget *parent = nullptr, const QByteArray &transportUri = QByteArray(),
                 const QString &from = QString(), const QString &to = QString(), const QString &cc = QString(),
                 const QString &subject = QString(), const QString &body = QString(), bool replyCursorBefore = false);

    void addPartFile(const QString &path);
    void addPartMessage(const QByteArray &folderUri, const QByteArray &messageId, const QString &display, bool asAttachment);
    QVector<ComposePart> parts() const;
};

#endif // COMPOSEDIALOG_H
