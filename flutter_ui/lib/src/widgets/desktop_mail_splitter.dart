/*
 * desktop_mail_splitter.dart
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

import '../layout/mail_layout.dart';

/// Draggable boundary between two panes (desktop mail layout).
class DesktopMailSplitter extends StatelessWidget {
  const DesktopMailSplitter({
    super.key,
    required this.axis,
    required this.onDragDelta,
    this.onDragEnd,
  });

  final Axis axis;
  final ValueChanged<double> onDragDelta;
  final VoidCallback? onDragEnd;

  @override
  Widget build(BuildContext context) {
    final bool horizontal = axis == Axis.horizontal;
    final Color lineColor = Theme.of(context).dividerColor;

    return MouseRegion(
      cursor: horizontal
          ? SystemMouseCursors.resizeColumn
          : SystemMouseCursors.resizeRow,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onHorizontalDragUpdate:
            horizontal ? (DragUpdateDetails d) => onDragDelta(d.delta.dx) : null,
        onVerticalDragUpdate:
            horizontal ? null : (DragUpdateDetails d) => onDragDelta(d.delta.dy),
        onHorizontalDragEnd:
            horizontal ? (_) => onDragEnd?.call() : null,
        onVerticalDragEnd:
            horizontal ? null : (_) => onDragEnd?.call(),
        child: SizedBox(
          width: horizontal ? kMailDesktopSplitterHitSize : double.infinity,
          height: horizontal ? double.infinity : kMailDesktopSplitterHitSize,
          child: Center(
            child: Container(
              width: horizontal ? 1 : double.infinity,
              height: horizontal ? double.infinity : 1,
              color: lineColor,
            ),
          ),
        ),
      ),
    );
  }
}
