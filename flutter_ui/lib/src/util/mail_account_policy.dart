/*
 * mail_account_policy.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import '../rust/tagliacarte_api.dart';

/// Same rules as [accountRequiresOutboundTransport] but for a backend id string.
bool backendTypeRequiresOutboundTransport(String backendType) {
  switch (backendType) {
    case 'IMAP':
    case 'POP3':
    case 'Gmail':
    case 'Exchange':
    case 'NNTP':
    case 'Maildir':
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
