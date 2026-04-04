/*
 * native_mail_uri.dart
 * Copyright (C) 2026 Chris Burdess
 */

/// Mail stores backed by native FRB mail ops (folder list, message list, …).
bool isNativeMailStoreUri(String uri) {
  return uri.startsWith('maildir:') ||
      uri.startsWith('mbox:') ||
      uri.startsWith('imap://') ||
      uri.startsWith('imaps://');
}
