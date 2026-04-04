/*
 * settings_transports_navigator.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../rust/tagliacarte_api.dart';
import '../widgets/lucide_icon.dart';
import '../widgets/transport_strip_avatar.dart';
import 'settings_accounts_navigator.dart';

class TransportDetailRouteArgs {
  const TransportDetailRouteArgs({required this.isNew, this.existing});

  final bool isNew;
  final AppTransport? existing;
}

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
  }) {
    Navigator.of(context).pushNamed(
      '/transport',
      arguments: TransportDetailRouteArgs(isNew: isNew, existing: existing),
    );
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
          onTap: () => _openDetail(context, isNew: true),
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

class _TransportDetailPage extends StatefulWidget {
  const _TransportDetailPage({
    required this.api,
    required this.args,
    required this.onConfigReplaced,
  });

  final TagliacarteApi api;
  final TransportDetailRouteArgs args;
  final ValueChanged<AppSettingsConfig> onConfigReplaced;

  @override
  State<_TransportDetailPage> createState() => _TransportDetailPageState();
}

class _TransportDetailPageState extends State<_TransportDetailPage> {
  late final TextEditingController _displayName;
  late final TextEditingController _host;
  late final TextEditingController _port;
  String _security = 'starttls';
  bool _busy = false;
  String _snapshot = '';

  static const List<String> _securityChoices = <String>[
    'starttls',
    'tls',
    'plain',
  ];

  @override
  void initState() {
    super.initState();
    final AppTransport? e = widget.args.existing;
    _displayName = TextEditingController(
      text: e?.displayName ?? 'SMTP',
    );
    _host = TextEditingController(text: e?.host ?? 'smtp.example.com');
    _port = TextEditingController(text: '${e?.port ?? 587}');
    _security = e?.security ?? 'starttls';
    for (final TextEditingController c in <TextEditingController>[
      _displayName,
      _host,
      _port,
    ]) {
      c.addListener(() => setState(() {}));
    }
    _captureSnapshot();
  }

  void _captureSnapshot() {
    _snapshot = _serialize();
  }

  String _serialize() =>
      '${_displayName.text}\u0001${_host.text}\u0001${_port.text}\u0001$_security';

  bool get _dirty => _serialize() != _snapshot;

  @override
  void dispose() {
    _displayName.dispose();
    _host.dispose();
    _port.dispose();
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
    return p == null || p <= 0 || p > 65535 ? 587 : p;
  }

  Future<void> _save() async {
    final String name = _displayName.text.trim();
    final String host = _host.text.trim();
    if (name.isEmpty || host.isEmpty) {
      final AppLocalizations l10n = AppLocalizations.of(context);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.transportDisplayHostRequired)),
      );
      return;
    }
    final int port = _parsePort();
    final String uri = AppTransport.deriveSmtpUri(host, port);
    final String id = widget.args.isNew
        ? 't${DateTime.now().microsecondsSinceEpoch}'
        : widget.args.existing!.id;

    final AppTransport built = AppTransport(
      id: id,
      transportType: 'smtp',
      displayName: name,
      host: host,
      port: port,
      security: _security,
      transportUri: uri,
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
      final AppLocalizations l10n = AppLocalizations.of(context);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.transportSaved)),
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
          ],
        ),
      ),
    );
  }
}
