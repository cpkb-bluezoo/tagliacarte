/*
 * mail_drag_data.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

/// Drag payload from the message list (desktop same-store move/copy).
class MailListDragPayload {
  const MailListDragPayload({
    required this.sourceAccountId,
    required this.sourceFolder,
    required this.messageIds,
  });

  final String sourceAccountId;
  final String sourceFolder;
  final List<String> messageIds;
}
