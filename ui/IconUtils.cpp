#include "IconUtils.h"
#include <QFile>
#include <QCoreApplication>
#include <QSvgRenderer>
#include <QPainter>
#include <QImage>
#include <QRectF>
#include <QFont>

QString resolveIconPath(const QString &resourcePath) {
    QFile f(resourcePath);
    if (f.open(QIODevice::ReadOnly)) {
        f.close();
        return resourcePath;
    }
    QString base = QCoreApplication::applicationDirPath();
    int lastSlash = resourcePath.lastIndexOf(QLatin1Char('/'));
    QString name = lastSlash >= 0 ? resourcePath.mid(lastSlash + 1) : resourcePath;
    QStringList tries;
    tries << (base + QLatin1String("/icons/") + name);
#if defined(Q_OS_MACOS) || defined(Q_OS_DARWIN)
    tries << (base + QLatin1String("/../Resources/icons/") + name);
#endif
    tries << (base + QLatin1String("/../ui/icons/") + name);
    tries << (base + QLatin1String("/../../ui/icons/") + name);
    for (const QString &p : tries) {
        if (QFile::exists(p)) {
            return p;
        }
    }
    return resourcePath;
}

QPixmap renderSvgToPixmap(const QString &path, const QColor &color, int w, int h, qreal scaleFactor) {
    QString resolved = resolveIconPath(path);
    QFile f(resolved);
    if (!f.open(QIODevice::ReadOnly)) {
        return QPixmap();
    }
    QByteArray svg = f.readAll();
    f.close();
    svg.replace("currentColor", color.name(QColor::HexRgb).toLatin1());
    QSvgRenderer r(svg);
    if (!r.isValid()) {
        return QPixmap();
    }
    QImage img(w, h, QImage::Format_ARGB32);
    img.fill(Qt::transparent);
    QPainter p(&img);
    if (scaleFactor > 1.0) {
        p.translate(w / 2.0, h / 2.0);
        p.scale(scaleFactor, scaleFactor);
        p.translate(-w / 2.0, -h / 2.0);
    }
    r.render(&p, QRectF(0, 0, w, h));
    p.end();
    return QPixmap::fromImage(img);
}

const char *const storeCircleColours[] = {
    "#6699CC", "#996633", "#339966", "#993366", "#666699", "#CC9933", "#33CC99", "#CC6699"
};
const int storeCircleColourCount = int(sizeof(storeCircleColours) / sizeof(storeCircleColours[0]));

QString storeCircleStyleSheet(int colourIndex) {
    QString hex(storeCircleColours[colourIndex % storeCircleColourCount]);
    return QStringLiteral("QToolButton { border-radius: 20px; background-color: palette(button); color: palette(button-text); font-weight: bold; padding: 0; min-width: 40px; min-height: 40px; border: 2px solid %1; }"
                         "QToolButton:hover { background-color: palette(light); }"
                         "QToolButton:checked { background-color: %1; color: #fff; border-color: %1; }"
                         "QToolButton:checked:hover { background-color: %1; color: #fff; }").arg(hex);
}

QIcon iconFromSvgResource(const QString &path, const QColor &color, int size, qreal scaleFactor) {
    QString resolved = resolveIconPath(path);
    QFile f(resolved);
    if (!f.open(QIODevice::ReadOnly)) {
        return QIcon();
    }
    QByteArray svg = f.readAll();
    f.close();
    svg.replace("currentColor", color.name(QColor::HexRgb).toLatin1());
    QSvgRenderer r(svg);
    if (!r.isValid()) {
        return QIcon();
    }
    auto renderAt = [&r, scaleFactor](int w, int h) {
        QImage img(w, h, QImage::Format_ARGB32);
        img.fill(Qt::transparent);
        QPainter p(&img);
        if (scaleFactor > 1.0) {
            p.translate(w / 2.0, h / 2.0);
            p.scale(scaleFactor, scaleFactor);
            p.translate(-w / 2.0, -h / 2.0);
        }
        r.render(&p, QRectF(0, 0, w, h));
        p.end();
        return QPixmap::fromImage(img);
    };
    QIcon icon;
    QPixmap px1 = renderAt(size, size);
    icon.addPixmap(px1);
    const int size2 = qMin(size * 2, 128);
    QPixmap px2 = renderAt(size2, size2);
    px2.setDevicePixelRatio(2.0);
    icon.addPixmap(px2);
    return icon;
}

QPixmap circularAvatar(const QPixmap &src, int size) {
    QPixmap scaled = src.scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    if (scaled.width() != scaled.height()) {
        int sz = qMin(scaled.width(), scaled.height());
        scaled = scaled.copy((scaled.width() - sz) / 2, (scaled.height() - sz) / 2, sz, sz);
    }
    QPixmap rounded(size, size);
    rounded.fill(Qt::transparent);
    QPainter p(&rounded);
    p.setRenderHint(QPainter::Antialiasing);
    p.setBrush(QBrush(scaled));
    p.setPen(Qt::NoPen);
    p.drawEllipse(0, 0, size, size);
    p.end();
    return rounded;
}

QPixmap letterAvatar(QChar letter, const QColor &bg, int size) {
    QPixmap pix(size, size);
    pix.fill(Qt::transparent);
    QPainter p(&pix);
    p.setRenderHint(QPainter::Antialiasing);
    p.setBrush(bg);
    p.setPen(Qt::NoPen);
    p.drawEllipse(0, 0, size, size);
    p.setPen(Qt::white);
    QFont f = p.font();
    f.setPixelSize(size * 2 / 3);
    f.setBold(true);
    p.setFont(f);
    p.drawText(QRect(0, 0, size, size), Qt::AlignCenter, QString(letter.toUpper()));
    p.end();
    return pix;
}
