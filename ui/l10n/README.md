# Tagliacarte UI translations

Only **translation sources** (`.ts`) live in this directory. Message **keys** are simple lowercase English with dot separation (e.g. `accounts.add_imap`, `settings.folder_poll.every_minute`). Translation text may use **ICU MessageFormat** where needed.

- **lupdate** (target `tagliacarte_ui_lupdate` or `update_translations`) scans C++ for `TR(…)` / `QCoreApplication::translate("Tagliacarte", …)` and updates `.ts` files here.
- **lrelease** runs as part of the normal build: it compiles `.ts` → `.qm` in the **build tree** (`ui/build/l10n/`), then the build copies `.qm` into the app (e.g. `Resources/translations` on macOS). Compiled `.qm` are never stored in this source directory.
- The app loads `.qm` at runtime from the deployed path; if a translation is missing, it falls back to English.

Locales: `en`, `fr`, `de`, `es`, `it`, `pt`, `el`, `ru`, `zh_CN`, `ja`.
