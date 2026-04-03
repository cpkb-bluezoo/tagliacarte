/*
 * message_sort_persist.dart
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

import '../models/message_row.dart';
import '../rust/tagliacarte_api.dart';
import 'app_state.dart';

/// Stored in [AppSettingsConfig.messageListSort] (JSON / Rust config).
const String kMessageListSortFromAsc = 'from_asc';
const String kMessageListSortFromDesc = 'from_desc';
const String kMessageListSortSubjectAsc = 'subject_asc';
const String kMessageListSortSubjectDesc = 'subject_desc';
const String kMessageListSortDateAsc = 'date_asc';
const String kMessageListSortDateDesc = 'date_desc';

const String kDefaultMessageListSort = kMessageListSortDateDesc;

String messageListSortSymbolic(MessageSortField field, bool ascending) {
  return switch (field) {
    MessageSortField.byFrom =>
      ascending ? kMessageListSortFromAsc : kMessageListSortFromDesc,
    MessageSortField.bySubject =>
      ascending ? kMessageListSortSubjectAsc : kMessageListSortSubjectDesc,
    MessageSortField.byDate =>
      ascending ? kMessageListSortDateAsc : kMessageListSortDateDesc,
  };
}

/// Returns null if [symbolic] is unknown (caller should keep current sort).
(MessageSortField, bool)? parseMessageListSort(String symbolic) {
  switch (symbolic.trim()) {
    case kMessageListSortFromAsc:
      return (MessageSortField.byFrom, true);
    case kMessageListSortFromDesc:
      return (MessageSortField.byFrom, false);
    case kMessageListSortSubjectAsc:
      return (MessageSortField.bySubject, true);
    case kMessageListSortSubjectDesc:
      return (MessageSortField.bySubject, false);
    case kMessageListSortDateAsc:
      return (MessageSortField.byDate, true);
    case kMessageListSortDateDesc:
      return (MessageSortField.byDate, false);
    // Legacy UI tokens (migrate to snake_case on next save).
    case 'fromAsc':
      return (MessageSortField.byFrom, true);
    case 'fromDesc':
      return (MessageSortField.byFrom, false);
    case 'subAsc':
      return (MessageSortField.bySubject, true);
    case 'subDesc':
      return (MessageSortField.bySubject, false);
    case 'dateAsc':
      return (MessageSortField.byDate, true);
    case 'dateDesc':
      return (MessageSortField.byDate, false);
    default:
      return null;
  }
}

void applyMessageListSortFromConfig(WidgetRef ref, String? symbolic) {
  final String sym =
      symbolic == null || symbolic.trim().isEmpty
          ? kDefaultMessageListSort
          : symbolic.trim();
  final (MessageSortField, bool) parsed =
      parseMessageListSort(sym) ??
      parseMessageListSort(kDefaultMessageListSort)!;
  ref.read(messageSortFieldProvider.notifier).state = parsed.$1;
  ref.read(messageSortAscendingProvider.notifier).state = parsed.$2;
}

Future<void> persistCurrentMessageSort(WidgetRef ref) async {
  final TagliacarteApi api = ref.read(tagliacarteApiProvider);
  final MessageSortField field = ref.read(messageSortFieldProvider);
  final bool asc = ref.read(messageSortAscendingProvider);
  final String sym = messageListSortSymbolic(field, asc);
  final AppSettingsConfig cfg = await api.loadConfig();
  if (cfg.messageListSort == sym) {
    return;
  }
  await api.saveConfig(cfg.copyWith(messageListSort: sym));
}
