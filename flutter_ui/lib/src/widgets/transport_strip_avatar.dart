/*
 * transport_strip_avatar.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';

import '../rust/tagliacarte_api.dart';
import 'account_strip_visuals.dart';

const double _kTransportStripAvatar = 40;

/// Same visual recipe as [AccountStripAvatar] (initials + stable colour from id).
class TransportStripAvatar extends StatelessWidget {
  const TransportStripAvatar({
    super.key,
    required this.transport,
    required this.brightness,
    this.selected = false,
  });

  final AppTransport transport;
  final Brightness brightness;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    return SizedBox(
      width: _kTransportStripAvatar,
      height: _kTransportStripAvatar,
      child: DecoratedBox(
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          border: Border.all(
            color: selected ? scheme.primary : Colors.transparent,
            width: selected ? 3 : 0,
          ),
        ),
        child: Padding(
          padding: EdgeInsets.all(selected ? 1.5 : 0),
          child: _generatedGlyph(context),
        ),
      ),
    );
  }

  Widget _generatedGlyph(BuildContext context) {
    final Color bg = accountStripBackgroundColor(transport.id, brightness);
    final Color fg = accountStripForegroundColor(transport.id, brightness);
    final String initials =
        accountStripInitials(transport.primaryListTitle, null);
    return CircleAvatar(
      radius: _kTransportStripAvatar / 2,
      backgroundColor: bg,
      foregroundColor: fg,
      child: Text(
        initials,
        style: TextStyle(
          fontSize: initials.length >= 2 ? 14 : 16,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
