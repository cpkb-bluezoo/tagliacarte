/*
 * message_row.dart
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

class MessageListRow {
  const MessageListRow({
    required this.id,
    required this.from,
    required this.subject,
    required this.date,
    this.isRead = true,
    this.markedForDeletion = false,
  });

  final String id;
  final String from;
  final String subject;
  final DateTime date;
  /// When false, message list shows subject bold; opening detail marks read on server.
  final bool isRead;
  final bool markedForDeletion;
}

enum MessageSortField {
  byFrom,
  bySubject,
  byDate,
}

List<MessageListRow> sortMessageRows(
  List<MessageListRow> rows,
  MessageSortField field,
  bool ascending,
) {
  final List<MessageListRow> copy = List<MessageListRow>.from(rows);
  int compare(MessageListRow a, MessageListRow b) {
    final int c = switch (field) {
      MessageSortField.byFrom =>
        a.from.toLowerCase().compareTo(b.from.toLowerCase()),
      MessageSortField.bySubject =>
        a.subject.toLowerCase().compareTo(b.subject.toLowerCase()),
      MessageSortField.byDate => a.date.compareTo(b.date),
    };
    return ascending ? c : -c;
  }

  copy.sort(compare);
  return copy;
}
