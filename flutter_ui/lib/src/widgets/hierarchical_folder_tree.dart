/*
 * hierarchical_folder_tree.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../l10n/app_localizations.dart';
import '../models/mail_drag_data.dart';
import '../util/folder_display.dart';
import '../util/folder_hierarchy.dart';
import 'folder_unread_pill.dart';

const double _kTreeIndent = 18;
const double _kChevronSize = 32;
const double _kFolderRowExtent = 48;

/// Renders [folders] as an indented tree when names contain [hierarchyDelimiter].
class HierarchicalFolderTree extends StatefulWidget {
  const HierarchicalFolderTree({
    super.key,
    required this.folders,
    required this.hierarchyDelimiter,
    required this.onSelect,
    this.selectedFolder,
    this.onFolderContext,
    this.mailDropPredicate,
    this.onMailDrop,
    this.unreadByFolder = const <String, int>{},
  });

  final List<String> folders;
  final String hierarchyDelimiter;
  final ValueChanged<String> onSelect;
  final String? selectedFolder;

  final void Function(BuildContext context, String folder, Offset globalPosition)?
      onFolderContext;

  /// When non-null with [onMailDrop], rows accept [MailListDragPayload] from the message list.
  final bool Function(MailListDragPayload payload, String folderPath)?
      mailDropPredicate;

  /// [asCopy] is true when **Alt**/**Option** was held at drop (desktop convention).
  final Future<void> Function(
    MailListDragPayload payload,
    String folderPath,
    bool asCopy,
  )? onMailDrop;

  final Map<String, int> unreadByFolder;

  @override
  State<HierarchicalFolderTree> createState() => _HierarchicalFolderTreeState();
}

class _HierarchicalFolderTreeState extends State<HierarchicalFolderTree> {
  late Set<String> _expanded;

  @override
  void initState() {
    super.initState();
    _expanded = _computeInitialExpanded();
  }

