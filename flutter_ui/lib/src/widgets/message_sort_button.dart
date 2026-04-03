/*
 * message_sort_button.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../models/message_row.dart';
import '../providers/app_state.dart';
import '../providers/message_sort_persist.dart';
import 'lucide_icon.dart';

class MessageSortButton extends ConsumerWidget {
  const MessageSortButton({super.key, this.iconOnly = true});

  final bool iconOnly;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    return PopupMenuButton<String>(
      tooltip: l10n.sortMessagesTooltip,
      icon: iconOnly ? const LucideIcon(LucideIcons.arrowUpDown, size: 22) : null,
      child: iconOnly
          ? null
          : Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                const LucideIcon(LucideIcons.arrowUpDown, size: 20),
                const SizedBox(width: 4),
                Text(l10n.sort),
              ],
            ),
      onSelected: (String value) {
        switch (value) {
          case kMessageListSortFromAsc:
            ref.read(messageSortFieldProvider.notifier).state =
                MessageSortField.byFrom;
            ref.read(messageSortAscendingProvider.notifier).state = true;
            break;
          case kMessageListSortFromDesc:
            ref.read(messageSortFieldProvider.notifier).state =
                MessageSortField.byFrom;
            ref.read(messageSortAscendingProvider.notifier).state = false;
            break;
          case kMessageListSortSubjectAsc:
            ref.read(messageSortFieldProvider.notifier).state =
                MessageSortField.bySubject;
            ref.read(messageSortAscendingProvider.notifier).state = true;
            break;
          case kMessageListSortSubjectDesc:
            ref.read(messageSortFieldProvider.notifier).state =
                MessageSortField.bySubject;
            ref.read(messageSortAscendingProvider.notifier).state = false;
            break;
          case kMessageListSortDateAsc:
            ref.read(messageSortFieldProvider.notifier).state =
                MessageSortField.byDate;
            ref.read(messageSortAscendingProvider.notifier).state = true;
            break;
          case kMessageListSortDateDesc:
            ref.read(messageSortFieldProvider.notifier).state =
                MessageSortField.byDate;
            ref.read(messageSortAscendingProvider.notifier).state = false;
            break;
        }
        unawaited(persistCurrentMessageSort(ref));
      },
      itemBuilder: (BuildContext context) => [
        PopupMenuItem(
          value: kMessageListSortFromAsc,
          child: Text(l10n.sortFromAz),
        ),
        PopupMenuItem(
          value: kMessageListSortFromDesc,
          child: Text(l10n.sortFromZa),
        ),
        PopupMenuItem(
          value: kMessageListSortSubjectAsc,
          child: Text(l10n.sortSubjectAz),
        ),
        PopupMenuItem(
          value: kMessageListSortSubjectDesc,
          child: Text(l10n.sortSubjectZa),
        ),
        PopupMenuItem(
          value: kMessageListSortDateAsc,
          child: Text(l10n.sortDateOldest),
        ),
        PopupMenuItem(
          value: kMessageListSortDateDesc,
          child: Text(l10n.sortDateNewest),
        ),
      ],
    );
  }
}
