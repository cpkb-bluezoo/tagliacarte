/*
 * attachment_cards.dart
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

import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

/// Tile width floor: room for one-line filename, subtitle, and one trailing control.
const double kAttachmentCardMinWidth = 176;

const double _kAttachmentGridGap = 8;

/// Picked file on disk (compose / chat attachment staging).
class PickedAttachmentFile {
  const PickedAttachmentFile({
    required this.path,
    required this.filename,
    required this.sizeBytes,
  });

  final String path;
  final String filename;
  final int sizeBytes;
}

/// If [name] has more than [maxRunes] Unicode scalar values, show the first
/// [keepRunes] runes plus an ellipsis (helps dense grids; layout still uses
/// [TextOverflow.ellipsis] within the tile).
String attachmentFilenameForDisplay(
  String name, {
  int maxRunes = 24,
  int keepRunes = 21,
}) {
  final String t = name.trim();
  final int len = t.runes.length;
  if (len <= maxRunes) {
    return t;
  }
  return '${String.fromCharCodes(t.runes.take(keepRunes))}…';
}

String attachmentSizeLabel(int bytes) {
  if (bytes < 1024) {
    return '$bytes B';
  }
  if (bytes < 1024 * 1024) {
    return '${(bytes / 1024).toStringAsFixed(1)} KB';
  }
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}

/// How many equal-width columns fit in [maxWidth] for [itemCount] items (1…itemCount).
int attachmentGridColumnCount(double maxWidth, int itemCount) {
  if (itemCount <= 0 || !maxWidth.isFinite || maxWidth <= 0) {
    return 1;
  }
  final double gap = _kAttachmentGridGap;
  final double minW = kAttachmentCardMinWidth;
  int n = ((maxWidth + gap) / (minW + gap)).floor();
  if (n < 1) {
    n = 1;
  }
  if (n > itemCount) {
    n = itemCount;
  }
  return n;
}

Future<List<PickedAttachmentFile>> pickAttachmentFiles() async {
  final FilePickerResult? r = await FilePicker.platform.pickFiles(
    allowMultiple: true,
    withData: false,
  );
  if (r == null) {
    return <PickedAttachmentFile>[];
  }
  final List<PickedAttachmentFile> out = <PickedAttachmentFile>[];
  for (final PlatformFile f in r.files) {
    final String? p = f.path;
    if (p == null || p.isEmpty) {
      continue;
    }
    final File file = File(p);
    if (!await file.exists()) {
      continue;
    }
    final int len = await file.length();
    final String name = f.name.trim().isNotEmpty
        ? f.name.trim()
        : p.split(Platform.pathSeparator).last;
    out.add(PickedAttachmentFile(path: p, filename: name, sizeBytes: len));
  }
  return out;
}

/// Shared card chrome: filename (ellipsis), subtitle row, optional trailing control.
class AttachmentDisplayCard extends StatelessWidget {
  const AttachmentDisplayCard({
    super.key,
    required this.filename,
    required this.subtitle,
    this.trailing,
    this.elideLongDisplayName = true,
  });

  final String filename;
  final String subtitle;
  final Widget? trailing;

  /// When true, names longer than 24 graphemes are shortened with an ellipsis
  /// before layout (see [attachmentFilenameForDisplay]).
  final bool elideLongDisplayName;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final String showName = elideLongDisplayName
        ? attachmentFilenameForDisplay(filename)
        : filename.trim();
    return Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 8, 4, 8),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Text(
                    showName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  if (subtitle.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 2),
                    Text(
                      subtitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ],
              ),
            ),
            if (trailing case final Widget w) w,
          ],
        ),
      ),
    );
  }
}

/// Lays out [children] in rows of equal width: up to [attachmentGridColumnCount] per row;
/// short last row keeps the same column width (empty slots).
class AttachmentCardsGrid extends StatelessWidget {
  const AttachmentCardsGrid({
    super.key,
    required this.children,
  });

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) {
      return const SizedBox.shrink();
    }
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double w = constraints.maxWidth;
        final int cols = attachmentGridColumnCount(w, children.length);
        final List<Widget> rows = <Widget>[];
        for (int i = 0; i < children.length; i += cols) {
          final int end = (i + cols > children.length) ? children.length : i + cols;
          final List<Widget> chunk = children.sublist(i, end);
          final bool moreRows = end < children.length;
          rows.add(
            Padding(
              padding: EdgeInsets.only(bottom: moreRows ? _kAttachmentGridGap : 0),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: List<Widget>.generate(cols, (int j) {
                  return Expanded(
                    child: Padding(
                      padding: EdgeInsets.only(
                        right: j < cols - 1 ? _kAttachmentGridGap : 0,
                      ),
                      child: j < chunk.length
                          ? chunk[j]
                          : const SizedBox.shrink(),
                    ),
                  );
                }),
              ),
            ),
          );
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: rows,
        );
      },
    );
  }
}
