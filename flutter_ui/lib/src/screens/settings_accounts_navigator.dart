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
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../util/account_save_credential_prompt.dart';
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

/// Rust / `config.xml` use lowercase (`imap`, `gmail`). Settings UI and [_AccountDetailPageState._save]
/// use [kAccountBackendChoices] ids (`IMAP`, `Gmail`, …).
String _settingsUiBackendType(String raw) {
  final String t = raw.trim();
  if (t.isEmpty) {
    return t;
  }
  switch (t.toLowerCase()) {
    case 'imap':
    case 'imaps':
      return 'IMAP';
    case 'gmail':
      return 'Gmail';
    case 'exchange':
    case 'graph':
      return 'Exchange';
    case 'pop3':
      return 'POP3';
    case 'maildir':
      return 'Maildir';
    case 'mbox':
      return 'mbox';
    case 'nntp':
    case 'nntps':
      return 'NNTP';
    case 'nostr':
      return 'Nostr';
    case 'matrix':
      return 'Matrix';
    default:
      return t;
  }
}

/// Legacy configs may store a short Matrix username plus homeserver URL; prefer a canonical MXID.
String _matrixMxidDisplayFromAccount(AppAccount? e) {
  if (e == null || _settingsUiBackendType(e.backendType) != 'Matrix') {
    return '';
  }
  final String u = (e.attrs['username'] ?? e.attrs['email'] ?? '').trim();
  if (u.startsWith('@') && u.contains(':')) {
    return u;
  }
  final String hs = (e.attrs['homeserver'] ?? e.attrs['host'] ?? '').trim();
  if (u.isEmpty) {
    return '';
  }
  final Uri? uri = Uri.tryParse(
    hs.isEmpty ? '' : (hs.contains('://') ? hs : 'https://$hs'),
  );
  if (uri != null && uri.host.isNotEmpty) {
    return '@$u:${uri.host}';
  }
  return u;
}

