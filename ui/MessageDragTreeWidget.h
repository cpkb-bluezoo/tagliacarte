#ifndef MESSAGEDRAGTREEWIDGET_H
#define MESSAGEDRAGTREEWIDGET_H

#include <QTreeWidget>
#include <QMimeData>
#include <QStringList>

#include "EventBridge.h"

/**
 * QTreeWidget subclass for the message list pane.
 * Provides drag support: selected messages are encoded in a custom MIME type
 * for drop onto the folder tree (FolderDropTreeWidget).
 *
 * Format of "application/x-tagliacarte-messages" MIME data:
 *   Line 0: source folder URI
 *   Lines 1..N: message IDs
 */
class MessageDragTreeWidget : public QTreeWidget {
    Q_OBJECT
public:
    explicit MessageDragTreeWidget(QWidget *parent = nullptr)
        : QTreeWidget(parent)
    {}

    /** Set the folder URI for the currently displayed folder. */
    void setSourceFolderUri(const QByteArray &uri) { m_sourceFolderUri = uri; }
    QByteArray sourceFolderUri() const { return m_sourceFolderUri; }

protected:
    QStringList mimeTypes() const override {
        return { QStringLiteral("application/x-tagliacarte-messages") };
    }

    // Note: supportedDragActions is not virtual in QTreeWidget, but
    // mimeTypes and mimeData are sufficient for drag support.

    QMimeData *mimeData(const QList<QTreeWidgetItem *> &items) const override {
        if (items.isEmpty() || m_sourceFolderUri.isEmpty()) {
            return nullptr;
        }
        QByteArray payload;
        payload.append(m_sourceFolderUri);
        payload.append('\n');
        for (auto *item : items) {
            QVariant idVar = item->data(0, MessageIdRole);
            if (idVar.isValid()) {
                payload.append(idVar.toString().toUtf8());
                payload.append('\n');
            }
        }
        auto *data = new QMimeData;
        data->setData(QStringLiteral("application/x-tagliacarte-messages"), payload);
        return data;
    }

private:
    QByteArray m_sourceFolderUri;
};

#endif // MESSAGEDRAGTREEWIDGET_H
