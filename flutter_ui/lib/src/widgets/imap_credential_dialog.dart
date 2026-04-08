/*
 * imap_credential_dialog.dart
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

import '../l10n/app_localizations.dart';
import '../rust/frb_api.dart';

/// `true` if the user saved credentials; `false` if cancelled; `null` if dismissed.
Future<bool?> showImapCredentialDialog(
  BuildContext context, {
  required String accountId,
  String? usernameHint,
  String? subtitle,
  /// When set (e.g. [AppLocalizations.matrixSignInTitle] for Matrix), replaces the default IMAP title.
  String? dialogTitle,
}) {
  return showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext ctx) => _ImapCredentialDialog(
      accountId: accountId,
      initialUsername: (usernameHint ?? '').trim(),
      subtitle: subtitle,
      dialogTitle: dialogTitle,
    ),
  );
}

class _ImapCredentialDialog extends StatefulWidget {
  const _ImapCredentialDialog({
    required this.accountId,
    required this.initialUsername,
    this.subtitle,
    this.dialogTitle,
  });

  final String accountId;
  final String initialUsername;
  final String? subtitle;
  final String? dialogTitle;

  @override
  State<_ImapCredentialDialog> createState() => _ImapCredentialDialogState();
}

class _ImapCredentialDialogState extends State<_ImapCredentialDialog> {
  late final TextEditingController _userController;
  late final TextEditingController _passwordController;
  bool _obscure = true;
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _userController = TextEditingController(text: widget.initialUsername);
    _passwordController = TextEditingController();
  }

  @override
  void dispose() {
    _userController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final String user = _userController.text.trim();
    final String pass = _passwordController.text;
    if (user.isEmpty || pass.isEmpty) {
      final AppLocalizations l10n = AppLocalizations.of(context);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.enterUsernameAndPassword)),
      );
      return;
    }
    setState(() => _busy = true);
    try {
      await frbSaveStoreCredential(
        accountId: widget.accountId,
        username: user,
        password: pass,
      );
      if (mounted) {
        Navigator.of(context).pop(true);
      }
    } catch (e) {
      if (mounted) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.operationFailed(e.toString()))),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String? sub = widget.subtitle?.trim();
    return AlertDialog(
      title: Text(widget.dialogTitle ?? l10n.imapSignInTitle),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (sub != null && sub.isNotEmpty)
              Text(
                sub,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
            if (sub != null && sub.isNotEmpty) const SizedBox(height: 16),
            TextField(
              controller: _userController,
              decoration: InputDecoration(
                labelText: l10n.usernameLabel,
                border: const OutlineInputBorder(),
              ),
              textInputAction: TextInputAction.next,
              autocorrect: false,
              enabled: !_busy,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _passwordController,
              decoration: InputDecoration(
                labelText: l10n.passwordLabel,
                border: const OutlineInputBorder(),
                suffixIcon: IconButton(
                  tooltip: _obscure
                      ? l10n.showPasswordTooltip
                      : l10n.hidePasswordTooltip,
                  icon: Icon(
                    _obscure ? Icons.visibility : Icons.visibility_off,
                  ),
                  onPressed: _busy
                      ? null
                      : () => setState(() => _obscure = !_obscure),
                ),
              ),
              obscureText: _obscure,
              onSubmitted: (_) => unawaited(_save()),
              enabled: !_busy,
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _busy ? null : () => Navigator.of(context).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _busy ? null : () => unawaited(_save()),
          child: _busy
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.save),
        ),
      ],
    );
  }
}
