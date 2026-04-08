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

bool isMatrixMailboxBackend(AppAccount account) {
  return account.backendType.trim().toLowerCase() == 'matrix';
}

/// Localpart of the Matrix user ID (`@name:server` → `name`) for `m.login.password` username field.
String matrixCredentialUsernameHint(AppAccount account) {
  final String raw =
      (account.attrs['username'] ?? account.attrs['email'] ?? '').trim();
  if (raw.isEmpty) {
    return '';
  }
  final String withoutAt = raw.startsWith('@') ? raw.substring(1) : raw;
  final int colon = withoutAt.indexOf(':');
  if (colon > 0) {
    return withoutAt.substring(0, colon);
  }
  return withoutAt;
}

/// Prefill for store sign-in dialogs (IMAP username/email; Matrix localpart only).
String storeCredentialUsernameHint(AppAccount account) {
  if (isMatrixMailboxBackend(account)) {
    return matrixCredentialUsernameHint(account);
  }
  return (account.attrs['username'] ?? account.attrs['email'] ?? '').trim();
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
/// Excludes POP3 (no hierarchical folder / IMAP-style FETCH parts in this stack).
bool isImapStyleMailboxBackend(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  return b == 'imap' || b == 'gmail' || b == 'exchange';
}

/// IMAP `EXPUNGE` (and Gmail). Not Graph/POP3/mbox/NNTP/Maildir in the current implementation.
bool accountSupportsImapProtocolExpunge(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  return b == 'imap' || b == 'gmail';
}

/// Move/delete to configurable Trash or Junk subfolders (same-store transfer).
bool accountSupportsMailboxTrashAndJunkMoves(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  return b == 'imap' ||
      b == 'gmail' ||
      b == 'maildir' ||
      b == 'exchange' ||
      b == 'graph';
}

String mailboxTrashFolderDisplayName(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  if (b == 'maildir') {
    final String t = (account.attrs['maildirTrashFolderName'] ?? '').trim();
    return t.isEmpty ? 'Trash' : t;
  }
  if (b == 'exchange' || b == 'graph') {
    final String t = (account.attrs['imapTrashFolderName'] ?? '').trim();
    return t.isEmpty ? 'Deleted Items' : t;
  }
  final String t = (account.attrs['imapTrashFolderName'] ?? '').trim();
  return t.isEmpty ? 'Trash' : t;
}

String mailboxJunkFolderDisplayName(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  if (b == 'maildir') {
    final String j = (account.attrs['maildirJunkFolderName'] ?? '').trim();
    return j.isEmpty ? 'Junk' : j;
  }
  if (b == 'exchange' || b == 'graph') {
    final String j = (account.attrs['imapJunkFolderName'] ?? '').trim();
    return j.isEmpty ? 'Junk Email' : j;
  }
  final String j = (account.attrs['imapJunkFolderName'] ?? '').trim();
  return j.isEmpty ? 'Junk' : j;
}

/// True when [mailboxName] is the configured junk folder (move-to-junk must be disabled).
bool mailboxJunkMoveUnavailable(AppAccount account, String? mailboxName) {
  if (!accountSupportsMailboxTrashAndJunkMoves(account)) {
    return true;
  }
  final String? m = mailboxName?.trim();
  if (m == null || m.isEmpty) {
    return false;
  }
  final String junk = mailboxJunkFolderDisplayName(account);
  return m.toLowerCase() == junk.toLowerCase();
}

/// True when delete would be a no-op (already in trash with move-to-trash semantics).
bool imapMoveToTrashDeleteUnavailable(
  AppAccount account,
  String? mailboxName,
) {
  return mailboxMoveToTrashDeleteUnavailable(account, mailboxName);
}

bool mailboxMoveToTrashDeleteUnavailable(
  AppAccount account,
  String? mailboxName,
) {
  final String? m = mailboxName?.trim();
  if (m == null || m.isEmpty) {
    return false;
  }
  final String b = account.backendType.trim().toLowerCase();
  if (b == 'imap' || b == 'gmail') {
    final String mode =
        (account.attrs['imapDeleteMode'] ?? 'Move to Trash').trim();
    if (mode != 'Move to Trash') {
      return false;
    }
    final String trash = mailboxTrashFolderDisplayName(account);
    return m.toLowerCase() == trash.toLowerCase();
  }
  if (b == 'maildir') {
    final String mode =
        (account.attrs['maildirDeleteMode'] ?? 'Move to Trash').trim();
    final bool immediate = mode == 'Delete immediately' ||
        mode.toLowerCase() == 'delete immediately';
    if (immediate) {
      return false;
    }
    final String trash = mailboxTrashFolderDisplayName(account);
    return m.toLowerCase() == trash.toLowerCase();
  }
  return false;
}

/// NNTP / Usenet store (newsgroups as folders; POST on same server as reads).
bool isNntpMailboxBackend(AppAccount account) {
  return account.backendType.trim().toLowerCase() == 'nntp';
}

