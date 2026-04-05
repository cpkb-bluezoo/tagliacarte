/*
 * folder_mail_policy.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import '../rust/tagliacarte_api.dart';

/// Maildir and IMAP support creating / renaming / deleting mailboxes (within server rules).
bool storeSupportsFolderManagement(AppAccount account) {
  switch (account.backendType.trim().toLowerCase()) {
    case 'maildir':
    case 'imap':
    case 'gmail':
    case 'exchange':
      return true;
    default:
      return false;
  }
}

/// The special inbox folder cannot be renamed or deleted in the UI (backends may also reject).
bool folderIsReservedInboxName(String folderName) {
  return folderName.toUpperCase() == 'INBOX';
}

String childFolderPath(String parent, String hierarchyDelimiter, String childName) {
  final String c = childName.trim();
  if (c.isEmpty) {
    return parent;
  }
  return '$parent$hierarchyDelimiter$c';
}
