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
        qDebug() << "loadResource: type=" << type << "url=" << url.toString()
                 << "scheme=" << url.scheme();

        if (m_resourceLoadPolicy == 0) {
            qDebug() << "  blocked (policy=0)";
            return transparentPixel();
        }
        if (url.scheme() == QLatin1String("cid")) {
            if (!m_cidRegistry) {
                qDebug() << "  cid: registry not set";
                return transparentPixel();
            }
            // Extract bare Content-ID from the URL.
            // QUrl may parse cid:foo@bar as authority (due to @), so
            // fall back to stripping the scheme from the string form.
            QString cidKey = url.path();
            if (cidKey.isEmpty()) {
                QString full = url.toString();
                if (full.startsWith(QLatin1String("cid:"))) {
                    cidKey = full.mid(4);
                }
            }
            qDebug() << "  cid: lookup key=" << cidKey
                     << "registry keys=" << m_cidRegistry->keys();

            auto it = m_cidRegistry->constFind(cidKey);
            if (it != m_cidRegistry->constEnd()) {
                QImage img;
                if (img.loadFromData(it.value())) {
                    qDebug() << "  cid: resolved image" << img.size();
                    return img;
                }
                // Not a recognised image format; return raw bytes for Qt to handle
                qDebug() << "  cid: returning raw bytes," << it.value().size() << "bytes";
                return it.value();
            }
            qDebug() << "  cid: NOT FOUND in registry";
            return transparentPixel();
        }
        if (m_resourceLoadPolicy == 2) {
            QString scheme = url.scheme();
            if (scheme == QLatin1String("http") || scheme == QLatin1String("https")) {
                qDebug() << "  external allowed (policy=2)";
                return QTextBrowser::loadResource(type, url);
            }
        }
        // Block anything else (e.g., file:, data:, or http/https when policy == 1)
        qDebug() << "  blocked:" << url.toString();
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
