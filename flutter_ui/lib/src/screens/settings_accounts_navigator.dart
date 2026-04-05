/*
 * settings_accounts_navigator.dart
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

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../l10n/app_localizations.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../util/mail_account_policy.dart';
import '../widgets/lucide_icon.dart';
import '../widgets/nostr_credential_dialog.dart';
import '../widgets/store_switcher.dart';
import '../widgets/transport_strip_avatar.dart';

/// Supplies the latest [AppSettingsConfig] to the accounts navigator subtree
/// so list and detail routes stay in sync when accounts change.
class SettingsAccountsConfigScope extends InheritedWidget {
  const SettingsAccountsConfigScope({
    super.key,
    required this.config,
    required super.child,
  });

  final AppSettingsConfig config;

  static AppSettingsConfig of(BuildContext context) {
    final SettingsAccountsConfigScope? scope =
        context.dependOnInheritedWidgetOfExactType<SettingsAccountsConfigScope>();
    assert(scope != null, 'SettingsAccountsConfigScope not found');
    return scope!.config;
  }

  @override
  bool updateShouldNotify(SettingsAccountsConfigScope oldWidget) {
    return !identical(oldWidget.config, config);
  }
}

/// Ordered choices for the new-account type picker (matches account detail dropdown).
const List<({String id, String label})> kAccountBackendChoices =
    <({String id, String label})>[
      (id: 'IMAP', label: 'IMAP'),
      (id: 'Gmail', label: 'Gmail'),
      (id: 'Exchange', label: 'Exchange'),
      (id: 'POP3', label: 'POP3'),
      (id: 'Maildir', label: 'Maildir'),
      (id: 'mbox', label: 'mbox'),
      (id: 'NNTP', label: 'NNTP'),
      (id: 'Nostr', label: 'Nostr'),
      (id: 'Matrix', label: 'Matrix'),
    ];

bool _isLocalMailBackend(String backendType) {
  switch (backendType) {
    case 'Maildir':
    case 'maildir':
    case 'mbox':
      return true;
    default:
      return false;
  }
}

/// Normalizes to a single leading slash (matches core `path_with_leading_slash`).
String _pathWithLeadingSlashForLocalStore(String path) {
  final String trimmed = path.trim().replaceFirst(RegExp(r'^/+'), '');
  if (trimmed.isEmpty) {
    return '/';
  }
  return '/$trimmed';
}

String _localPathFromStoreUri(String uri) {
  final Uri? u = Uri.tryParse(uri);
  if (u == null || (u.scheme != 'maildir' && u.scheme != 'mbox')) {
    return '';
  }
  return u.path;
}

bool _showsTcpMailServerFields(String backendType) {
  switch (backendType) {
    case 'IMAP':
    case 'POP3':
    case 'NNTP':
      return true;
    default:
      return false;
  }
}

({String host, String portText, String security}) _imapSeedFromStoreUri(
  String uri,
) {
  final Uri? u = Uri.tryParse(uri);
  if (u == null || (u.scheme != 'imap' && u.scheme != 'imaps')) {
    return (
      host: 'imap.example.com',
      portText: '993',
      security: 'tls',
    );
  }
  final String host = u.host;
  if (u.scheme == 'imaps') {
    final int p = u.hasPort ? u.port : 993;
    return (host: host, portText: '$p', security: 'tls');
  }
  final int p = u.hasPort ? u.port : 143;
  final String? q = u.queryParameters['security'];
  final String sec = q == 'plain' ? 'plain' : 'starttls';
  return (host: host, portText: '$p', security: sec);
}

class AccountDetailRouteArgs {
  const AccountDetailRouteArgs({
    required this.isNew,
    required this.backendType,
    this.existing,
  });

  final bool isNew;
  final String backendType;
  final AppAccount? existing;
}

class AccountsSettingsNavigator extends StatelessWidget {
  const AccountsSettingsNavigator({
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
            builder: (BuildContext context) => _AccountsListPage(
              api: api,
              onConfigReplaced: onConfigReplaced,
            ),
          );
        }
        if (settings.name == '/account') {
          final AccountDetailRouteArgs args =
              settings.arguments! as AccountDetailRouteArgs;
          return MaterialPageRoute<void>(
            settings: settings,
            builder: (BuildContext context) => _AccountDetailPage(
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

class _AccountsListPage extends StatelessWidget {
  const _AccountsListPage({
    required this.api,
    required this.onConfigReplaced,
  });

  final TagliacarteApi api;
  final ValueChanged<AppSettingsConfig> onConfigReplaced;

  void _openDetail(
    BuildContext context, {
    required bool isNew,
    required String backendType,
    AppAccount? existing,
  }) {
    Navigator.of(context).pushNamed(
      '/account',
      arguments: AccountDetailRouteArgs(
        isNew: isNew,
        backendType: backendType,
        existing: existing,
      ),
    );
  }

  Future<void> _onAddAccount(BuildContext context) async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String? backendId = await showDialog<String>(
      context: context,
      builder: (BuildContext ctx) => SimpleDialog(
        title: Text(l10n.accountTypeDialogTitle),
        children: <Widget>[
          for (final ({String id, String label}) e in kAccountBackendChoices)
            SimpleDialogOption(
              onPressed: () => Navigator.pop(ctx, e.id),
              child: Text(e.label),
            ),
        ],
      ),
    );
    if (backendId != null && context.mounted) {
      _openDetail(
        context,
        isNew: true,
        backendType: backendId,
      );
    }
  }

  Future<void> _deleteAccount(BuildContext context, AppAccount account) async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.removeAccountTitle),
        content: Text(
          l10n.removeAccountBody(account.label),
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
    final AppSettingsConfig cfg = SettingsAccountsConfigScope.of(context);
    final AppSettingsConfig next =
        await api.removeAccount(cfg, account.id);
    if (!context.mounted) {
      return;
    }
    onConfigReplaced(next);
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(l10n.removedAccount(account.label))),
    );
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final AppSettingsConfig config = SettingsAccountsConfigScope.of(context);
    final Brightness brightness = Theme.of(context).brightness;
    final ColorScheme scheme = Theme.of(context).colorScheme;

    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 24),
      children: [
        Text(
          l10n.accountsListTitle,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 4),
        Text(
          l10n.accountsListSubtitle,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
        ),
        const SizedBox(height: 16),
        for (final AppAccount account in config.accounts)
          ListTile(
            leading: AccountStripAvatar(
              account: account,
              brightness: brightness,
              selected: false,
            ),
            title: Text(account.label),
            subtitle: Text(account.backendType),
            trailing: IconButton(
              icon: const Icon(Icons.delete_outline),
              tooltip: l10n.deleteTooltip,
              onPressed: () => _deleteAccount(context, account),
            ),
            onTap: () => _openDetail(
              context,
              isNew: false,
              backendType: account.backendType,
              existing: account,
            ),
          ),
        ListTile(
          leading: LucideIcon(
            LucideIcons.circlePlus,
            size: 24,
            color: scheme.primary,
          ),
          title: Text(l10n.addAccount),
          onTap: () => _onAddAccount(context),
        ),
        if (config.accounts.isEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 24),
            child: Text(
              l10n.noAccountsYet,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
            ),
          ),
      ],
    );
  }
}

class _AccountDetailPage extends StatefulWidget {
  const _AccountDetailPage({
    required this.api,
    required this.args,
    required this.onConfigReplaced,
  });

  final TagliacarteApi api;
  final AccountDetailRouteArgs args;
  final ValueChanged<AppSettingsConfig> onConfigReplaced;

  @override
  State<_AccountDetailPage> createState() => _AccountDetailPageState();
}

class _AccountDetailPageState extends State<_AccountDetailPage> {
  late String _backendType;
  late final TextEditingController _accountName;
  late final TextEditingController _imapHost;
  late final TextEditingController _imapPort;
  late final TextEditingController _imapMinIdleSeconds;
  late final TextEditingController _username;
  String _imapSecurity = 'tls';
  late final TextEditingController _avatarUrl;
  late final TextEditingController _homeserver;
  late final TextEditingController _nip05;
  /// One [TextEditingController] per relay row (Nostr only).
  List<TextEditingController> _nostrRelayControllers = <TextEditingController>[];
  late final TextEditingController _nostrNewRelayRow;
  late final TextEditingController _localStorePath;
  late List<String> _orderedTransportIds;
  String _nostrNpub = '';
  /// When creating a new account, set when user creates a Nostr identity so credential id matches save.
  String? _provisionalAccountId;
  bool _isSaving = false;
  String _snapshot = '';

  @override
  void initState() {
    super.initState();
    final AppAccount? e = widget.args.existing;
    _backendType = widget.args.backendType;
    _accountName = TextEditingController(text: e?.label ?? '');

    String imapHostText = 'imap.example.com';
    String portText = '993';
    _imapSecurity = 'tls';

    if (e != null && e.backendType == 'IMAP') {
      final String? h = e.attrs['host'];
      if (h != null && h.isNotEmpty) {
        imapHostText = h;
        portText = e.attrs['port'] ??
            '${(e.attrs['security'] == 'starttls' || e.attrs['security'] == 'plain' ? 143 : 993)}';
        _imapSecurity = e.attrs['security'] ?? 'tls';
      } else {
        final ({String host, String portText, String security}) seed =
            _imapSeedFromStoreUri(e.storeUri);
        imapHostText = seed.host;
        portText = seed.portText;
        _imapSecurity = seed.security;
      }
    } else if (e != null && e.backendType == 'POP3') {
      imapHostText = _hostFromPop3Uri(e.storeUri);
      portText = '${_portFromPop3Uri(e.storeUri) ?? 995}';
    } else if (e != null && e.backendType == 'NNTP') {
      imapHostText = _hostFromNntpUri(e.storeUri);
      portText = '${_portFromNntpUri(e.storeUri) ?? 563}';
    } else {
      switch (widget.args.backendType) {
        case 'POP3':
          imapHostText = 'pop.example.com';
          portText = '995';
          break;
        case 'NNTP':
          imapHostText = 'news.example.com';
          portText = '563';
          break;
        case 'IMAP':
        default:
          if (_showsTcpMailServerFields(widget.args.backendType)) {
            imapHostText = widget.args.backendType == 'NNTP'
                ? 'news.example.com'
                : widget.args.backendType == 'POP3'
                    ? 'pop.example.com'
                    : 'imap.example.com';
            portText = widget.args.backendType == 'NNTP'
                ? '563'
                : widget.args.backendType == 'POP3'
                    ? '995'
                    : '993';
            _imapSecurity = 'tls';
          }
          break;
      }
    }

    _imapHost = TextEditingController(text: imapHostText);
    _imapPort = TextEditingController(text: portText);
    _imapMinIdleSeconds = TextEditingController(
      text: e != null &&
              e.backendType == 'IMAP' &&
              (e.attrs['imapIdleMinIdleSeconds'] ?? '').isNotEmpty
          ? e.attrs['imapIdleMinIdleSeconds']!
          : '',
    );
    _username = TextEditingController(
      text: e?.attrs['email'] ?? e?.attrs['username'] ?? '',
    );
    _avatarUrl = TextEditingController(text: e?.avatarUrl ?? '');
    _homeserver = TextEditingController(
      text: e != null && e.backendType == 'Matrix'
          ? _homeserverFromMatrixStore(e.storeUri)
          : 'https://matrix.org',
    );
    _nip05 = TextEditingController(text: e?.attrs['nip05'] ?? '');
    _nostrNpub = e?.attrs['npub'] ?? '';
    _nostrNewRelayRow = TextEditingController()..addListener(_onFieldChanged);
    _initNostrRelayControllers(e, widget.args.backendType);
    _localStorePath = TextEditingController(
      text: e != null && _isLocalMailBackend(e.backendType)
          ? _localPathFromStoreUri(e.storeUri)
          : '',
    );
    _orderedTransportIds = List<String>.from(e?.transportIds ?? const <String>[]);
    for (final TextEditingController c in <TextEditingController>[
      _accountName,
      _imapHost,
      _imapPort,
      _imapMinIdleSeconds,
      _username,
      _avatarUrl,
      _homeserver,
      _nip05,
      _localStorePath,
    ]) {
      c.addListener(_onFieldChanged);
    }
    _captureSnapshot();
  }

  void _captureSnapshot() {
    _snapshot = _serializeForm();
  }

  void _onFieldChanged() {
    setState(() {});
  }

  bool get _isDirty => _serializeForm() != _snapshot;

  String _serializeForm() {
    return <String>[
      _backendType,
      _accountName.text,
      _imapHost.text,
      _imapPort.text,
      _imapSecurity,
      _imapMinIdleSeconds.text,
      _username.text,
      _avatarUrl.text,
      _homeserver.text,
      _nip05.text,
      _nostrNpub,
      _backendType == 'Nostr'
          ? _nostrRelayControllers.map((TextEditingController c) => c.text).join('\u0002')
          : '',
      _localStorePath.text,
      _orderedTransportIds.join(','),
    ].join('\u0001');
  }

  @override
  void dispose() {
    for (final TextEditingController c in <TextEditingController>[
      _accountName,
      _imapHost,
      _imapPort,
      _imapMinIdleSeconds,
      _username,
      _avatarUrl,
      _homeserver,
      _nip05,
      _localStorePath,
    ]) {
      c.dispose();
    }
    for (final TextEditingController c in _nostrRelayControllers) {
      c.dispose();
    }
    _nostrNewRelayRow.dispose();
    super.dispose();
  }

  Future<bool> _confirmDiscard() async {
    if (!_isDirty) {
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

  Future<void> _pickLocalMailboxPath() async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (kIsWeb) {
      _toast(l10n.pickNotSupportedWeb);
      return;
    }
    // Backend ids use title case in the UI (e.g. [kAccountBackendChoices] `'Maildir'`).
    switch (_backendType) {
      case 'Maildir':
      case 'maildir':
        final String? dir = await FilePicker.platform.getDirectoryPath(
          dialogTitle: l10n.chooseMaildirFolderTitle,
        );
        if (dir != null && mounted) {
          _localStorePath.text = dir;
        }
      case 'mbox':
        final FilePickerResult? result = await FilePicker.platform.pickFiles(
          type: FileType.any,
          allowMultiple: false,
          dialogTitle: l10n.chooseMboxFileTitle,
        );
        if (result != null && result.files.isNotEmpty) {
          final String? path = result.files.single.path;
          if (path != null && mounted) {
            _localStorePath.text = path;
          }
        }
      default:
        break;
    }
  }

  Future<void> _save() async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String label = _accountName.text.trim();
    final String user = _username.text.trim();
    if (label.isEmpty) {
      _toast(l10n.validationAccountNameRequired);
      return;
    }
    if (_isLocalMailBackend(_backendType)) {
      if (_localStorePath.text.trim().isEmpty) {
        _toast(l10n.validationLocalPathRequired);
        return;
      }
    } else if (_backendType != 'Nostr' && user.isEmpty) {
      _toast(l10n.validationUsernameRequired);
      return;
    }
    if (_backendType == 'Nostr') {
      final List<String> ru = _effectiveNostrRelayUrls();
      if (ru.isEmpty) {
        _toast(l10n.nostrRelayUrlsRequired);
        return;
      }
    }
    if (_showsTcpMailServerFields(_backendType) &&
        _imapHost.text.trim().isEmpty) {
      _toast(l10n.validationHostRequired);
      return;
    }
    if (_showsTcpMailServerFields(_backendType) &&
        int.tryParse(_imapPort.text.trim()) == null) {
      _toast(l10n.validationPortRequired);
      return;
    }
    int? imapIdleMinIdleSeconds;
    if (_backendType == 'IMAP') {
      final String idleRaw = _imapMinIdleSeconds.text.trim();
      if (idleRaw.isNotEmpty) {
        final int? idleParsed = int.tryParse(idleRaw);
        if (idleParsed == null || idleParsed < 15 || idleParsed > 864000) {
          _toast(l10n.validationImapMinIdleSeconds);
          return;
        }
        imapIdleMinIdleSeconds = idleParsed;
      }
    } else {
      imapIdleMinIdleSeconds = null;
    }
    setState(() => _isSaving = true);
    try {
      final String id;
      if (_provisionalAccountId != null) {
        id = _provisionalAccountId!;
      } else if (widget.args.isNew) {
        id = 's${DateTime.now().microsecondsSinceEpoch}';
        _provisionalAccountId = id;
      } else {
        id = widget.args.existing!.id;
      }

      bool linkedNsecThisSave = false;
      if (_backendType == 'Nostr' && _nostrNpub.trim().isEmpty) {
        if (!mounted) {
          return;
        }
        final String? npubFromVault = await showNostrCredentialDialog(
          context,
          account: AppAccount(
            id: id,
            label: label,
            backendType: 'Nostr',
            storeUri: 'nostr:',
            attrs: <String, String>{},
            lists: <String, List<String>>{},
          ),
        );
        if (!mounted) {
          return;
        }
        if (npubFromVault == null || npubFromVault.isEmpty) {
          return;
        }
        linkedNsecThisSave = true;
        setState(() => _nostrNpub = npubFromVault);
      }
      final String avatarTrim = _avatarUrl.text.trim();
      final AppAccount? e = widget.args.existing;
      final Map<String, String> attrs = e != null
          ? Map<String, String>.from(e.attrs)
          : <String, String>{};
      final Map<String, List<String>> lists = e != null
          ? <String, List<String>>{
              for (final MapEntry<String, List<String>> x in e.lists.entries)
                x.key: List<String>.from(x.value),
            }
          : <String, List<String>>{};

      if (_backendType == 'IMAP' ||
          _backendType == 'POP3' ||
          _backendType == 'NNTP') {
        attrs['host'] = _imapHost.text.trim();
        attrs['port'] = _imapPort.text.trim();
        attrs['security'] = _imapSecurity;
        attrs['username'] = user;
        attrs['email'] = user;
        if (_backendType == 'IMAP') {
          if (imapIdleMinIdleSeconds != null) {
            attrs['imapIdleMinIdleSeconds'] = '$imapIdleMinIdleSeconds';
          } else {
            attrs.remove('imapIdleMinIdleSeconds');
          }
        }
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      } else if (_backendType == 'Gmail' || _backendType == 'Exchange') {
        attrs['email'] = user;
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      } else if (_isLocalMailBackend(_backendType)) {
        attrs['path'] = _localStorePath.text.trim();
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      } else if (_backendType == 'Nostr') {
        attrs['npub'] = _nostrNpub.trim();
        final String n5 = _nip05.text.trim();
        if (n5.isEmpty) {
          attrs.remove('nip05');
        } else {
          attrs['nip05'] = n5;
        }
        lists['relayUrls'] = _effectiveNostrRelayUrls();
        lists.remove('transportIds');
        attrs.remove('username');
        attrs.remove('email');
        attrs.remove('host');
        attrs.remove('port');
        attrs.remove('security');
      } else {
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      }

      final AppAccount account = AppAccount(
        id: id,
        label: label,
        backendType: _backendType,
        storeUri: _deriveStoreUri(_backendType, user),
        avatarUrl: avatarTrim.isEmpty ? null : avatarTrim,
        attrs: attrs,
        lists: lists,
      );
      final AppSettingsConfig cfg = SettingsAccountsConfigScope.of(context);
      final AppSettingsConfig next =
          await widget.api.addOrUpdateAccount(cfg, account);
      if (!mounted) {
        return;
      }
      widget.onConfigReplaced(next);
      if (_backendType == 'Nostr') {
        String? xmlPath;
        try {
          xmlPath = await widget.api.configXmlPath();
          await frbNostrSyncRemoteProfile(path: xmlPath, accountId: id);
          final AppSettingsConfig afterSync =
              await widget.api.loadConfig();
          if (mounted) {
            widget.onConfigReplaced(afterSync);
          }
        } catch (_) {
          /* sync is best-effort */
        }
        if (!linkedNsecThisSave) {
          try {
            xmlPath ??= await widget.api.configXmlPath();
            await frbNostrPublishProfile(path: xmlPath, accountId: id);
          } catch (_) {
            /* publish is best-effort */
          }
        }
      }
      if (!mounted) {
        return;
      }
      _captureSnapshot();
      _toast(l10n.accountSaved);
      Navigator.of(context).pop();
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  AppTransport? _transportById(String tid) {
    for (final AppTransport t
        in SettingsAccountsConfigScope.of(context).transports) {
      if (t.id == tid) {
        return t;
      }
    }
    return null;
  }

  void _moveTransportSlot(int index, int delta) {
    final int j = index + delta;
    if (j < 0 || j >= _orderedTransportIds.length) {
      return;
    }
    setState(() {
      final String tmp = _orderedTransportIds[index];
      _orderedTransportIds[index] = _orderedTransportIds[j];
      _orderedTransportIds[j] = tmp;
    });
  }

  void _removeTransportSlot(int index) {
    setState(() {
      _orderedTransportIds.removeAt(index);
    });
  }

  Future<void> _addTransportSlot() async {
    final AppSettingsConfig cfg = SettingsAccountsConfigScope.of(context);
    final List<AppTransport> avail = cfg.transports
        .where((AppTransport t) => !_orderedTransportIds.contains(t.id))
        .toList();
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (avail.isEmpty) {
      _toast(l10n.createTransportFirst);
      return;
    }
    final String? picked = await showDialog<String>(
      context: context,
      builder: (BuildContext ctx) => SimpleDialog(
        title: Text(l10n.addTransportDialogTitle),
        children: <Widget>[
          for (final AppTransport t in avail)
            SimpleDialogOption(
              onPressed: () => Navigator.pop(ctx, t.id),
              child: Row(
                children: <Widget>[
                  TransportStripAvatar(
                    transport: t,
                    brightness: Theme.of(ctx).brightness,
                    selected: false,
                  ),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        Text(
                          t.primaryListTitle,
                          style: Theme.of(ctx).textTheme.titleSmall,
                        ),
                        Text(
                          t.typeDisplayLabel,
                          style: Theme.of(ctx).textTheme.bodySmall?.copyWith(
                                color: Theme.of(ctx)
                                    .colorScheme
                                    .onSurfaceVariant,
                              ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
    if (picked != null && mounted) {
      setState(() => _orderedTransportIds.add(picked));
    }
  }

  void _toast(String message) {
    ScaffoldMessenger.of(context)
      ..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(message)));
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String title = widget.args.isNew
        ? l10n.accountDetailTitleNew(_backendLabel(_backendType))
        : l10n.accountDetailTitleEdit(widget.args.existing!.label);

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
          actions: <Widget>[
            if (_backendType == 'Nostr')
              IconButton(
                tooltip: l10n.nostrNewIdentityTooltip,
                onPressed: _isSaving ? null : _createNostrIdentity,
                icon: const Icon(Icons.vpn_key_outlined),
              ),
            TextButton.icon(
              onPressed: _isSaving ? null : _save,
              icon: _isSaving
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
        body: Column(
          children: <Widget>[
            Expanded(
              child: ListView(
                padding: const EdgeInsets.all(16),
                children: <Widget>[
                  DropdownButtonFormField<String>(
                    key: ValueKey<String>(_backendType),
                    initialValue: _backendType,
                    items: kAccountBackendChoices
                        .map(
                          (({String id, String label}) e) =>
                              DropdownMenuItem<String>(
                            value: e.id,
                            child: Text(e.label),
                          ),
                        )
                        .toList(),
                    onChanged: widget.args.isNew
                        ? (String? value) {
                            if (value != null) {
                              setState(() {
                                final String prev = _backendType;
                                _backendType = value;
                                if (prev == 'Nostr' && value != 'Nostr') {
                                  for (final TextEditingController c
                                      in _nostrRelayControllers) {
                                    c.removeListener(_onFieldChanged);
                                    c.dispose();
                                  }
                                  _nostrRelayControllers =
                                      <TextEditingController>[];
                                } else if (prev != 'Nostr' &&
                                    value == 'Nostr') {
                                  _initNostrRelayControllers(
                                    widget.args.existing,
                                    'Nostr',
                                  );
                                }
                                if (value == 'POP3') {
                                  _imapPort.text = '995';
                                  _imapHost.text = 'pop.example.com';
                                } else if (value == 'NNTP') {
                                  _imapPort.text = '563';
                                  _imapHost.text = 'news.example.com';
                                } else if (value == 'IMAP') {
                                  _imapPort.text = '993';
                                  _imapSecurity = 'tls';
                                  _imapHost.text = 'imap.example.com';
                                }
                              });
                            }
                          }
                        : null,
                    decoration: InputDecoration(
                      labelText: l10n.accountTypeLabel,
                      helperText: l10n.accountTypeHelper,
                    ),
                  ),
                  TextField(
                    controller: _accountName,
                    decoration: InputDecoration(
                      labelText: l10n.accountNameLabel,
                    ),
                  ),
                  if (_backendType != 'Nostr')
                    TextField(
                      controller: _username,
                      decoration: InputDecoration(
                        labelText: _isLocalMailBackend(_backendType)
                            ? l10n.usernameEmailOptional
                            : l10n.usernameEmailRequired,
                      ),
                    ),
                  TextField(
                    controller: _avatarUrl,
                    decoration: InputDecoration(
                      labelText: l10n.avatarUrlLabel,
                      helperText: l10n.avatarUrlHelper,
                    ),
                  ),
                  if (_isLocalMailBackend(_backendType)) ...<Widget>[
                    const SizedBox(height: 12),
                    Text(
                      l10n.localMailboxSection,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    TextField(
                      controller: _localStorePath,
                      decoration: InputDecoration(
                        labelText: _backendType == 'mbox'
                            ? l10n.pathMboxFile
                            : l10n.pathMaildirRoot,
                        helperText: _backendType == 'mbox'
                            ? l10n.helperMboxPath
                            : l10n.helperMaildirPath,
                        suffixIcon: IconButton(
                          icon: Icon(
                            _backendType == 'mbox'
                                ? Icons.insert_drive_file_outlined
                                : Icons.folder_open_outlined,
                          ),
                          tooltip: _backendType == 'mbox'
                              ? l10n.chooseMboxTooltip
                              : l10n.chooseMaildirTooltip,
                          onPressed: _pickLocalMailboxPath,
                        ),
                      ),
                    ),
                  ],
                  if (_showsTcpMailServerFields(_backendType)) ...<Widget>[
                    const SizedBox(height: 12),
                    Text(
                      _backendType == 'IMAP'
                          ? l10n.imapServerSection
                          : _backendType == 'POP3'
                              ? l10n.pop3ServerSection
                              : l10n.nntpServerSection,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    TextField(
                      controller: _imapHost,
                      decoration: InputDecoration(
                        labelText:
                            _backendType == 'NNTP'
                                ? l10n.serverHostLabel
                                : l10n.hostLabel,
                      ),
                    ),
                    TextField(
                      controller: _imapPort,
                      keyboardType: TextInputType.number,
                      decoration: InputDecoration(
                        labelText: l10n.portLabel,
                        helperText: _backendType == 'IMAP'
                            ? l10n.portHelperImap
                            : _backendType == 'POP3'
                                ? l10n.portHelperPop3
                                : l10n.portHelperNntp,
                      ),
                    ),
                    if (_backendType == 'IMAP')
                      DropdownButtonFormField<String>(
                        key: ValueKey<String>(_imapSecurity),
                        initialValue: _imapSecurity,
                        decoration: InputDecoration(
                          labelText: l10n.securityLabel,
                        ),
                        items: <DropdownMenuItem<String>>[
                          DropdownMenuItem(
                            value: 'tls',
                            child: Text(l10n.mailSecurityImplicitTlsImap),
                          ),
                          DropdownMenuItem(
                            value: 'starttls',
                            child: Text(l10n.mailSecurityStarttls),
                          ),
                          DropdownMenuItem(
                            value: 'plain',
                            child: Text(l10n.mailSecurityNoEncryption),
                          ),
                        ],
                        onChanged: (String? value) {
                          if (value != null) {
                            setState(() => _imapSecurity = value);
                          }
                        },
                      ),
                    if (_backendType == 'IMAP')
                      TextField(
                        controller: _imapMinIdleSeconds,
                        keyboardType: TextInputType.number,
                        decoration: InputDecoration(
                          labelText: l10n.accountImapMinIdleSecondsLabel,
                          helperText: l10n.accountImapMinIdleSecondsHelper,
                        ),
                      ),
                  ],
                  if (backendTypeRequiresOutboundTransport(_backendType)) ...[
                    const SizedBox(height: 16),
                    Text(
                      l10n.outgoingTransportsSection,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 4),
                    Text(
                      _orderedTransportIds.isEmpty
                          ? l10n.noTransportsHintLinked
                          : l10n.transportsOrderHint,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: Theme.of(context).colorScheme.onSurfaceVariant,
                          ),
                    ),
                    const SizedBox(height: 8),
                    ...List<Widget>.generate(_orderedTransportIds.length, (
                      int i,
                    ) {
                      final AppTransport? tr =
                          _transportById(_orderedTransportIds[i]);
                      if (tr == null) {
                        return ListTile(
                          title: Text(l10n.unknownTransport),
                          trailing: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: <Widget>[
                              IconButton(
                                icon: const Icon(Icons.arrow_upward),
                                tooltip: l10n.moveUpTooltip,
                                onPressed: i > 0
                                    ? () => _moveTransportSlot(i, -1)
                                    : null,
                              ),
                              IconButton(
                                icon: const Icon(Icons.arrow_downward),
                                tooltip: l10n.moveDownTooltip,
                                onPressed: i < _orderedTransportIds.length - 1
                                    ? () => _moveTransportSlot(i, 1)
                                    : null,
                              ),
                              IconButton(
                                icon: const Icon(Icons.close),
                                tooltip: l10n.removeFromAccountTooltip,
                                onPressed: () => _removeTransportSlot(i),
                              ),
                            ],
                          ),
                        );
                      }
                      return ListTile(
                        leading: TransportStripAvatar(
                          transport: tr,
                          brightness: Theme.of(context).brightness,
                          selected: false,
                        ),
                        title: Text(tr.primaryListTitle),
                        subtitle: Text(tr.typeDisplayLabel),
                        trailing: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            IconButton(
                              icon: const Icon(Icons.arrow_upward),
                              tooltip: l10n.moveUpTooltip,
                              onPressed: i > 0
                                  ? () => _moveTransportSlot(i, -1)
                                  : null,
                            ),
                            IconButton(
                              icon: const Icon(Icons.arrow_downward),
                              tooltip: l10n.moveDownTooltip,
                              onPressed: i < _orderedTransportIds.length - 1
                                  ? () => _moveTransportSlot(i, 1)
                                  : null,
                            ),
                            IconButton(
                              icon: const Icon(Icons.close),
                              tooltip: l10n.removeFromAccountTooltip,
                              onPressed: () => _removeTransportSlot(i),
                            ),
                          ],
                        ),
                      );
                    }),
                    Align(
                      alignment: Alignment.centerLeft,
                      child: TextButton.icon(
                        onPressed: _addTransportSlot,
                        icon: const Icon(Icons.add),
                        label: Text(l10n.addTransportToAccount),
                      ),
                    ),
                  ],
                  if (_backendType == 'Matrix') ...<Widget>[
                    const SizedBox(height: 12),
                    Text(
                      l10n.matrixSection,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    TextField(
                      controller: _homeserver,
                      decoration: InputDecoration(
                        labelText: l10n.homeserverLabel,
                      ),
                    ),
                  ],
                  if (_backendType == 'Nostr') ...<Widget>[
                    const SizedBox(height: 12),
                    Text(
                      l10n.nostrSection,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    TextField(
                      controller: _nip05,
                      decoration: const InputDecoration(
                        labelText: 'NIP-05 (optional)',
                      ),
                    ),
                    const SizedBox(height: 8),
                    InputDecorator(
                      decoration: const InputDecoration(
                        labelText: 'npub (read-only)',
                      ),
                      child: Row(
                        children: <Widget>[
                          Expanded(
                            child: Text(
                              _nostrNpub.isEmpty ? '—' : _nostrNpub,
                              style: Theme.of(context).textTheme.bodyMedium,
                            ),
                          ),
                          IconButton(
                            tooltip: 'Copy npub',
                            onPressed: _nostrNpub.isEmpty
                                ? null
                                : () async {
                                    await Clipboard.setData(
                                      ClipboardData(text: _nostrNpub),
                                    );
                                    _toast('Copied npub');
                                  },
                            icon: const Icon(Icons.copy_outlined),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      l10n.relayUrlsLabel,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 4),
                    Text(
                      l10n.relayUrlsHelper,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: Theme.of(context)
                                .colorScheme
                                .onSurfaceVariant,
                          ),
                    ),
                    const SizedBox(height: 8),
                    ...List<Widget>.generate(_nostrRelayControllers.length, (
                      int i,
                    ) {
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Expanded(
                              child: TextField(
                                controller: _nostrRelayControllers[i],
                                autocorrect: false,
                                decoration: InputDecoration(
                                  hintText: 'wss://...',
                                ),
                                onSubmitted: (_) =>
                                    FocusScope.of(context).unfocus(),
                              ),
                            ),
                            if (_nostrRelayControllers.length > 1)
                              IconButton(
                                tooltip: l10n.relayRemoveTooltip,
                                onPressed: _isSaving
                                    ? null
                                    : () => _removeNostrRelayAt(i),
                                icon: const Icon(Icons.remove_circle_outline),
                              ),
                          ],
                        ),
                      );
                    }),
                    Padding(
                      padding: const EdgeInsets.only(bottom: 8),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Expanded(
                            child: TextField(
                              controller: _nostrNewRelayRow,
                              autocorrect: false,
                              decoration: InputDecoration(
                                hintText: l10n.relayAddFieldHint,
                              ),
                              onSubmitted: (_) {
                                _addNostrRelayFromPending();
                                FocusScope.of(context).unfocus();
                              },
                            ),
                          ),
                          IconButton(
                            tooltip: l10n.relayAddTooltip,
                            onPressed:
                                _isSaving ? null : _addNostrRelayFromPending,
                            icon: const Icon(Icons.add_circle_outline),
                          ),
                        ],
                      ),
                    ),
                  ],
                  if (widget.args.existing != null) ...<Widget>[
                    const SizedBox(height: 16),
                    Text(
                      l10n.storeUriLabel(widget.args.existing!.storeUri),
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    if ((widget.args.existing!.attrs['transportUri'] ?? '')
                        .isNotEmpty)
                      Text(
                        l10n.transportUriLabel(
                          widget.args.existing!.attrs['transportUri']!,
                        ),
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  void _initNostrRelayControllers(AppAccount? e, String backendType) {
    for (final TextEditingController c in _nostrRelayControllers) {
      c.removeListener(_onFieldChanged);
      c.dispose();
    }
    _nostrRelayControllers = <TextEditingController>[];
    if (e?.backendType != 'Nostr' && backendType != 'Nostr') {
      return;
    }
    List<String> urls;
    if (e != null && e.backendType == 'Nostr') {
      urls = e.relayUrls.isNotEmpty
          ? List<String>.from(e.relayUrls)
          : _relayUrlsFromLegacyNostrUri(e.storeUri);
    } else {
      urls = <String>['wss://relay.damus.io', 'wss://nos.lol'];
    }
    if (urls.isEmpty) {
      urls = <String>['wss://relay.damus.io'];
    }
    _nostrRelayControllers = urls
        .map(
          (String u) =>
              TextEditingController(text: u)..addListener(_onFieldChanged),
        )
        .toList();
  }

  List<String> _effectiveNostrRelayUrls() {
    return _nostrRelayControllers
        .map((TextEditingController c) => c.text.trim())
        .where((String s) => s.isNotEmpty)
        .toList();
  }

  void _removeNostrRelayAt(int index) {
    if (_nostrRelayControllers.length <= 1 ||
        index < 0 ||
        index >= _nostrRelayControllers.length) {
      return;
    }
    final TextEditingController removed = _nostrRelayControllers.removeAt(index);
    removed.removeListener(_onFieldChanged);
    removed.dispose();
    setState(() {});
  }

  void _addNostrRelayFromPending() {
    final String t = _nostrNewRelayRow.text.trim();
    if (t.isEmpty) {
      return;
    }
    _nostrRelayControllers.add(
      TextEditingController(text: t)..addListener(_onFieldChanged),
    );
    _nostrNewRelayRow.clear();
    setState(() {});
  }

  Future<void> _createNostrIdentity() async {
    try {
      final String jsonStr = await frbNostrGenerateKeypairJson();
      final Map<String, dynamic> m = jsonDecode(jsonStr) as Map<String, dynamic>;
      final String sk = m['secretHex'] as String? ?? '';
      final String pk = m['pubkeyHex'] as String? ?? '';
      if (sk.isEmpty || pk.isEmpty) {
        _toast('Key generation failed');
        return;
      }
      final String npub = await frbNostrHexToNpub(hexPubkey: pk);
      if (!mounted) {
        return;
      }
      final AppLocalizations l10n = AppLocalizations.of(context);
      final bool? ok = await showDialog<bool>(
        context: context,
        builder: (BuildContext ctx) => AlertDialog(
          title: const Text('Secret key'),
          content: SingleChildScrollView(
            child: Text(
              'Copy your nsec once and store it safely. It will be saved to the app vault when you confirm.\n\n$sk',
            ),
          ),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: const Text('Save to vault'),
            ),
          ],
        ),
      );
      if (ok != true || !mounted) {
        return;
      }
      final String cid;
      if (_provisionalAccountId != null) {
        cid = _provisionalAccountId!;
      } else if (widget.args.isNew) {
        cid = 's${DateTime.now().microsecondsSinceEpoch}';
        _provisionalAccountId = cid;
      } else {
        cid = widget.args.existing!.id;
      }
      await frbSaveStoreCredential(
        accountId: cid,
        username: npub,
        password: sk,
      );
      setState(() => _nostrNpub = npub);
      _toast('Nostr identity ready — save the account to finish.');
    } catch (e) {
      _toast('$e');
    }
  }

  String _backendLabel(String id) {
    for (final ({String id, String label}) e in kAccountBackendChoices) {
      if (e.id == id) {
        return e.label;
      }
    }
    return id;
  }

  String _deriveStoreUri(String backendType, String user) {
    switch (backendType) {
      case 'Maildir':
      case 'maildir': {
        final String p =
            _pathWithLeadingSlashForLocalStore(_localStorePath.text);
        return 'maildir://$p';
      }
      case 'mbox': {
        final String p =
            _pathWithLeadingSlashForLocalStore(_localStorePath.text);
        return 'mbox://$p';
      }
      case 'IMAP': {
        final String h = _imapHost.text.trim();
        final int port = int.tryParse(_imapPort.text.trim()) ??
            (_imapSecurity == 'tls' ? 993 : 143);
        final String enc = Uri.encodeComponent(user);
        if (_imapSecurity == 'tls') {
          return 'imaps://$enc@$h:$port';
        }
        final String q = _imapSecurity == 'plain' ? '?security=plain' : '';
        return 'imap://$enc@$h:$port$q';
      }
      case 'Gmail':
        return 'gmail://$user';
      case 'Exchange':
        return 'graph://$user';
      case 'POP3': {
        final int p = int.tryParse(_imapPort.text.trim()) ?? 995;
        return 'pop3s://${Uri.encodeComponent(user)}@${_imapHost.text.trim()}:$p';
      }
      case 'NNTP': {
        final int p = int.tryParse(_imapPort.text.trim()) ?? 563;
        return 'nntps://${Uri.encodeComponent(user)}@${_imapHost.text.trim()}:$p';
      }
      case 'Nostr':
        return 'nostr:${_nostrNpub.trim()}';
      case 'Matrix':
        return 'matrix:store:${_homeserver.text.trim()}:$user';
      default:
        return 'store://$user';
    }
  }

}

String _hostFromPop3Uri(String uri) {
  final Uri? u = Uri.tryParse(uri);
  if (u == null || (u.scheme != 'pop3' && u.scheme != 'pop3s')) {
    return 'pop.example.com';
  }
  return u.host;
}

int? _portFromPop3Uri(String uri) {
  final Uri? u = Uri.tryParse(uri);
  if (u == null || (u.scheme != 'pop3' && u.scheme != 'pop3s')) {
    return null;
  }
  return u.hasPort ? u.port : null;
}

String _hostFromNntpUri(String uri) {
  final Uri? u = Uri.tryParse(uri);
  if (u == null || (u.scheme != 'nntp' && u.scheme != 'nntps')) {
    return 'news.example.com';
  }
  return u.host;
}

int? _portFromNntpUri(String uri) {
  final Uri? u = Uri.tryParse(uri);
  if (u == null || (u.scheme != 'nntp' && u.scheme != 'nntps')) {
    return null;
  }
  return u.hasPort ? u.port : null;
}

String _homeserverFromMatrixStore(String uri) {
  // matrix:store:https://matrix.org:user
  final int i = uri.indexOf('matrix:store:');
  if (i < 0) {
    return 'https://matrix.org';
  }
  final String rest = uri.substring(i + 'matrix:store:'.length);
  final int colon = rest.lastIndexOf(':');
  if (colon <= 0) {
    return rest;
  }
  return rest.substring(0, colon);
}

List<String> _relayUrlsFromLegacyNostrUri(String uri) {
  final Uri? u = Uri.tryParse(uri);
  if (u == null) {
    return <String>['wss://relay.damus.io', 'wss://nos.lol'];
  }
  final String? r = u.queryParameters['relays'];
  if (r == null || r.isEmpty) {
    return <String>['wss://relay.damus.io', 'wss://nos.lol'];
  }
  return r
      .split(RegExp(r'[,;\s]+'))
      .map((String s) => s.trim())
      .where((String s) => s.isNotEmpty)
      .toList();
}
