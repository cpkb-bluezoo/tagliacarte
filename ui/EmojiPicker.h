/*
 * EmojiPicker.h
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

#ifndef EMOJIPICKER_H
#define EMOJIPICKER_H

#include <QWidget>

class QTabWidget;

class EmojiPicker : public QWidget {
    Q_OBJECT
public:
    explicit EmojiPicker(QWidget *parent = nullptr);

    /** Show the picker positioned above the given widget. */
    void showRelativeTo(QWidget *anchor);

signals:
    void emojiSelected(const QString &emoji);

private:
    QWidget *buildCategoryPage(const QStringList &emojis);
};

#endif // EMOJIPICKER_H
