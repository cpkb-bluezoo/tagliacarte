#ifndef CONFIG_H
#define CONFIG_H

#include <QString>
#include <QList>
#include <QMap>
#include <Qt>

// Config under ~/.tagliacarte/config.xml. Core fields on <store>; store-specific data in <param key="..." value="..."/>.
struct StoreEntry {
    QString id;
    QString type;
    QString displayName;
    QString emailAddress;  // From address; also NIP-05 for Nostr
    QString picture;       // Optional; for future use
    QMap<QString, QString> params;  // Store-specific: hostname, path, username, port, security, transport*, imap*, etc.
};

struct Config {
    QList<StoreEntry> stores;
    QString lastSelectedStoreId;
    bool useKeychain = false;  // true = system keychain, false = encrypted file
    QString dateFormat;        // empty = locale default; otherwise Qt date format string for message list
    int resourceLoadPolicy = 1; // 0 = no resource loading, 1 = cid: only (default), 2 = external URLs
    // Message list
    QString messageListColumnOrder;   // e.g. "0,1,2" (from, subject, date)
    QString messageListColumnWidths;  // e.g. "120,0,80" (0 = stretch)
    int messageListSortColumn = 2;   // default: date
    Qt::SortOrder messageListSortOrder = Qt::AscendingOrder;  // ascending = oldest first, newest at bottom
    // Composing
    QString forwardMode;       // "inline", "embedded", "attachment"
    bool quoteUsePrefix = true;
    QString quotePrefix;       // e.g. "> "
    QString replyPosition;    // "before" or "after" (reply text before or after quoted text)
};

QString param(const StoreEntry &e, const char *key);
int paramInt(const StoreEntry &e, const char *key, int defaultVal);
QString storeHostOrPath(const StoreEntry &e);

QString tagliacarteConfigDir();
QString tagliacarteConfigPath();

Config loadConfig();
void saveConfig(const Config &c);

#endif // CONFIG_H
