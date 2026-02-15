#ifndef ICONUTILS_H
#define ICONUTILS_H

#include <QString>
#include <QIcon>
#include <QPixmap>
#include <QColor>

// Resolve icon path: try Qt resource first, then filesystem (app dir, bundle Resources, build dir).
QString resolveIconPath(const QString &resourcePath);

// Render SVG from resource to a QPixmap at exact size (no QIcon; we control pixels).
QPixmap renderSvgToPixmap(const QString &path, const QColor &color, int w, int h, qreal scaleFactor = 1.0);

// Web-safe colours for store/account circles.
extern const char *const storeCircleColours[];
extern const int storeCircleColourCount;

// Stylesheet for a store/account circle: unselected = thin border of colour, selected = full background of colour.
QString storeCircleStyleSheet(int colourIndex);

// Load SVG from resource and render as QIcon with currentColor replaced by palette color.
// Renders at size and at 2*size for HiDPI. scaleFactor zooms the graphic (e.g. 1.35 to fill the frame better).
QIcon iconFromSvgResource(const QString &path, const QColor &color, int size = 24, qreal scaleFactor = 1.0);

#endif // ICONUTILS_H
