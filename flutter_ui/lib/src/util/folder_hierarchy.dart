/*
 * folder_hierarchy.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

/// One node in a mailbox hierarchy (IMAP / Maildir paths split by [delimiter]).
class FolderHierarchyNode {
  FolderHierarchyNode({
    required this.segment,
    required this.fullPath,
  });

  /// Last path segment (display name under parent).
  final String segment;

  /// Full mailbox name as used by the store (SELECT, DELETE, …).
  final String fullPath;

  final Map<String, FolderHierarchyNode> children = <String, FolderHierarchyNode>{};
}

/// Builds a forest of nodes from flat LIST names. Merges shared prefixes (`a` + `a/b`).
Map<String, FolderHierarchyNode> buildFolderHierarchy(
  List<String> folderPaths,
  String delimiter,
) {
  final Map<String, FolderHierarchyNode> roots = <String, FolderHierarchyNode>{};
  for (final String raw in folderPaths) {
    final String path = raw.trim();
    if (path.isEmpty) {
      continue;
    }
    final List<String> parts = path
        .split(delimiter)
        .map((String s) => s.trim())
        .where((String s) => s.isNotEmpty)
        .toList();
    if (parts.isEmpty) {
      continue;
    }
    Map<String, FolderHierarchyNode> level = roots;
    String accum = '';
    for (int i = 0; i < parts.length; i++) {
      accum = i == 0 ? parts[0] : '$accum$delimiter${parts[i]}';
      final String seg = parts[i];
      level.putIfAbsent(
        seg,
        () => FolderHierarchyNode(segment: seg, fullPath: accum),
      );
      level = level[seg]!.children;
    }
  }
  return roots;
}

/// Sort sibling keys: `INBOX` first (any case), then case-insensitive alpha.
List<String> sortedHierarchyKeys(Iterable<String> keys) {
  final List<String> list = keys.toList();
  list.sort((String a, String b) {
    final int ia = a.toUpperCase() == 'INBOX' ? 0 : 1;
    final int ib = b.toUpperCase() == 'INBOX' ? 0 : 1;
    if (ia != ib) {
      return ia.compareTo(ib);
    }
    return a.toLowerCase().compareTo(b.toLowerCase());
  });
  return list;
}

/// Every [fullPath] that has at least one child (for default-expanded UI).
Set<String> branchPathsInHierarchy(Map<String, FolderHierarchyNode> roots) {
  final Set<String> out = <String>{};
  final List<FolderHierarchyNode> stack = <FolderHierarchyNode>[];
  for (final FolderHierarchyNode n in roots.values) {
    stack.add(n);
  }
  while (stack.isNotEmpty) {
    final FolderHierarchyNode n = stack.removeLast();
    if (n.children.isNotEmpty) {
      out.add(n.fullPath);
      for (final FolderHierarchyNode c in n.children.values) {
        stack.add(c);
      }
    }
  }
  return out;
}

/// Ancestor paths of [selected] (excluding the leaf), so the path to selection can be expanded.
Set<String> ancestorBranchPaths(String? selected, String delimiter) {
  final Set<String> out = <String>{};
  if (selected == null || selected.isEmpty) {
    return out;
  }
  final List<String> parts = selected
      .split(delimiter)
      .map((String s) => s.trim())
      .where((String s) => s.isNotEmpty)
      .toList();
  if (parts.length <= 1) {
    return out;
  }
  String accum = parts[0];
  for (int i = 1; i < parts.length; i++) {
    out.add(accum);
    accum = '$accum$delimiter${parts[i]}';
  }
  return out;
}