/// Parses `@localpart:server` into a base homeserver URL and full user id.
({String homeserverUrl, String userId})? _parseMatrixMxid(String raw) {
  final String t = raw.trim();
  final RegExp re = RegExp(r'^@([^:@]+):(.+)$');
  final Match? m = re.firstMatch(t);
  if (m == null) {
    return null;
  }
  final String domain = m.group(2)!.trim();
  if (domain.isEmpty) {
    return null;
  }
  final String homeserverUrl =
      domain.contains('://') ? domain : 'https://$domain';
  return (homeserverUrl: homeserverUrl, userId: t);
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

class _AccountDetailPage extends ConsumerStatefulWidget {
  const _AccountDetailPage({
    required this.api,
    required this.args,
    required this.onConfigReplaced,
  });

  final TagliacarteApi api;
  final AccountDetailRouteArgs args;
  final ValueChanged<AppSettingsConfig> onConfigReplaced;

  @override
  ConsumerState<_AccountDetailPage> createState() => _AccountDetailPageState();
}

class _AccountDetailPageState extends ConsumerState<_AccountDetailPage> {
  late String _backendType;
  late final TextEditingController _accountName;
  late final TextEditingController _imapHost;
  late final TextEditingController _imapPort;
  late final TextEditingController _imapMinIdleSeconds;
  late final TextEditingController _imapTrashFolder;
  late final TextEditingController _imapJunkFolder;
  late final TextEditingController _imapSentFolder;
  late final TextEditingController _imapDraftsFolder;
  late final TextEditingController _imapMirrorSentIfMissing;
  late final TextEditingController _draftAutosaveSeconds;
  late final TextEditingController _gmailEmail;
  late final TextEditingController _gmailSentLabel;
  late final TextEditingController _gmailDraftLabel;
  late final TextEditingController _gmailInboxLabel;
  late final TextEditingController _maildirTrashFolder;
  late final TextEditingController _maildirJunkFolder;
  late final TextEditingController _username;
  late final TextEditingController _nntpDefaultFrom;
  String _imapSecurity = 'tls';
  String _oauthProvider = '';
  late final TextEditingController _avatarUrl;
  late final TextEditingController _nip05;
  /// One [TextEditingController] per relay row (Nostr only).
  List<TextEditingController> _nostrRelayControllers = <TextEditingController>[];
  late final TextEditingController _nostrNewRelayRow;
  late final TextEditingController _localStorePath;
  late List<String> _orderedTransportIds;
  String _nostrNpub = '';
  /// Stored values: `Move to Trash` / `Mark Deleted` (same as persisted `imapDeleteMode`).
  String _imapDeleteMode = 'Move to Trash';
  /// Maildir: `Move to Trash` / `Delete immediately` (`maildirDeleteMode`).
  String _maildirDeleteMode = 'Move to Trash';
  /// When creating a new account, set when user creates a Nostr identity so credential id matches save.
  String? _provisionalAccountId;
  bool _isSaving = false;
  String _snapshot = '';

  @override
  void initState() {
    super.initState();
    final AppAccount? e = widget.args.existing;
    _backendType = _settingsUiBackendType(widget.args.backendType);
    _accountName = TextEditingController(text: e?.label ?? '');

    String imapHostText = 'imap.example.com';
    String portText = '993';
    _imapSecurity = 'tls';

    if (e != null && _backendType == 'IMAP') {
      final String? h = e.attrs['host'];
      if (h != null && h.isNotEmpty) {
        imapHostText = h;
        portText = e.attrs['port'] ??
            '${(e.attrs['security'] == 'starttls' || e.attrs['security'] == 'plain' ? 143 : 993)}';
        _imapSecurity = e.attrs['security'] ?? 'tls';
      }
    } else if (e != null && _backendType == 'POP3') {
      final String? h = e.attrs['host'];
      if (h != null && h.isNotEmpty) {
        imapHostText = h;
        portText = e.attrs['port'] ?? '995';
        _imapSecurity = e.attrs['security'] ?? 'tls';
      }
    } else if (e != null && _backendType == 'NNTP') {
      final String? h = e.attrs['host'];
      if (h != null && h.isNotEmpty) {
        imapHostText = h;
        portText = e.attrs['port'] ?? '563';
        _imapSecurity = e.attrs['security'] ?? 'tls';
      }
    } else {
      switch (_backendType) {
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
          if (_showsTcpMailServerFields(_backendType)) {
            imapHostText = _backendType == 'NNTP'
                ? 'news.example.com'
                : _backendType == 'POP3'
                    ? 'pop.example.com'
                    : 'imap.example.com';
            portText = _backendType == 'NNTP'
                ? '563'
                : _backendType == 'POP3'
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
              (_backendType == 'IMAP' || _backendType == 'Gmail') &&
              (e.attrs['imapIdleMinIdleSeconds'] ?? '').isNotEmpty
          ? e.attrs['imapIdleMinIdleSeconds']!
          : '',
    );
    _imapTrashFolder = TextEditingController(
      text: e != null &&
              (_backendType == 'IMAP' ||
                  _backendType == 'Gmail' ||
                  _backendType == 'Exchange')
          ? (_backendType == 'Gmail'
              ? (e.attrs['gmailTrashLabelId'] ?? '')
              : (e.attrs['imapTrashFolderName'] ?? ''))
          : '',
    );
    _imapJunkFolder = TextEditingController(
      text: e != null &&
              (_backendType == 'IMAP' ||
                  _backendType == 'Gmail' ||
                  _backendType == 'Exchange')
          ? (_backendType == 'Gmail'
              ? (e.attrs['gmailSpamLabelId'] ?? '')
              : (e.attrs['imapJunkFolderName'] ?? ''))
          : '',
    );
    _imapSentFolder = TextEditingController(
      text: e != null && _backendType == 'IMAP'
          ? (e.attrs['imapSentFolderName'] ?? '')
          : '',
    );
    _imapDraftsFolder = TextEditingController(
      text: e != null && _backendType == 'IMAP'
          ? (e.attrs['imapDraftsFolderName'] ?? '')
          : '',
    );
    _imapMirrorSentIfMissing = TextEditingController(
      text: e != null && _backendType == 'IMAP'
          ? (e.attrs['imapMirrorSentIfMissing'] ?? '')
          : '',
    );
    _draftAutosaveSeconds = TextEditingController(
      text: e != null && _backendType == 'IMAP'
          ? (e.attrs['draftAutosaveSeconds'] ?? '')
          : '',
    );
    _gmailEmail = TextEditingController(
      text: e != null && _backendType == 'Gmail' ? (e.attrs['email'] ?? '') : '',
    );
    _gmailSentLabel = TextEditingController(
      text: e != null && _backendType == 'Gmail'
          ? (e.attrs['gmailSentLabelId'] ?? 'SENT')
          : 'SENT',
    );
    _gmailDraftLabel = TextEditingController(
      text: e != null && _backendType == 'Gmail'
          ? (e.attrs['gmailDraftLabelId'] ?? 'DRAFT')
          : 'DRAFT',
    );
    _gmailInboxLabel = TextEditingController(
      text: e != null && _backendType == 'Gmail'
          ? (e.attrs['gmailInboxLabelId'] ?? 'INBOX')
          : 'INBOX',
    );
    _maildirTrashFolder = TextEditingController(
      text: e != null && _backendType == 'Maildir'
          ? (e.attrs['maildirTrashFolderName'] ?? '')
          : '',
    );
    _maildirJunkFolder = TextEditingController(
      text: e != null && _backendType == 'Maildir'
          ? (e.attrs['maildirJunkFolderName'] ?? '')
          : '',
    );
    if (e != null && _backendType == 'IMAP') {
      final String? dm = e.attrs['imapDeleteMode'];
      if (dm == 'Mark Deleted' || dm == 'Move to Trash') {
        _imapDeleteMode = dm!;
      }
    }
    if (_backendType == 'IMAP' || _backendType == 'Gmail') {
      final String configured = (e?.attrs['oauthProvider'] ?? '').trim().toLowerCase();
      if (configured == 'google' || configured == 'microsoft') {
        _oauthProvider = configured;
      } else if (_backendType == 'Gmail') {
        _oauthProvider = 'google';
      } else {
        _oauthProvider = '';
      }
    } else {
      _oauthProvider = '';
    }
    if (e != null && _backendType == 'Maildir') {
      final String? mdm = e.attrs['maildirDeleteMode'];
      if (mdm == 'Delete immediately' || mdm == 'Move to Trash') {
        _maildirDeleteMode = mdm!;
      }
    }
    _username = TextEditingController(
      text: e != null && _backendType == 'Matrix'
          ? _matrixMxidDisplayFromAccount(e)
          : '',
    );
    _nntpDefaultFrom = TextEditingController(
      text: e != null && _backendType == 'NNTP'
          ? (e.attrs['defaultFrom'] ?? '')
          : '',
    );
    _avatarUrl = TextEditingController(text: e?.avatarUrl ?? '');
    _nip05 = TextEditingController(text: e?.attrs['nip05'] ?? '');
    _nostrNpub = e?.attrs['npub'] ?? '';
    _nostrNewRelayRow = TextEditingController()..addListener(_onFieldChanged);
    _initNostrRelayControllers(e, _backendType);
    _localStorePath = TextEditingController(
      text: e != null && _isLocalMailBackend(e.backendType)
          ? (e.attrs['path'] ?? '')
          : '',
    );
    _orderedTransportIds = List<String>.from(e?.transportIds ?? const <String>[]);
    for (final TextEditingController c in <TextEditingController>[
      _accountName,
      _imapHost,
      _imapPort,
      _imapMinIdleSeconds,
      _imapTrashFolder,
      _imapJunkFolder,
      _imapSentFolder,
      _imapDraftsFolder,
      _imapMirrorSentIfMissing,
      _draftAutosaveSeconds,
      _gmailEmail,
      _maildirTrashFolder,
      _maildirJunkFolder,
      _gmailSentLabel,
      _gmailDraftLabel,
      _gmailInboxLabel,
      _username,
      _nntpDefaultFrom,
      _avatarUrl,
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
      _imapDeleteMode,
      _imapTrashFolder.text,
      _imapJunkFolder.text,
      _imapSentFolder.text,
      _imapDraftsFolder.text,
      _imapMirrorSentIfMissing.text,
      _draftAutosaveSeconds.text,
      _gmailEmail.text,
      _oauthProvider,
      _maildirDeleteMode,
      _maildirTrashFolder.text,
      _maildirJunkFolder.text,
      _gmailSentLabel.text,
      _gmailDraftLabel.text,
      _gmailInboxLabel.text,
      _backendType == 'Matrix' ? _username.text : '',
      _backendType == 'NNTP' ? _nntpDefaultFrom.text : '',
      _avatarUrl.text,
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
      _imapTrashFolder,
      _imapJunkFolder,
      _imapSentFolder,
      _imapDraftsFolder,
      _imapMirrorSentIfMissing,
      _draftAutosaveSeconds,
      _gmailEmail,
      _maildirTrashFolder,
      _maildirJunkFolder,
      _gmailSentLabel,
      _gmailDraftLabel,
      _gmailInboxLabel,
      _username,
      _nntpDefaultFrom,
      _avatarUrl,
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
    if (label.isEmpty) {
      _toast(l10n.validationAccountNameRequired);
      return;
    }
    if (_isLocalMailBackend(_backendType)) {
      if (_localStorePath.text.trim().isEmpty) {
        _toast(l10n.validationLocalPathRequired);
        return;
      }
    } else if (_backendType == 'Matrix') {
      final String mxid = _username.text.trim();
      if (mxid.isEmpty) {
        _toast(l10n.validationMatrixUserIdRequired);
        return;
      }
      if (_parseMatrixMxid(mxid) == null) {
        _toast(l10n.validationMatrixMxidInvalid);
        return;
      }
    }
    if (_backendType == 'Nostr') {
      final List<String> ru = _effectiveNostrRelayUrls();
      if (ru.isEmpty) {
        _toast(l10n.nostrRelayUrlsRequired);
        return;
      }
    }
    if (_backendType == 'Gmail' && _gmailEmail.text.trim().isEmpty) {
      _toast('Gmail address is required');
      return;
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

      if (_backendType == 'IMAP') {
        attrs['host'] = _imapHost.text.trim();
        attrs['port'] = _imapPort.text.trim();
        attrs['security'] = _imapSecurity;
        attrs.remove('username');
        attrs.remove('email');
        if (imapIdleMinIdleSeconds != null) {
          attrs['imapIdleMinIdleSeconds'] = '$imapIdleMinIdleSeconds';
        } else {
          attrs.remove('imapIdleMinIdleSeconds');
        }
        attrs['imapDeleteMode'] = _imapDeleteMode;
        if (_imapDeleteMode == 'Move to Trash') {
          final String t = _imapTrashFolder.text.trim();
          if (t.isNotEmpty) {
            attrs['imapTrashFolderName'] = t;
          } else {
            attrs.remove('imapTrashFolderName');
          }
        } else {
          attrs.remove('imapTrashFolderName');
        }
        final String junkImap = _imapJunkFolder.text.trim();
        if (junkImap.isNotEmpty) {
          attrs['imapJunkFolderName'] = junkImap;
        } else {
          attrs.remove('imapJunkFolderName');
        }
        final String sentF = _imapSentFolder.text.trim();
        if (sentF.isNotEmpty) {
          attrs['imapSentFolderName'] = sentF;
        } else {
          attrs.remove('imapSentFolderName');
        }
        final String draftsF = _imapDraftsFolder.text.trim();
        if (draftsF.isNotEmpty) {
          attrs['imapDraftsFolderName'] = draftsF;
        } else {
          attrs.remove('imapDraftsFolderName');
        }
        final String mir = _imapMirrorSentIfMissing.text.trim();
        if (mir.isNotEmpty) {
          attrs['imapMirrorSentIfMissing'] = mir;
        } else {
          attrs.remove('imapMirrorSentIfMissing');
        }
        final String das = _draftAutosaveSeconds.text.trim();
        if (das.isNotEmpty) {
          attrs['draftAutosaveSeconds'] = das;
        } else {
          attrs.remove('draftAutosaveSeconds');
        }
        if (_oauthProvider.isNotEmpty) {
          attrs['oauthProvider'] = _oauthProvider;
        } else {
          attrs.remove('oauthProvider');
        }
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      } else if (_backendType == 'POP3') {
        attrs['host'] = _imapHost.text.trim();
        attrs['port'] = _imapPort.text.trim();
        attrs['security'] = _imapSecurity;
        attrs.remove('username');
        attrs.remove('email');
        attrs.remove('oauthProvider');
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      } else if (_backendType == 'NNTP') {
        attrs['host'] = _imapHost.text.trim();
        attrs['port'] = _imapPort.text.trim();
        attrs['security'] = _imapSecurity;
        attrs.remove('username');
        attrs.remove('email');
        attrs.remove('oauthProvider');
        final String df = _nntpDefaultFrom.text.trim();
        if (df.isEmpty) {
          attrs.remove('defaultFrom');
        } else {
          attrs['defaultFrom'] = df;
        }
        lists.remove('transportIds');
      } else if (_backendType == 'Gmail') {
        attrs.remove('username');
        attrs.remove('host');
        attrs.remove('port');
        attrs.remove('security');
        attrs.remove('imapIdleMinIdleSeconds');
        attrs.remove('imapDeleteMode');
        attrs.remove('imapTrashFolderName');
        attrs.remove('imapJunkFolderName');
        attrs.remove('imapSentFolderName');
        attrs.remove('imapDraftsFolderName');
        attrs.remove('imapMirrorSentIfMissing');
        attrs.remove('draftAutosaveSeconds');
        attrs['email'] = _gmailEmail.text.trim();
        attrs['gmailTrashLabelId'] =
            (_imapTrashFolder.text.trim().isEmpty ? 'TRASH' : _imapTrashFolder.text.trim());
        attrs['gmailSpamLabelId'] =
            (_imapJunkFolder.text.trim().isEmpty ? 'SPAM' : _imapJunkFolder.text.trim());
        attrs['gmailSentLabelId'] =
            (_gmailSentLabel.text.trim().isEmpty ? 'SENT' : _gmailSentLabel.text.trim());
        attrs['gmailDraftLabelId'] =
            (_gmailDraftLabel.text.trim().isEmpty ? 'DRAFT' : _gmailDraftLabel.text.trim());
        attrs['gmailInboxLabelId'] =
            (_gmailInboxLabel.text.trim().isEmpty ? 'INBOX' : _gmailInboxLabel.text.trim());
        attrs['oauthProvider'] = 'google';
        lists.remove('transportIds');
      } else if (_backendType == 'Exchange') {
        attrs.remove('email');
        attrs.remove('username');
        final String t = _imapTrashFolder.text.trim();
        if (t.isNotEmpty) {
          attrs['imapTrashFolderName'] = t;
        } else {
          attrs.remove('imapTrashFolderName');
        }
        final String junkEx = _imapJunkFolder.text.trim();
        if (junkEx.isNotEmpty) {
          attrs['imapJunkFolderName'] = junkEx;
        } else {
          attrs.remove('imapJunkFolderName');
        }
        attrs['oauthProvider'] = 'microsoft';
        lists.remove('transportIds');
      } else if (_backendType == 'Maildir') {
        attrs['path'] =
            _pathWithLeadingSlashForLocalStore(_localStorePath.text.trim());
        attrs.remove('username');
        attrs.remove('email');
        attrs.remove('oauthProvider');
        attrs['maildirDeleteMode'] = _maildirDeleteMode;
        if (_maildirDeleteMode == 'Move to Trash') {
          final String t = _maildirTrashFolder.text.trim();
          if (t.isNotEmpty) {
            attrs['maildirTrashFolderName'] = t;
          } else {
            attrs.remove('maildirTrashFolderName');
          }
        } else {
          attrs.remove('maildirTrashFolderName');
        }
        final String j = _maildirJunkFolder.text.trim();
        if (j.isNotEmpty) {
          attrs['maildirJunkFolderName'] = j;
        } else {
          attrs.remove('maildirJunkFolderName');
        }
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      } else if (_backendType == 'mbox') {
        attrs['path'] =
            _pathWithLeadingSlashForLocalStore(_localStorePath.text.trim());
        attrs.remove('username');
        attrs.remove('email');
        attrs.remove('oauthProvider');
        attrs.remove('maildirDeleteMode');
        attrs.remove('maildirTrashFolderName');
        attrs.remove('maildirJunkFolderName');
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
        attrs.remove('oauthProvider');
      } else if (_backendType == 'Matrix') {
        final ({String homeserverUrl, String userId}) parsed =
            _parseMatrixMxid(_username.text.trim())!;
        attrs['username'] = parsed.userId;
        attrs['homeserver'] = parsed.homeserverUrl;
        attrs['host'] = parsed.homeserverUrl;
        attrs.remove('email');
        lists.remove('transportIds');
      } else {
        lists['transportIds'] = List<String>.from(_orderedTransportIds);
      }

      final AppAccount account = AppAccount(
        id: id,
        label: label,
        backendType: _backendType,
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
      await promptMailboxCredentialsIfNeededAfterSave(ref, context, account);
      if (!mounted) {
        return;
      }
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
                  InputDecorator(
                    decoration: InputDecoration(
                      labelText: l10n.accountTypeLabel,
                      helperText: l10n.accountTypeHelper,
                    ),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      child: Text(
                        _backendLabel(_backendType),
                        style: Theme.of(context).textTheme.bodyLarge,
                      ),
                    ),
                  ),
                  TextField(
                    controller: _accountName,
                    decoration: InputDecoration(
                      labelText: l10n.accountNameLabel,
                    ),
                  ),
                  TextField(
                    controller: _avatarUrl,
                    decoration: InputDecoration(
                      labelText: l10n.avatarUrlLabel,
                      helperText: l10n.avatarUrlHelper,
                    ),
                  ),
                  if (_backendType == 'Matrix')
                    TextField(
                      controller: _username,
                      decoration: InputDecoration(
                        labelText: l10n.accountMatrixUserIdLabel,
                        helperText: l10n.accountMatrixMxidHelper,
                      ),
                    ),
                  if (_backendType == 'NNTP')
                    TextField(
                      controller: _nntpDefaultFrom,
                      decoration: InputDecoration(
                        labelText: l10n.accountNntpDefaultFromLabel,
                        helperText: l10n.accountNntpDefaultFromHelper,
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
                    if (_backendType == 'Maildir') ...<Widget>[
                      const SizedBox(height: 12),
                      DropdownButtonFormField<String>(
                        key: ValueKey<String>(_maildirDeleteMode),
                        initialValue: _maildirDeleteMode,
                        decoration: InputDecoration(
                          labelText: l10n.deleteModeLabel,
                        ),
                        items: <DropdownMenuItem<String>>[
                          DropdownMenuItem(
                            value: 'Move to Trash',
                            child: Text(l10n.deleteModeMoveToTrash),
                          ),
                          DropdownMenuItem(
                            value: 'Delete immediately',
                            child: Text(l10n.deleteModeDeleteImmediately),
                          ),
                        ],
                        onChanged: (String? value) {
                          if (value != null) {
                            setState(() => _maildirDeleteMode = value);
                          }
                        },
                      ),
                      if (_maildirDeleteMode == 'Move to Trash')
                        TextField(
                          controller: _maildirTrashFolder,
                          decoration: InputDecoration(
                            labelText: l10n.trashFolderNameLabel,
                          ),
                        ),
                      TextField(
                        controller: _maildirJunkFolder,
                        decoration: InputDecoration(
                          labelText: l10n.junkFolderNameLabel,
                        ),
                      ),
                    ],
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
                    if (_backendType == 'IMAP' ||
                        _backendType == 'POP3' ||
                        _backendType == 'NNTP')
                      DropdownButtonFormField<String>(
                        key: ValueKey<String>(_imapSecurity),
                        initialValue: _imapSecurity,
                        decoration: InputDecoration(
                          labelText: l10n.securityLabel,
                        ),
                        items: <DropdownMenuItem<String>>[
                          DropdownMenuItem(
                            value: 'tls',
                            child: Text(
                              _backendType == 'POP3'
                                  ? l10n.mailSecurityImplicitTlsPop3
                                  : _backendType == 'NNTP'
                                      ? l10n.mailSecurityImplicitTlsNntp
                                      : l10n.mailSecurityImplicitTlsImap,
                            ),
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
                  ],
                  if (_backendType == 'IMAP') ...<Widget>[
                    const SizedBox(height: 12),
                    DropdownButtonFormField<String>(
                      key: ValueKey<String>(_oauthProvider),
                      initialValue: _oauthProvider,
                      decoration: const InputDecoration(
                        labelText: 'OAuth provider',
                        helperText: 'Optional: enables XOAUTH2 for this IMAP account',
                      ),
                      items: const <DropdownMenuItem<String>>[
                        DropdownMenuItem(
                          value: '',
                          child: Text('None (username/password)'),
                        ),
                        DropdownMenuItem(
                          value: 'google',
                          child: Text('Google'),
                        ),
                        DropdownMenuItem(
                          value: 'microsoft',
                          child: Text('Microsoft'),
                        ),
                      ],
                      onChanged: (String? value) {
                        if (value != null) {
                          setState(() => _oauthProvider = value);
                        }
                      },
                    ),
                    TextField(
                      controller: _imapMinIdleSeconds,
                      keyboardType: TextInputType.number,
                      decoration: InputDecoration(
                        labelText: l10n.accountImapMinIdleSecondsLabel,
                        helperText: l10n.accountImapMinIdleSecondsHelper,
                      ),
                    ),
                    DropdownButtonFormField<String>(
                      key: ValueKey<String>(_imapDeleteMode),
                      initialValue: _imapDeleteMode,
                      decoration: InputDecoration(
                        labelText: l10n.deleteModeLabel,
                      ),
                      items: <DropdownMenuItem<String>>[
                        DropdownMenuItem(
                          value: 'Move to Trash',
                          child: Text(l10n.deleteModeMoveToTrash),
                        ),
                        DropdownMenuItem(
                          value: 'Mark Deleted',
                          child: Text(l10n.deleteModeMarkDeleted),
                        ),
                      ],
                      onChanged: (String? value) {
                        if (value != null) {
                          setState(() => _imapDeleteMode = value);
                        }
                      },
                    ),
                    if (_imapDeleteMode == 'Move to Trash')
                      TextField(
                        controller: _imapTrashFolder,
                        decoration: InputDecoration(
                          labelText: l10n.trashFolderNameLabel,
                        ),
                      ),
                    TextField(
                      controller: _imapJunkFolder,
                      decoration: InputDecoration(
                        labelText: l10n.junkFolderNameLabel,
                      ),
                    ),
                    TextField(
                      controller: _imapSentFolder,
                      decoration: const InputDecoration(
                        labelText: 'Sent folder name',
                        helperText:
                            'Optional. Used to verify/append Sent copy after SMTP (LIST \\Sent if empty)',
                      ),
                    ),
                    TextField(
                      controller: _imapDraftsFolder,
                      decoration: const InputDecoration(
                        labelText: 'Drafts folder name',
                        helperText:
                            'Optional. Autosave target; LIST \\Drafts if empty',
                      ),
                    ),
                    TextField(
                      controller: _imapMirrorSentIfMissing,
                      decoration: const InputDecoration(
                        labelText: 'Mirror sent if missing',
                        helperText:
                            'Leave empty for yes. Set to 0/false/off to skip IMAP Sent check/append',
                      ),
                    ),
                    TextField(
                      controller: _draftAutosaveSeconds,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Draft autosave interval (seconds)',
                        helperText: '0 or empty = no periodic IMAP draft save',
                      ),
                    ),
                  ],
                  if (_backendType == 'Gmail') ...<Widget>[
                    const SizedBox(height: 12),
                    TextField(
                      controller: _gmailEmail,
                      decoration: const InputDecoration(
                        labelText: 'Gmail address',
                        helperText: 'Example: username@gmail.com',
                      ),
                    ),
                    TextField(
                      controller: _imapTrashFolder,
                      decoration: const InputDecoration(
                        labelText: 'Trash label ID',
                        helperText: 'Default: TRASH',
                      ),
                    ),
                    TextField(
                      controller: _imapJunkFolder,
                      decoration: const InputDecoration(
                        labelText: 'Spam label ID',
                        helperText: 'Default: SPAM',
                      ),
                    ),
                    TextField(
                      controller: _gmailSentLabel,
                      decoration: const InputDecoration(
                        labelText: 'Sent label ID',
                        helperText: 'Default: SENT',
                      ),
                    ),
                    TextField(
                      controller: _gmailDraftLabel,
                      decoration: const InputDecoration(
                        labelText: 'Draft label ID',
                        helperText: 'Default: DRAFT',
                      ),
                    ),
                    TextField(
                      controller: _gmailInboxLabel,
                      decoration: const InputDecoration(
                        labelText: 'Inbox label ID',
                        helperText: 'Default: INBOX',
                      ),
                    ),
                  ],
                  if (_backendType == 'Exchange') ...<Widget>[
                    const SizedBox(height: 12),
                    TextField(
                      controller: _imapTrashFolder,
                      decoration: InputDecoration(
                        labelText: l10n.trashFolderNameLabel,
                        helperText: l10n.exchangeTrashFolderHelper,
                      ),
                    ),
                    TextField(
                      controller: _imapJunkFolder,
                      decoration: InputDecoration(
                        labelText: l10n.junkFolderNameLabel,
                        helperText: l10n.exchangeJunkFolderHelper,
                      ),
                    ),
                  ],
                  if (backendTypeRequiresOutboundTransport(_backendType) &&
                      !backendTypeUsesEmbeddedTransport(
                          _backendType)) ...[
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
                      l10n.storeUriLabel(widget.args.existing!.connectionSummary),
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
    final String bt = _settingsUiBackendType(backendType);
    final String? ebt =
        e != null ? _settingsUiBackendType(e.backendType) : null;
    if (ebt != 'Nostr' && bt != 'Nostr') {
      return;
    }
    List<String> urls;
    if (e != null && ebt == 'Nostr') {
      urls = e.relayUrls.isNotEmpty
          ? List<String>.from(e.relayUrls)
          : <String>['wss://relay.damus.io', 'wss://nos.lol'];
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

}

