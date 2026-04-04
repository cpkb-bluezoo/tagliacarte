/*
 * folder_unread_pill.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';

/// Rounded capsule for folder unread count (smaller than row title text).
class FolderUnreadPill extends StatelessWidget {
  const FolderUnreadPill({
    super.key,
    required this.count,
    this.capAt = 1 << 30,
    this.compact = false,
  });

  final int count;
  final int capAt;
  final bool compact;

  String get _label => count > capAt ? '$capAt+' : '$count';

  @override
  Widget build(BuildContext context) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    final TextStyle base =
        Theme.of(context).textTheme.labelSmall ?? const TextStyle();
    final double scale = compact ? 0.78 : 0.88;
    final EdgeInsets pad = compact
        ? const EdgeInsets.symmetric(horizontal: 4, vertical: 1)
        : const EdgeInsets.symmetric(horizontal: 7, vertical: 2);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: scheme.primaryContainer,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Padding(
        padding: pad,
        child: Text(
          _label,
          style: base.copyWith(
            fontSize: (base.fontSize ?? 12) * scale,
            fontWeight: FontWeight.w600,
            color: scheme.onPrimaryContainer,
          ),
        ),
      ),
    );
  }
}
