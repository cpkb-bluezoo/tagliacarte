/*
 * mail_toolbar.dart
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

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../providers/app_state.dart';
import 'lucide_icon.dart';
import 'message_sort_button.dart';

class MailToolbar extends ConsumerWidget {
  const MailToolbar({
    super.key,
    required this.folderDisplay,
    required this.accountLabel,
    required this.desktopActions,
    required this.messageActionsEnabled,
    required this.sendActionsEnabled,
    required this.onCompose,
    required this.onStub,
    this.onTagMove,
    this.onTagCopy,
  });

  /// Localised folder name (large); e.g. INBOX → "Inbox".
  final String folderDisplay;

  /// Account display name (smaller line below folder).
  final String accountLabel;
  final bool desktopActions;

  /// When false, message-scoped actions (reply, delete, …) are disabled.
  final bool messageActionsEnabled;

  /// When false, compose / reply / forward / reply-all are disabled (e.g. no outgoing transport).
  final bool sendActionsEnabled;
  final VoidCallback onCompose;
  final void Function(String action) onStub;
  final VoidCallback? onTagMove;
  final VoidCallback? onTagCopy;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool multi = ref.watch(mailMultiSelectActiveProvider);
    final Set<String> selected = ref.watch(mailSelectedIdsProvider);
    final bool canReply =
        sendActionsEnabled && messageActionsEnabled;

    if (multi) {
      return Material(
        elevation: 0,
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        child: SafeArea(
          bottom: false,
          child: SizedBox(
            height: kToolbarHeight,
            child: Row(
              children: [
                IconButton(
                  tooltip: l10n.cancelSelectionTooltip,
                  icon: const Icon(Icons.close),
                  onPressed: () {
                    ref.read(mailMultiSelectActiveProvider.notifier).state =
                        false;
                    ref.read(mailSelectedIdsProvider.notifier).clear();
                  },
                ),
                Expanded(
                  child: Text(
                    l10n.multiSelectCount(selected.length),
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                TextButton(
                  onPressed: selected.isEmpty ? null : onTagMove,
                  child: Text(l10n.messageActionMove),
                ),
                TextButton(
                  onPressed: selected.isEmpty ? null : onTagCopy,
                  child: Text(l10n.messageActionCopy),
                ),
                TextButton(
                  onPressed: selected.isEmpty
                      ? null
                      : () => onStub('delete ${selected.length}'),
                  child: Text(l10n.messageActionDelete),
                ),
              ],
            ),
          ),
        ),
      );
    }

    return Material(
      elevation: 1,
      color: Theme.of(context).colorScheme.surface,
      child: SafeArea(
        bottom: false,
        child: SizedBox(
          height: 72,
          child: Row(
            children: [
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        folderDisplay.isEmpty ? l10n.folderLabel : folderDisplay,
                        style: Theme.of(context).textTheme.titleLarge,
                        overflow: TextOverflow.ellipsis,
                      ),
                      Text(
                        accountLabel.isEmpty ? ' ' : accountLabel,
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: Theme.of(context).colorScheme.onSurfaceVariant,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
              ),
              const MessageSortButton(),
              if (desktopActions) ...[
                IconButton(
                  tooltip: l10n.messageActionReply,
                  onPressed: canReply ? () => onStub('reply') : null,
                  icon: const LucideIcon(LucideIcons.reply),
                ),
                IconButton(
                  tooltip: l10n.messageActionReplyAll,
                  onPressed: canReply ? () => onStub('reply-all') : null,
                  icon: const LucideIcon(LucideIcons.replyAll),
                ),
                IconButton(
                  tooltip: l10n.messageActionForward,
                  onPressed: canReply ? () => onStub('forward') : null,
                  icon: const LucideIcon(LucideIcons.forward),
                ),
                IconButton(
                  tooltip: l10n.messageActionDelete,
                  onPressed:
                      messageActionsEnabled ? () => onStub('delete') : null,
                  icon: const LucideIcon(LucideIcons.trash2),
                ),
                IconButton(
                  tooltip: l10n.messageActionJunk,
                  onPressed:
                      messageActionsEnabled ? () => onStub('junk') : null,
                  icon: const LucideIcon(LucideIcons.ban),
                ),
                IconButton(
                  tooltip: l10n.messageActionMove,
                  onPressed: messageActionsEnabled ? onTagMove : null,
                  icon: const Icon(Icons.drive_file_move_outline),
                ),
                IconButton(
                  tooltip: l10n.messageActionCopy,
                  onPressed: messageActionsEnabled ? onTagCopy : null,
                  icon: const Icon(Icons.copy_outlined),
                ),
              ] else
                PopupMenuButton<String>(
                  tooltip: l10n.mailToolbarMoreTooltip,
                  icon: const LucideIcon(LucideIcons.ellipsisVertical),
                  onSelected: (String value) {
                    if (value == 'tag-move') {
                      onTagMove?.call();
                      return;
                    }
                    if (value == 'tag-copy') {
                      onTagCopy?.call();
                      return;
                    }
                    if (!sendActionsEnabled &&
                        (value == 'reply' ||
                            value == 'reply-all' ||
                            value == 'forward')) {
                      return;
                    }
                    if (!messageActionsEnabled &&
                        (value == 'reply' ||
                            value == 'reply-all' ||
                            value == 'forward' ||
                            value == 'delete' ||
                            value == 'junk')) {
                      return;
                    }
                    onStub(value);
                  },
                  itemBuilder: (BuildContext context) => [
                    PopupMenuItem(
                      value: 'reply',
                      enabled: canReply,
                      child: Text(l10n.messageActionReply),
                    ),
                    PopupMenuItem(
                      value: 'reply-all',
                      enabled: canReply,
                      child: Text(l10n.messageActionReplyAll),
                    ),
                    PopupMenuItem(
                      value: 'forward',
                      enabled: canReply,
                      child: Text(l10n.messageActionForward),
                    ),
                    PopupMenuItem(
                      value: 'delete',
                      enabled: messageActionsEnabled,
                      child: Text(l10n.messageActionDelete),
                    ),
                    PopupMenuItem(
                      value: 'junk',
                      enabled: messageActionsEnabled,
                      child: Text(l10n.messageActionJunk),
                    ),
                    PopupMenuItem(
                      value: 'tag-move',
                      enabled: messageActionsEnabled && onTagMove != null,
                      child: Text(l10n.messageActionMove),
                    ),
                    PopupMenuItem(
                      value: 'tag-copy',
                      enabled: messageActionsEnabled && onTagCopy != null,
                      child: Text(l10n.messageActionCopy),
                    ),
                  ],
                ),
              IconButton(
                tooltip: sendActionsEnabled
                    ? l10n.composeTooltip
                    : l10n.composeNeedTransportTooltip,
                onPressed: sendActionsEnabled ? onCompose : null,
                icon: const LucideIcon(LucideIcons.squarePen),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
