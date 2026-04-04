/*
 * folder_mail_parse.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'dart:convert';

/// Result of [parseMailFoldersJson] from the native mail FRB list call.
class ParsedMailFolders {
  const ParsedMailFolders({
    required this.folders,
    this.hierarchyDelimiter,
    this.unreadByFolder = const <String, int>{},
  });

  final List<String> folders;

  /// Single hierarchy character from the store (e.g. `/` or `.`), or null.
  final String? hierarchyDelimiter;

  /// Unread counts keyed by exact folder name from [folders].
  final Map<String, int> unreadByFolder;
}

/// Parses `frb_list_mail_folders` JSON: either a legacy array of names or
/// `{ "folders": [...], "hierarchyDelimiter": "/" }`.
/// Sum of per-folder UNSEEN (or equivalent) counts for store-level badges.
int sumFolderUnreadCounts(Map<String, int> unreadByFolder) {
  int s = 0;
  for (final int v in unreadByFolder.values) {
    s += v;
  }
  return s;
}

ParsedMailFolders parseMailFoldersJson(String json) {
  final dynamic decoded = jsonDecode(json);
  if (decoded is List<dynamic>) {
    return ParsedMailFolders(
      folders: decoded.map((dynamic e) => e.toString()).toList(),
    );
  }
  if (decoded is Map<String, dynamic>) {
    final List<dynamic>? raw = decoded['folders'] as List<dynamic>?;
    if (raw == null) {
      throw const FormatException('mail folders: missing folders');
    }
    final String? hd = decoded['hierarchyDelimiter'] as String?;
    String? delim;
    if (hd != null && hd.isNotEmpty) {
      delim = hd.substring(0, 1);
    }
    final Map<String, int> unread = <String, int>{};
    final List<dynamic>? ur =
        decoded['folderUnreadCounts'] as List<dynamic>? ??
            decoded['folder_unread_counts'] as List<dynamic>?;
    if (ur != null) {
      for (final dynamic e in ur) {
        if (e is! Map<String, dynamic>) {
          continue;
        }
        final String? name =
            e['folderName'] as String? ?? e['folder_name'] as String?;
        final num? n = e['unread'] as num?;
        if (name != null && n != null) {
          unread[name] = n.toInt();
        }
      }
    }
    return ParsedMailFolders(
      folders: raw.map((dynamic e) => e.toString()).toList(),
      hierarchyDelimiter: delim,
      unreadByFolder: unread,
    );
  }
  throw const FormatException('mail folders: expected array or object');
}
