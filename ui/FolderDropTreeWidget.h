#ifndef FOLDERDROPTREEWIDGET_H
#define FOLDERDROPTREEWIDGET_H

#include <QTreeWidget>
#include <QDragEnterEvent>
#include <QDragMoveEvent>
#include <QDropEvent>
#include <QMimeData>
#include <QApplication>

#include "EventBridge.h"

/**
 * QTreeWidget subclass for the folder tree pane.
 * Accepts drops of "application/x-tagliacarte-messages" MIME data.
 *
 * Drop validation:
 *  - Target item must have a non-empty FolderNameRole.
 *  - Target must not have \Noselect attribute.
 *  - Target must not be the same as the source folder.
 *
 * Shift+drop = move, otherwise = copy.
 * Emits messagesDropped() signal with the parsed data.
 */
class FolderDropTreeWidget : public QTreeWidget {
    Q_OBJECT
public:
    explicit FolderDropTreeWidget(QWidget *parent = nullptr)
        : QTreeWidget(parent)
    {}

Q_SIGNALS:
    /** Emitted when messages are dropped on a valid folder.
     *  sourceFolderUri: the folder the messages come from.
     *  messageIds: list of message IDs.
     *  destFolderName: the real protocol name of the destination folder.
     *  isMove: true if Shift was held (move operation), false for copy. */
    void messagesDropped(const QByteArray &sourceFolderUri,
                         const QStringList &messageIds,
                         const QString &destFolderName,
                         bool isMove);

protected:
    void dragEnterEvent(QDragEnterEvent *event) override {
        if (event->mimeData()->hasFormat(QStringLiteral("application/x-tagliacarte-messages"))) {
            event->acceptProposedAction();
        } else {
            QTreeWidget::dragEnterEvent(event);
        }
    }

    void dragMoveEvent(QDragMoveEvent *event) override {
        if (!event->mimeData()->hasFormat(QStringLiteral("application/x-tagliacarte-messages"))) {
            QTreeWidget::dragMoveEvent(event);
            return;
        }

        auto *targetItem = itemAt(event->position().toPoint());
        if (!isValidDropTarget(targetItem, event->mimeData())) {
            event->ignore();
            return;
        }
        event->acceptProposedAction();
    }

    void dropEvent(QDropEvent *event) override {
        if (!event->mimeData()->hasFormat(QStringLiteral("application/x-tagliacarte-messages"))) {
            QTreeWidget::dropEvent(event);
            return;
        }

        auto *targetItem = itemAt(event->position().toPoint());
        if (!isValidDropTarget(targetItem, event->mimeData())) {
            event->ignore();
            return;
        }

        QString destFolderName = targetItem->data(0, FolderNameRole).toString();
        bool isMove = (QApplication::keyboardModifiers() & Qt::ShiftModifier) != 0;

        // Parse MIME data
        QByteArray payload = event->mimeData()->data(QStringLiteral("application/x-tagliacarte-messages"));
        QList<QByteArray> lines = payload.split('\n');
        if (lines.size() < 2) {
            event->ignore();
            return;
        }

        QByteArray sourceFolderUri = lines.at(0);
        QStringList messageIds;
        for (int i = 1; i < lines.size(); ++i) {
            QByteArray line = lines.at(i).trimmed();
            if (!line.isEmpty()) {
                messageIds.append(QString::fromUtf8(line));
            }
        }

        if (messageIds.isEmpty()) {
            event->ignore();
            return;
        }

        event->acceptProposedAction();
        emit messagesDropped(sourceFolderUri, messageIds, destFolderName, isMove);
    }

private:
    bool isValidDropTarget(QTreeWidgetItem *item, const QMimeData *mimeData) const {
        if (!item) {
            return false;
        }
        QString folderName = item->data(0, FolderNameRole).toString();
        if (folderName.isEmpty()) {
            return false;
        }
        QString attrs = item->data(0, FolderAttrsRole).toString().toLower();
        if (attrs.contains(QLatin1String("\\noselect"))) {
            return false;
        }

        // Check that we're not dropping onto the source folder
        QByteArray payload = mimeData->data(QStringLiteral("application/x-tagliacarte-messages"));
        int newlinePos = payload.indexOf('\n');
        if (newlinePos > 0) {
            QByteArray sourceFolderUri = payload.left(newlinePos);
            // The source folder URI ends with the folder name after the last '/'
            // We compare the dest folder name with the source to avoid self-drop
            // A simple check: if the source URI ends with "/folderName", reject
            QByteArray destUtf8 = folderName.toUtf8();
            if (sourceFolderUri.endsWith("/" + destUtf8)) {
                return false;
            }
        }
        return true;
    }
};

#endif // FOLDERDROPTREEWIDGET_H
