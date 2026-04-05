/*
 * compose_screen.dart
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

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../providers/app_state.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';

class ComposeScreen extends ConsumerStatefulWidget {
  const ComposeScreen({super.key});

  @override
  ConsumerState<ComposeScreen> createState() => _ComposeScreenState();
}

class _ComposeScreenState extends ConsumerState<ComposeScreen> {
  final _from = TextEditingController();
  final _to = TextEditingController();
  final _cc = TextEditingController();
  final _bcc = TextEditingController();
  final _subject = TextEditingController();
  final _body = TextEditingController();

  String? _selectedTransportId;
  bool _sending = false;

  @override
  void dispose() {
    _from.dispose();
    _to.dispose();
    _cc.dispose();
    _bcc.dispose();
    _subject.dispose();
    _body.dispose();
    super.dispose();
  }

  AppAccount? _accountFor(
    AppSettingsConfig? cfg,
    String? selectedId,
  ) {
    if (cfg == null || cfg.accounts.isEmpty) {
      return null;
    }
    final String? id = selectedId ?? cfg.selectedStoreId;
    if (id != null) {
      for (final AppAccount a in cfg.accounts) {
        if (a.id == id) {
          return a;
        }
      }
    }
    return cfg.accounts.first;
  }

  List<AppTransport> _transportsForAccount(
    AppSettingsConfig? cfg,
    AppAccount? account,
  ) {
    if (cfg == null || account == null) {
      return <AppTransport>[];
    }
    final List<AppTransport> out = <AppTransport>[];
    for (final String tid in account.transportIds) {
      for (final AppTransport t in cfg.transports) {
        if (t.id == tid) {
          out.add(t);
          break;
        }
      }
    }
    return out;
  }

  String? _effectiveTransportId(List<AppTransport> outgoing) {
    if (outgoing.isEmpty) {
      return null;
    }
    final String? cur = _selectedTransportId;
    if (cur != null && outgoing.any((AppTransport t) => t.id == cur)) {
      return cur;
    }
    return outgoing.first.id;
  }

  void _seedFromIfNeeded(AppAccount? account) {
    if (account == null || _from.text.trim().isNotEmpty) {
      return;
    }
    final String e =
        account.attrs['email'] ?? account.attrs['username'] ?? '';
    if (e.isNotEmpty) {
      _from.text = e;
    }
  }

  List<String> _splitRecipients(String raw) {
    return raw
        .split(',')
        .map((String s) => s.trim())
        .where((String s) => s.isNotEmpty)
        .toList();
  }

  Future<void> _send(
    BuildContext context,
    AppLocalizations l10n,
    String transportId,
  ) async {
    final String from = _from.text.trim();
    if (from.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.composeMissingFrom)),
      );
      return;
    }
    final List<String> to = _splitRecipients(_to.text);
    if (to.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.composeMissingTo)),
      );
      return;
    }
    setState(() => _sending = true);
    try {
      final Map<String, dynamic> payload = <String, dynamic>{
        'from': from,
        'to': to,
        'cc': _splitRecipients(_cc.text),
        'bcc': _splitRecipients(_bcc.text),
        'subject': _subject.text.trim(),
        'bodyPlain': _body.text,
      };
      await frbSendSmtpMessage(
        transportId: transportId,
        composeJson: jsonEncode(payload),
      );
      if (!context.mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.composeSendSucceeded)),
      );
    } catch (e) {
      if (!context.mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('$e')),
      );
    } finally {
      if (mounted) {
        setState(() => _sending = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final AsyncValue<AppSettingsConfig> cfgAsync =
        ref.watch(accountsConfigProvider);
    final String? selectedAccountId = ref.watch(selectedAccountIdProvider);

    return cfgAsync.when(
      loading: () => Scaffold(
        appBar: AppBar(title: Text(l10n.compose)),
        body: const Center(child: CircularProgressIndicator()),
      ),
      error: (Object e, StackTrace _) => Scaffold(
        appBar: AppBar(title: Text(l10n.compose)),
        body: Center(child: Text('$e')),
      ),
      data: (AppSettingsConfig cfg) {
        final AppAccount? account = _accountFor(cfg, selectedAccountId);
        _seedFromIfNeeded(account);
        final List<AppTransport> outgoing =
            _transportsForAccount(cfg, account);
        final String? transportId = _effectiveTransportId(outgoing);

        return Scaffold(
          appBar: AppBar(
            title: Text(l10n.compose),
            actions: [
              TextButton(
                onPressed: _sending || transportId == null
                    ? null
                    : () => _send(context, l10n, transportId),
                child: _sending
                    ? const SizedBox(
                        width: 20,
                        height: 20,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Text(l10n.send),
              ),
            ],
          ),
          body: ListView(
            padding: const EdgeInsets.all(12),
            children: [
              if (outgoing.isEmpty)
                Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: Text(
                    l10n.composeNeedTransportTooltip,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                )
              else
                DropdownButtonFormField<String>(
                  value: transportId,
                  decoration: InputDecoration(
                    labelText: l10n.composeOutgoingTransport,
                  ),
                  items: outgoing
                      .map(
                        (AppTransport t) => DropdownMenuItem<String>(
                          value: t.id,
                          child: Text(t.primaryListTitle),
                        ),
                      )
                      .toList(),
                  onChanged: (String? v) {
                    setState(() => _selectedTransportId = v);
                  },
                ),
              const SizedBox(height: 8),
              TextField(
                controller: _from,
                decoration: InputDecoration(labelText: l10n.fieldFrom),
                keyboardType: TextInputType.emailAddress,
              ),
              TextField(
                controller: _to,
                decoration: InputDecoration(labelText: l10n.fieldTo),
              ),
              TextField(
                controller: _cc,
                decoration: InputDecoration(labelText: l10n.fieldCc),
              ),
              TextField(
                controller: _bcc,
                decoration: InputDecoration(labelText: l10n.fieldBcc),
              ),
              TextField(
                controller: _subject,
                decoration: InputDecoration(labelText: l10n.fieldSubject),
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _body,
                minLines: 12,
                maxLines: 24,
                decoration: InputDecoration(
                  labelText: l10n.fieldBody,
                  border: const OutlineInputBorder(),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}
