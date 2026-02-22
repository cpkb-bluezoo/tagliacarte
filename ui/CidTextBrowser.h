#ifndef CIDTEXTBROWSER_H
#define CIDTEXTBROWSER_H

#include <QTextBrowser>
#include <QMap>
#include <QImage>
#include <QUrl>
#include <QDebug>

// QTextBrowser subclass with cid: resource resolution and configurable resource loading policy.
class CidTextBrowser : public QTextBrowser {
public:
    explicit CidTextBrowser(QWidget *parent = nullptr) : QTextBrowser(parent) {}

    // 0 = no resource loading, 1 = cid: only (default), 2 = external URLs allowed
    int resourceLoadPolicy() const { return m_resourceLoadPolicy; }
    void setResourceLoadPolicy(int p) { m_resourceLoadPolicy = p; }

    /** Set the CID registry (owned by EventBridge) for direct cid: URL resolution. */
    void setCidRegistry(const QMap<QString, QByteArray> *reg) { m_cidRegistry = reg; }

protected:
    QVariant loadResource(int type, const QUrl &url) override {
        if (m_resourceLoadPolicy == 0) {
            qDebug() << "[resource] blocked (policy=none):" << url.toString();
            return transparentPixel();
        }
        if (url.scheme() == QLatin1String("cid")) {
            if (!m_cidRegistry) {
                return transparentPixel();
            }
            QString cidKey = url.path();
            if (cidKey.isEmpty()) {
                QString full = url.toString();
                if (full.startsWith(QLatin1String("cid:"))) {
                    cidKey = full.mid(4);
                }
            }

            auto it = m_cidRegistry->constFind(cidKey);
            if (it != m_cidRegistry->constEnd()) {
                QImage img;
                if (img.loadFromData(it.value())) {
                    return img;
                }
                return it.value();
            }
            return transparentPixel();
        }
        if (url.scheme() == QLatin1String("data")) {
            QString s = url.toString();
            int comma = s.indexOf(QLatin1Char(','));
            if (comma > 0) {
                QByteArray decoded = QByteArray::fromBase64(s.mid(comma + 1).toLatin1());
                QImage img;
                if (img.loadFromData(decoded))
                    return img;
            }
            return transparentPixel();
        }
        if (m_resourceLoadPolicy == 2) {
            QString scheme = url.scheme();
            if (scheme == QLatin1String("http") || scheme == QLatin1String("https")) {
                return QTextBrowser::loadResource(type, url);
            }
        }
        qDebug() << "[resource] blocked:" << url.toString();
        return transparentPixel();
    }

private:
    int m_resourceLoadPolicy = 1;
    const QMap<QString, QByteArray> *m_cidRegistry = nullptr;

    static QVariant transparentPixel() {
        QImage img(1, 1, QImage::Format_ARGB32);
        img.fill(Qt::transparent);
        return img;
    }
};

#endif // CIDTEXTBROWSER_H
