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

/// Returns the user fragment from [storeUri] when present (`imap(s)://user@host`).
String? imapUsernameHintFromStoreUri(String storeUri) {
  try {
    final Uri u = Uri.parse(storeUri);
    if (u.scheme != 'imap' && u.scheme != 'imaps') {
      return null;
    }
    final String info = u.userInfo;
    if (info.isEmpty) {
      return null;
    }
    final int colon = info.indexOf(':');
    final String user = colon < 0 ? info : info.substring(0, colon);
    if (user.isEmpty) {
      return null;
    }
    return Uri.decodeComponent(user);
  } catch (_) {
    return null;
  }
}

/// `true` if the user saved credentials; `false` if cancelled; `null` if dismissed.
Future<bool?> showImapCredentialDialog(
  BuildContext context, {
  required String credentialId,
  required String storeUri,
  required bool useKeychain,
}) {
  final String? hint = imapUsernameHintFromStoreUri(storeUri);
  return showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext ctx) => _ImapCredentialDialog(
      credentialId: credentialId,
      storeUri: storeUri,
      useKeychain: useKeychain,
      initialUsername: hint ?? '',
    ),
  );
}

class _ImapCredentialDialog extends StatefulWidget {
  const _ImapCredentialDialog({
    required this.credentialId,
    required this.storeUri,
    required this.useKeychain,
    required this.initialUsername,
  });

  final String credentialId;
  final String storeUri;
  final bool useKeychain;
  final String initialUsername;

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
        credentialId: widget.credentialId,
        storeUri: widget.storeUri,
        username: user,
        password: pass,
        useKeychain: widget.useKeychain,
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
    return AlertDialog(
      title: Text(l10n.imapSignInTitle),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              widget.storeUri,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
            ),
            const SizedBox(height: 16),
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
