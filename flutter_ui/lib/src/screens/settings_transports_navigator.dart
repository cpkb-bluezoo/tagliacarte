/*
 * settings_transports_navigator.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../providers/mail_sync.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../widgets/lucide_icon.dart';
import '../widgets/smtp_google_oauth_dialog.dart';
import '../widgets/smtp_transport_credential_dialog.dart';
import '../widgets/transport_strip_avatar.dart';
import 'settings_accounts_navigator.dart';

String _dsnChoiceLabel(AppLocalizations l10n, String s) {
  switch (s) {
    case 'never':
      return l10n.dsnNever;
    case 'failure':
      return l10n.dsnFailure;
    case 'success':
      return l10n.dsnSuccess;
    case 'delay':
      return l10n.dsnDelay;
    case 'failure,success':
      return l10n.dsnFailureAndSuccess;
    default:
      return s;
  }
}

class TransportDetailRouteArgs {
  const TransportDetailRouteArgs({
    required this.isNew,
    required this.transportKind,
    this.existing,
  });

  final bool isNew;
  /// Lowercase transport kind for this screen. New rows use `smtp`; legacy `gmail` rows open as smtp.
  final String transportKind;
  final AppTransport? existing;
}

String _transportKindFromExisting(AppTransport t) =>
    t.transportType.toLowerCase() == 'smtp' ? 'smtp' : 'smtp';

/// Outgoing tab: list transports, add/edit/delete; writes full config via [saveConfig].
class OutgoingSettingsNavigator extends StatelessWidget {
  const OutgoingSettingsNavigator({
    super.key,
    required this.api,
    required this.onConfigReplaced,
  });

  final TagliacarteApi api;
  final ValueChanged<AppSettingsConfig> onConfigReplaced;

  @override
  Widget build(BuildContext context) {
    return Navigator(
      initialRoute: '/',
      onGenerateRoute: (RouteSettings settings) {
        if (settings.name == '/') {
          return MaterialPageRoute<void>(
            settings: settings,
            builder: (BuildContext context) => _TransportsListPage(
              api: api,
              onConfigReplaced: onConfigReplaced,
            ),
          );
        }
        if (settings.name == '/transport') {
          final TransportDetailRouteArgs args =
              settings.arguments! as TransportDetailRouteArgs;
          return MaterialPageRoute<void>(
            settings: settings,
            builder: (BuildContext context) => _TransportDetailPage(
              api: api,
              args: args,
              onConfigReplaced: onConfigReplaced,
            ),
          );
        }
        return null;
      },
    );
  }
}

class _TransportsListPage extends StatelessWidget {
  const _TransportsListPage({
    required this.api,
    required this.onConfigReplaced,
  });

  final TagliacarteApi api;
  final ValueChanged<AppSettingsConfig> onConfigReplaced;

  void _openDetail(
    BuildContext context, {
    required bool isNew,
    AppTransport? existing,
    String? transportKind,
  }) {
    final String kind = transportKind ??
        (existing != null
            ? _transportKindFromExisting(existing)
            : 'smtp');
    Navigator.of(context).pushNamed(
      '/transport',
      arguments: TransportDetailRouteArgs(
        isNew: isNew,
        existing: existing,
        transportKind: kind,
      ),
    );
  }

  Future<void> _onAddTransport(BuildContext context) async {
    if (context.mounted) {
      _openDetail(
        context,
        isNew: true,
        existing: null,
        transportKind: 'smtp',
      );
    }
  }

  Future<void> _deleteTransport(
    BuildContext context,
    AppTransport transport,
  ) async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final AppSettingsConfig cfg = SettingsAccountsConfigScope.of(context);
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.removeTransportTitle),
        content: Text(
          l10n.removeTransportBody(transport.uiListLabel),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l10n.remove),
          ),
        ],
      ),
    );
    if (ok != true || !context.mounted) {
      return;
    }
    final String id = transport.id;
    final List<AppTransport> nextTransports =
        cfg.transports.where((AppTransport t) => t.id != id).toList();
    final List<AppAccount> nextAccounts = cfg.accounts.map((AppAccount a) {
      final List<String> nextT =
          a.transportIds.where((String x) => x != id).toList();
      final Map<String, List<String>> nl = <String, List<String>>{};
      for (final MapEntry<String, List<String>> e in a.lists.entries) {
        nl[e.key] = List<String>.from(e.value);
      }
      nl['transportIds'] = nextT;
      return a.copyWith(lists: nl);
    }).toList();
    final AppSettingsConfig next = cfg.copyWith(
      transports: nextTransports,
      accounts: nextAccounts,
    );
    await api.saveConfig(next);
    if (!context.mounted) {
      return;
    }
    onConfigReplaced(next);
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(l10n.removedTransport(transport.uiListLabel))),
    );
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final AppSettingsConfig config = SettingsAccountsConfigScope.of(context);
    final ColorScheme scheme = Theme.of(context).colorScheme;
    final Brightness brightness = Theme.of(context).brightness;

    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 24),
      children: [
        Text(l10n.outgoingListTitle, style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 4),
        Text(
          l10n.outgoingListSubtitle,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
        ),
        const SizedBox(height: 16),
        for (final AppTransport t in config.transports)
          ListTile(
            leading: TransportStripAvatar(
              transport: t,
              brightness: brightness,
              selected: false,
            ),
            title: Text(t.primaryListTitle),
            subtitle: Text(t.typeDisplayLabel),
            trailing: IconButton(
              icon: const Icon(Icons.delete_outline),
              tooltip: l10n.deleteTooltip,
              onPressed: () => _deleteTransport(context, t),
            ),
            onTap: () => _openDetail(context, isNew: false, existing: t),
          ),
        ListTile(
          leading: LucideIcon(
            LucideIcons.circlePlus,
            size: 24,
            color: scheme.primary,
          ),
          title: Text(l10n.addTransport),
          onTap: () => _onAddTransport(context),
        ),
        if (config.transports.isEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 24),
            child: Text(
              l10n.noTransportsYet,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
            ),
          ),
      ],
    );
  }
}

class _TransportDetailPage extends ConsumerStatefulWidget {
  const _TransportDetailPage({
    required this.api,
    required this.args,
    required this.onConfigReplaced,
  });

  final TagliacarteApi api;
  final TransportDetailRouteArgs args;
  final ValueChanged<AppSettingsConfig> onConfigReplaced;

  @override
  ConsumerState<_TransportDetailPage> createState() =>
      _TransportDetailPageState();
}

class _TransportDetailPageState extends ConsumerState<_TransportDetailPage> {
  late final TextEditingController _displayName;
  late final TextEditingController _host;
  late final TextEditingController _port;
  late final TextEditingController _defaultFrom;
  /// Fixed `smtp` for this screen (legacy gmail rows are edited as smtp).
  late final String _kind;
  String _oauthProvider = '';
  String _security = 'tls';
  String _dsnNotify = 'failure';
  bool _busy = false;
  String _snapshot = '';

  static const List<String> _securityChoices = <String>[
    'starttls',
    'tls',
    'plain',
  ];

  static const List<String> _dsnChoices = <String>[
    'never',
    'failure',
    'success',
    'delay',
    'failure,success',
  ];

  static const List<String> _oauthProviderChoices = <String>[
    '',
    'google',
    'microsoft',
  ];

  @override
  void initState() {
    super.initState();
    final AppTransport? e = widget.args.existing;
    _kind = 'smtp';
    _displayName = TextEditingController(
      text: e?.displayName ?? '',
    );
    _host = TextEditingController(
      text: (e != null && e.host.trim().isNotEmpty) ? e.host : 'smtp.example.com',
    );
    _port = TextEditingController(
      text: '${e?.port ?? 465}',
    );
    _defaultFrom = TextEditingController(text: e?.defaultFrom ?? '');
    _security = e?.security ?? 'tls';
    final String provider = (e?.oauthProvider ?? '').trim().toLowerCase();
    _oauthProvider = _oauthProviderChoices.contains(provider) ? provider : '';
    final String rawDsn = (e?.dsnNotify ?? 'failure').trim();
    _dsnNotify = _dsnChoices.contains(rawDsn) ? rawDsn : 'failure';
    for (final TextEditingController c in <TextEditingController>[
      _displayName,
      _defaultFrom,
      if (_kind == 'smtp') ...<TextEditingController>[_host, _port],
    ]) {
      c.addListener(() => setState(() {}));
    }
    _captureSnapshot();
  }

  void _captureSnapshot() {
    _snapshot = _serialize();
  }

  String _serialize() {
    return 'smtp\u0001${_displayName.text}\u0001${_host.text}\u0001${_port.text}\u0001$_security\u0001${_defaultFrom.text}\u0001$_dsnNotify\u0001$_oauthProvider';
  }

  bool get _dirty => _serialize() != _snapshot;

  @override
  void dispose() {
    _displayName.dispose();
    _host.dispose();
    _port.dispose();
    _defaultFrom.dispose();
    super.dispose();
  }

  Future<bool> _confirmDiscard() async {
    if (!_dirty) {
      return true;
    }
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.discardChangesTitle),
        content: Text(l10n.discardChangesBody),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l10n.keepEditing),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l10n.discard),
          ),
        ],
      ),
    );
    return ok ?? false;
  }

  Future<void> _maybePop() async {
    if (await _confirmDiscard() && mounted) {
      Navigator.of(context).pop();
    }
  }

  int _parsePort() {
    final int? p = int.tryParse(_port.text.trim());
    return p == null || p <= 0 || p > 65535 ? 465 : p;
  }

  /// Connect, authenticate (prompting for SMTP password if needed), QUIT. Returns false if verify did not complete.
  Future<bool> _verifySmtpCredentials(
    AppTransport transport,
  ) async {
    final String transportId = transport.id;
    final String name = transport.displayName.trim().isEmpty
        ? transportId
        : transport.displayName.trim();
    final String host =
        transport.host.trim().isEmpty ? '—' : transport.host.trim();
    final AppLocalizations l10n = AppLocalizations.of(context);
    while (mounted) {
      try {
        await frbVerifySmtpTransport(transportId: transportId);
        return true;
      } catch (e) {
        if (!smtpSendShouldOfferCredentialPrompt(e)) {
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text(l10n.operationFailed(e.toString()))),
            );
          }
          return false;
        }
        if (!mounted) {
          return false;
        }
        if (smtpOfferGoogleBrowserOAuth(
            oauthProviderAttr: transport.oauthProvider, e: e)) {
          final bool? oauthOk = await showSmtpGoogleOAuthDialog(
            context,
            transportId: transportId,
          );
          if (!mounted) {
            return false;
          }
          if (oauthOk != true) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text(l10n.composeSendCancelledNoSmtpCredentials)),
            );
            return false;
          }
          continue;
        }
        final bool? saved = await showSmtpTransportCredentialDialog(
          context,
          transportId: transportId,
          transportName: name,
          host: host,
          usernameHint: transport.defaultFrom.trim(),
        );
        if (!mounted) {
          return false;
        }
        if (saved != true) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(l10n.composeSendCancelledNoSmtpCredentials)),
          );
          return false;
        }
      }
    }
    return false;
  }

  Future<void> _save() async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String name = _displayName.text.trim();
    final String hostTrim = _host.text.trim();
    if (name.isEmpty || hostTrim.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.transportDisplayHostRequired)),
      );
      return;
    }
    final String host = hostTrim;
    final int port = _parsePort();
    final String security = _security;
    final String id = widget.args.isNew
        ? 't${DateTime.now().microsecondsSinceEpoch}'
        : widget.args.existing!.id;

    final AppTransport built = AppTransport(
      id: id,
      transportType: 'smtp',
      displayName: name,
      host: host,
      port: port,
      security: security,
      defaultFrom: _defaultFrom.text.trim(),
      dsnNotify: _dsnNotify,
      oauthProvider: _oauthProvider,
    );

    setState(() => _busy = true);
    try {
      final AppSettingsConfig cfg = SettingsAccountsConfigScope.of(context);
      final List<AppTransport> list = List<AppTransport>.from(cfg.transports);
      if (widget.args.isNew) {
        list.add(built);
      } else {
        final int i = list.indexWhere((AppTransport t) => t.id == id);
        if (i >= 0) {
          list[i] = built;
        } else {
          list.add(built);
        }
      }
      final AppSettingsConfig next = cfg.copyWith(transports: list);
      await widget.api.saveConfig(next);
      if (!mounted) {
        return;
      }
      widget.onConfigReplaced(next);
      _captureSnapshot();
      final bool verified = await _verifySmtpCredentials(built);
      if (!mounted) {
        return;
      }
      if (!verified) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.transportSavedVerifyPending)),
        );
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.transportSavedAndVerified)),
      );
      Navigator.of(context).pop();
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String title =
        widget.args.isNew ? l10n.newTransport : l10n.editTransport;

    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (bool didPop, Object? result) async {
        if (didPop) {
          return;
        }
        if (await _confirmDiscard() && context.mounted) {
          Navigator.of(context).pop();
        }
      },
      child: Scaffold(
        appBar: AppBar(
          title: Text(title),
          leading: IconButton(
            icon: const Icon(Icons.arrow_back),
            onPressed: _maybePop,
            tooltip: l10n.back,
          ),
          actions: [
            TextButton.icon(
              onPressed: _busy ? null : _save,
              icon: _busy
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.save_outlined),
              label: Text(l10n.save),
            ),
            const SizedBox(width: 8),
          ],
        ),
        body: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            InputDecorator(
              decoration: InputDecoration(
                labelText: l10n.transportKindLabel,
                helperText: l10n.transportTypeFixedHelper,
              ),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Text(
                  l10n.transportKindSmtp,
                  style: Theme.of(context).textTheme.bodyLarge,
                ),
              ),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _displayName,
              decoration: InputDecoration(
                labelText: l10n.displayNameLabel,
              ),
            ),
            TextField(
              controller: _host,
              decoration: InputDecoration(labelText: l10n.smtpHostLabel),
            ),
            TextField(
              controller: _port,
              keyboardType: TextInputType.number,
              decoration: InputDecoration(
                labelText: l10n.portLabel,
                helperText: l10n.smtpPortHelper,
              ),
            ),
            const SizedBox(height: 8),
            DropdownButtonFormField<String>(
              key: ValueKey<String>(_security),
              initialValue: _security,
              decoration: InputDecoration(labelText: l10n.securityLabel),
              items: _securityChoices
                  .map(
                    (String s) => DropdownMenuItem<String>(
                      value: s,
                      child: Text(
                        s == 'starttls'
                            ? l10n.mailSecurityStarttls
                            : s == 'tls'
                                ? l10n.mailSecurityImplicitTlsSmtp
                                : l10n.mailSecurityNoEncryption,
                      ),
                    ),
                  )
                  .toList(),
              onChanged: (String? v) {
                if (v != null) {
                  setState(() => _security = v);
                }
              },
            ),
            const SizedBox(height: 8),
            DropdownButtonFormField<String>(
              key: ValueKey<String>(_oauthProvider),
              initialValue: _oauthProvider,
              decoration: const InputDecoration(
                labelText: 'OAuth provider',
                helperText: 'Optional: enables XOAUTH2 for this SMTP transport',
              ),
              items: _oauthProviderChoices
                  .map(
                    (String p) => DropdownMenuItem<String>(
                      value: p,
                      child: Text(
                        p.isEmpty
                            ? 'None (username/password)'
                            : (p == 'google' ? 'Google' : 'Microsoft'),
                      ),
                    ),
                  )
                  .toList(),
              onChanged: (String? v) {
                if (v != null) {
                  setState(() => _oauthProvider = v);
                }
              },
            ),
            TextField(
              controller: _defaultFrom,
              decoration: InputDecoration(
                labelText: l10n.defaultFromLabel,
                helperText: l10n.defaultFromHelper,
              ),
            ),
            const SizedBox(height: 8),
            DropdownButtonFormField<String>(
              key: ValueKey<String>(_dsnNotify),
              initialValue: _dsnNotify,
              decoration: InputDecoration(
                labelText: l10n.dsnNotifyLabel,
              ),
              items: _dsnChoices
                  .map(
                    (String s) => DropdownMenuItem<String>(
                      value: s,
                      child: Text(_dsnChoiceLabel(l10n, s)),
                    ),
                  )
                  .toList(),
              onChanged: (String? v) {
                if (v != null) {
                  setState(() => _dsnNotify = v);
                }
              },
            ),
          ],
        ),
      ),
    );
  }
}
