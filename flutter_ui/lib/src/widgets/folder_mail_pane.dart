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
import '../rust/tagliacarte_api.dart';
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

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
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
