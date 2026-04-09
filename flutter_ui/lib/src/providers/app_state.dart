/*
 * app_state.dart
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

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/mail_pending_transfer.dart';
import '../models/subscription_folder_row.dart';
import '../models/message_row.dart';
import '../rust/tagliacarte_api.dart';

final tagliacarteApiProvider = Provider<TagliacarteApi>(
  (_) => const TagliacarteApi(),
);

/// Bumped from Settings after save so [accountsConfigProvider] reloads.
final settingsRevisionProvider = StateProvider<int>((_) => 0);

final accountsConfigProvider =
    FutureProvider.autoDispose<AppSettingsConfig>((Ref ref) async {
      ref.watch(settingsRevisionProvider);
      return ref.read(tagliacarteApiProvider).loadConfig();
    });

final selectedAccountIdProvider = StateProvider<String?>((_) => null);

/// True while [ComposeScreen] is mounted (macOS mail menu defers to no selection).
final composeActiveProvider = StateProvider<bool>((_) => false);
final selectedFolderProvider = StateProvider<String?>((_) => null);

final selectedMessageProvider = StateProvider<String?>((_) => null);

/// Folder names and optional per-folder unread counts (from Rust session events only).
@immutable
class MailFoldersState {
  const MailFoldersState({
    this.folders = const <String>[],
    this.unreadByFolder = const <String, int>{},
    this.folderDisplayLabels = const <String, String>{},
    this.subscriptionAvailable = const <SubscriptionFolderRow>[],
  });

  final List<String> folders;
  final Map<String, int> unreadByFolder;
  /// Lowercase folder id → UI title (Matrix room display / DM peer name).
  final Map<String, String> folderDisplayLabels;
  /// **Available** tab rows (IMAP / NNTP / Matrix); may be empty until refresh.
  final List<SubscriptionFolderRow> subscriptionAvailable;
}

final messageSortFieldProvider = StateProvider<MessageSortField>(
  (_) => MessageSortField.byDate,
);

final messageSortAscendingProvider = StateProvider<bool>((_) => false);

final mailMultiSelectActiveProvider = StateProvider<bool>((_) => false);

final mailSelectedIdsProvider =
    StateNotifierProvider<MailSelectedIdsNotifier, Set<String>>(
      (_) => MailSelectedIdsNotifier(),
    );

class MailSelectedIdsNotifier extends StateNotifier<Set<String>> {
  MailSelectedIdsNotifier() : super(<String>{});

  void enterWith(String id) {
    state = <String>{id};
  }

  void toggle(String id) {
    final Set<String> next = Set<String>.from(state);
    if (next.contains(id)) {
      next.remove(id);
    } else {
      next.add(id);
    }
    state = next;
  }

  void clear() {
    state = <String>{};
  }
}

/// Menu-tagged move/copy (see [MailPendingTransfer]). Replaced when user picks Move/Copy again.
final mailPendingTransferProvider =
    StateProvider<MailPendingTransfer?>((_) => null);

/// Fired when [FolderMailboxListNotifier] detects new messages (SnackBar / OS notify).
@immutable
class NewMailToastSignal {
  const NewMailToastSignal({
    required this.accountLabel,
    required this.folderName,
    required this.countHint,
  });

  final String accountLabel;
  final String folderName;
  final int countHint;
}

final newMailToastSignalProvider = StateProvider<NewMailToastSignal?>(
  (_) => null,
);

