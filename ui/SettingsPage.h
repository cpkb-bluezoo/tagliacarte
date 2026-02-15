#ifndef SETTINGSPAGE_H
#define SETTINGSPAGE_H

class QWidget;
class QMainWindow;
class MainController;

/**
 * Build the entire Settings page widget including all tabs
 * (Accounts, Security, Viewing, Composing, About) and connect
 * all signal/slot wiring.
 *
 * Returns the top-level settings QWidget suitable for adding to a QStackedWidget.
 * All account creation/save/delete handlers, keychain migration, and settings
 * persistence are wired up internally.
 */
QWidget *buildSettingsPage(MainController *ctrl, QMainWindow *win, const char *version);

#endif // SETTINGSPAGE_H
