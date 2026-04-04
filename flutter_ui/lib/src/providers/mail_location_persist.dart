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
import '../util/process_log.dart';
import 'app_state.dart';
import 'message_sort_persist.dart';

String? _lastPersistedStore;
String? _lastPersistedFolder;
String? _lastPersistedMessageId;

/// Writes [selectedAccountIdProvider], [selectedFolderProvider], and
/// [selectedMessageProvider] into `config.xml` via Rust (JSON round-trip).
///
/// Does not bump [settingsRevisionProvider] to avoid resetting the mail UI
/// on save.
Future<void> persistMailLocation(WidgetRef ref) async {
  final String? store = ref.read(selectedAccountIdProvider);
  final String? folder = ref.read(selectedFolderProvider);
  final String? messageId = ref.read(selectedMessageProvider);
  if (store == _lastPersistedStore &&
      folder == _lastPersistedFolder &&
      messageId == _lastPersistedMessageId) {
    return;
  }
  final TagliacarteApi api = ref.read(tagliacarteApiProvider);
  // Do not use [accountsConfigProvider].valueOrNull as the save base: it is often still null
  // while the FutureProvider is loading, and it can lag disk after other saves. Read-merge-write
  // from [loadConfig] so last-folder / message-id updates are not skipped or applied to a stale
  // account list.
  late final AppSettingsConfig base;
  try {
    base = await api.loadConfig();
  } catch (e, st) {
    appLogStderr('persistMailLocation: loadConfig failed: $e\n$st');
    final AppSettingsConfig? snap = ref.read(accountsConfigProvider).valueOrNull;
    if (snap == null) {
      return;
    }
    base = snap;
  }
  final List<AppAccount> nextAccounts = List<AppAccount>.from(base.accounts);
  if (store != null) {
    int i = nextAccounts.indexWhere((AppAccount a) => a.id == store);
    if (i < 0) {
      final AppSettingsConfig? snap = ref.read(accountsConfigProvider).valueOrNull;
      if (snap != null) {
        final int j = snap.accounts.indexWhere((AppAccount a) => a.id == store);
        if (j >= 0) {
          nextAccounts
            ..clear()
            ..addAll(snap.accounts);
          i = j;
        }
      }
    }
    if (i >= 0 && i < nextAccounts.length) {
      nextAccounts[i] = nextAccounts[i].copyWith(
        lastFolder: folder,
        lastMessageId: messageId,
      );
    } else {
      appLogStderr(
        'persistMailLocation: account id not in config (store=$store, '
        '${nextAccounts.length} accounts loaded)',
      );
      return;
    }
  }
  // Merge live sort so we do not overwrite toolbar choice with a stale snapshot.
  final String sortSym = messageListSortSymbolic(
    ref.read(messageSortFieldProvider),
    ref.read(messageSortAscendingProvider),
  );
  try {
    await api.saveConfig(
      base.copyWith(
        selectedStoreId: store,
        accounts: nextAccounts,
        messageListSort: sortSym,
      ),
    );
  } catch (e, st) {
    appLogStderr('persistMailLocation: saveConfig failed: $e\n$st');
    return;
  }
  _lastPersistedStore = store;
  _lastPersistedFolder = folder;
  _lastPersistedMessageId = messageId;
}
