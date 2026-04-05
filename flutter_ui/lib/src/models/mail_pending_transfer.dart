/*
 * mail_pending_transfer.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

/// User chose **Move** or **Copy** from a menu; holds source scope until **Move here** / **Copy here**
/// on a folder or until replaced. List selection changes do **not** update this object.
enum MailPendingTransferKind {
  moveOp,
  copyOp,
}

/// Pending move/copy operation (menu-driven). Drag-drop does not use this type.
class MailPendingTransfer {
  const MailPendingTransfer({
    required this.kind,
    required this.sourceAccountId,
    required this.sourceFolder,
    required this.messageIds,
  });

  final MailPendingTransferKind kind;
  final String sourceAccountId;
  final String sourceFolder;
  final List<String> messageIds;

  bool isSameSource({
    required String accountId,
    required String folder,
  }) {
    return accountId == sourceAccountId && folder == sourceFolder;
  }
}
