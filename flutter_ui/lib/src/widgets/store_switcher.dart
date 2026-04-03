/*
 * store_switcher.dart
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

import 'dart:io' show File;

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/material.dart';

import '../rust/tagliacarte_api.dart';
import 'account_strip_visuals.dart';

const double _kAvatarSize = 40;

class StoreSwitcher extends StatelessWidget {
  const StoreSwitcher({
    super.key,
    required this.accounts,
    required this.onSelect,
    this.showLabels = false,
    this.selectedAccountId,
  });

  final List<AppAccount> accounts;
  final ValueChanged<AppAccount> onSelect;
  final bool showLabels;
  final String? selectedAccountId;

  @override
  Widget build(BuildContext context) {
    final Brightness brightness = Theme.of(context).brightness;
    if (showLabels) {
      return ListView.builder(
        itemCount: accounts.length,
        itemBuilder: (BuildContext context, int index) {
          final AppAccount account = accounts[index];
          final bool selected = selectedAccountId == account.id;
          return ListTile(
            selected: selected,
            leading: AccountStripAvatar(
              account: account,
              brightness: brightness,
              selected: selected,
            ),
            title: Text(account.label),
            onTap: () => onSelect(account),
          );
        },
      );
    }

    return ListView.builder(
      itemCount: accounts.length,
      itemBuilder: (BuildContext context, int index) {
        final AppAccount account = accounts[index];
        final bool selected = selectedAccountId == account.id;
        return Tooltip(
          message: account.label,
          child: Align(
            alignment: Alignment.center,
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 6),
              child: InkWell(
                borderRadius: BorderRadius.circular(_kAvatarSize / 2),
                onTap: () => onSelect(account),
                child: AccountStripAvatar(
                  account: account,
                  brightness: brightness,
                  selected: selected,
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

/// Same 40×40 strip avatar as the mail chrome; use anywhere the user should
/// recognise accounts by the same look (e.g. Settings → Accounts).
class AccountStripAvatar extends StatelessWidget {
  const AccountStripAvatar({
    super.key,
    required this.account,
    required this.brightness,
    required this.selected,
  });

  final AppAccount account;
  final Brightness brightness;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    final Widget avatar = _buildAvatar(context);
    return SizedBox(
      width: _kAvatarSize,
      height: _kAvatarSize,
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
          child: avatar,
        ),
      ),
    );
  }

  Widget _buildAvatar(BuildContext context) {
    final String? raw = account.avatarUrl?.trim();
    if (raw != null && raw.isNotEmpty) {
      final Uri? u = Uri.tryParse(raw);
      if (u != null &&
          u.hasScheme &&
          (u.scheme == 'http' || u.scheme == 'https')) {
        return ClipOval(
          child: Image.network(
            raw,
            width: _kAvatarSize,
            height: _kAvatarSize,
            fit: BoxFit.cover,
            loadingBuilder: (
              BuildContext c,
              Widget child,
              ImageChunkEvent? progress,
            ) {
              if (progress == null) {
                return child;
              }
              return _generatedGlyph(c);
            },
            errorBuilder:
                (BuildContext c, Object e, StackTrace? s) =>
                    _generatedGlyph(c),
          ),
        );
      }
      if (!kIsWeb) {
        final File f = File(raw);
        if (f.existsSync()) {
          return ClipOval(
            child: Image.file(
              f,
              width: _kAvatarSize,
              height: _kAvatarSize,
              fit: BoxFit.cover,
              errorBuilder:
                  (BuildContext c, Object e, StackTrace? s) =>
                      _generatedGlyph(c),
            ),
          );
        }
      }
    }

    return _generatedGlyph(context);
  }

  Widget _generatedGlyph(BuildContext context) {
    final Color bg = accountStripBackgroundColor(account.id, brightness);
    final Color fg = accountStripForegroundColor(account.id, brightness);
    final String initials = accountStripInitials(account.label, account.email);
    return CircleAvatar(
      radius: _kAvatarSize / 2,
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
