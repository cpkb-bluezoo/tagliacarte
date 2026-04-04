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
    required this.storeUri,
    required this.credentialKey,
    required this.sourceFolder,
    required this.messageIds,
    required this.useKeychain,
  });

  final MailPendingTransferKind kind;
  final String storeUri;
  final String credentialKey;
  final String sourceFolder;
  final List<String> messageIds;
  final bool useKeychain;

  bool isSameSource({
    required String storeUri,
    required String credentialKey,
    required String folder,
    required bool useKeychain,
  }) {
    return storeUri == this.storeUri &&
        credentialKey == this.credentialKey &&
        folder == sourceFolder &&
        useKeychain == this.useKeychain;
  }
}
