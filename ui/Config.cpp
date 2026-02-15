#include "Config.h"
#include <QDir>
#include <QFile>
#include <QSaveFile>
#include <QStandardPaths>
#include <QXmlStreamReader>
#include <QXmlStreamWriter>

QString param(const StoreEntry &e, const char *key) {
    return e.params.value(QLatin1String(key));
}

int paramInt(const StoreEntry &e, const char *key, int defaultVal) {
    QString v = e.params.value(QLatin1String(key));
    return v.isEmpty() ? defaultVal : v.toInt();
}

// Hostname for IMAP/POP3, path for Maildir/Nostr/Matrix/mbox
QString storeHostOrPath(const StoreEntry &e) {
    if (e.type == QLatin1String("imap") || e.type == QLatin1String("pop3")) {
        return param(e, "hostname");
    }
    return param(e, "path");
}

QString tagliacarteConfigDir() {
    QString path = QStandardPaths::writableLocation(QStandardPaths::HomeLocation) + QStringLiteral("/.tagliacarte");
    QDir().mkpath(path);
    return path;
}

QString tagliacarteConfigPath() {
    return tagliacarteConfigDir() + QStringLiteral("/config.xml");
}

Config loadConfig() {
    Config c;
    QFile f(tagliacarteConfigPath());
    if (!f.open(QIODevice::ReadOnly)) {
        return c;
    }
    QXmlStreamReader r(&f);
    auto attr = [](const QXmlStreamAttributes &a, const char *kebab, const char *oldCamel = nullptr) {
        QString v = a.value(QLatin1String(kebab)).toString();
        if (v.isEmpty() && oldCamel) {
            v = a.value(QLatin1String(oldCamel)).toString();
        }
        return v;
    };
    while (!r.atEnd()) {
        r.readNext();
        if (r.isStartElement()) {
            if (r.name() == QLatin1String("selected-store")) {
                c.lastSelectedStoreId = r.attributes().value(QLatin1String("id")).toString().trimmed();
            } else if (r.name() == QLatin1String("lastSelectedStoreId")) {
                if (c.lastSelectedStoreId.isEmpty()) {
                    c.lastSelectedStoreId = r.readElementText().trimmed();
                } else {
                    r.readElementText();
                }
            } else if (r.name() == QLatin1String("store")) {
                const QXmlStreamAttributes a = r.attributes();
                StoreEntry e;
                e.id = a.value(QLatin1String("id")).toString();
                e.type = a.value(QLatin1String("type")).toString();
                e.displayName = attr(a, "display-name", "displayName");
                e.emailAddress = attr(a, "email-address", "emailAddress");
                e.picture = attr(a, "picture", "picture");
                // Backward compat: nip-05 -> emailAddress if emailAddress empty
                QString nip05Val = attr(a, "nip-05", "nip05");
                if (!nip05Val.isEmpty() && e.emailAddress.isEmpty()) {
                    e.emailAddress = nip05Val;
                }
                // Old-style attributes -> params
                QString host = attr(a, "hostname", "path");
                if (!host.isEmpty()) {
                    e.params[QStringLiteral("hostname")] = host;
                }
                QString pathVal = a.value(QLatin1String("path")).toString();
                if (!pathVal.isEmpty()) {
                    e.params[QStringLiteral("path")] = pathVal;
                }
                QString u = attr(a, "username", "user-id");
                if (u.isEmpty()) {
                    u = attr(a, "userId");
                }
                if (!u.isEmpty()) {
                    e.params[QStringLiteral("username")] = u;
                }
                QString uid = attr(a, "user-id", "userId");
                if (!uid.isEmpty()) {
                    e.params[QStringLiteral("userId")] = uid;
                }
                if (!attr(a, "key-path", "keyPath").isEmpty()) {
                    e.params[QStringLiteral("keyPath")] = attr(a, "key-path", "keyPath");
                }
                if (!attr(a, "access-token", "accessToken").isEmpty()) {
                    e.params[QStringLiteral("accessToken")] = attr(a, "access-token", "accessToken");
                }
                int p = attr(a, "port").toInt();
                if (p > 0) {
                    e.params[QStringLiteral("port")] = QString::number(p);
                }
                QString sec = attr(a, "security");
                if (sec == QLatin1String("none")) {
                    e.params[QStringLiteral("security")] = QStringLiteral("0");
                } else if (sec == QLatin1String("starttls")) {
                    e.params[QStringLiteral("security")] = QStringLiteral("1");
                } else if (sec == QLatin1String("ssl")) {
                    e.params[QStringLiteral("security")] = QStringLiteral("2");
                }
                int pollMin = attr(a, "poll-interval-minutes").toInt();
                if (pollMin == 1 || pollMin == 5 || pollMin == 10 || pollMin == 60) {
                    e.params[QStringLiteral("imapPollSeconds")] = QString::number(pollMin * 60);
                }
                if (attr(a, "deletion") == QLatin1String("move_to_trash")) {
                    e.params[QStringLiteral("imapDeletion")] = QStringLiteral("1");
                }
                QString tf = attr(a, "trash-folder");
                if (!tf.isEmpty()) {
                    e.params[QStringLiteral("imapTrashFolder")] = tf;
                }
                int idleSec = attr(a, "idle-seconds").toInt();
                if (idleSec == 30 || idleSec == 60 || idleSec == 300) {
                    e.params[QStringLiteral("imapIdleSeconds")] = QString::number(idleSec);
                }
                while (!r.atEnd()) {
                    r.readNext();
                    if (r.isEndElement() && r.name() == QLatin1String("store")) {
                        break;
                    }
                    if (r.isStartElement() && r.name() == QLatin1String("transport")) {
                        const QXmlStreamAttributes ta = r.attributes();
                        if (ta.value(QLatin1String("type")).toString() == QLatin1String("smtp")) {
                            QString th = ta.value(QLatin1String("hostname")).toString();
                            if (!th.isEmpty()) {
                                e.params[QStringLiteral("transportHostname")] = th;
                            }
                            int tp = ta.value(QLatin1String("port")).toString().toInt();
                            e.params[QStringLiteral("transportPort")] = QString::number(tp > 0 ? tp : 586);
                            QString tu = ta.value(QLatin1String("username")).toString();
                            if (!tu.isEmpty()) {
                                e.params[QStringLiteral("transportUsername")] = tu;
                            }
                            QString tsec = ta.value(QLatin1String("security")).toString();
                            if (tsec == QLatin1String("none")) {
                                e.params[QStringLiteral("transportSecurity")] = QStringLiteral("0");
                            } else if (tsec == QLatin1String("ssl")) {
                                e.params[QStringLiteral("transportSecurity")] = QStringLiteral("2");
                            } else {
                                e.params[QStringLiteral("transportSecurity")] = QStringLiteral("1");
                            }
                            QString tid = ta.value(QLatin1String("id")).toString();
                            if (!tid.isEmpty()) {
                                e.params[QStringLiteral("transportId")] = tid;
                            }
                        }
                    } else if (r.isStartElement() && r.name() == QLatin1String("param")) {
                        QString k = r.attributes().value(QLatin1String("key")).toString();
                        QString v = r.attributes().value(QLatin1String("value")).toString();
                        if (!k.isEmpty()) {
                            e.params[k] = v;
                        }
                    }
                }
                if (!e.id.isEmpty()) {
                    c.stores.append(e);
                }
            } else if (r.name() == QLatin1String("security")) {
                while (!r.atEnd()) {
                    r.readNext();
                    if (r.isEndElement() && r.name() == QLatin1String("security")) {
                        break;
                    }
                    if (r.isStartElement() && r.name() == QLatin1String("credentials")) {
                        QString storage = r.attributes().value(QLatin1String("storage")).toString();
                        c.useKeychain = (storage == QLatin1String("keychain"));
                    }
                }
            } else if (r.name() == QLatin1String("viewing")) {
                while (!r.atEnd()) {
                    r.readNext();
                    if (r.isEndElement() && r.name() == QLatin1String("viewing")) {
                        break;
                    }
                    if (r.isStartElement() && r.name() == QLatin1String("date-format")) {
                        c.dateFormat = r.attributes().value(QLatin1String("value")).toString();
                    } else if (r.isStartElement() && r.name() == QLatin1String("message-list-column-order")) {
                        c.messageListColumnOrder = r.attributes().value(QLatin1String("value")).toString();
                    } else if (r.isStartElement() && r.name() == QLatin1String("message-list-column-widths")) {
                        c.messageListColumnWidths = r.attributes().value(QLatin1String("value")).toString();
                    } else if (r.isStartElement() && r.name() == QLatin1String("message-list-sort-column")) {
                        c.messageListSortColumn = r.attributes().value(QLatin1String("value")).toInt();
                    } else if (r.isStartElement() && r.name() == QLatin1String("message-list-sort-order")) {
                        c.messageListSortOrder = (r.attributes().value(QLatin1String("value")) == QLatin1String("desc"))
                            ? Qt::DescendingOrder : Qt::AscendingOrder;
                    } else if (r.isStartElement() && r.name() == QLatin1String("resource-load-policy")) {
                        c.resourceLoadPolicy = r.attributes().value(QLatin1String("value")).toInt();
                    }
                }
            } else if (r.name() == QLatin1String("composing")) {
                while (!r.atEnd()) {
                    r.readNext();
                    if (r.isEndElement() && r.name() == QLatin1String("composing")) {
                        break;
                    }
                    if (r.isStartElement() && r.name() == QLatin1String("forward-mode")) {
                        c.forwardMode = r.attributes().value(QLatin1String("value")).toString();
                    } else if (r.isStartElement() && r.name() == QLatin1String("quote-use-prefix")) {
                        c.quoteUsePrefix = (r.attributes().value(QLatin1String("value")).toString() == QLatin1String("1"));
                    } else if (r.isStartElement() && r.name() == QLatin1String("quote-prefix")) {
                        c.quotePrefix = r.attributes().value(QLatin1String("value")).toString();
                    } else if (r.isStartElement() && r.name() == QLatin1String("reply-position")) {
                        c.replyPosition = r.attributes().value(QLatin1String("value")).toString();
                    }
                }
            }
        }
    }
    f.close();
    if (c.forwardMode.isEmpty()) {
        c.forwardMode = QStringLiteral("inline");
    }
    if (c.quotePrefix.isEmpty()) {
        c.quotePrefix = QStringLiteral("> ");
    }
    if (c.replyPosition.isEmpty()) {
        c.replyPosition = QStringLiteral("after");
    }
    return c;
}

