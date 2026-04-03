/*
 * app_assets.dart
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

import 'package:flutter/foundation.dart' show defaultTargetPlatform, kIsWeb;
import 'package:flutter/material.dart' show TargetPlatform;

/// In-app branded icon (drawer, About). On macOS, `app-icon-macos.svg` is a
/// full-bleed square so system squircle masking does not show a white rim; see
/// `icons/app-icon-macos.svg` at repo root and `scripts/gen-icons.sh`.
String brandedAppIconAssetPath() {
  if (kIsWeb) {
    return 'assets/icons/app-icon.svg';
  }
  switch (defaultTargetPlatform) {
    case TargetPlatform.macOS:
      return 'assets/icons/app-icon-macos.svg';
    default:
      return 'assets/icons/app-icon.svg';
  }
}
