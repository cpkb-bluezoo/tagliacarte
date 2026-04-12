/*
 * matrix_strings.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import '../l10n/app_localizations.dart';
import '../models/subscription_folder_row.dart';
import '../providers/app_state.dart';
import '../rust/tagliacarte_api.dart';
import 'mail_account_policy.dart';

/// Matches [tagliacarte_core::protocol::matrix::MATRIX_UNDECRYPTABLE_PLACEHOLDER].
const String kMatrixUndecryptablePlaceholder = '[Encrypted message]';

/// List/chat snippet when Megolm decryption failed — points users to the reader for steps.
String matrixConversationPreviewText(
  AppLocalizations l10n,
  AppAccount? account,
  String subjectOrSnippet,
) {
  if (account != null &&
      isMatrixMailboxBackend(account) &&
      subjectOrSnippet == kMatrixUndecryptablePlaceholder) {
    return l10n.matrixE2eeUndecryptableListPreview;
  }
  return subjectOrSnippet;
}

/// Lowercase room id → title: merges session [MailFoldersState.folderDisplayLabels] with
/// [MailFoldersState.subscriptionAvailable] rows (defense in depth when one side has a name).
Map<String, String> matrixMergedFolderLabels(MailFoldersState ms) {
  final Map<String, String> out =
      Map<String, String>.from(ms.folderDisplayLabels);
  for (final SubscriptionFolderRow r in ms.subscriptionAvailable) {
    final String? d = r.displayName;
    if (d == null) {
      continue;
    }
    final String t = d.trim();
    if (t.isEmpty) {
      continue;
    }
    out[r.id.trim().toLowerCase()] = t;
  }
  return out;
}
