/*
 * mail_location_persist.dart
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

import '../rust/tagliacarte_api.dart';
import 'app_state.dart';

String? _lastPersistedStore;
String? _lastPersistedFolder;
String? _lastPersistedMessageId;

AppAccount? _accountById(List<AppAccount> accounts, String? id) {
  if (id == null) {
    return null;
  }
  for (final AppAccount a in accounts) {
    if (a.id == id) {
      return a;
    }
  }
  return null;
}

/// Writes [selectedAccountIdProvider], [selectedFolderProvider], and
/// [selectedMessageProvider] into `config.xml` via Rust (JSON round-trip).
///
/// Does not bump [settingsRevisionProvider] to avoid resetting the mail UI
/// on save.
Future<void> persistMailLocation(WidgetRef ref) async {
  final AppSettingsConfig? base = ref.read(accountsConfigProvider).valueOrNull;
  if (base == null) {
    return;
  }
  final String? store = ref.read(selectedAccountIdProvider);
  final String? folder = ref.read(selectedFolderProvider);
  final String? messageId = ref.read(selectedMessageProvider);
  if (store == _lastPersistedStore &&
      folder == _lastPersistedFolder &&
      messageId == _lastPersistedMessageId) {
    return;
  }
  final AppAccount? acc = _accountById(base.accounts, store);
  if (base.selectedStoreId == store &&
      acc != null &&
      acc.lastFolder == folder &&
      acc.lastMessageId == messageId) {
    _lastPersistedStore = store;
    _lastPersistedFolder = folder;
    _lastPersistedMessageId = messageId;
    return;
  }
  final List<AppAccount> nextAccounts = List<AppAccount>.from(base.accounts);
  if (store != null) {
    final int i = nextAccounts.indexWhere((AppAccount a) => a.id == store);
    if (i >= 0) {
      nextAccounts[i] = nextAccounts[i].copyWith(
        lastFolder: folder,
        lastMessageId: messageId,
      );
    }
  }
  final TagliacarteApi api = ref.read(tagliacarteApiProvider);
  await api.saveConfig(
    base.copyWith(
      selectedStoreId: store,
      accounts: nextAccounts,
    ),
  );
  _lastPersistedStore = store;
  _lastPersistedFolder = folder;
  _lastPersistedMessageId = messageId;
}
