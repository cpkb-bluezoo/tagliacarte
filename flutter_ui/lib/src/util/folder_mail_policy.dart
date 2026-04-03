/*
 * folder_mail_policy.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

/// Maildir and IMAP support creating / renaming / deleting mailboxes (within server rules).
bool storeSupportsFolderManagement(String storeUri) {
  return storeUri.startsWith('maildir:') ||
      storeUri.startsWith('imap://') ||
      storeUri.startsWith('imaps://');
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
