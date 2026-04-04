/*
 * native_inbox_unread.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

export 'session_state.dart'
    show nativeAccountInboxUnreadProvider, nativeTotalInboxUnreadProvider;

int inboxUnreadFromFolderMap(Map<String, int> unreadByFolder) {
  for (final MapEntry<String, int> e in unreadByFolder.entries) {
    if (e.key.toUpperCase() == 'INBOX') {
      return e.value;
    }
  }
  return 0;
}

bool folderNameIsInbox(String name) {
  return name.toUpperCase() == 'INBOX';
}
