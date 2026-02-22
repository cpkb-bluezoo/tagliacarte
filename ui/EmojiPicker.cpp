/*
 * EmojiPicker.cpp
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

#include "EmojiPicker.h"

#include <QGridLayout>
#include <QVBoxLayout>
#include <QPushButton>
#include <QTabWidget>
#include <QScrollArea>
#include <QScreen>
#include <QApplication>

static const int COLUMNS = 8;
static const int BTN_SIZE = 34;

// ── Emoji sets by category ──────────────────────────────────────────

static const QStringList smileys = {
    QStringLiteral("\U0001F600"), QStringLiteral("\U0001F603"),
    QStringLiteral("\U0001F604"), QStringLiteral("\U0001F601"),
    QStringLiteral("\U0001F606"), QStringLiteral("\U0001F605"),
    QStringLiteral("\U0001F602"), QStringLiteral("\U0001F923"),
    QStringLiteral("\U0001F60A"), QStringLiteral("\U0001F607"),
    QStringLiteral("\U0001F642"), QStringLiteral("\U0001F643"),
    QStringLiteral("\U0001F609"), QStringLiteral("\U0001F60C"),
    QStringLiteral("\U0001F60D"), QStringLiteral("\U0001F970"),
    QStringLiteral("\U0001F618"), QStringLiteral("\U0001F617"),
    QStringLiteral("\U0001F619"), QStringLiteral("\U0001F61A"),
    QStringLiteral("\U0001F60B"), QStringLiteral("\U0001F61B"),
    QStringLiteral("\U0001F61C"), QStringLiteral("\U0001F92A"),
    QStringLiteral("\U0001F61D"), QStringLiteral("\U0001F911"),
    QStringLiteral("\U0001F917"), QStringLiteral("\U0001F92D"),
    QStringLiteral("\U0001F92B"), QStringLiteral("\U0001F914"),
    QStringLiteral("\U0001F910"), QStringLiteral("\U0001F928"),
    QStringLiteral("\U0001F610"), QStringLiteral("\U0001F611"),
    QStringLiteral("\U0001F636"), QStringLiteral("\U0001F60F"),
    QStringLiteral("\U0001F612"), QStringLiteral("\U0001F644"),
    QStringLiteral("\U0001F62C"), QStringLiteral("\U0001F925"),
    QStringLiteral("\U0001F60E"), QStringLiteral("\U0001F913"),
    QStringLiteral("\U0001F9D0"), QStringLiteral("\U0001F615"),
    QStringLiteral("\U0001F61F"), QStringLiteral("\U0001F641"),
    QStringLiteral("\U0001F62E"), QStringLiteral("\U0001F62F"),
    QStringLiteral("\U0001F632"), QStringLiteral("\U0001F633"),
    QStringLiteral("\U0001F97A"), QStringLiteral("\U0001F626"),
    QStringLiteral("\U0001F627"), QStringLiteral("\U0001F628"),
    QStringLiteral("\U0001F630"), QStringLiteral("\U0001F625"),
    QStringLiteral("\U0001F622"), QStringLiteral("\U0001F62D"),
    QStringLiteral("\U0001F631"), QStringLiteral("\U0001F616"),
    QStringLiteral("\U0001F623"), QStringLiteral("\U0001F61E"),
    QStringLiteral("\U0001F613"), QStringLiteral("\U0001F629"),
};

static const QStringList gestures = {
    QStringLiteral("\U0001F44D"), QStringLiteral("\U0001F44E"),
    QStringLiteral("\U0001F44F"), QStringLiteral("\U0001F64C"),
    QStringLiteral("\U0001F91D"), QStringLiteral("\U0001F64F"),
    QStringLiteral("\U0000270D"), QStringLiteral("\U0001F485"),
    QStringLiteral("\U0001F933"), QStringLiteral("\U0001F4AA"),
    QStringLiteral("\U0001F44B"), QStringLiteral("\U0001F91A"),
    QStringLiteral("\U0001F590"), QStringLiteral("\U0000270B"),
    QStringLiteral("\U0001F596"), QStringLiteral("\U0001F44C"),
    QStringLiteral("\U0000270C"), QStringLiteral("\U0001F91E"),
    QStringLiteral("\U0001F91F"), QStringLiteral("\U0001F918"),
    QStringLiteral("\U0001F919"), QStringLiteral("\U0001F448"),
    QStringLiteral("\U0001F449"), QStringLiteral("\U0001F446"),
    QStringLiteral("\U0001F595"), QStringLiteral("\U0001F447"),
    QStringLiteral("\U0000261D"), QStringLiteral("\U0001F44A"),
    QStringLiteral("\U0001F91B"), QStringLiteral("\U0001F91C"),
    QStringLiteral("\U0001F90F"), QStringLiteral("\U0001F9B5"),
};

static const QStringList hearts = {
    QStringLiteral("\U00002764"), QStringLiteral("\U0001F9E1"),
    QStringLiteral("\U0001F49B"), QStringLiteral("\U0001F49A"),
    QStringLiteral("\U0001F499"), QStringLiteral("\U0001F49C"),
    QStringLiteral("\U0001F5A4"), QStringLiteral("\U0001F90D"),
    QStringLiteral("\U0001F90E"), QStringLiteral("\U0001F494"),
    QStringLiteral("\U00002763"), QStringLiteral("\U0001F495"),
    QStringLiteral("\U0001F49E"), QStringLiteral("\U0001F493"),
    QStringLiteral("\U0001F497"), QStringLiteral("\U0001F496"),
    QStringLiteral("\U0001F498"), QStringLiteral("\U0001F49D"),
    QStringLiteral("\U0001F49F"), QStringLiteral("\U0001F48C"),
    QStringLiteral("\U0001F4AF"), QStringLiteral("\U0001F4A2"),
    QStringLiteral("\U0001F4A5"), QStringLiteral("\U0001F4AB"),
};

static const QStringList nature = {
    QStringLiteral("\U0001F436"), QStringLiteral("\U0001F431"),
    QStringLiteral("\U0001F42D"), QStringLiteral("\U0001F439"),
    QStringLiteral("\U0001F430"), QStringLiteral("\U0001F98A"),
    QStringLiteral("\U0001F43B"), QStringLiteral("\U0001F43C"),
    QStringLiteral("\U0001F428"), QStringLiteral("\U0001F42F"),
    QStringLiteral("\U0001F981"), QStringLiteral("\U0001F42E"),
    QStringLiteral("\U0001F437"), QStringLiteral("\U0001F438"),
    QStringLiteral("\U0001F435"), QStringLiteral("\U0001F648"),
    QStringLiteral("\U0001F649"), QStringLiteral("\U0001F64A"),
    QStringLiteral("\U0001F412"), QStringLiteral("\U0001F414"),
    QStringLiteral("\U0001F427"), QStringLiteral("\U0001F426"),
    QStringLiteral("\U0001F986"), QStringLiteral("\U0001F985"),
    QStringLiteral("\U0001F333"), QStringLiteral("\U0001F334"),
    QStringLiteral("\U0001F335"), QStringLiteral("\U0001F33B"),
    QStringLiteral("\U0001F337"), QStringLiteral("\U0001F339"),
    QStringLiteral("\U0001F33A"), QStringLiteral("\U0001F338"),
};

static const QStringList food = {
    QStringLiteral("\U0001F34E"), QStringLiteral("\U0001F34A"),
    QStringLiteral("\U0001F34B"), QStringLiteral("\U0001F34C"),
    QStringLiteral("\U0001F349"), QStringLiteral("\U0001F347"),
    QStringLiteral("\U0001F353"), QStringLiteral("\U0001F352"),
    QStringLiteral("\U0001F351"), QStringLiteral("\U0001F34D"),
    QStringLiteral("\U0001F965"), QStringLiteral("\U0001F951"),
    QStringLiteral("\U0001F355"), QStringLiteral("\U0001F354"),
    QStringLiteral("\U0001F35F"), QStringLiteral("\U0001F32E"),
    QStringLiteral("\U0001F32F"), QStringLiteral("\U0001F37F"),
    QStringLiteral("\U0001F366"), QStringLiteral("\U0001F370"),
    QStringLiteral("\U0001F382"), QStringLiteral("\U0001F36B"),
    QStringLiteral("\U0001F36D"), QStringLiteral("\U0001F36A"),
    QStringLiteral("\U00002615"), QStringLiteral("\U0001F375"),
    QStringLiteral("\U0001F37A"), QStringLiteral("\U0001F377"),
    QStringLiteral("\U0001F379"), QStringLiteral("\U0001F378"),
    QStringLiteral("\U0001F376"), QStringLiteral("\U0001F37E"),
};

static const QStringList objects = {
    QStringLiteral("\U0001F4E7"), QStringLiteral("\U0001F4E8"),
    QStringLiteral("\U0001F4E9"), QStringLiteral("\U0001F4E4"),
    QStringLiteral("\U0001F4E5"), QStringLiteral("\U0001F4EC"),
    QStringLiteral("\U0001F4ED"), QStringLiteral("\U0001F4EE"),
    QStringLiteral("\U0001F4DD"), QStringLiteral("\U0001F4C4"),
    QStringLiteral("\U0001F4CB"), QStringLiteral("\U0001F4C5"),
    QStringLiteral("\U0001F4C6"), QStringLiteral("\U0001F4C7"),
    QStringLiteral("\U0001F4C8"), QStringLiteral("\U0001F4C9"),
    QStringLiteral("\U0001F512"), QStringLiteral("\U0001F513"),
    QStringLiteral("\U0001F510"), QStringLiteral("\U0001F511"),
    QStringLiteral("\U0001F4A1"), QStringLiteral("\U0001F4BB"),
    QStringLiteral("\U00002328"), QStringLiteral("\U0001F4F1"),
    QStringLiteral("\U0001F4F7"), QStringLiteral("\U0001F4F9"),
    QStringLiteral("\U0001F3A4"), QStringLiteral("\U0001F3B5"),
    QStringLiteral("\U0001F3B6"), QStringLiteral("\U0001F514"),
    QStringLiteral("\U0001F389"), QStringLiteral("\U0001F388"),
};

static const QStringList symbols = {
    QStringLiteral("\U00002705"), QStringLiteral("\U0000274C"),
    QStringLiteral("\U00002753"), QStringLiteral("\U00002757"),
    QStringLiteral("\U0001F4A4"), QStringLiteral("\U0001F4AC"),
    QStringLiteral("\U0001F4AD"), QStringLiteral("\U0001F6AB"),
    QStringLiteral("\U000026A0"), QStringLiteral("\U00002B50"),
    QStringLiteral("\U0001F31F"), QStringLiteral("\U00002728"),
    QStringLiteral("\U0001F525"), QStringLiteral("\U0001F4A8"),
    QStringLiteral("\U0001F4A7"), QStringLiteral("\U0001F30A"),
    QStringLiteral("\U00002600"), QStringLiteral("\U0001F324"),
    QStringLiteral("\U00002601"), QStringLiteral("\U0001F327"),
    QStringLiteral("\U000026A1"), QStringLiteral("\U00002744"),
    QStringLiteral("\U0001F308"), QStringLiteral("\U0001F315"),
    QStringLiteral("\U0001F680"), QStringLiteral("\U00002708"),
    QStringLiteral("\U0001F3E0"), QStringLiteral("\U0001F30D"),
    QStringLiteral("\U0001F3C6"), QStringLiteral("\U0001F3C5"),
    QStringLiteral("\U0001F947"), QStringLiteral("\U0001F948"),
};

// ── EmojiPicker implementation ──────────────────────────────────────

EmojiPicker::EmojiPicker(QWidget *parent)
    : QWidget(parent, Qt::Popup)
{
    setFixedSize(310, 340);

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(4, 4, 4, 4);
    layout->setSpacing(0);

    auto *tabs = new QTabWidget(this);
    tabs->setTabPosition(QTabWidget::South);
    tabs->setDocumentMode(true);

    tabs->addTab(buildCategoryPage(smileys),  QStringLiteral("\U0001F600"));
    tabs->addTab(buildCategoryPage(gestures), QStringLiteral("\U0001F44B"));
    tabs->addTab(buildCategoryPage(hearts),   QStringLiteral("\U00002764"));
    tabs->addTab(buildCategoryPage(nature),   QStringLiteral("\U0001F43E"));
    tabs->addTab(buildCategoryPage(food),     QStringLiteral("\U0001F354"));
    tabs->addTab(buildCategoryPage(objects),  QStringLiteral("\U0001F4E7"));
    tabs->addTab(buildCategoryPage(symbols),  QStringLiteral("\U00002B50"));

    layout->addWidget(tabs);
}

QWidget *EmojiPicker::buildCategoryPage(const QStringList &emojis)
{
    auto *scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    scroll->setFrameShape(QFrame::NoFrame);

    auto *page = new QWidget(scroll);
    auto *grid = new QGridLayout(page);
    grid->setSpacing(2);
    grid->setContentsMargins(2, 2, 2, 2);

    for (int i = 0; i < emojis.size(); ++i) {
        auto *btn = new QPushButton(emojis[i], page);
        btn->setFixedSize(BTN_SIZE, BTN_SIZE);
        btn->setFlat(true);
        btn->setCursor(Qt::PointingHandCursor);
        btn->setStyleSheet(QStringLiteral(
            "QPushButton { font-size: 18px; border: none; border-radius: 4px; }"
            "QPushButton:hover { background: palette(midlight); }"));
        connect(btn, &QPushButton::clicked, this, [this, emoji = emojis[i]]() {
            emit emojiSelected(emoji);
            close();
        });
        grid->addWidget(btn, i / COLUMNS, i % COLUMNS);
    }

    scroll->setWidget(page);
    return scroll;
}

void EmojiPicker::showRelativeTo(QWidget *anchor)
{
    QPoint pos = anchor->mapToGlobal(QPoint(0, 0));
    pos.setY(pos.y() - height() - 4);
    // Keep within screen bounds
    QRect screen = QApplication::primaryScreen()->availableGeometry();
    if (pos.x() + width() > screen.right())
        pos.setX(screen.right() - width());
    if (pos.x() < screen.left())
        pos.setX(screen.left());
    if (pos.y() < screen.top())
        pos = anchor->mapToGlobal(QPoint(0, anchor->height() + 4));
    move(pos);
    show();
}
