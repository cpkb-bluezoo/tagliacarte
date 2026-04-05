/*
 * mail_account_policy.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import '../rust/tagliacarte_api.dart';

/// Same rules as [accountRequiresOutboundTransport] but for a backend id string.
///
/// Values are compared case-insensitively — Rust config normalizes store types to lowercase
/// (e.g. `gmail`), while the account picker may use title case (`Gmail`).
bool backendTypeRequiresOutboundTransport(String backendType) {
  switch (backendType.trim().toLowerCase()) {
    case 'imap':
    case 'pop3':
    case 'gmail':
    case 'maildir':
    case 'mbox':
      return true;
    default:
      return false;
  }
}

/// Store types that need at least one configured outgoing transport to send mail.
bool accountRequiresOutboundTransport(AppAccount account) {
  return backendTypeRequiresOutboundTransport(account.backendType);
}

/// When [accountRequiresOutboundTransport] is true, sending needs a non-empty
/// [AppAccount.transportIds] list (first id is default SMTP transport).
bool accountCanSendMail(AppAccount account) {
  if (!accountRequiresOutboundTransport(account)) {
    return true;
  }
  return account.transportIds.isNotEmpty;
}

/// Classic mailbox backends (IMAP, Maildir, …) — not Nostr/Matrix conversation stores.
bool isEmailMailboxBackend(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  if (b == 'nostr' || b == 'matrix') {
    return false;
  }
  if (b == 'imap' ||
      b == 'pop3' ||
      b == 'gmail' ||
      b == 'exchange' ||
      b == 'nntp' ||
      b == 'maildir' ||
      b == 'mbox') {
    return true;
  }
  return b.isEmpty || b == 'email';
}

bool isConversationBackend(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  return b == 'nostr' || b == 'matrix';
}

bool isNostrBackend(AppAccount account) {
  return account.backendType.trim().toLowerCase() == 'nostr';
}

/// Google Gmail store (IMAP + XOAUTH2), regardless of `Gmail` vs `gmail` in config JSON.
bool isGmailMailboxBackend(AppAccount account) {
  return account.backendType.trim().toLowerCase() == 'gmail';
}

/// Microsoft 365 / Outlook via Graph (`exchange` in UI, `graph` when normalized on disk).
///
/// Outgoing mail uses the same Graph API as the mailbox — no separate SMTP transport row.
bool isMicrosoftGraphMailboxBackend(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  return b == 'exchange' || b == 'graph';
}

/// Same as [isMicrosoftGraphMailboxBackend] for settings dropdown ids (`Exchange`, …).
bool backendTypeUsesMicrosoftGraphEmbeddedTransport(String backendType) {
  final String b = backendType.trim().toLowerCase();
  return b == 'exchange' || b == 'graph';
}

/// Features that only apply to IMAP-like remote mailboxes (attachments, IDLE-adjacent UI, …).
bool isImapStyleMailboxBackend(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  return b == 'imap' || b == 'gmail' || b == 'exchange' || b == 'pop3';
}

/// NNTP / Usenet store (newsgroups as folders; POST on same server as reads).
bool isNntpMailboxBackend(AppAccount account) {
  return account.backendType.trim().toLowerCase() == 'nntp';
}
