/*
 * view_prefs.dart
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

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

const String _kMessageDetailInline = 'tagliacarte.view.message_detail_inline';
const String _kMessageHeadersMinimal = 'tagliacarte.view.message_headers_minimal';

/// Desktop: when true, message body is below the list; when false, open a full route.
final messageDetailInlineProvider =
    StateNotifierProvider<MessageDetailInlineNotifier, bool>(
      (Ref ref) => MessageDetailInlineNotifier(),
    );

class MessageDetailInlineNotifier extends StateNotifier<bool> {
  MessageDetailInlineNotifier() : super(true) {
    _load();
  }

  Future<void> _load() async {
    final SharedPreferences p = await SharedPreferences.getInstance();
    state = p.getBool(_kMessageDetailInline) ?? true;
  }

  Future<void> setInline(bool value) async {
    final SharedPreferences p = await SharedPreferences.getInstance();
    await p.setBool(_kMessageDetailInline, value);
    state = value;
  }
}

/// When true, compact headers (From, To, Date); when false, also show Cc.
final messageHeadersMinimalProvider =
    StateNotifierProvider<MessageHeadersMinimalNotifier, bool>(
      (Ref ref) => MessageHeadersMinimalNotifier(),
    );

class MessageHeadersMinimalNotifier extends StateNotifier<bool> {
  MessageHeadersMinimalNotifier() : super(true) {
    _load();
  }

  Future<void> _load() async {
    final SharedPreferences p = await SharedPreferences.getInstance();
    state = p.getBool(_kMessageHeadersMinimal) ?? true;
  }

  Future<void> setMinimal(bool value) async {
    final SharedPreferences p = await SharedPreferences.getInstance();
    await p.setBool(_kMessageHeadersMinimal, value);
    state = value;
  }
}
