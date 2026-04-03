/*
 * mail_layout.dart
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

import 'dart:math' as math;

/// Width reserved for account rail + folder column + minimum list + gap (approx).
const double kMailDesktopMinWidth = 920;

// --- Desktop resizable panes (see [HomeScreen] non-compact layout) ---

/// SharedPreferences: folder column width in logical pixels.
const String kPrefDesktopFolderPaneWidth =
    'tagliacarte.view.desktop_folder_pane_width';

/// SharedPreferences: fraction of the area below the mail toolbar for the
/// message list when inline detail is on (0–1).
const String kPrefDesktopListPaneFraction =
    'tagliacarte.view.desktop_list_pane_fraction';

/// Default folder pane width when no preference is stored.
const double kMailDesktopDefaultFolderPaneWidth = 240;

/// Hit target and visual line for pane splitters.
const double kMailDesktopSplitterHitSize = 6;

/// Minimum folder column width.
const double kMailDesktopMinFolderWidth = 160;

/// Minimum width for the main column (list ± detail).
const double kMailDesktopMinMainRestWidth = 360;

/// Minimum height for the message list in inline-detail desktop layout.
const double kMailDesktopMinListHeight = 120;

/// Minimum height for the message detail in inline-detail desktop layout.
const double kMailDesktopMinDetailHeight = 120;

/// When true, use hamburger + drawer and single-column mail. When false, persistent
/// rail + folder column + (list | list+detail below).
bool mailLayoutIsCompact(double width, double height) {
  if (width >= kMailDesktopMinWidth) {
    return false;
  }
  final double longest = math.max(width, height);
  final double shortest = math.min(width, height);
  // Large windows (e.g. near-square on a big display) still get desktop layout.
  if (longest >= 1180 && shortest >= 640 && width >= 800) {
    return false;
  }
  return true;
}
