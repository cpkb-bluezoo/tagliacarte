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

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../models/mail_pending_transfer.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../util/mail_account_policy.dart';
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
  void _onMenuAction(BuildContext context, String action) {
    if (!accountCanSendMail(widget.account) &&
        (action == 'reply' || action == 'reply-all' || action == 'forward')) {
      return;
    }
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (action == 'move') {
      ref.read(mailPendingTransferProvider.notifier).state = MailPendingTransfer(
        kind: MailPendingTransferKind.moveOp,
        storeUri: widget.params.storeUri,
        credentialKey: widget.params.credentialKey,
        sourceFolder: widget.params.folderName,
        messageIds: <String>[widget.params.messageId],
        useKeychain: widget.params.useKeychain,
      );
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.pendingMoveTagged(1))),
      );
      return;
    }
    if (action == 'copy') {
      ref.read(mailPendingTransferProvider.notifier).state = MailPendingTransfer(
        kind: MailPendingTransferKind.copyOp,
        storeUri: widget.params.storeUri,
        credentialKey: widget.params.credentialKey,
        sourceFolder: widget.params.folderName,
        messageIds: <String>[widget.params.messageId],
        useKeychain: widget.params.useKeychain,
      );
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.pendingCopyTagged(1))),
      );
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
    final AppLocalizations l10n = AppLocalizations.of(context);
    final AsyncValue<MailMessageDetailView> async =
        ref.watch(mailMessageDetailProvider(params));

    if (isNativeMailStoreUri(params.storeUri)) {
      ref.listen<AsyncValue<MailMessageDetailView>>(
        mailMessageDetailProvider(params),
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
              params,
              accountIdOverride: widget.account.id,
            ),
          );
        },
      );
    }

    if (isImapStoreUri(params.storeUri)) {
      ref.listen<AsyncValue<MailMessageDetailView>>(
        mailMessageDetailProvider(params),
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
                final bool? saved = await showImapCredentialDialog(
                  context,
                  credentialId: params.credentialKey,
                  storeUri: params.storeUri,
                  useKeychain: params.useKeychain,
                );
                if (saved == true && context.mounted) {
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
      data: (MailMessageDetailView d) => Scaffold(
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
                PopupMenuItem(
                  value: 'reply-all',
                  enabled: sendOk,
                  child: Text(l10n.messageActionReplyAll),
                ),
                PopupMenuItem(
                  value: 'forward',
                  enabled: sendOk,
                  child: Text(l10n.messageActionForward),
                ),
                PopupMenuItem(
                  value: 'delete',
                  child: Text(l10n.messageActionDelete),
                ),
                PopupMenuItem(value: 'junk', child: Text(l10n.messageActionJunk)),
                PopupMenuItem(value: 'move', child: Text(l10n.messageActionMove)),
                PopupMenuItem(value: 'copy', child: Text(l10n.messageActionCopy)),
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
      ),
    );
  }
}
