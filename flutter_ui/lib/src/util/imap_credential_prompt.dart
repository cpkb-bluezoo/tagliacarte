/*
 * imap_credential_prompt.dart
 *
 * IMAP credential UI driven by server-advertised AUTH= mechanisms (see Rust
 * StoreError::NeedsCredential + TAGLIACARTE_IMAP_CAPS), not account oauthProvider.
 */

import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../rust/tagliacarte_api.dart';
import '../widgets/gmail_oauth_dialog.dart';
import '../widgets/imap_credential_dialog.dart';
import 'mail_account_policy.dart';

/// Suffix on `credential required for …` errors from IMAP probe (pre-login CAPABILITY).
const String kTagliacarteImapCapsMarker = 'TAGLIACARTE_IMAP_CAPS:';

/// Parses [kTagliacarteImapCapsMarker] from a Rust error string; `null` if absent.
List<String>? parseTagliacarteImapCapsFromError(Object err) {
  final String s = err.toString();
  final int i = s.indexOf(kTagliacarteImapCapsMarker);
  if (i < 0) {
    return null;
  }
  final String tail = s.substring(i + kTagliacarteImapCapsMarker.length).trim();
  if (tail.isEmpty) {
    return const <String>[];
  }
  return tail
      .split(',')
      .map((String x) => x.trim().toUpperCase())
      .where((String x) => x.isNotEmpty)
      .toList();
}

enum ImapAuthPromptKind {
  xoauth2,
  password,
}

/// First server-advertised AUTH= mechanism we support, in capability list order
/// (matches core SASL picker intent).
ImapAuthPromptKind pickImapAuthPromptKindFromCaps(List<String> caps) {
  const Set<String> passwordMechs = <String>{
    'PLAIN',
    'LOGIN',
    'CRAM-MD5',
    'SCRAM-SHA-256',
  };
  for (final String cap in caps) {
    final String c = cap.trim().toUpperCase();
    if (!c.startsWith('AUTH=')) {
      continue;
    }
    final String mech = c.substring(5).trim().toUpperCase();
    if (mech == 'XOAUTH2') {
      return ImapAuthPromptKind.xoauth2;
    }
    if (passwordMechs.contains(mech)) {
      return ImapAuthPromptKind.password;
    }
  }
  return ImapAuthPromptKind.password;
}

bool _isImapProtocolBackend(AppAccount account) {
  final String b = account.backendType.trim().toLowerCase();
  return b == 'imap' || b == 'imaps';
}

/// Gmail REST, Matrix, IMAP/IMAPS (capability-driven), or other IMAP-style backends.
Future<bool?> showImapStyleMailboxCredentialPrompt({
  required BuildContext context,
  required AppAccount account,
  required Object err,
}) async {
  if (!context.mounted) {
    return null;
  }
  final AppLocalizations l10n = AppLocalizations.of(context);

  if (isGmailMailboxBackend(account)) {
    return showGmailOAuthDialog(
      context,
      accountId: account.id,
      subtitle: account.label,
    );
  }

  if (isMatrixMailboxBackend(account)) {
    return showImapCredentialDialog(
      context,
      accountId: account.id,
      usernameHint: storeCredentialUsernameHint(account),
      subtitle: account.label,
      dialogTitle: l10n.matrixSignInTitle,
    );
  }

  if (_isImapProtocolBackend(account)) {
    final List<String>? caps = parseTagliacarteImapCapsFromError(err);
    final ImapAuthPromptKind kind = caps == null || caps.isEmpty
        ? ImapAuthPromptKind.password
        : pickImapAuthPromptKindFromCaps(caps);
    if (kind == ImapAuthPromptKind.xoauth2) {
      return showGmailOAuthDialog(
        context,
        accountId: account.id,
        subtitle: account.label,
      );
    }
  }

  return showImapCredentialDialog(
    context,
    accountId: account.id,
    usernameHint: storeCredentialUsernameHint(account),
    subtitle: account.label,
    dialogTitle: null,
  );
}
