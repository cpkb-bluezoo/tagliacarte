/*
 * folder_display.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

import 'package:flutter/widgets.dart';

import '../l10n/app_localizations.dart';

/// Localised folder label for UI (e.g. INBOX → "Inbox" in English).
String folderDisplayName(BuildContext context, String folder) {
  if (folder.toUpperCase() == 'INBOX') {
    return AppLocalizations.of(context).folderInbox;
  }
  return folder;
}
