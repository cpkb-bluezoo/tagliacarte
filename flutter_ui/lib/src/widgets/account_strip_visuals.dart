/*
 * account_strip_visuals.dart
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

/// Web-safe muted backgrounds (light theme) and matching readable foregrounds.
/// Pairs are indexed by [accountColorIndex].
const List<int> _lightBg = <int>[
  0xFFCCCCCC, 0xFFCCCC99, 0xFFCCFFCC, 0xFFCCFFFF, 0xFFCC99CC, 0xFFFFCCCC,
  0xFFFFFFCC, 0xFF99CCCC, 0xFF99CC99, 0xFFCCCCFF, 0xFFFFCC99, 0xFFCCFF99,
  0xFFFF99CC, 0xFF99CCFF, 0xFFCC99FF, 0xFF99FFCC,
];

const List<int> _lightFg = <int>[
  0xFF333333, 0xFF333333, 0xFF333333, 0xFF333333, 0xFF333333, 0xFF333333,
  0xFF333333, 0xFF333333, 0xFF333333, 0xFF333333, 0xFF333333, 0xFF333333,
  0xFF333333, 0xFF333333, 0xFF333333, 0xFF333333,
];

/// Darker web-safe muted fills for dark theme.
const List<int> _darkBg = <int>[
  0xFF333366, 0xFF336633, 0xFF663333, 0xFF666633, 0xFF663366, 0xFF336666,
  0xFF666666, 0xFF333399, 0xFF339966, 0xFF993333, 0xFF996633, 0xFF669933,
  0xFF993366, 0xFF336699, 0xFF663399, 0xFF339933,
];

const List<int> _darkFg = <int>[
  0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8,
  0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8,
  0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8, 0xFFE8E8E8,
];

int accountColorIndex(String accountId) {
  int h = 0;
  for (final int u in accountId.codeUnits) {
    h = (h * 31 + u) & 0x7fffffff;
  }
  return h % _lightBg.length;
}

Color accountStripBackgroundColor(
  String accountId,
  Brightness brightness,
) {
  final int i = accountColorIndex(accountId);
  final int argb = brightness == Brightness.dark ? _darkBg[i] : _lightBg[i];
  return Color(argb);
}

Color accountStripForegroundColor(
  String accountId,
  Brightness brightness,
) {
  final int i = accountColorIndex(accountId);
  final int argb = brightness == Brightness.dark ? _darkFg[i] : _lightFg[i];
  return Color(argb);
}

/// First extended grapheme cluster, or empty if none.
String _firstGrapheme(String s) {
  final Characters cs = s.characters;
  if (cs.isEmpty) {
    return '';
  }
  return cs.first;
}

/// Display name is [label] per `docs/ui-decisions.md`.
String accountStripInitials(String label, String? email) {
  final String trimmed = label.trim();
  if (trimmed.isNotEmpty) {
    final List<String> words = trimmed
        .split(RegExp(r'\s+'))
        .where((String w) => w.isNotEmpty)
        .toList();
    if (words.length == 1) {
      final String g = _firstGrapheme(words.first);
      return g.isEmpty ? '?' : g;
    }
    final String a = _firstGrapheme(words.first);
    final String b = _firstGrapheme(words.last);
    if (a.isEmpty && b.isEmpty) {
      return '?';
    }
    if (b.isEmpty) {
      return a.isEmpty ? '?' : a;
    }
    if (a.isEmpty) {
      return b;
    }
    return '$a$b';
  }
  final String? em = email?.trim();
  if (em != null && em.isNotEmpty) {
    final String g = _firstGrapheme(em);
    return g.isEmpty ? '?' : g;
  }
  return '?';
}
