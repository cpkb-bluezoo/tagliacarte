/*
 * settings_screen.dart
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

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../l10n/app_localizations.dart';
import '../providers/app_state.dart';
import '../theme/app_assets.dart';
import '../providers/view_prefs.dart';
import '../rust/tagliacarte_api.dart';
import 'settings_accounts_navigator.dart';
import 'settings_transports_navigator.dart';

class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key, this.initialTabIndex = 0});

  /// Tab index: 0 Accounts, 1 Outgoing, 2 Security, …
  final int initialTabIndex;

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  static const TagliacarteApi _api = TagliacarteApi();

  AppSettingsConfig _config = AppSettingsConfig.defaults();
  final TextEditingController _trashFolder = TextEditingController(text: 'Trash');
  bool _useKeychain = true;
  bool _loadRemoteImages = false;
  bool _threadedView = true;
  bool _quoteOriginal = true;
  String _deleteMode = 'Move to Trash';

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final AppSettingsConfig config = await _api.loadConfig();
    if (!mounted) {
      return;
    }
    setState(() {
      _config = config;
      _useKeychain = config.useKeychain;
      _loadRemoteImages = config.loadRemoteImages;
      _threadedView = config.threadedView;
      _quoteOriginal = config.quoteOriginal;
      _deleteMode = config.deleteMode;
      _trashFolder.text = config.trashFolderName;
    });
  }

  @override
  void dispose() {
    _trashFolder.dispose();
    super.dispose();
  }

  void _replaceConfig(AppSettingsConfig next) {
    setState(() => _config = next);
    ref.read(settingsRevisionProvider.notifier).state++;
  }

  /// Persists global preferences (not account CRUD — that happens in account detail).
  Future<void> _persistAppPreferences() async {
    final AppSettingsConfig config = _config.copyWith(
      useKeychain: _useKeychain,
      loadRemoteImages: _loadRemoteImages,
      threadedView: _threadedView,
      quoteOriginal: _quoteOriginal,
      deleteMode: _deleteMode,
      trashFolderName: _trashFolder.text.trim().isEmpty
          ? 'Trash'
          : _trashFolder.text.trim(),
      resourcePolicy: _loadRemoteImages ? 'allow-remote' : 'block-remote',
    );
    await _api.saveConfig(config);
    if (!mounted) {
      return;
    }
    setState(() => _config = config);
    ref.read(settingsRevisionProvider.notifier).state++;
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final int tabIndex = widget.initialTabIndex.clamp(0, 5);
    return DefaultTabController(
      length: 6,
      initialIndex: tabIndex,
      child: Scaffold(
        appBar: AppBar(
          leading: IconButton(
            icon: const Icon(Icons.arrow_back),
            tooltip: l10n.back,
            onPressed: () {
              final NavigatorState nav = Navigator.of(context);
              if (nav.canPop()) {
                nav.pop();
              } else {
                nav.pushReplacementNamed('/');
              }
            },
          ),
          title: Text(l10n.settings),
          bottom: TabBar(
            tabs: [
              Tab(text: l10n.settingsTabAccounts),
              Tab(text: l10n.settingsTabOutgoing),
              Tab(text: l10n.settingsTabSecurity),
              Tab(text: l10n.settingsTabViewing),
              Tab(text: l10n.settingsTabComposing),
              Tab(text: l10n.settingsTabAbout),
            ],
          ),
        ),
        body: SettingsAccountsConfigScope(
          config: _config,
          child: TabBarView(
            children: [
              AccountsSettingsNavigator(
                api: _api,
                onConfigReplaced: _replaceConfig,
              ),
              OutgoingSettingsNavigator(
                api: _api,
                onConfigReplaced: _replaceConfig,
              ),
              _securityPane(),
              _viewingPane(),
              _composingPane(),
              _aboutPane(),
            ],
          ),
        ),
      ),
    );
  }

  Widget _securityPane() {
    final AppLocalizations l10n = AppLocalizations.of(context);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        SwitchListTile(
          value: _useKeychain,
          title: Text(l10n.useSystemKeychain),
          subtitle: Text(l10n.storeCredentialsInKeychain),
          onChanged: (bool value) async {
            setState(() {
              _useKeychain = value;
              _config = _config.copyWith(useKeychain: value);
            });
            await _persistAppPreferences();
          },
        ),
        const SizedBox(height: 8),
        Text(l10n.oauthSection, style: Theme.of(context).textTheme.titleSmall),
        Wrap(
          spacing: 8,
          children: [
            ElevatedButton(
              onPressed: () => _showStubCall(l10n.authenticateGoogle),
              child: Text(l10n.authenticateGoogle),
            ),
            ElevatedButton(
              onPressed: () => _showStubCall(l10n.authenticateMicrosoft),
              child: Text(l10n.authenticateMicrosoft),
            ),
            OutlinedButton(
              onPressed: () => _showStubCall(l10n.reloadOAuthToken),
              child: Text(l10n.reloadOAuthToken),
            ),
          ],
        ),
        const Divider(height: 24),
        Text(
          l10n.matrixE2eeSection,
          style: Theme.of(context).textTheme.titleSmall,
        ),
        Wrap(
          spacing: 8,
          children: [
            ElevatedButton(
              onPressed: () => _showStubCall(l10n.initCrypto),
              child: Text(l10n.initCrypto),
            ),
            ElevatedButton(
              onPressed: () => _showStubCall(l10n.setupBackup),
              child: Text(l10n.setupBackup),
            ),
            OutlinedButton(
              onPressed: () => _showStubCall(l10n.restoreBackup),
              child: Text(l10n.restoreBackup),
            ),
            OutlinedButton(
              onPressed: () => _showStubCall(l10n.showDeviceFingerprint),
              child: Text(l10n.showDeviceFingerprint),
            ),
          ],
        ),
      ],
    );
  }

  Widget _viewingPane() {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool messageDetailInline = ref.watch(messageDetailInlineProvider);
    final bool headersMinimal = ref.watch(messageHeadersMinimalProvider);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        SwitchListTile(
          value: messageDetailInline,
          title: Text(l10n.messageDetailInlineDesktopTitle),
          subtitle: Text(l10n.messageDetailInlineDesktopSubtitle),
          onChanged: (bool value) {
            ref.read(messageDetailInlineProvider.notifier).setInline(value);
          },
        ),
        SwitchListTile(
          value: headersMinimal,
          title: Text(l10n.settingsViewMinimalHeaders),
          subtitle: Text(l10n.settingsViewMinimalHeadersSubtitle),
          onChanged: (bool value) {
            ref
                .read(messageHeadersMinimalProvider.notifier)
                .setMinimal(value);
          },
        ),
        SwitchListTile(
          value: _loadRemoteImages,
          title: Text(l10n.loadRemoteImages),
          subtitle: Text(l10n.loadRemoteImagesSubtitle),
          onChanged: (bool value) async {
            setState(() {
              _loadRemoteImages = value;
              _config = _config.copyWith(loadRemoteImages: value);
            });
            await _persistAppPreferences();
          },
        ),
        SwitchListTile(
          value: _threadedView,
          title: Text(l10n.threadedView),
          subtitle: Text(l10n.threadedViewSubtitle),
          onChanged: (bool value) async {
            setState(() {
              _threadedView = value;
              _config = _config.copyWith(threadedView: value);
            });
            await _persistAppPreferences();
          },
        ),
        const Divider(height: 24),
        Text(
          l10n.deletionAndTrashSection,
          style: Theme.of(context).textTheme.titleSmall,
        ),
        const SizedBox(height: 4),
        Text(
          l10n.deletionAppliesGlobally,
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 8),
        DropdownButtonFormField<String>(
          key: ValueKey<String>(_deleteMode),
          initialValue: _deleteMode,
          items: [
            DropdownMenuItem(
              value: 'Move to Trash',
              child: Text(l10n.deleteModeMoveToTrash),
            ),
            DropdownMenuItem(
              value: 'Mark Deleted',
              child: Text(l10n.deleteModeMarkDeleted),
            ),
          ],
          onChanged: (String? value) async {
            setState(() {
              _deleteMode = value ?? _deleteMode;
              _config = _config.copyWith(deleteMode: _deleteMode);
            });
            await _persistAppPreferences();
          },
          decoration: InputDecoration(labelText: l10n.deleteModeLabel),
        ),
        TextField(
          controller: _trashFolder,
          decoration: InputDecoration(labelText: l10n.trashFolderNameLabel),
          onEditingComplete: () => _persistAppPreferences(),
        ),
      ],
    );
  }

  Widget _composingPane() {
    final AppLocalizations l10n = AppLocalizations.of(context);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        SwitchListTile(
          value: _quoteOriginal,
          title: Text(l10n.quoteOriginalOnReply),
          onChanged: (bool value) async {
            setState(() {
              _quoteOriginal = value;
              _config = _config.copyWith(quoteOriginal: value);
            });
            await _persistAppPreferences();
          },
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          children: [
            ElevatedButton(
              onPressed: () => _showStubCall(l10n.testSend),
              child: Text(l10n.testSend),
            ),
            OutlinedButton(
              onPressed: () => _showStubCall(l10n.openSignatureEditor),
              child: Text(l10n.openSignatureEditor),
            ),
          ],
        ),
      ],
    );
  }

  Widget _aboutPane() {
    final AppLocalizations l10n = AppLocalizations.of(context);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        ListTile(
          leading: SvgPicture.asset(
            brandedAppIconAssetPath(),
            width: 36,
            height: 36,
          ),
          title: Text(l10n.appTitle),
          subtitle: Text(l10n.aboutSubtitle),
        ),
        ListTile(
          title: Text(l10n.supportedBackends),
          subtitle: Text(l10n.supportedBackendsList),
        ),
        ListTile(
          title: Text(l10n.licenseTitle),
          subtitle: Text(l10n.licenseGpl),
        ),
        ListTile(
          title: Text(l10n.copyrightTitle),
          subtitle: Text(l10n.copyrightLine),
        ),
      ],
    );
  }

  void _showStubCall(String operation) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    ScaffoldMessenger.of(context)
      ..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(l10n.stubInvoked(operation))));
  }
}
