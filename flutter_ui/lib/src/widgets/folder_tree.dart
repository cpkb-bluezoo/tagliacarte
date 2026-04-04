/*
 * folder_tree.dart
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

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../models/mail_drag_data.dart';
import '../util/folder_display.dart';
import 'folder_unread_pill.dart';

class FolderTree extends StatelessWidget {
  const FolderTree({
    super.key,
    required this.folders,
    required this.onSelect,
    this.selectedFolder,
    this.onFolderContext,
    this.mailDropPredicate,
    this.onMailDrop,
    this.unreadByFolder = const <String, int>{},
  });

  final List<String> folders;
  final ValueChanged<String> onSelect;
  final String? selectedFolder;

  /// Long-press (mobile) or right-click (desktop): show folder actions.
  final void Function(BuildContext context, String folder, Offset globalPosition)?
      onFolderContext;

  final bool Function(MailListDragPayload payload, String folderPath)?
      mailDropPredicate;

  final Future<void> Function(
    MailListDragPayload payload,
    String folderPath,
    bool asCopy,
  )? onMailDrop;

  final Map<String, int> unreadByFolder;

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      itemCount: folders.length,
      itemBuilder: (BuildContext context, int index) {
        final String folder = folders[index];
        final int unread = unreadByFolder[folder] ?? 0;
        Widget innerRow = ListTile(
          selected: selectedFolder == folder,
          leading: const Icon(Icons.folder_outlined),
          title: Text(
            folderDisplayName(context, folder),
            style: TextStyle(
              fontWeight: unread > 0 ? FontWeight.bold : FontWeight.normal,
            ),
          ),
          trailing:
              unread > 0 ? FolderUnreadPill(count: unread) : null,
          onTap: onFolderContext == null ? () => onSelect(folder) : null,
        );
        if (onFolderContext != null) {
          innerRow = GestureDetector(
            onTap: () => onSelect(folder),
            onLongPressStart: (LongPressStartDetails d) {
              onFolderContext!(context, folder, d.globalPosition);
            },
            onSecondaryTapDown: (TapDownDetails d) {
              onFolderContext!(context, folder, d.globalPosition);
            },
            child: innerRow,
          );
        }
        if (mailDropPredicate != null && onMailDrop != null) {
          return DragTarget<MailListDragPayload>(
            onWillAcceptWithDetails: (DragTargetDetails<MailListDragPayload> d) {
              return mailDropPredicate!(d.data, folder);
            },
            onAcceptWithDetails: (DragTargetDetails<MailListDragPayload> d) {
              final bool asCopy = HardwareKeyboard.instance.isAltPressed;
              onMailDrop!(d.data, folder, asCopy);
            },
            builder: (
              BuildContext context,
              List<MailListDragPayload?> accepted,
              List<dynamic> rejected,
            ) {
              final bool over = accepted.isNotEmpty;
              return ColoredBox(
                color: over
                    ? Theme.of(context)
                        .colorScheme
                        .primary
                        .withValues(alpha: 0.14)
                    : Colors.transparent,
                child: innerRow,
              );
            },
          );
        }
        return innerRow;
      },
    );
  }
}
