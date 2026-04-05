/*
 * gmail_oauth_dialog.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'dart:async';

import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../rust/frb_api.dart';

/// `true` if the user completed Google sign-in; `false` if cancelled; `null` if dismissed.
Future<bool?> showGmailOAuthDialog(
  BuildContext context, {
  required String accountId,
  String? subtitle,
}) {
  return showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext ctx) => _GmailOAuthDialog(
      accountId: accountId,
      subtitle: subtitle,
    ),
  );
}

class _GmailOAuthDialog extends StatefulWidget {
  const _GmailOAuthDialog({
    required this.accountId,
    this.subtitle,
  });

  final String accountId;
  final String? subtitle;

  @override
  State<_GmailOAuthDialog> createState() => _GmailOAuthDialogState();
}

class _GmailOAuthDialogState extends State<_GmailOAuthDialog> {
  bool _busy = false;

  Future<void> _signIn() async {
    setState(() => _busy = true);
    try {
      await frbGmailOauthSignIn(accountId: widget.accountId);
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
      title: Text(l10n.gmailSignInTitle),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(l10n.gmailSignInBody),
            if (sub != null && sub.isNotEmpty) ...[
              const SizedBox(height: 12),
              Text(
                sub,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _busy ? null : () => Navigator.of(context).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _busy ? null : () => unawaited(_signIn()),
          child: _busy
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.gmailSignInBrowserButton),
        ),
      ],
    );
  }
}
