/*
 * folder_mail_pane.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../models/mail_drag_data.dart';
import '../models/mail_pending_transfer.dart';
import '../models/message_row.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../providers/message_sort_persist.dart';
import '../providers/nostr_peer_labels.dart';
import '../providers/session_state.dart';
import '../rust/frb_api.dart';
import '../rust/frb_api/frb_mail.dart';
import '../rust/tagliacarte_api.dart';
import '../models/subscription_folder_row.dart';
import '../util/folder_display.dart';
import '../util/folder_mail_policy.dart';
import '../util/mail_account_policy.dart';
import 'folder_tree.dart';
import 'hierarchical_folder_tree.dart';
import 'lucide_icon.dart';

/// Folder list with optional FAB (new top-level folder) and per-row context actions.
class FolderMailPane extends ConsumerStatefulWidget {
  const FolderMailPane({
    super.key,
    required this.account,
    required this.folders,
    required this.selectedFolder,
    required this.onSelectFolder,
    required this.onReloadFolders,
    this.unreadByFolder = const <String, int>{},
    this.onPendingTransferToFolder,
    this.enableMailDragTarget = false,
    this.onMailDragToFolder,
  });

  final AppAccount account;
  final List<String> folders;
  final Map<String, int> unreadByFolder;
  final String? selectedFolder;
  final ValueChanged<String> onSelectFolder;
  final Future<void> Function() onReloadFolders;
  /// Completes a menu-tagged move/copy when user picks a target folder (same or other account pane).
  final Future<void> Function(String folderPath)? onPendingTransferToFolder;

  /// Desktop: message list rows can be dragged onto folders (same store only).
  final bool enableMailDragTarget;

  final Future<void> Function(
    MailListDragPayload payload,
    String destFolder, {
    required bool asCopy,
  })? onMailDragToFolder;

  @override
  ConsumerState<FolderMailPane> createState() => _FolderMailPaneState();
}

class _FolderMailPaneState extends ConsumerState<FolderMailPane> {
  int _folderTabIndex = 0;
  final TextEditingController _nntpWildmatController = TextEditingController();
  List<SubscriptionFolderRow>? _nntpAvailableRows;

  @override
  void dispose() {
    _nntpWildmatController.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(FolderMailPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.account.id != widget.account.id) {
      _folderTabIndex = 0;
      _nntpAvailableRows = null;
    }
  }

  /// Subscribe / Unsubscribe menu labels; Matrix uses room terminology.
  String _l10nSubscriptionSubscribeLabel(AppLocalizations l10n) {
    return isMatrixMailboxBackend(widget.account)
        ? l10n.folderActionJoinRoom
        : l10n.folderActionSubscribe;
  }

  String _l10nSubscriptionUnsubscribeLabel(AppLocalizations l10n) {
    return isMatrixMailboxBackend(widget.account)
        ? l10n.folderActionLeaveRoom
        : l10n.folderActionUnsubscribe;
  }

  bool _mailDropPredicate(MailListDragPayload p, String f) {
    return p.sourceAccountId == widget.account.id && p.sourceFolder != f;
  }

  Future<void> _onMailDrop(MailListDragPayload p, String f, bool c) {
    return widget.onMailDragToFolder!(p, f, asCopy: c);
  }

  /// Stable callback for folder rows + [DragTarget]s; must not be recreated every [build]
  /// (new closures made [HierarchicalFolderTree] / [DragTarget] churn and can blow the stack).
  void _openFolderMenu(BuildContext ctx, String folder, Offset position) {
    if (!isEmailMailboxBackend(widget.account)) {
      return;
    }
    final AppLocalizations l10n = AppLocalizations.of(ctx);
    final MailPendingTransfer? pending = ref.read(mailPendingTransferProvider);
    final RenderBox overlay =
        Overlay.of(ctx).context.findRenderObject()! as RenderBox;
    final RelativeRect positionRect = RelativeRect.fromRect(
      Rect.fromLTWH(position.dx, position.dy, 0, 0),
      Offset.zero & overlay.size,
    );
    final List<PopupMenuEntry<String>> entries = <PopupMenuEntry<String>>[];

    final MailPendingTransfer? pen = pending;
    if (widget.onPendingTransferToFolder != null &&
        pen != null &&
        !pen.isSameSource(
          accountId: widget.account.id,
          folder: folder,
        )) {
      if (pen.kind == MailPendingTransferKind.moveOp) {
        entries.add(
          PopupMenuItem<String>(
            value: 'move_here',
            child: Text(l10n.folderMoveHere),
          ),
        );
      } else {
        entries.add(
          PopupMenuItem<String>(
            value: 'copy_here',
            child: Text(l10n.folderCopyHere),
          ),
        );
      }
    }

    if (accountSupportsImapProtocolExpunge(widget.account)) {
      final String sort = messageListSortSymbolic(
        ref.read(messageSortFieldProvider),
        ref.read(messageSortAscendingProvider),
      );
      final FolderListVm vm = ref.read(
        folderMailboxListProvider(
          SessionFolderParams(
            accountId: widget.account.id,
            folderName: folder,
            messageListSort: sort,
          ),
        ),
      );
      final bool hasDeleted = vm.slots.any(
        (MessageListRow? r) => r?.markedForDeletion == true,
      );
      if (hasDeleted) {
        entries.add(
          PopupMenuItem<String>(
            value: 'expunge',
            child: Text(l10n.folderExpunge),
          ),
        );
      }
    }

    final bool canManage = storeSupportsFolderManagement(widget.account);
    final String? delimForMenu =
        ref.read(folderHierarchyDelimiterProvider);
    if (canManage) {
      if (delimForMenu != null &&
          delimForMenu.isNotEmpty &&
          folder.trim().isNotEmpty) {
        entries.add(
          PopupMenuItem<String>(
            value: 'sub',
            child: Text(l10n.folderNewSubfolder),
          ),
        );
      }
      if (!folderIsReservedInboxName(folder)) {
        entries.addAll(<PopupMenuItem<String>>[
          PopupMenuItem<String>(
            value: 'rename',
            child: Text(l10n.folderRename),
          ),
          PopupMenuItem<String>(
            value: 'delete',
            child: Text(l10n.folderDelete),
          ),
        ]);
      }
    }

    final String bt = widget.account.backendType.trim().toLowerCase();
    if (accountUsesSubscriptionFolderPane(widget.account) &&
        (bt == 'imap' || bt == 'imaps' || bt == 'gmail') &&
        _subscribedRowCanUnsubscribe(folder)) {
      entries.add(
        PopupMenuItem<String>(
          value: 'unsub_mailbox',
          child: Text(l10n.folderActionUnsubscribe),
        ),
      );
    }

    if (entries.isEmpty) {
      return;
    }
    showMenu<String>(
      context: ctx,
      position: positionRect,
      items: entries,
    ).then((String? action) {
      if (action == null || !ctx.mounted) {
        return;
      }
      final String? delimNow = ref.read(folderHierarchyDelimiterProvider);
      switch (action) {
        case 'unsub_mailbox':
          unawaited(_runUnsubscribeSubscribed(folder));
          break;
        case 'move_here':
        case 'copy_here':
          unawaited(
            widget.onPendingTransferToFolder?.call(folder) ??
                Future<void>.value(),
          );
          break;
        case 'expunge':
          unawaited(
            _runExpungeFolder(
              ctx,
              ref,
              widget.account,
              folder,
              widget.onReloadFolders,
            ),
          );
          break;
        case 'sub':
          if (delimNow == null || delimNow.isEmpty) {
            return;
          }
          unawaited(
            _promptSubfolder(
              ctx,
              widget.account,
              folder,
              delimNow,
              widget.onReloadFolders,
            ),
          );
          break;
        case 'rename':
          unawaited(
            _promptRenameFolder(
              ctx,
              widget.account,
              folder,
              widget.onReloadFolders,
            ),
          );
          break;
        case 'delete':
          unawaited(
            _confirmDeleteFolder(
              ctx,
              widget.account,
              folder,
              widget.onReloadFolders,
            ),
          );
          break;
      }
    });
  }

  bool _subscribedRowCanUnsubscribe(String folder) {
    return folder.trim().toUpperCase() != 'INBOX';
  }

  Future<void> _runUnsubscribeSubscribed(String folder) async {
    final String id = widget.account.id;
    final String b = widget.account.backendType.trim().toLowerCase();
    try {
      if (b == 'imap' || b == 'imaps' || b == 'gmail') {
        await frbImapUnsubscribeMailbox(accountId: id, mailbox: folder);
      } else if (b == 'nntp') {
        await frbNntpSetGroupSubscribed(
          accountId: id,
          group: folder,
          subscribed: false,
        );
      } else if (b == 'matrix') {
        await frbMatrixLeaveRoom(accountId: id, roomId: folder);
      }
      await widget.onReloadFolders();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              AppLocalizations.of(context).operationFailed(e.toString()),
            ),
          ),
        );
      }
    }
  }

  Future<void> _applySubscriptionAction(SubscriptionFolderRow row, bool subscribe) async {
    final String id = widget.account.id;
    final String b = widget.account.backendType.trim().toLowerCase();
    try {
      if (subscribe) {
        if (b == 'imap' || b == 'imaps' || b == 'gmail') {
          await frbImapSubscribeMailbox(accountId: id, mailbox: row.id);
        } else if (b == 'nntp') {
          await frbNntpSetGroupSubscribed(
            accountId: id,
            group: row.id,
            subscribed: true,
          );
        } else if (b == 'matrix') {
          await frbMatrixJoinRoom(
            accountId: id,
            roomIdOrAlias: row.id,
          );
        }
      } else {
        if (b == 'imap' || b == 'imaps' || b == 'gmail') {
          await frbImapUnsubscribeMailbox(accountId: id, mailbox: row.id);
        } else if (b == 'nntp') {
          await frbNntpSetGroupSubscribed(
            accountId: id,
            group: row.id,
            subscribed: false,
          );
        } else if (b == 'matrix') {
          await frbMatrixLeaveRoom(accountId: id, roomId: row.id);
        }
      }
      await widget.onReloadFolders();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              AppLocalizations.of(context).operationFailed(e.toString()),
            ),
          ),
        );
      }
    }
  }

  void _openSubscribedSubscriptionMenu(
    BuildContext ctx,
    String folder,
    Offset position,
  ) {
    final AppLocalizations loc = AppLocalizations.of(ctx);
    final RenderBox overlay =
        Overlay.of(ctx).context.findRenderObject()! as RenderBox;
    final RelativeRect positionRect = RelativeRect.fromRect(
      Rect.fromLTWH(position.dx, position.dy, 0, 0),
      Offset.zero & overlay.size,
    );
    final List<PopupMenuEntry<String>> entries = <PopupMenuEntry<String>>[];
    final String b = widget.account.backendType.trim().toLowerCase();

    if ((b == 'nntp' || b == 'matrix') &&
        _subscribedRowCanUnsubscribe(folder)) {
      entries.add(
        PopupMenuItem<String>(
          value: 'unsub',
          child: Text(_l10nSubscriptionUnsubscribeLabel(loc)),
        ),
      );
    }

    if (entries.isEmpty) {
      return;
    }
    showMenu<String>(
      context: ctx,
      position: positionRect,
      items: entries,
    ).then((String? action) {
      if (action == 'unsub' && ctx.mounted) {
        unawaited(_runUnsubscribeSubscribed(folder));
      }
    });
  }

  void _openAvailableRowMenu(
    BuildContext ctx,
    SubscriptionFolderRow row,
    Offset position,
  ) {
    final AppLocalizations loc = AppLocalizations.of(ctx);
    final RenderBox overlay =
        Overlay.of(ctx).context.findRenderObject()! as RenderBox;
    final RelativeRect positionRect = RelativeRect.fromRect(
      Rect.fromLTWH(position.dx, position.dy, 0, 0),
      Offset.zero & overlay.size,
    );
    final List<PopupMenuEntry<String>> entries = <PopupMenuEntry<String>>[];

    if (!row.isSubscribed) {
      entries.add(
        PopupMenuItem<String>(
          value: 'sub',
          child: Text(_l10nSubscriptionSubscribeLabel(loc)),
        ),
      );
    } else if (row.allowUnsubscribe) {
      entries.add(
        PopupMenuItem<String>(
          value: 'unsub',
          child: Text(_l10nSubscriptionUnsubscribeLabel(loc)),
        ),
      );
    }

    if (entries.isEmpty) {
      return;
    }
    showMenu<String>(
      context: ctx,
      position: positionRect,
      items: entries,
    ).then((String? action) {
      if (action == null || !ctx.mounted) {
        return;
      }
      if (action == 'sub') {
        unawaited(_applySubscriptionAction(row, true));
      } else if (action == 'unsub') {
        unawaited(_applySubscriptionAction(row, false));
      }
    });
  }

  Future<void> _runNntpWildmatQuery() async {
    final String wm = _nntpWildmatController.text.trim();
    if (wm.isEmpty) {
      return;
    }
    try {
      final List<FrbMailSubscriptionAvailableRow> decoded =
          await frbNntpListActiveWildmat(
        accountId: widget.account.id,
        wildmat: wm,
      );
      final List<SubscriptionFolderRow> rows = decoded
          .map(
            (FrbMailSubscriptionAvailableRow e) => SubscriptionFolderRow(
              id: e.id,
              isSubscribed: e.isSubscribed,
              displayName: e.displayName,
              unread: e.unread?.toInt(),
              allowUnsubscribe: e.allowUnsubscribe,
            ),
          )
          .toList();
      if (mounted) {
        setState(() {
          _nntpAvailableRows = rows;
        });
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(AppLocalizations.of(context).operationFailed(e.toString()))),
        );
      }
    }
  }

  Widget _buildSubscriptionPane(BuildContext context, AppLocalizations l10n) {
    final MailFoldersState ms = ref.watch(foldersProvider);
    final bool isNntp =
        widget.account.backendType.trim().toLowerCase() == 'nntp';
    final List<SubscriptionFolderRow> availableRows =
        isNntp && _nntpAvailableRows != null
            ? _nntpAvailableRows!
            : ms.subscriptionAvailable;

    void onSubscribedContext(BuildContext c, String folder, Offset o) {
      final String b = widget.account.backendType.trim().toLowerCase();
      if (b == 'nntp' || b == 'matrix') {
        _openSubscribedSubscriptionMenu(c, folder, o);
      } else {
        _openFolderMenu(c, folder, o);
      }
    }

    final String? delim = isConversationBackend(widget.account)
        ? null
        : ref.watch(folderHierarchyDelimiterProvider);
    final Map<String, String> folderLabelOverrides =
        isNostrBackend(widget.account)
            ? ref.watch(nostrPeerLabelsProvider)
            : isMatrixMailboxBackend(widget.account)
                ? ms.folderDisplayLabels
                : const <String, String>{};

    final bool dragOn = widget.enableMailDragTarget &&
        widget.onMailDragToFolder != null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(
          child: _folderTabIndex == 0
              ? (delim != null && delim.isNotEmpty
                  ? HierarchicalFolderTree(
                      folders: widget.folders,
                      hierarchyDelimiter: delim,
                      selectedFolder: widget.selectedFolder,
                      onSelect: widget.onSelectFolder,
                      onFolderContext: onSubscribedContext,
                      mailDropPredicate: dragOn ? _mailDropPredicate : null,
                      onMailDrop: dragOn ? _onMailDrop : null,
                      unreadByFolder: widget.unreadByFolder,
                      folderLabelOverrides: folderLabelOverrides,
                    )
                  : FolderTree(
                      folders: widget.folders,
                      selectedFolder: widget.selectedFolder,
                      onSelect: widget.onSelectFolder,
                      onFolderContext: onSubscribedContext,
                      mailDropPredicate: dragOn ? _mailDropPredicate : null,
                      onMailDrop: dragOn ? _onMailDrop : null,
                      unreadByFolder: widget.unreadByFolder,
                      folderLabelOverrides: folderLabelOverrides,
                    ))
              : _buildAvailableList(
                  context,
                  l10n,
                  availableRows,
                  isNntp,
                  folderLabelOverrides,
                  delim,
                ),
        ),
        NavigationBar(
          selectedIndex: _folderTabIndex,
          onDestinationSelected: (int i) {
            setState(() {
              _folderTabIndex = i;
            });
            if (i == 0) {
              final String? sel = widget.selectedFolder;
              if (sel != null &&
                  widget.folders.isNotEmpty &&
                  !widget.folders.contains(sel)) {
                WidgetsBinding.instance.addPostFrameCallback((_) {
                  if (mounted) {
                    widget.onSelectFolder(widget.folders.first);
                  }
                });
              }
            }
          },
          destinations: <NavigationDestination>[
            NavigationDestination(
              icon: const Icon(Icons.inbox_outlined),
              label: l10n.folderTabSubscribed,
            ),
            NavigationDestination(
              icon: const Icon(Icons.list_alt_outlined),
              label: l10n.folderTabAvailable,
            ),
          ],
        ),
      ],
    );
  }

  Widget _buildAvailableList(
    BuildContext context,
    AppLocalizations l10n,
    List<SubscriptionFolderRow> rows,
    bool isNntp,
    Map<String, String> labelOverrides,
    String? hierarchyDelimiter,
  ) {
    void onAvailableContext(BuildContext c, String folder, Offset o) {
      final SubscriptionFolderRow? row =
          _subscriptionRowForPath(rows, folder);
      if (row != null) {
        _openAvailableRowMenu(c, row, o);
      }
    }

    final Map<String, String> mergedLabels =
        _mergeAvailableDisplayLabels(labelOverrides, rows);
    final Map<String, int> unreadAvailable = <String, int>{
      for (final SubscriptionFolderRow r in rows)
        if ((r.unread ?? 0) > 0) r.id: r.unread!,
    };
    final List<String> paths =
        _pathsForSubscriptionAvailable(rows, hierarchyDelimiter);

    final bool dragOn = widget.enableMailDragTarget &&
        widget.onMailDragToFolder != null;

    final Widget listWidget = paths.isEmpty
        ? const SizedBox.shrink()
        : hierarchyDelimiter != null && hierarchyDelimiter.isNotEmpty
            ? HierarchicalFolderTree(
                folders: paths,
                hierarchyDelimiter: hierarchyDelimiter,
                selectedFolder: widget.selectedFolder,
                onSelect: widget.onSelectFolder,
                onFolderContext: onAvailableContext,
                mailDropPredicate: dragOn ? _mailDropPredicate : null,
                onMailDrop: dragOn ? _onMailDrop : null,
                unreadByFolder: unreadAvailable,
                folderLabelOverrides: mergedLabels,
              )
            : FolderTree(
                folders: paths,
                selectedFolder: widget.selectedFolder,
                onSelect: widget.onSelectFolder,
                onFolderContext: onAvailableContext,
                mailDropPredicate: dragOn ? _mailDropPredicate : null,
                onMailDrop: dragOn ? _onMailDrop : null,
                unreadByFolder: unreadAvailable,
                folderLabelOverrides: mergedLabels,
              );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (isNntp)
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 8, 8, 0),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: TextField(
                    controller: _nntpWildmatController,
                    decoration: InputDecoration(
                      hintText: l10n.nntpWildmatHint,
                      isDense: true,
                    ),
                    onSubmitted: (_) => unawaited(_runNntpWildmatQuery()),
                  ),
                ),
                const SizedBox(width: 8),
                FilledButton(
                  onPressed: () => unawaited(_runNntpWildmatQuery()),
                  child: Text(l10n.nntpWildmatQuery),
                ),
              ],
            ),
          ),
        Expanded(child: listWidget),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (accountUsesSubscriptionFolderPane(widget.account)) {
      return Stack(
        clipBehavior: Clip.none,
        children: <Widget>[
          Positioned.fill(child: _buildSubscriptionPane(context, l10n)),
          if (storeSupportsFolderManagement(widget.account))
            Positioned(
              left: 8,
              bottom: 8,
              child: FloatingActionButton.small(
                heroTag: 'folder_add_${widget.account.id}',
                tooltip: l10n.folderNewTooltip,
                onPressed: () => unawaited(
                  _promptTopLevelFolder(
                    context,
                    widget.account,
                    widget.onReloadFolders,
                  ),
                ),
                child: const LucideIcon(LucideIcons.circlePlus, size: 22),
              ),
            ),
        ],
      );
    }
    final bool canManage = storeSupportsFolderManagement(widget.account);
    // Nostr/Matrix: always flat list; ignore IMAP-style hierarchy delimiter from session.
    final String? delim = isConversationBackend(widget.account)
        ? null
        : ref.watch(folderHierarchyDelimiterProvider);
    final void Function(BuildContext, String, Offset)? folderContext =
        isEmailMailboxBackend(widget.account)
            ? _openFolderMenu
            : null;

    final bool dragOn = widget.enableMailDragTarget &&
        widget.onMailDragToFolder != null;

    final Map<String, String> folderLabelOverrides =
        isNostrBackend(widget.account)
            ? ref.watch(nostrPeerLabelsProvider)
            : isMatrixMailboxBackend(widget.account)
                ? ref.watch(foldersProvider).folderDisplayLabels
                : const <String, String>{};

    return Stack(
      clipBehavior: Clip.none,
      children: <Widget>[
        Positioned.fill(
          child: delim != null && delim.isNotEmpty
              ? HierarchicalFolderTree(
                  folders: widget.folders,
                  hierarchyDelimiter: delim,
                  selectedFolder: widget.selectedFolder,
                  onSelect: widget.onSelectFolder,
                  onFolderContext: folderContext,
                  mailDropPredicate: dragOn ? _mailDropPredicate : null,
                  onMailDrop: dragOn ? _onMailDrop : null,
                  unreadByFolder: widget.unreadByFolder,
                  folderLabelOverrides: folderLabelOverrides,
                )
              : FolderTree(
                  folders: widget.folders,
                  selectedFolder: widget.selectedFolder,
                  onSelect: widget.onSelectFolder,
                  onFolderContext: folderContext,
                  mailDropPredicate: dragOn ? _mailDropPredicate : null,
                  onMailDrop: dragOn ? _onMailDrop : null,
                  unreadByFolder: widget.unreadByFolder,
                  folderLabelOverrides: folderLabelOverrides,
                ),
        ),
        if (canManage)
          Positioned(
            left: 8,
            bottom: 8,
            child: FloatingActionButton.small(
              heroTag: 'folder_add_${widget.account.id}',
              tooltip: l10n.folderNewTooltip,
              onPressed: () => unawaited(
                _promptTopLevelFolder(
                  context,
                  widget.account,
                  widget.onReloadFolders,
                ),
              ),
              child: const LucideIcon(LucideIcons.circlePlus, size: 22),
            ),
          ),
      ],
    );
  }
}

SubscriptionFolderRow? _subscriptionRowForPath(
  List<SubscriptionFolderRow> rows,
  String path,
) {
  for (final SubscriptionFolderRow r in rows) {
    if (r.id == path) {
      return r;
    }
  }
  return null;
}

Map<String, String> _mergeAvailableDisplayLabels(
  Map<String, String> base,
  List<SubscriptionFolderRow> rows,
) {
  final Map<String, String> m = Map<String, String>.from(base);
  for (final SubscriptionFolderRow r in rows) {
    final String? d = r.displayName;
    if (d != null && d.isNotEmpty) {
      m[r.id.trim().toLowerCase()] = d;
    }
  }
  return m;
}

/// Folder paths for the Available tab: deduped, with `INBOX` first when the list is flat.
List<String> _pathsForSubscriptionAvailable(
  List<SubscriptionFolderRow> rows,
  String? delim,
) {
  final List<String> paths = <String>[];
  final Set<String> seen = <String>{};
  for (final SubscriptionFolderRow r in rows) {
    final String p = r.id.trim();
    if (p.isEmpty || seen.contains(p)) {
      continue;
    }
    seen.add(p);
    paths.add(p);
  }
  if (delim == null || delim.isEmpty) {
    paths.sort((String a, String b) {
      final int ia = a.toUpperCase() == 'INBOX' ? 0 : 1;
      final int ib = b.toUpperCase() == 'INBOX' ? 0 : 1;
      if (ia != ib) {
        return ia.compareTo(ib);
      }
      return a.toLowerCase().compareTo(b.toLowerCase());
    });
  } else {
    paths.sort(
      (String a, String b) => a.toLowerCase().compareTo(b.toLowerCase()),
    );
  }
  return paths;
}

Future<void> _runExpungeFolder(
  BuildContext context,
  WidgetRef ref,
  AppAccount account,
  String folderName,
  Future<void> Function() onReloadFolders,
) async {
  final AppLocalizations l10n = AppLocalizations.of(context);
  try {
    await frbExpungeMailFolder(
      accountId: account.id,
      folderName: folderName,
    );
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.folderExpungeDone)),
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
          folderName: folderName,
          messageListSort: sort,
        ),
      ),
    );
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.operationFailed(e.toString()))),
      );
    }
  }
  await onReloadFolders();
}

Future<void> _promptTopLevelFolder(
  BuildContext context,
  AppAccount account,
  Future<void> Function() onReloadFolders,
) async {
  final AppLocalizations l10n = AppLocalizations.of(context);
  final TextEditingController name = TextEditingController();
  final bool? ok = await showDialog<bool>(
    context: context,
    builder: (BuildContext ctx) => AlertDialog(
      title: Text(l10n.folderNewDialogTitle),
      content: TextField(
        controller: name,
        decoration: InputDecoration(
          labelText: l10n.folderNameLabel,
          helperText: l10n.folderNewTopLevelHelper,
        ),
        autofocus: true,
        onSubmitted: (_) => Navigator.pop(ctx, true),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(ctx, false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(ctx, true),
          child: Text(l10n.create),
        ),
      ],
    ),
  );
  if (ok != true || !context.mounted) {
    name.dispose();
    return;
  }
  final String path = name.text.trim();
  name.dispose();
  if (path.isEmpty) {
    return;
  }
  try {
    await frbCreateMailFolder(
      accountId: account.id,
      folderPath: path,
    );
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            l10n.folderCreated(folderDisplayName(context, path)),
          ),
        ),
      );
    }
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.operationFailed(e.toString()))),
      );
    }
  }
  await onReloadFolders();
}

Future<void> _promptSubfolder(
  BuildContext context,
  AppAccount account,
  String parentFolder,
  String hierarchyDelimiter,
  Future<void> Function() onReloadFolders,
) async {
  final AppLocalizations l10n = AppLocalizations.of(context);
  final TextEditingController name = TextEditingController();
  final bool? ok = await showDialog<bool>(
    context: context,
    builder: (BuildContext ctx) => AlertDialog(
      title: Text(
        l10n.subfolderDialogTitle(folderDisplayName(ctx, parentFolder)),
      ),
      content: TextField(
        controller: name,
        decoration: InputDecoration(
          labelText: l10n.subfolderNameLabel,
          helperText: l10n.subfolderPathHelper(
            '$parentFolder$hierarchyDelimiter…',
          ),
        ),
        autofocus: true,
        onSubmitted: (_) => Navigator.pop(ctx, true),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(ctx, false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(ctx, true),
          child: Text(l10n.create),
        ),
      ],
    ),
  );
  if (ok != true || !context.mounted) {
    name.dispose();
    return;
  }
  final String child = name.text.trim();
  name.dispose();
  if (child.isEmpty) {
    return;
  }
  final String path =
      childFolderPath(parentFolder, hierarchyDelimiter, child);
  try {
    await frbCreateMailFolder(
      accountId: account.id,
      folderPath: path,
    );
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            l10n.folderCreated(folderDisplayName(context, path)),
          ),
        ),
      );
    }
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.operationFailed(e.toString()))),
      );
    }
  }
  await onReloadFolders();
}

Future<void> _promptRenameFolder(
  BuildContext context,
  AppAccount account,
  String oldName,
  Future<void> Function() onReloadFolders,
) async {
  final AppLocalizations l10n = AppLocalizations.of(context);
  final TextEditingController name = TextEditingController(text: oldName);
  final bool? ok = await showDialog<bool>(
    context: context,
    builder: (BuildContext ctx) => AlertDialog(
      title: Text(l10n.renameFolderTitle),
      content: TextField(
        controller: name,
        decoration: InputDecoration(
          labelText: l10n.newFolderPathLabel,
        ),
        autofocus: true,
        onSubmitted: (_) => Navigator.pop(ctx, true),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(ctx, false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(ctx, true),
          child: Text(l10n.rename),
        ),
      ],
    ),
  );
  if (ok != true || !context.mounted) {
    name.dispose();
    return;
  }
  final String newName = name.text.trim();
  name.dispose();
  if (newName.isEmpty || newName == oldName) {
    return;
  }
  try {
    await frbRenameMailFolder(
      accountId: account.id,
      oldName: oldName,
      newName: newName,
    );
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.folderRenamed)),
      );
    }
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.operationFailed(e.toString()))),
      );
    }
  }
  await onReloadFolders();
}

Future<void> _confirmDeleteFolder(
  BuildContext context,
  AppAccount account,
  String folderName,
  Future<void> Function() onReloadFolders,
) async {
  final AppLocalizations l10n = AppLocalizations.of(context);
  final bool? ok = await showDialog<bool>(
    context: context,
    builder: (BuildContext ctx) => AlertDialog(
      title: Text(l10n.deleteFolderTitle),
      content: Text(
        l10n.deleteFolderBody(folderDisplayName(ctx, folderName)),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(ctx, false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(ctx, true),
          child: Text(l10n.delete),
        ),
      ],
    ),
  );
  if (ok != true || !context.mounted) {
    return;
  }
  try {
    await frbDeleteMailFolder(
      accountId: account.id,
      folderName: folderName,
    );
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.folderDeleted)),
      );
    }
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.operationFailed(e.toString()))),
      );
    }
  }
  await onReloadFolders();
}
