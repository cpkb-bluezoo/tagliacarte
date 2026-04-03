/*
 * selectable_header_line.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';

/// One mail header row: bold label + selectable value (and label for copy).
class SelectableHeaderLine extends StatelessWidget {
  const SelectableHeaderLine({
    super.key,
    required this.label,
    required this.value,
  });

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    if (value.isEmpty) {
      return const SizedBox.shrink();
    }
    final TextStyle base = Theme.of(context).textTheme.bodyMedium ?? const TextStyle();
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12),
      child: SelectableText.rich(
        TextSpan(
          style: base,
          children: <InlineSpan>[
            TextSpan(
              text: '$label ',
              style: base.copyWith(fontWeight: FontWeight.w600),
            ),
            TextSpan(text: value),
          ],
        ),
      ),
    );
  }
}
