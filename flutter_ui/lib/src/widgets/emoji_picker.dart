/*
 * emoji_picker.dart
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

class EmojiPicker extends StatelessWidget {
  const EmojiPicker({super.key, required this.onSelected});

  final ValueChanged<String> onSelected;

  static const List<String> _emoji = ['😀', '😂', '😍', '👍', '🎉', '🙏'];

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: _emoji
          .map(
            (e) => InkWell(
              onTap: () => onSelected(e),
              child: Text(e, style: const TextStyle(fontSize: 24)),
            ),
          )
          .toList(),
    );
  }
}
