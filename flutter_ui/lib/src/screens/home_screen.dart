/*
 * home_screen.dart
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

import 'dart:async';

import 'package:flutter/foundation.dart'
    show TargetPlatform, defaultTargetPlatform, kIsWeb;
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import '../l10n/app_localizations.dart';
import '../layout/mail_layout.dart';
import '../models/mail_drag_data.dart';
import '../models/mail_pending_transfer.dart';
import '../models/message_row.dart';
import '../providers/account_selection_flow.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../providers/new_mail_notification_service.dart';
import '../providers/mail_location_persist.dart';
import '../providers/message_sort_persist.dart';
import '../providers/session_state.dart';
import '../providers/view_prefs.dart';
import '../rust/frb_api.dart';
import '../rust/frb_api/frb_mail.dart';
import '../rust/tagliacarte_api.dart';
import '../widgets/folder_tree.dart';
import '../widgets/nostr_credential_dialog.dart';
import '../widgets/lucide_icon.dart';
import '../widgets/mail_toolbar.dart';
import '../widgets/message_list.dart';
import '../widgets/message_sort_button.dart';
import '../widgets/message_view.dart';
import '../util/folder_display.dart';
import '../util/imap_credential_prompt.dart';
import '../util/mail_account_policy.dart';
import '../util/matrix_strings.dart';
import 'compose_screen.dart';
import '../util/process_log.dart';
import '../widgets/desktop_mail_splitter.dart';
import '../widgets/chat_view.dart';
import '../widgets/folder_mail_pane.dart';
import '../widgets/store_switcher.dart';
import 'message_detail_screen.dart';

const MethodChannel _kDockBadgeChannel =
    MethodChannel('dev.tagliacarte/dock_badge');

/// Recomputes macOS native Mail menu item enablement when any dependency changes.
final Provider<void> macMailMenuPushTriggerProvider = Provider<void>((Ref ref) {
  ref.watch(composeActiveProvider);
  ref.watch(selectedMessageProvider);
  ref.watch(selectedFolderProvider);
  ref.watch(selectedAccountIdProvider);
  ref.watch(accountsConfigProvider);
  ref.watch(messageSortFieldProvider);
  ref.watch(messageSortAscendingProvider);
  ref.watch(selectedAccountConversationModeProvider);
});

Future<void> _invokeDockBadgeSetBadge(int total) async {
  try {
    final String? label =
        total <= 0 ? null : (total > 999 ? '999+' : '$total');
    await _kDockBadgeChannel.invokeMethod<void>('setBadge', <String, Object?>{
      'label': label,
    });
  } on MissingPluginException {
    // No handler (e.g. non-macOS or embedder without dock channel).
  } catch (e, st) {
    appLogStderr('dock badge: $e\n$st');
  }
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

class HomeScreen extends ConsumerStatefulWidget {
  const HomeScreen({super.key});

  @override
  ConsumerState<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends ConsumerState<HomeScreen>
    with WidgetsBindingObserver {
  bool _redirectToSettingsScheduled = false;

  static const double _kAccountRailWidth = 72;

  double _folderPaneWidth = kMailDesktopDefaultFolderPaneWidth;
  double _desktopListPaneFraction = 0.5;

  static const MethodChannel _macMailMenuChannel =
      MethodChannel('dev.tagliacarte/mail_menu');
  bool _macMailMenuHandlerInstalled = false;

  void _scheduleOpenSettingsWhenNoAccounts() {
    final ModalRoute<Object?>? route = ModalRoute.of(context);
    if (route?.isCurrent != true || route?.settings.name != '/') {
      return;
    }
    if (_redirectToSettingsScheduled) {
      return;
    }
    _redirectToSettingsScheduled = true;
    SchedulerBinding.instance.addPostFrameCallback((_) {
      _redirectToSettingsScheduled = false;
      if (!mounted) {
        return;
      }
      final List<AppAccount>? accounts =
          ref.read(accountsConfigProvider).valueOrNull?.accounts;
      final bool empty = accounts == null || accounts.isEmpty;
      if (!empty) {
        return;
      }
      if (ModalRoute.of(context)?.isCurrent != true) {
        return;
      }
      Navigator.of(context).pushReplacementNamed('/settings', arguments: 0);
    });
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(_loadDesktopPanePrefs());
  }

  Future<void> _loadDesktopPanePrefs() async {
    final SharedPreferences prefs = await SharedPreferences.getInstance();
    if (!mounted) {
      return;
    }
    setState(() {
      _folderPaneWidth =
          prefs.getDouble(kPrefDesktopFolderPaneWidth) ??
              kMailDesktopDefaultFolderPaneWidth;
      _desktopListPaneFraction =
          prefs.getDouble(kPrefDesktopListPaneFraction) ?? 0.5;
    });
  }

  void _persistDesktopPanePrefs() {
    final double folderW = _folderPaneWidth;
    final double listFrac = _desktopListPaneFraction;
    unawaited(
      SharedPreferences.getInstance().then((SharedPreferences prefs) async {
        await prefs.setDouble(kPrefDesktopFolderPaneWidth, folderW);
        await prefs.setDouble(kPrefDesktopListPaneFraction, listFrac);
      }),
    );
  }

  double _maxFolderPaneWidth(double windowWidth) {
    final double raw = windowWidth -
        _kAccountRailWidth -
        1 -
        kMailDesktopSplitterHitSize -
        kMailDesktopMinMainRestWidth;
    return raw < kMailDesktopMinFolderWidth
        ? kMailDesktopMinFolderWidth
        : raw;
  }

  @override
  void dispose() {
    if (!kIsWeb && defaultTargetPlatform == TargetPlatform.macOS) {
      _macMailMenuChannel.setMethodCallHandler(null);
      _macMailMenuHandlerInstalled = false;
    }
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.detached ||
        state == AppLifecycleState.hidden) {
      unawaited(persistCurrentMessageSort(ref));
      unawaited(persistMailLocation(ref));
    }
  }

  Widget _railSettingsButton({
    required double width,
    required VoidCallback onPressed,
  }) {
    return SizedBox(
      height: 40,
      width: width,
      child: Center(
        child: IconButton(
          padding: EdgeInsets.zero,
          visualDensity: VisualDensity.compact,
          constraints: const BoxConstraints.tightFor(width: 40, height: 40),
          tooltip: AppLocalizations.of(context).settingsTooltip,
          icon: const LucideIcon(LucideIcons.settings, size: 22),
          onPressed: onPressed,
        ),
      ),
    );
  }

  void _clearMultiSelect() {
    ref.read(mailMultiSelectActiveProvider.notifier).state = false;
    ref.read(mailSelectedIdsProvider.notifier).clear();
  }

  void _selectAccount(AppAccount account) {
    unawaited(
      _selectAccountAsync(
        account,
        restoreFolder: account.lastFolder,
        restoreMessageId: account.lastMessageId,
      ),
    );
  }

  Future<void> _reloadFoldersAfterMutation(AppAccount account) async {
    await sessionRefreshFolders(accountId: account.id);
    if (!mounted || ref.read(selectedAccountIdProvider) != account.id) {
      return;
    }
    final String? previous = ref.read(selectedFolderProvider);
    final List<String> folders = ref.read(foldersProvider).folders;
    if (previous != null && folders.contains(previous)) {
      return;
    }
    final String? pick = folders.isNotEmpty ? folders.first : null;
    _selectFolder(account, pick);
  }

  Future<void> _selectAccountAsync(
    AppAccount account, {
    String? restoreFolder,
    String? restoreMessageId,
  }) async {
    await applyAccountSelection(
      ref,
      account,
      restoreFolder: restoreFolder,
      restoreMessageId: restoreMessageId,
    );
    if (!mounted) {
      return;
    }
  }

  void _selectFolder(
    AppAccount account,
    String? folder, {
    String? selectMessageId,
  }) {
    _clearMultiSelect();
    ref.read(mailPendingTransferProvider.notifier).state = null;
    if (folder == null) {
      ref.read(selectedFolderProvider.notifier).state = null;
      ref.read(selectedMessageProvider.notifier).state = null;
      unawaited(persistMailLocation(ref));
      return;
    }
    ref.read(selectedFolderProvider.notifier).state = folder;
    ref.read(selectedMessageProvider.notifier).state = selectMessageId;
    // Ensure list/timeline refetch (IMAP SELECT, Nostr open folder, etc.) even if a family
    // instance was kept around or Riverpod skipped a rebuild edge case.
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

  void _openCompose({ComposeIntent? intent}) {
    if (!mounted) {
      return;
    }
    Navigator.of(context).pushNamed(
      '/compose',
      arguments: intent,
    );
  }

  /// Compose, or continue the selected draft when the user is in a drafts folder.
  void _openComposeSmart() {
    if (!mounted) {
      return;
    }
    final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
    final AppAccount? selectedAccount = _accountById(
      cfg?.accounts ?? <AppAccount>[],
      ref.read(selectedAccountIdProvider),
    );
    final String? selectedFolder = ref.read(selectedFolderProvider);
    final String? selectedMessageId = ref.read(selectedMessageProvider);
    final bool conversationMode =
        ref.read(selectedAccountConversationModeProvider);
    final bool messageSelected = !conversationMode &&
        selectedMessageId != null &&
        selectedFolder != null &&
        selectedAccount != null &&
        isEmailMailboxBackend(selectedAccount);
    if (messageSelected &&
        mailboxLooksLikeDrafts(selectedAccount, selectedFolder)) {
      _openCompose(
        intent: ComposeIntent(
          accountId: selectedAccount.id,
          replyFolderName: selectedFolder,
          replyMessageId: selectedMessageId,
          continueDraft: true,
        ),
      );
      return;
    }
    _openCompose();
  }

  /// Single-message delete (`delete`) or multi-select (`delete N`); must not match arbitrary stubs.
  bool _isMailToolbarDeleteAction(String action) {
    final String t = action.trim();
    if (t == 'delete') {
      return true;
    }
    return RegExp(r'^delete \d+$').hasMatch(t);
  }

  bool _isMailToolbarJunkAction(String action) {
    final String t = action.trim();
    if (t == 'junk') {
      return true;
    }
    return RegExp(r'^junk \d+$').hasMatch(t);
  }

  void _stubAction(String action) {
    if (!mounted) {
      return;
    }
    final AppSettingsConfig? cfg =
        ref.read(accountsConfigProvider).valueOrNull;
    final AppAccount? acc = _accountById(
      cfg?.accounts ?? <AppAccount>[],
      ref.read(selectedAccountIdProvider),
    );
    if (_isMailToolbarDeleteAction(action)) {
      final String? folder = ref.read(selectedFolderProvider);
      if (acc != null && mailboxMoveToTrashDeleteUnavailable(acc, folder)) {
        return;
      }
      if (acc == null ||
          folder == null ||
          !isEmailMailboxBackend(acc)) {
        return;
      }
      final bool multi = ref.read(mailMultiSelectActiveProvider);
      final List<String> ids;
      if (multi) {
        ids = ref.read(mailSelectedIdsProvider).toList();
      } else {
        final String? one = ref.read(selectedMessageProvider);
        if (one == null) {
          return;
        }
        ids = <String>[one];
      }
      if (ids.isEmpty) {
        return;
      }
      unawaited(_runMailDelete(account: acc, folder: folder, messageIds: ids));
      return;
    }
    if (_isMailToolbarJunkAction(action)) {
      final String? folder = ref.read(selectedFolderProvider);
      if (acc != null && mailboxJunkMoveUnavailable(acc, folder)) {
        return;
      }
      if (acc == null ||
          folder == null ||
          !accountSupportsMailboxTrashAndJunkMoves(acc)) {
        return;
      }
      final bool multi = ref.read(mailMultiSelectActiveProvider);
      final List<String> ids;
      if (multi) {
        ids = ref.read(mailSelectedIdsProvider).toList();
      } else {
        final String? one = ref.read(selectedMessageProvider);
        if (one == null) {
          return;
        }
        ids = <String>[one];
      }
      if (ids.isEmpty) {
        return;
      }
      unawaited(
        _runMailTransferCore(
          sourceAccountId: acc.id,
          sourceFolder: folder,
          messageIds: ids,
          destAccount: acc,
          destFolder: mailboxJunkFolderDisplayName(acc),
          isMove: true,
        ),
      );
      return;
    }
    if (acc != null &&
        (action == 'reply' ||
            action == 'reply-all' ||
            action == 'forward')) {
      final String? folder = ref.read(selectedFolderProvider);
      final String? mid = ref.read(selectedMessageProvider);
      if (folder != null && mid != null) {
        if (isNntpMailboxBackend(acc)) {
          if (action == 'reply') {
            _openCompose(
              intent: ComposeIntent(
                accountId: acc.id,
                replyFolderName: folder,
                replyMessageId: mid,
              ),
            );
            return;
          }
        } else if (isEmailMailboxBackend(acc)) {
          final ComposeReplyKind? kind = switch (action) {
            'reply' => ComposeReplyKind.reply,
            'reply-all' => ComposeReplyKind.replyAll,
            'forward' => ComposeReplyKind.forward,
            _ => null,
          };
          if (kind != null) {
            _openCompose(
              intent: ComposeIntent(
                accountId: acc.id,
                replyFolderName: folder,
                replyMessageId: mid,
                replyKind: kind,
              ),
            );
            return;
          }
        }
      }
    }
    final AppLocalizations l10n = AppLocalizations.of(context);
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(l10n.stubInvoked(action))),
    );
  }

  Future<void> _runMailDelete({
    required AppAccount account,
    required String folder,
    required List<String> messageIds,
  }) async {
    if (!mounted) {
      return;
    }
    final AppLocalizations l10n = AppLocalizations.of(context);
    try {
      final FrbBatchMailOperationResult batch = await frbDeleteMailMessages(
        accountId: account.id,
        folderName: folder,
        messageIds: messageIds,
      );
      final int failed = batch.failedCount.toInt();
      final int ok = batch.okCount.toInt();
      if (!mounted) {
        return;
      }
      if (failed == 0) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.transferResultOk(ok))),
        );
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(l10n.transferResultMixed(ok, failed)),
            duration: const Duration(seconds: 6),
          ),
        );
      }
      final String sort = messageListSortSymbolic(
        ref.read(messageSortFieldProvider),
        ref.read(messageSortAscendingProvider),
      );
      ref.invalidate(
        folderMailboxListProvider(
          SessionFolderParams(
            accountId: account.id,
            folderName: folder,
            messageListSort: sort,
          ),
        ),
      );
      ref.read(selectedMessageProvider.notifier).state = null;
      _clearMultiSelect();
      unawaited(persistMailLocation(ref));
      unawaited(_reloadFoldersAfterMutation(account));
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.deleteMessagesFailed(e.toString()))),
        );
      }
    }
  }

  void _tagMessagesForTransfer(MailPendingTransferKind kind) {
    if (!mounted) {
      return;
    }
    final bool multi = ref.read(mailMultiSelectActiveProvider);
    final Set<String> ids;
    if (multi) {
      ids = Set<String>.from(ref.read(mailSelectedIdsProvider));
    } else {
      final String? one = ref.read(selectedMessageProvider);
      if (one == null) {
        return;
      }
      ids = <String>{one};
    }
    if (ids.isEmpty) {
      return;
    }
    final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
    final AppAccount? account =
        _accountById(cfg?.accounts ?? <AppAccount>[], ref.read(selectedAccountIdProvider));
    final String? folder = ref.read(selectedFolderProvider);
    if (account == null || folder == null || !isEmailMailboxBackend(account)) {
      return;
    }
    ref.read(mailPendingTransferProvider.notifier).state = MailPendingTransfer(
      kind: kind,
      sourceAccountId: account.id,
      sourceFolder: folder,
      messageIds: ids.toList(),
    );
    final AppLocalizations l10n = AppLocalizations.of(context);
    final int n = ids.length;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          kind == MailPendingTransferKind.moveOp
              ? l10n.pendingMoveTagged(n)
              : l10n.pendingCopyTagged(n),
        ),
      ),
    );
  }

  Future<void> _runMailTransferCore({
    required String sourceAccountId,
    required String sourceFolder,
    required List<String> messageIds,
    required AppAccount destAccount,
    required String destFolder,
    required bool isMove,
  }) async {
    if (!mounted) {
      return;
    }
    final AppLocalizations l10n = AppLocalizations.of(context);
    try {
      final FrbBatchMailOperationResult batch = await frbTransferMailMessages(
        sourceAccountId: sourceAccountId,
        sourceFolder: sourceFolder,
        destAccountId: destAccount.id,
        destFolder: destFolder,
        messageIds: messageIds,
        isMove: isMove,
      );
      final int failed = batch.failedCount.toInt();
      final int ok = batch.okCount.toInt();
      if (!mounted) {
        return;
      }
      if (failed == 0) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.transferResultOk(ok))),
        );
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(l10n.transferResultMixed(ok, failed)),
            duration: const Duration(seconds: 6),
          ),
        );
      }
      final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
      final String sort = messageListSortSymbolic(
        ref.read(messageSortFieldProvider),
        ref.read(messageSortAscendingProvider),
      );
      void invalidateFolder(AppAccount a, String f) {
        ref.invalidate(
          folderMailboxListProvider(
            SessionFolderParams(
              accountId: a.id,
              folderName: f,
              messageListSort: sort,
            ),
          ),
        );
      }
      AppAccount? srcAcc;
      for (final AppAccount a in cfg?.accounts ?? <AppAccount>[]) {
        if (a.id == sourceAccountId) {
          srcAcc = a;
          break;
        }
      }
      if (srcAcc != null) {
        invalidateFolder(srcAcc, sourceFolder);
      }
      invalidateFolder(destAccount, destFolder);
      if (isMove) {
        ref.read(selectedMessageProvider.notifier).state = null;
        ref.read(mailMultiSelectActiveProvider.notifier).state = false;
        ref.read(mailSelectedIdsProvider.notifier).clear();
        unawaited(persistMailLocation(ref));
      }
      unawaited(_reloadFoldersAfterMutation(destAccount));
      if (srcAcc != null) {
        unawaited(_reloadFoldersAfterMutation(srcAcc));
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.transferFailed(e.toString()))),
        );
      }
    }
  }

  Future<void> _completePendingTransferToFolder(
    AppAccount destAccount,
    String destFolder,
  ) async {
    if (!mounted) {
      return;
    }
    final MailPendingTransfer? pending = ref.read(mailPendingTransferProvider);
    if (pending == null) {
      return;
    }
    ref.read(mailPendingTransferProvider.notifier).state = null;
    await _runMailTransferCore(
      sourceAccountId: pending.sourceAccountId,
      sourceFolder: pending.sourceFolder,
      messageIds: pending.messageIds,
      destAccount: destAccount,
      destFolder: destFolder,
      isMove: pending.kind == MailPendingTransferKind.moveOp,
    );
  }

  Future<void> _completeMailDragDrop(
    AppAccount destAccount,
    MailListDragPayload payload,
    String destFolder, {
    required bool asCopy,
  }) async {
    await _runMailTransferCore(
      sourceAccountId: payload.sourceAccountId,
      sourceFolder: payload.sourceFolder,
      messageIds: payload.messageIds,
      destAccount: destAccount,
      destFolder: destFolder,
      isMove: !asCopy,
    );
  }

  /// macOS: [MainMenu.xib] "Message" menu invokes [MethodChannel] `dev.tagliacarte/mail_menu`.
  void _handleMacMailMenuIntent(String action) {
    if (!mounted) {
      return;
    }
    if (ref.read(composeActiveProvider)) {
      return;
    }
    final AsyncValue<AppSettingsConfig> cfgAsync =
        ref.read(accountsConfigProvider);
    final List<AppAccount> stripAccounts =
        cfgAsync.valueOrNull?.accounts ?? const <AppAccount>[];
    final String? selectedAccountId = ref.read(selectedAccountIdProvider);
    final AppAccount? selectedAccount =
        _accountById(stripAccounts, selectedAccountId);
    final bool sendMailEnabled = selectedAccount == null ||
        accountCanSendMail(selectedAccount);

    final String? selectedFolder = ref.read(selectedFolderProvider);
    final String? selectedMessageId = ref.read(selectedMessageProvider);
    final bool conversationMode =
        ref.read(selectedAccountConversationModeProvider);
    final SessionFolderParams? folderParams =
        selectedAccount != null && selectedFolder != null
            ? SessionFolderParams(
                accountId: selectedAccount.id,
                folderName: selectedFolder,
                messageListSort: messageListSortSymbolic(
                  ref.read(messageSortFieldProvider),
                  ref.read(messageSortAscendingProvider),
                ),
              )
            : null;

    final bool messageSelected = !conversationMode &&
        selectedMessageId != null &&
        folderParams != null &&
        selectedFolder != null &&
        isEmailMailboxBackend(selectedAccount!);

    final bool messageDeleteEnabled = messageSelected &&
        !mailboxMoveToTrashDeleteUnavailable(selectedAccount, selectedFolder);
    final bool messageJunkEnabled = messageSelected &&
        accountSupportsMailboxTrashAndJunkMoves(selectedAccount) &&
        !mailboxJunkMoveUnavailable(selectedAccount, selectedFolder);

    final bool canReply = sendMailEnabled && messageSelected;

    switch (action) {
      case 'compose':
        if (!sendMailEnabled) {
          return;
        }
        _openComposeSmart();
        return;
      case 'reply':
        if (!canReply) {
          return;
        }
        _stubAction(action);
        return;
      case 'reply-all':
      case 'forward':
        if (!canReply) {
          return;
        }
        if (isNntpMailboxBackend(selectedAccount)) {
          return;
        }
        _stubAction(action);
        return;
      case 'delete':
        if (!messageDeleteEnabled) {
          return;
        }
        _stubAction(action);
        return;
      case 'junk':
        if (!messageJunkEnabled) {
          return;
        }
        _stubAction(action);
        return;
      case 'move':
        if (!messageSelected) {
          return;
        }
        _tagMessagesForTransfer(MailPendingTransferKind.moveOp);
        return;
      case 'copy':
        if (!messageSelected) {
          return;
        }
        _tagMessagesForTransfer(MailPendingTransferKind.copyOp);
        return;
      default:
        return;
    }
  }

  Future<void> _pushMacMailMenuState() async {
    if (!mounted ||
        kIsWeb ||
        defaultTargetPlatform != TargetPlatform.macOS) {
      return;
    }
    final AsyncValue<AppSettingsConfig> cfgAsync =
        ref.read(accountsConfigProvider);
    final List<AppAccount> stripAccounts =
        cfgAsync.valueOrNull?.accounts ?? const <AppAccount>[];
    final String? selectedAccountId = ref.read(selectedAccountIdProvider);
    final AppAccount? selectedAccount =
        _accountById(stripAccounts, selectedAccountId);
    final bool sendMailEnabled = selectedAccount == null ||
        accountCanSendMail(selectedAccount);

    final String? selectedFolder = ref.read(selectedFolderProvider);
    final String? selectedMessageId = ref.read(selectedMessageProvider);
    final bool conversationMode =
        ref.read(selectedAccountConversationModeProvider);
    final SessionFolderParams? folderParams =
        selectedAccount != null && selectedFolder != null
            ? SessionFolderParams(
                accountId: selectedAccount.id,
                folderName: selectedFolder,
                messageListSort: messageListSortSymbolic(
                  ref.read(messageSortFieldProvider),
                  ref.read(messageSortAscendingProvider),
                ),
              )
            : null;

    final bool messageSelected = !conversationMode &&
        selectedMessageId != null &&
        folderParams != null &&
        selectedFolder != null &&
        isEmailMailboxBackend(selectedAccount!);

    final bool messageDeleteEnabled = messageSelected &&
        !mailboxMoveToTrashDeleteUnavailable(selectedAccount, selectedFolder);
    final bool messageJunkEnabled = messageSelected &&
        accountSupportsMailboxTrashAndJunkMoves(selectedAccount) &&
        !mailboxJunkMoveUnavailable(selectedAccount, selectedFolder);

    final bool canReply = sendMailEnabled && messageSelected;
    final bool nntpNoBroadcast =
        selectedAccount != null && isNntpMailboxBackend(selectedAccount);
    final bool composeActive = ref.read(composeActiveProvider);
    final Map<String, bool> map = <String, bool>{
      'compose': !composeActive && sendMailEnabled,
      'reply': !composeActive && canReply,
      'reply-all': !composeActive && canReply && !nntpNoBroadcast,
      'forward': !composeActive && canReply && !nntpNoBroadcast,
      'delete': !composeActive && messageDeleteEnabled,
      'junk': !composeActive && messageJunkEnabled,
      'move': !composeActive && messageSelected,
      'copy': !composeActive && messageSelected,
    };
    try {
      await _macMailMenuChannel.invokeMethod<void>(
        'setMailMenuState',
        map,
      );
    } on MissingPluginException {
      // Non-macOS embedder or older Runner without handler.
    } catch (e, st) {
      appLogStderr('setMailMenuState: $e\n$st');
    }
  }

  void _openMessage(
    BuildContext context, {
    required AppAccount? account,
    required String? folder,
    required MessageListRow row,
    required bool isMobile,
    required bool inlineDesktop,
    required bool useKeychain,
  }) {
    ref.read(selectedMessageProvider.notifier).state = row.id;
    unawaited(persistMailLocation(ref));
    final bool separate = isMobile || !inlineDesktop;
    if (!separate || account == null || folder == null) {
      return;
    }
    if (!isEmailMailboxBackend(account)) {
      return;
    }
    if (ref.read(selectedAccountConversationModeProvider)) {
      return;
    }
    final AppLocalizations pushL10n = AppLocalizations.of(context);
    Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (_) => StoreMessageDetailScreen(
          params: MailMessageDetailParams(
            accountId: account.id,
            folderName: folder,
            messageId: row.id,
          ),
          titleFallback: matrixConversationPreviewText(
            pushL10n,
            account,
            row.subject,
          ),
          account: account,
        ),
      ),
    );
  }

  Widget _mailDrawer(
    BuildContext context,
    List<AppAccount> stripAccounts,
    String? selectedAccountId,
    AppAccount? selectedAccount,
    List<String> folders,
    Map<String, int> unreadByFolder,
    String? selectedFolder,
    Map<String, int> storeUnreadTotals, {
    required bool useKeychain,
  }) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    return Drawer(
      child: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox(
                    width: 56,
                    child: Material(
                      color: scheme.surfaceContainerHighest.withValues(alpha: 0.35),
                      child: Column(
                        children: [
                          Expanded(
                            child: StoreSwitcher(
                              accounts: stripAccounts,
                              showLabels: false,
                              selectedAccountId: selectedAccountId,
                              storeUnreadTotals: storeUnreadTotals,
                              onSelect: (AppAccount a) {
                                _selectAccount(a);
                              },
                            ),
                          ),
                          _railSettingsButton(
                            width: 56,
                            onPressed: () {
                              Navigator.of(context).pop();
                              Navigator.of(context).pushNamed('/settings');
                            },
                          ),
                        ],
                      ),
                    ),
                  ),
                  VerticalDivider(width: 1, thickness: 1, color: scheme.outlineVariant),
                  Expanded(
                    child: selectedAccount != null &&
                            (isEmailMailboxBackend(selectedAccount) ||
                                isConversationBackend(selectedAccount))
                        ? FolderMailPane(
                            account: selectedAccount,
                            folders: folders,
                            unreadByFolder: unreadByFolder,
                            selectedFolder: selectedFolder,
                            onSelectFolder: (String folder) {
                              _selectFolder(selectedAccount, folder);
                              Navigator.of(context).pop();
                            },
                            onReloadFolders: () =>
                                _reloadFoldersAfterMutation(selectedAccount),
                            onPendingTransferToFolder: (String folder) =>
                                _completePendingTransferToFolder(
                                  selectedAccount,
                                  folder,
                                ),
                            enableMailDragTarget: false,
                          )
                        : FolderTree(
                            folders: folders,
                            unreadByFolder: unreadByFolder,
                            selectedFolder: selectedFolder,
                            onSelect: (String folder) {
                              if (selectedAccount != null) {
                                _selectFolder(selectedAccount, folder);
                              }
                              Navigator.of(context).pop();
                            },
                          ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (!kIsWeb &&
        defaultTargetPlatform == TargetPlatform.macOS &&
        !_macMailMenuHandlerInstalled) {
      _macMailMenuHandlerInstalled = true;
      _macMailMenuChannel.setMethodCallHandler((MethodCall call) async {
        if (call.method != 'mailAction') {
          return;
        }
        final Object? raw = call.arguments;
        if (raw is! Map) {
          return;
        }
        final String? action = raw['action'] as String?;
        if (action == null) {
          return;
        }
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (context.mounted) {
            _handleMacMailMenuIntent(action);
          }
        });
      });
      WidgetsBinding.instance.addPostFrameCallback((_) {
        unawaited(_pushMacMailMenuState());
      });
    }

    if (!kIsWeb && defaultTargetPlatform == TargetPlatform.macOS) {
      ref.listen<void>(
        macMailMenuPushTriggerProvider,
        (Object? previous, Object? next) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            unawaited(_pushMacMailMenuState());
          });
        },
      );
    }

    if (!kIsWeb && defaultTargetPlatform == TargetPlatform.macOS) {
      ref.listen<int>(nativeTotalInboxUnreadProvider, (int? prev, int next) {
        unawaited(_invokeDockBadgeSetBadge(next));
      });
    }

    ref.listen<NewMailToastSignal?>(newMailToastSignalProvider, (
      NewMailToastSignal? previous,
      NewMailToastSignal? next,
    ) {
      if (next == null) {
        return;
      }
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        if (!context.mounted) {
          return;
        }
        final AppLocalizations l10n = AppLocalizations.of(context);
        final AppLifecycleState life =
            WidgetsBinding.instance.lifecycleState ??
                AppLifecycleState.resumed;
        final bool foreground = life == AppLifecycleState.resumed;
        final String body = l10n.newMailNotificationBody(
          next.countHint,
          next.folderName,
        );
        if (foreground) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('${next.accountLabel}: $body')),
          );
        } else {
          await NewMailNotificationService.instance.showOsNotification(
            title: l10n.newMailNotificationTitle,
            body: '${next.accountLabel} — $body',
          );
        }
        ref.read(newMailToastSignalProvider.notifier).state = null;
      });
    });

    final AsyncValue<AppSettingsConfig> cfgAsync = ref.watch(
      accountsConfigProvider,
    );
    // When config reloads (e.g. settingsRevision bumps), Riverpod often emits
    // AsyncLoading with the previous value attached. maybeWhen(data:...) alone
    // treats that as loading and hits orElse → empty strip and missing avatars.
    final List<AppAccount> stripAccounts = cfgAsync.maybeWhen(
      data: (AppSettingsConfig c) => c.accounts,
      loading: () =>
          cfgAsync.valueOrNull?.accounts ?? const <AppAccount>[],
      error: (Object _, StackTrace stackTrace) =>
          cfgAsync.valueOrNull?.accounts ?? const <AppAccount>[],
      orElse: () => const <AppAccount>[],
    );

    final String? selectedAccountId = ref.watch(selectedAccountIdProvider);
    final AppAccount? selectedAccount = _accountById(
      stripAccounts,
      selectedAccountId,
    );

    final MailFoldersState mailFoldersState = ref.watch(foldersProvider);
    final Map<String, int> storeUnreadTotals =
        ref.watch(storeTotalUnreadByAccountProvider);
    final List<String> folders = mailFoldersState.folders;
    final String? selectedFolder = ref.watch(selectedFolderProvider);
    final String? selectedMessageId = ref.watch(selectedMessageProvider);
    final bool inlineDesktop = ref.watch(messageDetailInlineProvider);

    final bool useKeychain =
        cfgAsync.valueOrNull?.useKeychain ?? true;
    final bool conversationMode =
        ref.watch(selectedAccountConversationModeProvider);
    final MessageSortField sortField = ref.watch(messageSortFieldProvider);
    final bool sortAsc = ref.watch(messageSortAscendingProvider);
    final SessionFolderParams? folderParams =
        selectedAccount != null && selectedFolder != null
            ? SessionFolderParams(
                accountId: selectedAccount.id,
                folderName: selectedFolder,
                messageListSort: messageListSortSymbolic(sortField, sortAsc),
              )
            : null;

    final FolderListVm? folderListVm = folderParams != null
        ? ref.watch(folderMailboxListProvider(folderParams))
        : null;

    if (folderParams != null) {
      ref.listen<FolderListVm>(
        folderMailboxListProvider(folderParams),
        (FolderListVm? prev, FolderListVm next) {
          if (next.totalCount == 0 && next.ready) {
            if (ref.read(selectedMessageProvider) != null) {
              ref.read(selectedMessageProvider.notifier).state = null;
              unawaited(persistMailLocation(ref));
            }
            return;
          }
          final String? cur = ref.read(selectedMessageProvider);
          final bool firstReady =
              next.ready && next.totalCount > 0 && (prev == null || !prev.ready);
          if (cur == null && firstReady) {
            final MessageListRow? last =
                next.rowAtDataIndex(next.totalCount - 1);
            final MessageListRow? first = next.rowAtDataIndex(0);
            final bool desc = !ref.read(messageSortAscendingProvider);
            final String? pick = desc
                ? (last?.id ?? first?.id)
                : (first?.id ?? last?.id);
            ref.read(selectedMessageProvider.notifier).state = pick;
            unawaited(persistMailLocation(ref));
          } else if (cur != null && next.ready && !next.containsId(cur)) {
            ref
                .read(folderMailboxListProvider(folderParams).notifier)
                .ensureMessageIdLoaded(cur);
          }
          final Object? err = next.error;
          if (err != null &&
              (prev == null || prev.error != err) &&
              (isImapStyleMailboxBackend(selectedAccount!) ||
                  isMatrixMailboxBackend(selectedAccount)) &&
              isMissingImapCredentialsError(err)) {
            WidgetsBinding.instance.addPostFrameCallback((_) async {
              if (!context.mounted) {
                return;
              }
              final bool? saved = await showImapStyleMailboxCredentialPrompt(
                context: context,
                account: selectedAccount,
                err: err,
              );
              if (!context.mounted) {
                return;
              }
              if (saved == true && context.mounted) {
                await sessionRefreshFolders(accountId: selectedAccount.id);
                ref.invalidate(folderMailboxListProvider(folderParams));
                final String? folder = ref.read(selectedFolderProvider);
                final String? mid = ref.read(selectedMessageProvider);
                if (folder != null && mid != null) {
                  ref.invalidate(
                    mailMessageDetailProvider(
                      MailMessageDetailParams(
                        accountId: selectedAccount.id,
                        folderName: folder,
                        messageId: mid,
                      ),
                    ),
                  );
                }
              }
            });
          }
          if (err != null &&
              (prev == null || prev.error != err) &&
              isNostrBackend(selectedAccount!) &&
              isMissingNostrCredentialsError(err)) {
            WidgetsBinding.instance.addPostFrameCallback((_) async {
              if (!context.mounted) {
                return;
              }
              final String? savedNpub = await showNostrCredentialDialog(
                context,
                account: selectedAccount,
              );
              if (savedNpub != null && context.mounted) {
                ref.invalidate(folderMailboxListProvider(folderParams));
              }
            });
          }
        },
      );
    }

    // Missing mailbox credentials are surfaced on the session connection when folder listing
    // fails (no folders → no selected folder → [folderMailboxListProvider] never runs).
    if (selectedAccount != null &&
        ((isImapStyleMailboxBackend(selectedAccount) && !conversationMode) ||
            isMatrixMailboxBackend(selectedAccount))) {
      ref.listen<AccountMailModel?>(
        selectedAccountMailModelProvider,
        (AccountMailModel? prev, AccountMailModel? next) {
          if (next == null) {
            return;
          }
          if (ref.read(selectedAccountIdProvider) != selectedAccount.id) {
            return;
          }
          if (next.connection != MailConnectionState.error) {
            return;
          }
          final String? msg = next.connectionMessage;
          if (msg == null || !isMissingImapCredentialsError(msg)) {
            return;
          }
          if (prev != null &&
              prev.connection == MailConnectionState.error &&
              prev.connectionMessage == msg) {
            return;
          }
          if (ref.read(selectedFolderProvider) != null) {
            return;
          }
          final AppAccount acc = selectedAccount;
          WidgetsBinding.instance.addPostFrameCallback((_) async {
            if (!context.mounted) {
              return;
            }
            if (ref.read(selectedAccountIdProvider) != acc.id) {
              return;
            }
            if (ref.read(selectedFolderProvider) != null) {
              return;
            }
            final bool? saved = await showImapStyleMailboxCredentialPrompt(
              context: context,
              account: acc,
              err: msg,
            );
            if (!context.mounted) {
              return;
            }
            if (saved == true && context.mounted) {
              await sessionRefreshFolders(accountId: acc.id);
              if (ref.read(selectedAccountIdProvider) == acc.id) {
                ensureSelectedFolderForCurrentAccount(ref);
              }
              final String? fold = ref.read(selectedFolderProvider);
              if (ref.read(selectedAccountIdProvider) == acc.id &&
                  fold != null) {
                ref.invalidate(
                  folderMailboxListProvider(
                    SessionFolderParams(
                      accountId: acc.id,
                      folderName: fold,
                      messageListSort: messageListSortSymbolic(
                        ref.read(messageSortFieldProvider),
                        ref.read(messageSortAscendingProvider),
                      ),
                    ),
                  ),
                );
              }
            }
          });
        },
      );
    }

    final MessageListRow? selectedRow = selectedMessageId != null &&
            folderListVm != null
        ? folderListVm.rowById(selectedMessageId)
        : null;
    final bool messageSelected = !conversationMode &&
        selectedMessageId != null &&
        selectedFolder != null &&
        selectedAccount != null &&
        isEmailMailboxBackend(selectedAccount);

    final bool messageDeleteEnabled = messageSelected &&
        !mailboxMoveToTrashDeleteUnavailable(
          selectedAccount,
          selectedFolder,
        );
    final bool messageJunkEnabled = messageSelected &&
        accountSupportsMailboxTrashAndJunkMoves(selectedAccount) &&
        !mailboxJunkMoveUnavailable(selectedAccount, selectedFolder);

    final MailMessageDetailParams? detailParams =
        !conversationMode &&
                selectedAccount != null &&
                isEmailMailboxBackend(selectedAccount) &&
                selectedFolder != null &&
                selectedMessageId != null
            ? MailMessageDetailParams(
                accountId: selectedAccount.id,
                folderName: selectedFolder,
                messageId: selectedMessageId,
              )
            : null;

    final AsyncValue<MailMessageDetailView>? detailAsync = detailParams != null
        ? ref.watch(mailMessageDetailProvider(detailParams))
        : null;

    if (detailParams != null) {
      ref.listen<AsyncValue<MailMessageDetailView>>(
        mailMessageDetailProvider(detailParams),
        (AsyncValue<MailMessageDetailView>? prev,
            AsyncValue<MailMessageDetailView> next) {
          if (prev is AsyncData<MailMessageDetailView>) {
            return;
          }
          if (next is! AsyncData<MailMessageDetailView>) {
            return;
          }
          unawaited(
            markMessageReadAfterDetailLoaded(
              ref,
              detailParams,
              accountIdOverride: ref.read(selectedAccountIdProvider),
            ),
          );
        },
      );
    }

    if (detailParams != null &&
        selectedAccount != null &&
        isImapStyleMailboxBackend(selectedAccount) &&
        inlineDesktop) {
      ref.listen<AsyncValue<MailMessageDetailView>>(
        mailMessageDetailProvider(detailParams),
        (AsyncValue<MailMessageDetailView>? prev,
            AsyncValue<MailMessageDetailView> next) {
          next.whenOrNull(
            error: (Object e, StackTrace _) {
              if (!isMissingImapCredentialsError(e)) {
                return;
              }
              WidgetsBinding.instance.addPostFrameCallback((_) async {
                if (!context.mounted) {
                  return;
                }
                final bool? saved =
                    await showImapStyleMailboxCredentialPrompt(
                  context: context,
                  account: selectedAccount,
                  err: e,
                );
                if (!context.mounted) {
                  return;
                }
                if (saved == true && context.mounted) {
                  await sessionRefreshFolders(accountId: selectedAccount.id);
                  ref.invalidate(mailMessageDetailProvider(detailParams));
                }
              });
            },
          );
        },
      );
    }

    final String rawFolder =
        selectedFolder ?? (folders.isNotEmpty ? folders.first : '');
    final Map<String, String> folderTitleMap =
        selectedAccount != null && isMatrixMailboxBackend(selectedAccount)
            ? matrixMergedFolderLabels(mailFoldersState)
            : mailFoldersState.folderDisplayLabels;
    final String? folderLabelOverride = rawFolder.isEmpty
        ? null
        : folderTitleMap[rawFolder.trim().toLowerCase()];
    final String folderDisplay = rawFolder.isEmpty
        ? ''
        : (folderLabelOverride != null && folderLabelOverride.trim().isNotEmpty)
            ? folderLabelOverride.trim()
            : folderDisplayName(context, rawFolder);
    final String accountLabel = selectedAccount?.label ?? '';
    final bool multiSelect = ref.watch(mailMultiSelectActiveProvider);
    final int multiCount = ref.watch(mailSelectedIdsProvider).length;
    final bool sendMailEnabled = selectedAccount == null ||
        accountCanSendMail(selectedAccount);
    final bool nntpMail = selectedAccount != null &&
        isNntpMailboxBackend(selectedAccount);

    if (cfgAsync.hasValue && stripAccounts.isEmpty) {
      _scheduleOpenSettingsWhenNoAccounts();
    }

    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        final bool compact = mailLayoutIsCompact(
          constraints.maxWidth,
          constraints.maxHeight,
        );

        final Widget mainPane = compact
            ? _mobileMainPane(
                context,
                l10n: l10n,
                account: selectedAccount,
                folder: selectedFolder,
                folderParams: folderParams,
                conversationMode: conversationMode,
                inlineDesktop: inlineDesktop,
                isMobile: compact,
                useKeychain: useKeychain,
                sendMailEnabled: sendMailEnabled,
                messageSelected: messageSelected,
              )
            : _desktopMainPane(
                context,
                l10n: l10n,
                account: selectedAccount,
                folder: selectedFolder,
                folderDisplay: folderDisplay,
                accountLabel: accountLabel,
                folderParams: folderParams,
                conversationMode: conversationMode,
                selectedRow: selectedRow,
                detailAsync: detailAsync,
                detailFetchParams: detailParams,
                inlineDesktop: inlineDesktop,
                listPaneFraction: _desktopListPaneFraction,
                onListPaneFractionChanged: (double v) {
                  setState(() => _desktopListPaneFraction = v);
                },
                onListDetailSplitDragEnd: _persistDesktopPanePrefs,
                useKeychain: useKeychain,
                sendMailEnabled: sendMailEnabled,
                messageSelected: messageSelected,
                messageDeleteEnabled: messageDeleteEnabled,
                messageJunkEnabled: messageJunkEnabled,
                nntpMail: nntpMail,
              );

        return Scaffold(
          drawer: compact
              ? _mailDrawer(
                  context,
                  stripAccounts,
                  selectedAccountId,
                  selectedAccount,
                  folders,
                  mailFoldersState.unreadByFolder,
                  selectedFolder,
                  storeUnreadTotals,
                  useKeychain: useKeychain,
                )
              : null,
          appBar: compact
              ? multiSelect
                    ? AppBar(
                        leading: IconButton(
                          tooltip: l10n.cancelSelectionTooltip,
                          icon: const Icon(Icons.close),
                          onPressed: _clearMultiSelect,
                        ),
                        title: Text(l10n.mailToolbarSelectedCount(multiCount)),
                        actions: [
                          TextButton(
                            onPressed: multiCount == 0
                                ? null
                                : () => _tagMessagesForTransfer(
                                      MailPendingTransferKind.moveOp,
                                    ),
                            child: Text(l10n.messageActionMove),
                          ),
                          TextButton(
                            onPressed: multiCount == 0
                                ? null
                                : () => _tagMessagesForTransfer(
                                      MailPendingTransferKind.copyOp,
                                    ),
                            child: Text(l10n.messageActionCopy),
                          ),
                          TextButton(
                            onPressed: multiCount == 0 || !messageDeleteEnabled
                                ? null
                                : () => _stubAction('delete $multiCount'),
                            child: Text(l10n.messageActionDelete),
                          ),
                          TextButton(
                            onPressed: multiCount == 0 || !messageJunkEnabled
                                ? null
                                : () => _stubAction('junk $multiCount'),
                            child: Text(l10n.messageActionJunk),
                          ),
                        ],
                      )
                    : AppBar(
                        leading: Builder(
                          builder: (BuildContext context) => IconButton(
                            tooltip: l10n.accountsAndFoldersTooltip,
                            icon: const LucideIcon(LucideIcons.menu),
                            onPressed: () => Scaffold.of(context).openDrawer(),
                          ),
                        ),
                        title: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Text(
                              folderDisplay.isEmpty
                                  ? l10n.folderLabel
                                  : folderDisplay,
                              style: Theme.of(context).textTheme.titleLarge,
                              overflow: TextOverflow.ellipsis,
                            ),
                            Text(
                              accountLabel.isEmpty ? ' ' : accountLabel,
                              style: Theme.of(context)
                                  .textTheme
                                  .bodySmall
                                  ?.copyWith(
                                    color: Theme.of(
                                      context,
                                    ).colorScheme.onSurfaceVariant,
                                  ),
                              overflow: TextOverflow.ellipsis,
                            ),
                          ],
                        ),
                        actions: [
                          const MessageSortButton(),
                          PopupMenuButton<String>(
                            tooltip: l10n.messageMenuTooltip,
                            icon: const LucideIcon(LucideIcons.ellipsisVertical),
                            onSelected: (String value) {
                              if (value == 'tag-move') {
                                _tagMessagesForTransfer(
                                  MailPendingTransferKind.moveOp,
                                );
                                return;
                              }
                              if (value == 'tag-copy') {
                                _tagMessagesForTransfer(
                                  MailPendingTransferKind.copyOp,
                                );
                                return;
                              }
                              final bool canReply =
                                  sendMailEnabled && messageSelected;
                              if (!canReply &&
                                  (value == 'reply' ||
                                      value == 'reply-all' ||
                                      value == 'forward')) {
                                return;
                              }
                              if (nntpMail &&
                                  (value == 'reply-all' || value == 'forward')) {
                                return;
                              }
                              if (value == 'junk' &&
                                  (!messageSelected || !messageJunkEnabled)) {
                                return;
                              }
                              if (value == 'delete' &&
                                  (!messageSelected || !messageDeleteEnabled)) {
                                return;
                              }
                              _stubAction(value);
                            },
                            itemBuilder: (BuildContext context) {
                              final bool canReply =
                                  sendMailEnabled && messageSelected;
                              return [
                                PopupMenuItem(
                                  value: 'reply',
                                  enabled: canReply,
                                  child: Text(l10n.messageActionReply),
                                ),
                                if (!nntpMail)
                                  PopupMenuItem(
                                    value: 'reply-all',
                                    enabled: canReply,
                                    child: Text(l10n.messageActionReplyAll),
                                  ),
                                if (!nntpMail)
                                  PopupMenuItem(
                                    value: 'forward',
                                    enabled: canReply,
                                    child: Text(l10n.messageActionForward),
                                  ),
                                PopupMenuItem(
                                  value: 'delete',
                                  enabled:
                                      messageSelected && messageDeleteEnabled,
                                  child: Text(l10n.messageActionDelete),
                                ),
                                PopupMenuItem(
                                  value: 'junk',
                                  enabled:
                                      messageSelected && messageJunkEnabled,
                                  child: Text(l10n.messageActionJunk),
                                ),
                                PopupMenuItem(
                                  value: 'tag-move',
                                  enabled: messageSelected,
                                  child: Text(l10n.messageActionMove),
                                ),
                                PopupMenuItem(
                                  value: 'tag-copy',
                                  enabled: messageSelected,
                                  child: Text(l10n.messageActionCopy),
                                ),
                              ];
                            },
                          ),
                          IconButton(
                            tooltip: sendMailEnabled
                                ? l10n.composeTooltip
                                : l10n.composeNeedTransportTooltip,
                            icon: const LucideIcon(LucideIcons.squarePen),
                            onPressed: sendMailEnabled
                                ? _openComposeSmart
                                : null,
                          ),
                        ],
                      )
              : null,
          body: compact
              ? mainPane
              : Row(
                  children: [
                    SizedBox(
                      width: _kAccountRailWidth,
                      child: Column(
                        children: [
                          Expanded(
                            child: StoreSwitcher(
                              accounts: stripAccounts,
                              showLabels: false,
                              selectedAccountId: selectedAccountId,
                              storeUnreadTotals: storeUnreadTotals,
                              onSelect: _selectAccount,
                            ),
                          ),
                          Padding(
                            padding: const EdgeInsets.only(bottom: 8),
                            child: _railSettingsButton(
                              width: _kAccountRailWidth,
                              onPressed: () => Navigator.of(
                                context,
                              ).pushNamed('/settings'),
                            ),
                          ),
                        ],
                      ),
                    ),
                    const VerticalDivider(width: 1),
                    SizedBox(
                      width: _folderPaneWidth.clamp(
                        kMailDesktopMinFolderWidth,
                        _maxFolderPaneWidth(constraints.maxWidth),
                      ),
                      child: selectedAccount != null &&
                              (isEmailMailboxBackend(selectedAccount) ||
                                  isConversationBackend(selectedAccount))
                          ? FolderMailPane(
                              account: selectedAccount,
                              folders: folders,
                              unreadByFolder: mailFoldersState.unreadByFolder,
                              selectedFolder: selectedFolder,
                              onSelectFolder: (String folder) {
                                _selectFolder(selectedAccount, folder);
                              },
                              onReloadFolders: () =>
                                  _reloadFoldersAfterMutation(selectedAccount),
                              onPendingTransferToFolder: (String folder) =>
                                  _completePendingTransferToFolder(
                                    selectedAccount,
                                    folder,
                                  ),
                              enableMailDragTarget:
                                  isEmailMailboxBackend(selectedAccount),
                              onMailDragToFolder: isEmailMailboxBackend(
                                      selectedAccount)
                                  ? (MailListDragPayload p, String folder,
                                          {required bool asCopy}) =>
                                      _completeMailDragDrop(
                                        selectedAccount,
                                        p,
                                        folder,
                                        asCopy: asCopy,
                                      )
                                  : null,
                            )
                          : FolderTree(
                              folders: folders,
                              unreadByFolder: mailFoldersState.unreadByFolder,
                              selectedFolder: selectedFolder,
                              onSelect: (String folder) {
                                if (selectedAccount != null) {
                                  _selectFolder(selectedAccount, folder);
                                }
                              },
                            ),
                    ),
                    DesktopMailSplitter(
                      axis: Axis.horizontal,
                      onDragDelta: (double dx) {
                        setState(() {
                          _folderPaneWidth = (_folderPaneWidth + dx).clamp(
                            kMailDesktopMinFolderWidth,
                            _maxFolderPaneWidth(constraints.maxWidth),
                          );
                        });
                      },
                      onDragEnd: _persistDesktopPanePrefs,
                    ),
                    Expanded(child: mainPane),
                  ],
                ),
        );
      },
    );
  }

  Widget _mobileMainPane(
    BuildContext context, {
    required AppLocalizations l10n,
    required AppAccount? account,
    required String? folder,
    required SessionFolderParams? folderParams,
    required bool conversationMode,
    required bool inlineDesktop,
    required bool isMobile,
    required bool useKeychain,
    required bool sendMailEnabled,
    required bool messageSelected,
  }) {
    if (folderParams == null) {
      return Center(child: Text(l10n.selectFolder));
    }
    if (conversationMode) {
      return ChatView(folderParams: folderParams);
    }
    return MessageList(
      folderParams: folderParams,
      onOpen: (MessageListRow row) {
        _openMessage(
          context,
          account: account,
          folder: folder,
          row: row,
          isMobile: isMobile,
          inlineDesktop: inlineDesktop,
          useKeychain: useKeychain,
        );
      },
    );
  }

  Widget _desktopMainPane(
    BuildContext context, {
    required AppLocalizations l10n,
    required AppAccount? account,
    required String? folder,
    required String folderDisplay,
    required String accountLabel,
    required SessionFolderParams? folderParams,
    required bool conversationMode,
    required MessageListRow? selectedRow,
    required AsyncValue<MailMessageDetailView>? detailAsync,
    required MailMessageDetailParams? detailFetchParams,
    required bool inlineDesktop,
    required double listPaneFraction,
    required ValueChanged<double> onListPaneFractionChanged,
    required VoidCallback onListDetailSplitDragEnd,
    required bool useKeychain,
    required bool sendMailEnabled,
    required bool messageSelected,
    required bool messageDeleteEnabled,
    required bool messageJunkEnabled,
    required bool nntpMail,
  }) {
    if (conversationMode) {
      // Nostr/Matrix: full conversation UI (composer below timeline); no duplicate mail toolbar —
      // same as mobile [ _mobileMainPane ].
      if (folderParams == null) {
        return Center(child: Text(l10n.selectFolder));
      }
      return ChatView(folderParams: folderParams);
    }

    final Widget list = folderParams == null
        ? Center(child: Text(l10n.selectFolder))
        : MessageList(
            folderParams: folderParams,
            enableDesktopMailDrag: true,
            onOpen: (MessageListRow row) {
              _openMessage(
                context,
                account: account,
                folder: folder,
                row: row,
                isMobile: false,
                inlineDesktop: inlineDesktop,
                useKeychain: useKeychain,
              );
            },
          );

    final Widget detail;
    if (detailAsync == null) {
      detail = MessageView(
        subject: selectedRow?.subject ?? l10n.selectMessage,
        subjectInAppBar: false,
        fromRaw: '',
        toRaw: '',
        ccRaw: null,
        dateMs: null,
        bodyHtml: null,
        bodyPlain: l10n.selectMessageToRead,
        attachmentFetchParams: detailFetchParams,
      );
    } else {
      detail = detailAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (Object e, StackTrace _) => Center(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Text(l10n.operationFailed(e.toString())),
          ),
        ),
        data: (MailMessageDetailView d) => MessageView(
          subject: d.subject.isEmpty
              ? (selectedRow?.subject ?? l10n.messageTitle)
              : d.subject,
          subjectInAppBar: false,
          fromRaw: d.fromRaw,
          toRaw: d.toRaw,
          ccRaw: d.ccRaw,
          dateMs: d.dateMs,
          bodyHtml: d.bodyHtml,
          bodyPlain: d.matrixE2eeUndecryptable == true
              ? ''
              : (d.bodyPlain ?? l10n.noTextBody),
          attachments: d.attachments,
          attachmentFetchParams: detailFetchParams,
          mailBodyStoreKey: d.mailBodyStoreKey,
          matrixE2eeUndecryptable: d.matrixE2eeUndecryptable == true,
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        MailToolbar(
          folderDisplay: folderDisplay,
          accountLabel: accountLabel,
          desktopActions: true,
          messageActionsEnabled: messageSelected,
          messageDeleteEnabled: messageDeleteEnabled,
          messageJunkEnabled: messageJunkEnabled,
          sendActionsEnabled: sendMailEnabled,
          showReplyAllForward: !nntpMail,
          onCompose: _openComposeSmart,
          onStub: _stubAction,
          onTagMove: messageSelected
              ? () => _tagMessagesForTransfer(MailPendingTransferKind.moveOp)
              : null,
          onTagCopy: messageSelected
              ? () => _tagMessagesForTransfer(MailPendingTransferKind.copyOp)
              : null,
        ),
        Expanded(
          child: inlineDesktop
              ? LayoutBuilder(
                  builder: (BuildContext context, BoxConstraints c) {
                    final double splitter = kMailDesktopSplitterHitSize;
                    final double avail = c.maxHeight - splitter;
                    if (avail <= 1) {
                      return list;
                    }
                    final double minF =
                        (kMailDesktopMinListHeight / avail).clamp(0.0, 1.0);
                    final double maxF =
                        (1.0 - kMailDesktopMinDetailHeight / avail)
                            .clamp(0.0, 1.0);
                    final double f;
                    if (minF <= maxF) {
                      f = listPaneFraction.clamp(minF, maxF);
                    } else {
                      f = 0.5;
                    }
                    final double listH = avail * f;
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        SizedBox(height: listH, child: list),
                        DesktopMailSplitter(
                          axis: Axis.vertical,
                          onDragDelta: (double dy) {
                            final double base = minF <= maxF
                                ? listPaneFraction.clamp(minF, maxF)
                                : 0.5;
                            final double next =
                                (base + dy / avail).clamp(minF, maxF);
                            onListPaneFractionChanged(next);
                          },
                          onDragEnd: onListDetailSplitDragEnd,
                        ),
                        Expanded(child: detail),
                      ],
                    );
                  },
                )
              : list,
        ),
      ],
    );
  }
}