  @override
  void didUpdateWidget(HierarchicalFolderTree oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_sameFolderList(oldWidget.folders, widget.folders) ||
        oldWidget.hierarchyDelimiter != widget.hierarchyDelimiter ||
        oldWidget.selectedFolder != widget.selectedFolder ||
        oldWidget.unreadByFolder != widget.unreadByFolder) {
      final Set<String> next = _computeInitialExpanded();
      _expanded = <String>{..._expanded, ...next};
    }
  }

  static bool _sameFolderList(List<String> a, List<String> b) {
    if (a.length != b.length) {
      return false;
    }
    for (int i = 0; i < a.length; i++) {
      if (a[i] != b[i]) {
        return false;
      }
    }
    return true;
  }

  Set<String> _computeInitialExpanded() {
    final Map<String, FolderHierarchyNode> roots = buildFolderHierarchy(
      widget.folders,
      widget.hierarchyDelimiter,
    );
    return {
      ...branchPathsInHierarchy(roots),
      ...ancestorBranchPaths(widget.selectedFolder, widget.hierarchyDelimiter),
    };
  }

  void _toggle(String fullPath) {
    setState(() {
      if (_expanded.contains(fullPath)) {
        _expanded.remove(fullPath);
      } else {
        _expanded.add(fullPath);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final Map<String, FolderHierarchyNode> roots = buildFolderHierarchy(
      widget.folders,
      widget.hierarchyDelimiter,
    );
    final List<(FolderHierarchyNode node, int depth)> flat =
        _flattenVisibleRowsPreorder(roots);
    // Use [ListView.builder] so stores with huge folder lists do not mount tens
    // of thousands of children in one frame (that can overflow the C/Dart stack).
    return ListView.builder(
      padding: EdgeInsets.zero,
      itemExtent: _kFolderRowExtent,
      itemCount: flat.length,
      itemBuilder: (BuildContext context, int index) {
        final (FolderHierarchyNode node, int depth) = flat[index];
        return KeyedSubtree(
          key: ValueKey<String>(node.fullPath),
          child: _folderRow(context, node, depth),
        );
      },
    );
  }

  /// Visible nodes in preorder; iterative DFS (no Dart stack growth with depth).
  List<(FolderHierarchyNode node, int depth)> _flattenVisibleRowsPreorder(
    Map<String, FolderHierarchyNode> roots,
  ) {
    final List<(FolderHierarchyNode node, int depth)> out =
        <(FolderHierarchyNode, int)>[];
    for (final String key in sortedHierarchyKeys(roots.keys)) {
      _flattenSubtreeIterative(roots[key]!, 0, out);
    }
    return out;
  }

  void _flattenSubtreeIterative(
    FolderHierarchyNode start,
    int startDepth,
    List<(FolderHierarchyNode node, int depth)> out,
  ) {
    final List<(FolderHierarchyNode node, int depth)> stack =
        <(FolderHierarchyNode, int)>[(start, startDepth)];
    while (stack.isNotEmpty) {
      final (FolderHierarchyNode node, int d) = stack.removeLast();
      out.add((node, d));
      if (node.children.isEmpty || !_expanded.contains(node.fullPath)) {
        continue;
      }
      final List<String> keys = sortedHierarchyKeys(node.children.keys);
      for (int i = keys.length - 1; i >= 0; i--) {
        stack.add((node.children[keys[i]]!, d + 1));
      }
    }
  }

  Widget _folderRow(BuildContext context, FolderHierarchyNode node, int depth) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool hasChildren = node.children.isNotEmpty;
    final bool isSelected = widget.selectedFolder == node.fullPath;
    final int unread = widget.unreadByFolder[node.fullPath] ?? 0;
    final Widget title = Text(
      folderDisplayName(context, node.segment),
      style: TextStyle(
        fontWeight: unread > 0 ? FontWeight.bold : FontWeight.normal,
      ),
      overflow: TextOverflow.ellipsis,
    );

    Widget core = InkWell(
      onTap: () => widget.onSelect(node.fullPath),
      child: Padding(
        padding: EdgeInsets.only(left: depth * _kTreeIndent),
        child: SizedBox(
          height: 48,
          child: Row(
            children: <Widget>[
              SizedBox(
                width: _kChevronSize,
                height: _kChevronSize,
                child: hasChildren
                    ? IconButton(
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints(
                          minWidth: _kChevronSize,
                          minHeight: _kChevronSize,
                        ),
                        icon: Icon(
                          _expanded.contains(node.fullPath)
                              ? Icons.expand_more
                              : Icons.chevron_right,
                          size: 22,
                        ),
                        onPressed: () => _toggle(node.fullPath),
                        tooltip: _expanded.contains(node.fullPath)
                            ? l10n.collapseFolder
                            : l10n.expandFolder,
                      )
                    : const SizedBox.shrink(),
              ),
              Icon(
                Icons.folder_outlined,
                size: 22,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
              const SizedBox(width: 8),
              Expanded(child: title),
              if (unread > 0) ...[
                const SizedBox(width: 6),
                Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: FolderUnreadPill(count: unread),
                ),
              ],
            ],
          ),
        ),
      ),
    );

    if (widget.onFolderContext != null) {
      core = GestureDetector(
        onLongPressStart: (LongPressStartDetails d) {
          widget.onFolderContext!(context, node.fullPath, d.globalPosition);
        },
        onSecondaryTapDown: (TapDownDetails d) {
          widget.onFolderContext!(context, node.fullPath, d.globalPosition);
        },
        child: core,
      );
    }

    final Widget rowContent = Material(
      color: isSelected
          ? Theme.of(context).colorScheme.primaryContainer.withValues(alpha: 0.35)
          : Colors.transparent,
      child: core,
    );

    if (widget.mailDropPredicate != null && widget.onMailDrop != null) {
      return DragTarget<MailListDragPayload>(
        onWillAcceptWithDetails: (DragTargetDetails<MailListDragPayload> d) {
          return widget.mailDropPredicate!(d.data, node.fullPath);
        },
        onAcceptWithDetails: (DragTargetDetails<MailListDragPayload> d) {
          final bool asCopy = HardwareKeyboard.instance.isAltPressed;
          widget.onMailDrop!(d.data, node.fullPath, asCopy);
        },
        builder: (
          BuildContext context,
          List<MailListDragPayload?> accepted,
          List<dynamic> rejected,
        ) {
          final bool over = accepted.isNotEmpty;
          return ColoredBox(
            color: over
                ? Theme.of(context).colorScheme.primary.withValues(alpha: 0.14)
                : Colors.transparent,
            child: rowContent,
          );
        },
      );
    }

    return rowContent;
  }
}
