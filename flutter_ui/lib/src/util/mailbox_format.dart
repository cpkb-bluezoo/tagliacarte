/*
 * mailbox_format.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

/// Loopback mail-body URLs expect a numeric IMAP UID in the path; [id] is often `imap(s)://…/mailbox/uid`.
String imapMessageIdForNativeApis(String id) {
  final String t = id.trim();
  if (t.startsWith('imap://') || t.startsWith('imaps://')) {
    final int slash = t.lastIndexOf('/');
    if (slash >= 0 && slash + 1 < t.length) {
      final String tail = t.substring(slash + 1).trim();
      if (tail.isNotEmpty &&
          tail.runes.every((int r) => r >= 0x30 && r <= 0x39)) {
        return tail;
      }
    }
  }
  return t;
}

/// Display [raw] as either a compact line ([short] true) or the full header text ([short] false).
///
/// Use [short: false] for message detail headers (show `Name <addr>` when both exist).
/// Use [messageListSenderLine] for list rows.
String formatMailboxLine(String raw, {required bool short}) {
  final String t = raw.trim();
  if (t.isEmpty) {
    return '';
  }
  if (!short) {
    return t;
  }
  return messageListSenderLine(t);
}

/// Sender column in the message list: display-name only if present, else address / raw line.
String messageListSenderLine(String raw) => _mailboxCompactDisplay(raw);

String _mailboxCompactDisplay(String raw) {
  final String t = raw.trim();
  final int lt = t.indexOf('<');
  final int gt = t.indexOf('>');
  if (lt != -1 && gt > lt) {
    final String name = t.substring(0, lt).trim();
    final String unquoted = name.replaceAll('"', '').trim();
    if (unquoted.isNotEmpty) {
      return unquoted;
    }
    return t.substring(lt + 1, gt).trim();
  }
  return t;
}

