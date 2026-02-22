#ifndef COMPOSEDIALOG_H
#define COMPOSEDIALOG_H

#include <QDialog>
#include <QLineEdit>
#include <QTextEdit>
#include <QVector>
#include <QByteArray>
#include <QString>
#include <QMetaType>

class QWidget;
class QToolButton;

enum ComposePartType { ComposePartFile, ComposePartMessage };
struct ComposePart {
    ComposePartType type = ComposePartFile;
    QString pathOrDisplay;
    QByteArray folderUri;
    QByteArray messageId;
    bool asAttachment = false;
    qint64 fileSize = 0;
};
Q_DECLARE_METATYPE(ComposePart)

class ComposeDialog : public QDialog {
public:
    QLineEdit *fromEdit;
    QLineEdit *toEdit;
    QLineEdit *ccEdit;
    QLineEdit *bccEdit;
    QLineEdit *subjectEdit;
    QTextEdit *bodyEdit;

    ComposeDialog(QWidget *parent = nullptr, const QByteArray &transportUri = QByteArray(),
                 const QString &from = QString(), const QString &to = QString(), const QString &cc = QString(),
                 const QString &subject = QString(), const QString &body = QString(), bool replyCursorBefore = false,
                 bool conversationMode = false, const QString &mediaServerUrl = QString());

    void addPartFile(const QString &path);
    void addPartMessage(const QByteArray &folderUri, const QByteArray &messageId, const QString &display, bool asAttachment);
    QVector<ComposePart> parts() const;

    void reject() override;
    void onMediaUploadComplete(const QString &url, const QByteArray &fileHash);

private:
    void rebuildAttachmentCards();
    void nostrUploadFile(const QString &path);
    void deleteUploadedMedia();
    static QString humanFileSize(qint64 bytes);

    QVector<ComposePart> m_parts;
    QWidget *attachmentsPane;
    bool m_isNostr = false;
    QByteArray m_transportUri;
    QString m_mediaServerUrl;
    QVector<QByteArray> m_uploadedHashes;
};

#endif // COMPOSEDIALOG_H
