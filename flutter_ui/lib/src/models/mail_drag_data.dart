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
    required this.storeUri,
    required this.credentialKey,
    required this.sourceFolder,
    required this.messageIds,
    required this.useKeychain,
  });

  /// Same-account drag target check ([frbTransferMailMessages] still uses URI + credential).
  final String sourceAccountId;
  final String storeUri;
  final String credentialKey;
  final String sourceFolder;
  final List<String> messageIds;
  final bool useKeychain;
}
