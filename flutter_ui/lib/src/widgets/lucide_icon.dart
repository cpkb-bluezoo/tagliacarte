/*
 * lucide_icon.dart
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

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

/// SVGs copied from the Lucide repository (`../lucide/icons` relative to repo root).
class LucideIcons {
  LucideIcons._();

  static const String menu = 'assets/lucide/menu.svg';
  static const String ellipsisVertical = 'assets/lucide/ellipsis-vertical.svg';
  static const String squarePen = 'assets/lucide/square-pen.svg';
  static const String reply = 'assets/lucide/reply.svg';
  static const String replyAll = 'assets/lucide/reply-all.svg';
  static const String forward = 'assets/lucide/forward.svg';
  static const String trash2 = 'assets/lucide/trash-2.svg';
  static const String ban = 'assets/lucide/ban.svg';
  static const String settings = 'assets/lucide/settings.svg';
  static const String arrowUpDown = 'assets/lucide/arrow-up-down.svg';
  static const String circlePlus = 'assets/lucide/circle-plus.svg';
}

class LucideIcon extends StatelessWidget {
  const LucideIcon(
    this.assetPath, {
    super.key,
    this.size = 22,
    this.color,
  });

  final String assetPath;
  final double size;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final Color c = color ?? Theme.of(context).colorScheme.onSurface;
    return SvgPicture.asset(
      assetPath,
      width: size,
      height: size,
      colorFilter: ColorFilter.mode(c, BlendMode.srcIn),
    );
  }
}
