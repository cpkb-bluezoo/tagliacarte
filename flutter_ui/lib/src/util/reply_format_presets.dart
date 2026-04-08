/*
 * reply_format_presets.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import '../l10n/app_localizations.dart';

/// ICU-style patterns passed to [DateFormat]; empty string = device locale default.
const List<String> kReplyDateFormatPatterns = <String>[
  '',
  'y-MM-dd',
  'MM/dd/y',
  'dd/MM/y',
  'yMMMd',
  'yMMMEd',
];

/// Same for reply time line in header template.
const List<String> kReplyTimeFormatPatterns = <String>[
  '',
  'jm',
  'HH:mm',
  'HH:mm:ss',
];

String replyDatePresetLabel(AppLocalizations l10n, String pattern) {
  switch (pattern) {
    case '':
      return l10n.replyDatePresetLocale;
    case 'y-MM-dd':
      return l10n.replyDatePresetIso;
    case 'MM/dd/y':
      return l10n.replyDatePresetUs;
    case 'dd/MM/y':
      return l10n.replyDatePresetEu;
    case 'yMMMd':
      return l10n.replyDatePresetMedium;
    case 'yMMMEd':
      return l10n.replyDatePresetWeekday;
    default:
      return l10n.replyDatePresetCustom(pattern);
  }
}

String replyTimePresetLabel(AppLocalizations l10n, String pattern) {
  switch (pattern) {
    case '':
      return l10n.replyTimePresetLocale;
    case 'jm':
      return l10n.replyTimePreset12h;
    case 'HH:mm':
      return l10n.replyTimePreset24h;
    case 'HH:mm:ss':
      return l10n.replyTimePreset24hSeconds;
    default:
      return l10n.replyTimePresetCustom(pattern);
  }
}