void saveConfig(const Config &c) {
    QSaveFile f(tagliacarteConfigPath());
    if (!f.open(QIODevice::WriteOnly)) {
        return;
    }
    QXmlStreamWriter w(&f);
    w.setAutoFormatting(true);
    w.writeStartDocument(QStringLiteral("1.0"), true);
    w.writeStartElement(QStringLiteral("tagliacarte"));
    if (!c.lastSelectedStoreId.isEmpty()) {
        w.writeStartElement(QStringLiteral("selected-store"));
        w.writeAttribute(QStringLiteral("id"), c.lastSelectedStoreId);
        w.writeEndElement();
    }
    w.writeStartElement(QStringLiteral("security"));
    w.writeStartElement(QStringLiteral("credentials"));
    w.writeAttribute(QStringLiteral("storage"), c.useKeychain ? QStringLiteral("keychain") : QStringLiteral("file"));
    w.writeEndElement();
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("viewing"));
    if (!c.dateFormat.isEmpty()) {
        w.writeStartElement(QStringLiteral("date-format"));
        w.writeAttribute(QStringLiteral("value"), c.dateFormat);
        w.writeEndElement();
    }
    if (!c.messageListColumnOrder.isEmpty()) {
        w.writeStartElement(QStringLiteral("message-list-column-order"));
        w.writeAttribute(QStringLiteral("value"), c.messageListColumnOrder);
        w.writeEndElement();
    }
    if (!c.messageListColumnWidths.isEmpty()) {
        w.writeStartElement(QStringLiteral("message-list-column-widths"));
        w.writeAttribute(QStringLiteral("value"), c.messageListColumnWidths);
        w.writeEndElement();
    }
    w.writeStartElement(QStringLiteral("message-list-sort-column"));
    w.writeAttribute(QStringLiteral("value"), QString::number(c.messageListSortColumn));
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("message-list-sort-order"));
    w.writeAttribute(QStringLiteral("value"), c.messageListSortOrder == Qt::DescendingOrder ? QStringLiteral("desc") : QStringLiteral("asc"));
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("resource-load-policy"));
    w.writeAttribute(QStringLiteral("value"), QString::number(c.resourceLoadPolicy));
    w.writeEndElement();
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("composing"));
    w.writeStartElement(QStringLiteral("forward-mode"));
    w.writeAttribute(QStringLiteral("value"), c.forwardMode.isEmpty() ? QStringLiteral("inline") : c.forwardMode);
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("quote-use-prefix"));
    w.writeAttribute(QStringLiteral("value"), c.quoteUsePrefix ? QStringLiteral("1") : QStringLiteral("0"));
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("quote-prefix"));
    w.writeAttribute(QStringLiteral("value"), c.quotePrefix.isEmpty() ? QStringLiteral("> ") : c.quotePrefix);
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("reply-position"));
    w.writeAttribute(QStringLiteral("value"), c.replyPosition.isEmpty() ? QStringLiteral("after") : c.replyPosition);
    w.writeEndElement();
    w.writeEndElement();
    w.writeStartElement(QStringLiteral("stores"));
    for (const StoreEntry &e : c.stores) {
        w.writeStartElement(QStringLiteral("store"));
        w.writeAttribute(QStringLiteral("id"), e.id);
        w.writeAttribute(QStringLiteral("type"), e.type);
        w.writeAttribute(QStringLiteral("display-name"), e.displayName);
        if (!e.emailAddress.isEmpty()) {
            w.writeAttribute(QStringLiteral("email-address"), e.emailAddress);
        }
        if (!e.picture.isEmpty()) {
            w.writeAttribute(QStringLiteral("picture"), e.picture);
        }
        for (auto it = e.params.constBegin(); it != e.params.constEnd(); ++it) {
            w.writeStartElement(QStringLiteral("param"));
            w.writeAttribute(QStringLiteral("key"), it.key());
            w.writeAttribute(QStringLiteral("value"), it.value());
            w.writeEndElement();
        }
        w.writeEndElement();
    }
    w.writeEndElement();
    w.writeEndElement();
    w.writeEndDocument();
    f.commit();
}
