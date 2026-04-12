/*
 * account_save_credential_prompt.dart
 *
 * After saving an account, nudge the session and show credential UI when the server
 * reports missing or rejected credentials (not only when the account is selected on Home).
 */

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../providers/account_selection_flow.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../providers/session_state.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../l10n/app_localizations.dart';
import '../widgets/imap_credential_dialog.dart';
import '../widgets/nostr_credential_dialog.dart';
import 'imap_credential_prompt.dart';
import 'mail_account_policy.dart';

bool _isLocalMailBackend(String backendType) {
  switch (backendType) {
    case 'Maildir':
    case 'maildir':
    case 'mbox':
      return true;
    default:
      return false;
  }
}

Future<void> _tryShowImapLikeCredentialUi(
  WidgetRef ref,
  BuildContext context,
  AppAccount account,
  Object err,
) async {
  if (!context.mounted) {
    return;
  }
  final bool matrix = isMatrixMailboxBackend(account);
  if (!matrix && !isMissingImapCredentialsError(err)) {
    return;
  }
  final bool? saved = await showImapStyleMailboxCredentialPrompt(
    context: context,
    account: account,
    err: err,
  );
  if (saved == true && context.mounted) {
    await sessionRefreshFolders(accountId: account.id);
    if (ref.read(selectedAccountIdProvider) == account.id) {
      ensureSelectedFolderForCurrentAccount(ref);
    }
  }
}

/// Called after [addOrUpdateAccount] + session reload so vault prompts appear immediately.
Future<void> promptMailboxCredentialsIfNeededAfterSave(
  WidgetRef ref,
  BuildContext context,
  AppAccount account,
) async {
  if (!context.mounted) {
    return;
  }
  if (_isLocalMailBackend(account.backendType)) {
    return;
  }

  try {
    final String path = await ref.read(tagliacarteApiProvider).configXmlPath();
    await frbSessionReloadAccounts(configXmlPath: path);
  } catch (_) {
    /* reload may race with SettingsScreen; polling still works */
  }
  if (!context.mounted) {
    return;
  }

  if (isNostrBackend(account)) {
    await _pollNostr(ref, context, account);
    return;
  }

  if (isMatrixMailboxBackend(account)) {
    await _matrixAfterSave(ref, context, account);
    return;
  }

  if (isImapStyleMailboxBackend(account) ||
      isNntpMailboxBackend(account) ||
      isMicrosoftGraphMailboxBackend(account)) {
    await _pollImapLike(ref, context, account);
  }
}

/// Matrix: prompt when the vault is still empty (folder list may not return a credential-shaped error).
Future<void> _matrixAfterSave(
  WidgetRef ref,
  BuildContext context,
  AppAccount account,
) async {
  bool vaultHasSecret = false;
  try {
    vaultHasSecret =
        await frbStoreHasSavedPassword(accountId: account.id);
  } catch (_) {
    vaultHasSecret = false;
  }
  if (!context.mounted) {
    return;
  }

  if (!vaultHasSecret) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool? saved = await showImapCredentialDialog(
      context,
      accountId: account.id,
      usernameHint: storeCredentialUsernameHint(account),
      subtitle: account.label,
      dialogTitle: l10n.matrixSignInTitle,
    );
    if (saved != true || !context.mounted) {
      return;
    }
  }

  try {
    await sessionRefreshFolders(accountId: account.id);
    if (ref.read(selectedAccountIdProvider) == account.id) {
      ensureSelectedFolderForCurrentAccount(ref);
    }
  } catch (e) {
    if (!context.mounted) {
      return;
    }
    await _tryShowImapLikeCredentialUi(ref, context, account, e);
  }
}

Future<void> _pollNostr(
  WidgetRef ref,
  BuildContext context,
  AppAccount account,
) async {
  try {
    await sessionRefreshFolders(accountId: account.id);
  } catch (_) {
    /* session may still emit */
  }
  for (int i = 0; i < 50; i++) {
    await Future<void>.delayed(const Duration(milliseconds: 100));
    if (!context.mounted) {
      return;
    }
    final AccountMailModel? m =
        ref.read(accountMailModelsProvider)[account.id];
    if (m == null) {
      continue;
    }
    if (m.connection == MailConnectionState.error) {
      final String msg = m.connectionMessage ?? '';
      if (isMissingNostrCredentialsError(msg)) {
        final String? saved = await showNostrCredentialDialog(
          context,
          account: account,
        );
        if (saved != null && context.mounted) {
          await sessionRefreshFolders(accountId: account.id);
        }
      }
      return;
    }
    if (m.folders.isNotEmpty) {
      return;
    }
  }
}

Future<void> _pollImapLike(
  WidgetRef ref,
  BuildContext context,
  AppAccount account,
) async {
  try {
    await sessionRefreshFolders(accountId: account.id);
    return;
  } catch (e) {
    if (!context.mounted) {
      return;
    }
    await _tryShowImapLikeCredentialUi(ref, context, account, e);
  }
}
