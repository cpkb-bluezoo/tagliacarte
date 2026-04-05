/*
 * nostr_credential_dialog.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';

/// Returns the account [npub] (bech32) if nsec was saved to the vault; otherwise `null`.
Future<String?> showNostrCredentialDialog(
  BuildContext context, {
  required AppAccount account,
}) {
  return showDialog<String>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext ctx) => _NostrCredentialDialog(
      account: account,
    ),
  );
}

class _NostrCredentialDialog extends StatefulWidget {
  const _NostrCredentialDialog({
    required this.account,
  });

  final AppAccount account;

  @override
  State<_NostrCredentialDialog> createState() => _NostrCredentialDialogState();
}

class _NostrCredentialDialogState extends State<_NostrCredentialDialog> {
  final TextEditingController _secret = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _secret.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String raw = _secret.text.trim();
    if (raw.isEmpty) {
      setState(() => _error = l10n.validationUsernameRequired);
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final String hex = await frbNostrSecretKeyToHex(input: raw);
      final String pk = await frbNostrGetPublicKeyFromSecret(secret: hex);
      final String npub = await frbNostrHexToNpub(hexPubkey: pk);
      await frbSaveStoreCredential(
        accountId: widget.account.id,
        username: npub,
        password: hex,
      );
      if (mounted) {
        Navigator.pop(context, npub);
      }
    } catch (e) {
      setState(() {
        _busy = false;
        _error = e.toString();
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    return AlertDialog(
      title: const Text('Nostr secret key'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              'Enter nsec or 64-character hex. Stored only in the app credential vault.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _secret,
              obscureText: true,
              autocorrect: false,
              decoration: const InputDecoration(
                labelText: 'nsec or hex',
              ),
              onSubmitted: (_) {
                _submit();
              },
            ),
            if (_error != null) ...<Widget>[
              const SizedBox(height: 8),
              Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: _busy ? null : () => Navigator.pop<String?>(context, null),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _busy ? null : _submit,
          child: _busy
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.save),
        ),
      ],
    );
  }
}
