#ifndef TR_H
#define TR_H

#include <QCoreApplication>

/** Translation helper: context is always "Tagliacarte", keys are lowercase with dots (e.g. compose.title). */
inline QString TR(const char *key) {
    return QCoreApplication::translate("Tagliacarte", key);
}

/** Plural form: pass count (n); key must have plural forms in .ts (numerus="yes"). %n in translation is replaced by n. */
inline QString TR_N(const char *key, int n) {
    return QCoreApplication::translate("Tagliacarte", key, nullptr, n);
}

#endif // TR_H
