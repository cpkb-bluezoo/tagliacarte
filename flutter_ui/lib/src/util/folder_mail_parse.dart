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
  });

  final List<String> folders;

  /// Single hierarchy character from the store (e.g. `/` or `.`), or null.
  final String? hierarchyDelimiter;
}

/// Parses `frb_list_mail_folders` JSON: either a legacy array of names or
/// `{ "folders": [...], "hierarchyDelimiter": "/" }`.
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
    return ParsedMailFolders(
      folders: raw.map((dynamic e) => e.toString()).toList(),
      hierarchyDelimiter: delim,
    );
  }
  throw const FormatException('mail folders: expected array or object');
}
