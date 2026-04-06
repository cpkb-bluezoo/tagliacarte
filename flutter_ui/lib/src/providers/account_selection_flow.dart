/*
 * account_selection_flow.dart
 *
 * Shared account selection + folder restore (used by HomeScreen and global sync).
 */

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/message_row.dart';
import '../rust/tagliacarte_api.dart';
import 'app_state.dart';
import 'mail_location_persist.dart';
import 'mail_sync.dart';
import 'message_sort_persist.dart';
import 'session_state.dart';

/// Prefer `INBOX` (any case); otherwise first folder (IMAP list already orders INBOX first server-side).
String? defaultMailboxFolder(List<String> folders) {
  if (folders.isEmpty) {
    return null;
  }
  for (final String f in folders) {
    if (f.toUpperCase() == 'INBOX') {
      return f;
    }
  }
  return folders.first;
}

/// When the session folder list appears but nothing is selected (e.g. credentials were just added),
/// pick [AppAccount.lastFolder] if it still exists, else [defaultMailboxFolder].
void ensureSelectedFolderForCurrentAccount(WidgetRef ref) {
  final String? id = ref.read(selectedAccountIdProvider);
  if (id == null) {
    return;
  }
  if (ref.read(selectedFolderProvider) != null) {
    return;
  }
  final List<String> folders = ref.read(foldersProvider).folders;
  if (folders.isEmpty) {
    return;
  }
  final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
  final AppAccount? account = _accountById(cfg?.accounts ?? const <AppAccount>[], id);
  if (account == null) {
    return;
  }
  final String? saved = account.lastFolder;
  final bool useSaved = saved != null && folders.contains(saved);
  final String? pick = useSaved ? saved : defaultMailboxFolder(folders);
  if (pick == null) {
    return;
  }
  _selectFolderProviders(
    ref,
    account,
    pick,
    selectMessageId: useSaved ? account.lastMessageId : null,
  );
}

/// After the message list finishes loading: if nothing selected, apply [AppAccount.lastMessageId]
/// when still valid, else select the **last** row in mailbox order (index `totalCount - 1`).
void trySelectDefaultMessageFromFolderListVm(
  WidgetRef ref,
  SessionFolderParams fp,
  FolderListVm next,
) {
  if (ref.read(selectedAccountIdProvider) != fp.accountId) {
    return;
  }
  if (ref.read(selectedFolderProvider) != fp.folderName) {
    return;
  }
  if (ref.read(selectedMessageProvider) != null) {
    return;
  }
  if (!next.ready || next.totalCount == 0) {
    return;
  }
  final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
  final AppAccount? account =
      _accountById(cfg?.accounts ?? const <AppAccount>[], fp.accountId);
  final String? savedMid = account?.lastMessageId;
  if (savedMid != null && savedMid.isNotEmpty && next.containsId(savedMid)) {
    ref.read(selectedMessageProvider.notifier).state = savedMid;
    unawaited(persistMailLocation(ref));
    return;
  }
  for (int i = next.totalCount - 1; i >= 0; i--) {
    final MessageListRow? r = next.rowAtDataIndex(i);
    if (r != null) {
      ref.read(selectedMessageProvider.notifier).state = r.id;
      unawaited(persistMailLocation(ref));
      return;
    }
  }
}

/// Best-effort wait until the session reports at least one folder for [accountId].
Future<void> waitForSessionFoldersBrief(WidgetRef ref, String accountId) async {
  const Duration step = Duration(milliseconds: 50);
  const int maxSteps = 300;
  for (int i = 0; i < maxSteps; i++) {
    if (ref.read(selectedAccountIdProvider) != accountId) {
      return;
    }
    final List<String> folders =
        ref.read(accountMailModelsProvider)[accountId]?.folders ??
            const <String>[];
    if (folders.isNotEmpty) {
      return;
    }
    await Future<void>.delayed(step);
  }
}

void _clearMultiSelect(WidgetRef ref) {
  ref.read(mailMultiSelectActiveProvider.notifier).state = false;
  ref.read(mailSelectedIdsProvider.notifier).clear();
}

