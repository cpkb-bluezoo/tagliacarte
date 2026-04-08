/*
 * message_detail_screen.dart
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
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../models/mail_pending_transfer.dart';
import '../providers/app_state.dart';
import '../providers/mail_location_persist.dart';
import '../providers/mail_sync.dart';
import '../providers/message_sort_persist.dart';
import '../providers/session_state.dart';
import '../rust/frb_api.dart';
import '../util/mail_account_policy.dart';
import 'compose_screen.dart';
import '../widgets/gmail_oauth_dialog.dart';
import '../widgets/imap_credential_dialog.dart';
import '../rust/tagliacarte_api.dart';
import '../widgets/lucide_icon.dart';
import '../widgets/message_view.dart';

String _menuActionLabel(AppLocalizations l10n, String action) {
  switch (action) {
    case 'reply':
      return l10n.messageActionReply;
    case 'reply-all':
      return l10n.messageActionReplyAll;
    case 'forward':
      return l10n.messageActionForward;
    case 'delete':
      return l10n.messageActionDelete;
    case 'junk':
      return l10n.messageActionJunk;
    case 'move':
      return l10n.messageActionMove;
    case 'copy':
      return l10n.messageActionCopy;
    default:
      return action;
  }
}

class StoreMessageDetailScreen extends ConsumerStatefulWidget {
  const StoreMessageDetailScreen({
    super.key,
    required this.params,
    required this.titleFallback,
    required this.account,
  });

  final MailMessageDetailParams params;
  final String titleFallback;

  /// Used to disable reply/forward when the store has no outgoing transport.
  final AppAccount account;

  @override
  ConsumerState<StoreMessageDetailScreen> createState() =>
      _StoreMessageDetailScreenState();
}

class _StoreMessageDetailScreenState
    extends ConsumerState<StoreMessageDetailScreen> {
  Future<void> _junkCurrentMessage(BuildContext context) async {
    final MailMessageDetailParams p = widget.params;
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String junkFolder = mailboxJunkFolderDisplayName(widget.account);
    try {
      final String json = await frbTransferMailMessages(
        sourceAccountId: p.accountId,
        sourceFolder: p.folderName,
        destAccountId: p.accountId,
        destFolder: junkFolder,
        messageIds: <String>[p.messageId],
        isMove: true,
      );
      final Map<String, dynamic> decoded =
          jsonDecode(json) as Map<String, dynamic>;
      final int failed = (decoded['failedCount'] as num?)?.toInt() ?? 0;
      final int ok = (decoded['okCount'] as num?)?.toInt() ?? 0;
      if (!context.mounted) {
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
      void inv(String folder) {
        ref.invalidate(
          folderMailboxListProvider(
            SessionFolderParams(
              accountId: p.accountId,
              folderName: folder,
              messageListSort: sort,
            ),
          ),
        );
      }

      inv(p.folderName);
      inv(junkFolder);
      ref.read(selectedMessageProvider.notifier).state = null;
      ref.read(mailMultiSelectActiveProvider.notifier).state = false;
      ref.read(mailSelectedIdsProvider.notifier).clear();
      unawaited(persistMailLocation(ref));
      unawaited(sessionRefreshFolders(accountId: widget.account.id));
      if (context.mounted) {
        Navigator.of(context).pop();
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.transferFailed(e.toString()))),
        );
      }
    }
  }

  Future<void> _deleteCurrentMessage(BuildContext context) async {
    final MailMessageDetailParams p = widget.params;
    final AppLocalizations l10n = AppLocalizations.of(context);
    try {
      final String json = await frbDeleteMailMessages(
        accountId: p.accountId,
        folderName: p.folderName,
        messageIds: <String>[p.messageId],
      );
      final Map<String, dynamic> decoded =
          jsonDecode(json) as Map<String, dynamic>;
      final int failed = (decoded['failedCount'] as num?)?.toInt() ?? 0;
      final int ok = (decoded['okCount'] as num?)?.toInt() ?? 0;
      if (!context.mounted) {
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
            accountId: p.accountId,
            folderName: p.folderName,
            messageListSort: sort,
          ),
        ),
      );
      ref.read(selectedMessageProvider.notifier).state = null;
      unawaited(persistMailLocation(ref));
      unawaited(sessionRefreshFolders(accountId: widget.account.id));
      if (context.mounted) {
        Navigator.of(context).pop();
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.deleteMessagesFailed(e.toString()))),
        );
      }
    }
  }

  void _onMenuAction(BuildContext context, String action) {
    if (!accountCanSendMail(widget.account) &&
        (action == 'reply' || action == 'reply-all' || action == 'forward')) {
      return;
    }
    if (isNntpMailboxBackend(widget.account) &&
        (action == 'reply-all' || action == 'forward')) {
      return;
    }
    if (action == 'delete' &&
        mailboxMoveToTrashDeleteUnavailable(
          widget.account,
          widget.params.folderName,
        )) {
      return;
    }
    if (action == 'junk' &&
        (!accountSupportsMailboxTrashAndJunkMoves(widget.account) ||
            mailboxJunkMoveUnavailable(
              widget.account,
              widget.params.folderName,
            ))) {
      return;
    }
    if (isEmailMailboxBackend(widget.account) &&
        (action == 'reply' || action == 'reply-all' || action == 'forward')) {
      final ComposeReplyKind kind = switch (action) {
        'reply' => ComposeReplyKind.reply,
        'reply-all' => ComposeReplyKind.replyAll,
        _ => ComposeReplyKind.forward,
      };
      Navigator.of(context).pushNamed(
        '/compose',
        arguments: ComposeIntent(
          accountId: widget.params.accountId,
          replyFolderName: widget.params.folderName,
          replyMessageId: widget.params.messageId,
          replyKind: kind,
        ),
      );
      return;
    }
    if (action == 'reply' && isNntpMailboxBackend(widget.account)) {
      Navigator.of(context).pushNamed(
        '/compose',
        arguments: ComposeIntent(
          accountId: widget.params.accountId,
          replyFolderName: widget.params.folderName,
          replyMessageId: widget.params.messageId,
        ),
      );
      return;
    }
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (action == 'move') {
      ref
          .read(mailPendingTransferProvider.notifier)
          .state = MailPendingTransfer(
        kind: MailPendingTransferKind.moveOp,
        sourceAccountId: widget.account.id,
        sourceFolder: widget.params.folderName,
        messageIds: <String>[widget.params.messageId],
      );
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.pendingMoveTagged(1))));
      return;
    }
    if (action == 'copy') {
      ref
          .read(mailPendingTransferProvider.notifier)
          .state = MailPendingTransfer(
        kind: MailPendingTransferKind.copyOp,
        sourceAccountId: widget.account.id,
        sourceFolder: widget.params.folderName,
        messageIds: <String>[widget.params.messageId],
      );
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.pendingCopyTagged(1))));
      return;
    }
    if (action == 'delete' && isEmailMailboxBackend(widget.account)) {
      unawaited(_deleteCurrentMessage(context));
      return;
    }
    if (action == 'junk' && isEmailMailboxBackend(widget.account)) {
      unawaited(_junkCurrentMessage(context));
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          l10n.messageActionFeedback(
            _menuActionLabel(l10n, action),
            widget.params.messageId,
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final MailMessageDetailParams params = widget.params;
    final bool sendOk = accountCanSendMail(widget.account);
    final bool nntp = isNntpMailboxBackend(widget.account);
    final AppLocalizations l10n = AppLocalizations.of(context);
    final AsyncValue<MailMessageDetailView> async = ref.watch(
      mailMessageDetailProvider(params),
    );

    if (isEmailMailboxBackend(widget.account)) {
      ref.listen<AsyncValue<MailMessageDetailView>>(
        mailMessageDetailProvider(params),
        (
          AsyncValue<MailMessageDetailView>? prev,
          AsyncValue<MailMessageDetailView> next,
        ) {
          if (prev is AsyncData<MailMessageDetailView>) {
            return;
          }
          if (next is! AsyncData<MailMessageDetailView>) {
            return;
          }
          unawaited(
            markMessageReadAfterDetailLoaded(
              ref,
              params,
              accountIdOverride: widget.account.id,
            ),
          );
        },
      );
    }

    if (isImapStyleMailboxBackend(widget.account)) {
      ref.listen<AsyncValue<MailMessageDetailView>>(
        mailMessageDetailProvider(params),
        (
          AsyncValue<MailMessageDetailView>? prev,
          AsyncValue<MailMessageDetailView> next,
        ) {
          next.whenOrNull(
            error: (Object e, StackTrace _) {
              if (!isMissingImapCredentialsError(e)) {
                return;
              }
              WidgetsBinding.instance.addPostFrameCallback((_) async {
                if (!context.mounted) {
                  return;
                }
                final bool? saved;
                if (isGmailMailboxBackend(widget.account)) {
                  saved = await showGmailOAuthDialog(
                    context,
                    accountId: widget.account.id,
                    subtitle: widget.account.label,
                  );
                } else {
                  saved = await showImapCredentialDialog(
                    context,
                    accountId: widget.account.id,
                    usernameHint:
                        widget.account.attrs['username'] ??
                        widget.account.attrs['email'],
                    subtitle: widget.account.label,
                  );
                }
                if (!context.mounted) {
                  return;
                }
                if (saved == true && context.mounted) {
                  await sessionRefreshFolders(accountId: widget.account.id);
                  ref.invalidate(mailMessageDetailProvider(params));
                }
              });
            },
          );
        },
      );
    }

    return async.when(
      loading: () => Scaffold(
        appBar: AppBar(
          title: Text(
            widget.titleFallback.isEmpty
                ? l10n.messageTitle
                : widget.titleFallback,
          ),
        ),
        body: const Center(child: CircularProgressIndicator()),
      ),
      error: (Object e, StackTrace _) => Scaffold(
        appBar: AppBar(title: Text(widget.titleFallback)),
        body: Center(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Text(l10n.operationFailed(e.toString())),
          ),
        ),
      ),
      data: (MailMessageDetailView d) {
        final bool deleteEnabled = !mailboxMoveToTrashDeleteUnavailable(
          widget.account,
          widget.params.folderName,
        );
        final bool junkEnabled = accountSupportsMailboxTrashAndJunkMoves(
              widget.account,
            ) &&
            !mailboxJunkMoveUnavailable(
              widget.account,
              widget.params.folderName,
            );
        return Scaffold(
          appBar: AppBar(
            title: Text(
              d.subject.isEmpty
                  ? (widget.titleFallback.isEmpty
                        ? l10n.messageTitle
                        : widget.titleFallback)
                  : d.subject,
            ),
            actions: [
              PopupMenuButton<String>(
                tooltip: l10n.messageMenuTooltip,
                icon: const LucideIcon(LucideIcons.ellipsisVertical),
                onSelected: (String value) => _onMenuAction(context, value),
                itemBuilder: (BuildContext context) => [
                  PopupMenuItem(
                    value: 'reply',
                    enabled: sendOk,
                    child: Text(l10n.messageActionReply),
                  ),
                  if (!nntp)
                    PopupMenuItem(
                      value: 'reply-all',
                      enabled: sendOk,
                      child: Text(l10n.messageActionReplyAll),
                    ),
                  if (!nntp)
                    PopupMenuItem(
                      value: 'forward',
                      enabled: sendOk,
                      child: Text(l10n.messageActionForward),
                    ),
                  PopupMenuItem(
                    value: 'delete',
                    enabled: deleteEnabled,
                    child: Text(l10n.messageActionDelete),
                  ),
                  PopupMenuItem(
                    value: 'junk',
                    enabled: junkEnabled,
                    child: Text(l10n.messageActionJunk),
                  ),
                  PopupMenuItem(
                    value: 'move',
                    child: Text(l10n.messageActionMove),
                  ),
                  PopupMenuItem(
                    value: 'copy',
                    child: Text(l10n.messageActionCopy),
                  ),
                ],
              ),
            ],
          ),
          body: MessageView(
            subject: d.subject,
            subjectInAppBar: true,
            fromRaw: d.fromRaw,
            toRaw: d.toRaw,
            ccRaw: d.ccRaw,
            dateMs: d.dateMs,
            bodyHtml: d.bodyHtml,
            bodyPlain: d.bodyPlain ?? l10n.noTextBody,
            attachments: d.attachments,
            attachmentFetchParams: params,
            mailBodyStoreKey: d.mailBodyStoreKey,
          ),
        );
      },
    );
  }
}
