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
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mime/mime.dart';

import '../l10n/app_localizations.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../widgets/attachment_cards.dart';
import '../widgets/lucide_icon.dart';
import '../widgets/smtp_transport_credential_dialog.dart';

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
  List<PickedAttachmentFile> _attachments = <PickedAttachmentFile>[];
  /// Per-message DSN override: null = use transport default.
  String? _dsnOverride;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        ref.read(composeActiveProvider.notifier).state = true;
      }
    });
  }

  @override
  void dispose() {
    // [ref] must not be used here: Riverpod tears down [WidgetRef] before [dispose]
    // completes, which throws "Cannot use ref after the widget was disposed".
    try {
      ProviderScope.containerOf(context, listen: false)
          .read(composeActiveProvider.notifier)
          .state = false;
    } catch (_) {
      // No [ProviderScope] or context already detached during teardown.
    }
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

  AppTransport? _transportById(AppSettingsConfig? cfg, String? id) {
    if (cfg == null || id == null) {
      return null;
    }
    for (final AppTransport t in cfg.transports) {
      if (t.id == id) {
        return t;
      }
    }
    return null;
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

  void _seedFromIfNeeded(AppTransport? transport) {
    if (transport == null || _from.text.trim().isNotEmpty) {
      return;
    }
    final String d = transport.defaultFrom.trim();
    if (d.isNotEmpty) {
      _from.text = d;
    }
  }

  List<String> _splitRecipients(String raw) {
    return raw
        .split(',')
        .map((String s) => s.trim())
        .where((String s) => s.isNotEmpty)
        .toList();
  }

  Future<void> _pickAttachments() async {
    final List<PickedAttachmentFile> picked = await pickAttachmentFiles();
    if (!mounted || picked.isEmpty) {
      return;
    }
    setState(() {
      _attachments = List<PickedAttachmentFile>.from(_attachments)..addAll(picked);
    });
  }

  /// First plausible SMTP auth id from the From field (angle-addr or first address).
  String _smtpUsernameHintFromFromField(String raw) {
    final String t = raw.trim();
    if (t.isEmpty) {
      return '';
    }
    final int lt = t.indexOf('<');
    final int gt = t.indexOf('>');
    if (lt >= 0 && gt > lt) {
      return t.substring(lt + 1, gt).trim();
    }
    final String first = t.split(',').first.trim();
    return first;
  }

  Future<void> _send(
    BuildContext context,
    AppLocalizations l10n,
    String transportId,
    AppTransport transport,
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
      final List<Map<String, dynamic>> atts = <Map<String, dynamic>>[];
      for (final PickedAttachmentFile a in _attachments) {
        final List<int> bytes = await File(a.path).readAsBytes();
        final String mt =
            lookupMimeType(a.filename) ?? 'application/octet-stream';
        atts.add(<String, dynamic>{
          'filename': a.filename,
          'mimeType': mt,
          'bytesBase64': base64Encode(bytes),
        });
      }
      final Map<String, dynamic> payload = <String, dynamic>{
        'from': from,
        'to': to,
        'cc': _splitRecipients(_cc.text),
        'bcc': _splitRecipients(_bcc.text),
        'subject': _subject.text.trim(),
        'bodyPlain': _body.text,
        'attachments': atts,
      };
      final String dsn = (_dsnOverride ?? transport.dsnNotify).trim();
      if (dsn.isNotEmpty) {
        payload['dsnNotify'] = dsn;
      }
      final String composeJson = jsonEncode(payload);
      final String name = transport.displayName.trim().isEmpty
          ? transport.id
          : transport.displayName.trim();
      final String host = transport.host.trim().isEmpty ? '—' : transport.host;
      final String userHint = _smtpUsernameHintFromFromField(from);

      while (true) {
        try {
          await frbSendSmtpMessage(
            transportId: transportId,
            composeJson: composeJson,
          );
          if (!context.mounted) {
            return;
          }
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(l10n.composeSendSucceeded)),
          );
          return;
        } catch (e) {
          if (!context.mounted) {
            return;
          }
          if (!smtpSendShouldOfferCredentialPrompt(e)) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('$e')),
            );
            return;
          }
          final bool? saved = await showSmtpTransportCredentialDialog(
            context,
            transportId: transportId,
            transportName: name,
            host: host,
            usernameHint: userHint,
          );
          if (!context.mounted) {
            return;
          }
          if (saved != true) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text(l10n.composeSendCancelledNoSmtpCredentials)),
            );
            return;
          }
        }
      }
    } finally {
      if (mounted) {
        setState(() => _sending = false);
      }
    }
  }

  Future<void> _showDsnDialog(
    BuildContext context,
    AppLocalizations l10n,
    AppTransport transport,
  ) async {
    final String transportDefault = transport.dsnNotify.trim().isEmpty
        ? 'failure'
        : transport.dsnNotify.trim();
    const String kUseTransport = '__transport__';
    final List<String> choiceHolder = <String>[
      _dsnOverride == null ? kUseTransport : _dsnOverride!,
    ];
    final String? picked = await showDialog<String>(
      context: context,
      builder: (BuildContext ctx) {
        return AlertDialog(
          title: Text(l10n.dsnLabel),
          content: StatefulBuilder(
            builder: (BuildContext c, void Function(void Function()) setS) {
              return DropdownButtonFormField<String>(
                // ignore: deprecated_member_use
                value: choiceHolder[0],
                decoration: InputDecoration(
                  labelText: l10n.dsnLabel,
                  helperText:
                      '${l10n.dsnUseTransportDefault}: $transportDefault',
                ),
                items: <DropdownMenuItem<String>>[
                  DropdownMenuItem<String>(
                    value: kUseTransport,
                    child: Text(l10n.dsnUseTransportDefault),
                  ),
                  DropdownMenuItem<String>(
                    value: 'never',
                    child: Text(l10n.dsnNever),
                  ),
                  DropdownMenuItem<String>(
                    value: 'failure',
                    child: Text(l10n.dsnFailure),
                  ),
                  DropdownMenuItem<String>(
                    value: 'success',
                    child: Text(l10n.dsnSuccess),
                  ),
                  DropdownMenuItem<String>(
                    value: 'delay',
                    child: Text(l10n.dsnDelay),
                  ),
                  DropdownMenuItem<String>(
                    value: 'failure,success',
                    child: Text(l10n.dsnFailureAndSuccess),
                  ),
                ],
                onChanged: (String? v) {
                  if (v != null) {
                    setS(() => choiceHolder[0] = v);
                  }
                },
              );
            },
          ),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.pop(ctx),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(ctx, choiceHolder[0]),
              child: Text(l10n.dialogOk),
            ),
          ],
        );
      },
    );
    if (!mounted || picked == null) {
      return;
    }
    setState(() {
      if (picked == kUseTransport) {
        _dsnOverride = null;
      } else {
        _dsnOverride = picked;
      }
    });
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
        final List<AppTransport> outgoing =
            _transportsForAccount(cfg, account);
        final String? transportId = _effectiveTransportId(outgoing);
        final AppTransport? transport =
            _transportById(cfg, transportId);
        _seedFromIfNeeded(transport);

        return Scaffold(
          appBar: AppBar(
            title: Text(l10n.compose),
            actions: <Widget>[
              if (transport != null)
                IconButton(
                  tooltip: l10n.dsnLabel,
                  icon: const Icon(Icons.notifications_outlined),
                  onPressed: () => _showDsnDialog(context, l10n, transport),
                ),
              IconButton(
                tooltip: l10n.attach,
                icon: const LucideIcon(LucideIcons.paperclip),
                onPressed: _pickAttachments,
              ),
              IconButton(
                tooltip: l10n.send,
                onPressed: _sending || transportId == null || transport == null
                    ? null
                    : () => _send(context, l10n, transportId, transport),
                icon: _sending
                    ? const SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const LucideIcon(LucideIcons.send),
              ),
            ],
          ),
          body: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      SingleChildScrollView(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: <Widget>[
                            if (outgoing.isEmpty)
                              Padding(
                                padding: const EdgeInsets.only(bottom: 12),
                                child: Text(
                                  l10n.composeNeedTransportTooltip,
                                  style: TextStyle(
                                    color:
                                        Theme.of(context).colorScheme.error,
                                  ),
                                ),
                              )
                              else
                                DropdownButtonFormField<String>(
                                  // ignore: deprecated_member_use
                                  value: transportId,
                                decoration: InputDecoration(
                                  labelText: l10n.composeOutgoingTransport,
                                ),
                                items: outgoing
                                    .map(
                                      (AppTransport t) =>
                                          DropdownMenuItem<String>(
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
                              decoration:
                                  InputDecoration(labelText: l10n.fieldFrom),
                              keyboardType: TextInputType.emailAddress,
                            ),
                            TextField(
                              controller: _to,
                              decoration:
                                  InputDecoration(labelText: l10n.fieldTo),
                            ),
                            TextField(
                              controller: _cc,
                              decoration:
                                  InputDecoration(labelText: l10n.fieldCc),
                            ),
                            TextField(
                              controller: _bcc,
                              decoration:
                                  InputDecoration(labelText: l10n.fieldBcc),
                            ),
                            TextField(
                              controller: _subject,
                              decoration: InputDecoration(
                                labelText: l10n.fieldSubject,
                              ),
                            ),
                            const SizedBox(height: 8),
                          ],
                        ),
                      ),
                      Expanded(
                        child: TextField(
                          controller: _body,
                          expands: true,
                          maxLines: null,
                          minLines: null,
                          textAlignVertical: TextAlignVertical.top,
                          decoration: InputDecoration(
                            labelText: l10n.fieldBody,
                            alignLabelWithHint: true,
                            border: const OutlineInputBorder(),
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              if (_attachments.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        l10n.attachments,
                        style: Theme.of(context).textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w600,
                            ),
                      ),
                      const SizedBox(height: 8),
                      AttachmentCardsGrid(
                        children: _attachments.asMap().entries.map((
                          MapEntry<int, PickedAttachmentFile> e,
                        ) {
                          final int i = e.key;
                          final PickedAttachmentFile a = e.value;
                          return AttachmentDisplayCard(
                            filename: a.filename,
                            subtitle: attachmentSizeLabel(a.sizeBytes),
                            trailing: IconButton(
                              tooltip: l10n.composeRemoveAttachment,
                              icon: const LucideIcon(LucideIcons.x, size: 20),
                              onPressed: () {
                                setState(() {
                                  _attachments =
                                      List<PickedAttachmentFile>.from(_attachments)
                                        ..removeAt(i);
                                });
                              },
                            ),
                          );
                        }).toList(),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}