void _selectFolderProviders(
  WidgetRef ref,
  AppAccount account,
  String? folder, {
  String? selectMessageId,
}) {
  _clearMultiSelect(ref);
  ref.read(mailPendingTransferProvider.notifier).state = null;
  if (folder == null) {
    ref.read(selectedFolderProvider.notifier).state = null;
    ref.read(selectedMessageProvider.notifier).state = null;
    unawaited(persistMailLocation(ref));
    return;
  }
  ref.read(selectedFolderProvider.notifier).state = folder;
  ref.read(selectedMessageProvider.notifier).state = selectMessageId;
  ref.invalidate(
    folderMailboxListProvider(
      SessionFolderParams(
        accountId: account.id,
        folderName: folder,
        messageListSort: messageListSortSymbolic(
          ref.read(messageSortFieldProvider),
          ref.read(messageSortAscendingProvider),
        ),
      ),
    ),
  );
  unawaited(persistMailLocation(ref));
}

/// Selects [account], refreshes folders, restores folder/message when possible.
Future<void> applyAccountSelection(
  WidgetRef ref,
  AppAccount account, {
  String? restoreFolder,
  String? restoreMessageId,
}) async {
  _clearMultiSelect(ref);
  final String? previousAccountId = ref.read(selectedAccountIdProvider);
  String? useRestoreFolder = restoreFolder;
  String? useRestoreMessageId = restoreMessageId;
  if (previousAccountId != null && previousAccountId != account.id) {
    await persistMailLocation(ref);
    final AppSettingsConfig fresh =
        await ref.read(tagliacarteApiProvider).loadConfig();
    for (final AppAccount a in fresh.accounts) {
      if (a.id == account.id) {
        useRestoreFolder = a.lastFolder;
        useRestoreMessageId = a.lastMessageId;
        break;
      }
    }
  }
  ref.read(selectedAccountIdProvider.notifier).state = account.id;
  if (ref.read(selectedAccountIdProvider) != account.id) {
    return;
  }
  try {
    await sessionRefreshFolders(accountId: account.id);
  } catch (_) {
    // ignore
  }
  if (ref.read(selectedAccountIdProvider) != account.id) {
    return;
  }
  await waitForSessionFoldersBrief(ref, account.id);
  if (ref.read(selectedAccountIdProvider) != account.id) {
    return;
  }
  final List<String> folders = ref.read(foldersProvider).folders;
  final bool usedRestoreFolder =
      useRestoreFolder != null && folders.contains(useRestoreFolder);
  final String? pickFolder =
      usedRestoreFolder ? useRestoreFolder : defaultMailboxFolder(folders);
  _selectFolderProviders(
    ref,
    account,
    pickFolder,
    selectMessageId: usedRestoreFolder ? useRestoreMessageId : null,
  );
}

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

bool _accountsContainId(List<AppAccount> accounts, String? id) {
  if (id == null) {
    return false;
  }
  for (final AppAccount a in accounts) {
    if (a.id == id) {
      return true;
    }
  }
  return false;
}

/// Keeps [selectedAccountIdProvider] valid whenever [accountsConfigProvider] changes.
void syncAccountSelectionFromConfig(WidgetRef ref, AppSettingsConfig config) {
  final List<AppAccount> list = config.accounts;
  if (list.isEmpty) {
    if (ref.read(selectedAccountIdProvider) != null) {
      ref.read(selectedAccountIdProvider.notifier).state = null;
      ref.read(selectedFolderProvider.notifier).state = null;
      ref.read(selectedMessageProvider.notifier).state = null;
    }
    return;
  }
  final String? cur = ref.read(selectedAccountIdProvider);
  if (_accountsContainId(list, cur)) {
    return;
  }
  final AppAccount pick =
      _accountById(list, config.selectedStoreId) ?? list.first;
  unawaited(
    applyAccountSelection(
      ref,
      pick,
      restoreFolder: pick.lastFolder,
      restoreMessageId: pick.lastMessageId,
    ),
  );
}
