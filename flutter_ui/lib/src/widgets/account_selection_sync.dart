/*
 * account_selection_sync.dart
 *
 * Watches account config so there is always a valid selected account when any exist.
 */

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../providers/account_selection_flow.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../providers/message_sort_persist.dart';
import '../providers/session_state.dart';
import '../rust/tagliacarte_api.dart';

/// Ensures [selectedAccountIdProvider] tracks the account list (add / delete / invalid id).
class AccountSelectionSync extends ConsumerWidget {
  const AccountSelectionSync({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Ensure [AccountMailModelsNotifier] exists and `frbSessionStart` is subscribed even when
    // Home was replaced (e.g. empty state → Settings) so session `accountConnectionChanged`
    // events are not dropped.
    ref.read(accountMailModelsProvider.notifier);
    ref.listen<MailFoldersState>(foldersProvider, (
      MailFoldersState? previous,
      MailFoldersState next,
    ) {
      if (next.folders.isEmpty) {
        return;
      }
      if (ref.read(selectedFolderProvider) != null) {
        return;
      }
      ensureSelectedFolderForCurrentAccount(ref);
    });
    final String? folderWatch = ref.watch(selectedFolderProvider);
    final String? accountWatch = ref.watch(selectedAccountIdProvider);
    final String sortWatch = messageListSortSymbolic(
      ref.watch(messageSortFieldProvider),
      ref.watch(messageSortAscendingProvider),
    );
    if (accountWatch != null && folderWatch != null) {
      final SessionFolderParams fp = SessionFolderParams(
        accountId: accountWatch,
        folderName: folderWatch,
        messageListSort: sortWatch,
      );
      ref.listen<FolderListVm>(folderMailboxListProvider(fp), (
        FolderListVm? previous,
        FolderListVm next,
      ) {
        trySelectDefaultMessageFromFolderListVm(ref, fp, next);
      });
    }
    ref.listen<AsyncValue<AppSettingsConfig>>(accountsConfigProvider, (
      AsyncValue<AppSettingsConfig>? previous,
      AsyncValue<AppSettingsConfig> next,
    ) {
      next.whenData((AppSettingsConfig config) {
        applyMessageListSortFromConfig(ref, config.messageListSort);
        syncAccountSelectionFromConfig(ref, config);
      });
    });
    return child;
  }
}
