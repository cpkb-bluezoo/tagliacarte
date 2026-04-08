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

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_svg/flutter_svg.dart';
import '../l10n/app_localizations.dart';
import '../providers/app_state.dart';
import '../rust/frb_api.dart';
import '../util/compose_reply.dart';
import '../util/reply_format_presets.dart';
import '../util/process_log.dart';
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

  /// False until [loadConfig] finishes. While false, must not call [saveConfig]: in-memory
  /// [_config] is still [AppSettingsConfig.defaults] (empty accounts/transports) and would
  /// overwrite disk on any Security/Viewing/Composing auto-save.
  bool _settingsReady = false;
  String? _loadError;

  AppSettingsConfig _config = AppSettingsConfig.defaults();
  bool _useKeychain = true;
  bool _loadRemoteImages = false;
  bool _threadedView = true;
  bool _quoteOriginal = true;
  final TextEditingController _replyHeaderTemplate = TextEditingController();
  String _replyDatePattern = '';
  String _replyTimePattern = '';
  final TextEditingController _replyLinePrefix = TextEditingController();
  String _replyPlainPosition = 'before_quote';
  String _replyQuoteMode = 'plain';
  bool _composeUseRichText = false;
  bool _matrixChatUseRichText = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loadError = null;
      _settingsReady = false;
    });
    try {
      final AppSettingsConfig config = await _api.loadConfig();
      if (!mounted) {
        return;
      }
      setState(() {
        _settingsReady = true;
        _config = config;
        _useKeychain = config.useKeychain;
        _loadRemoteImages = config.loadRemoteImages;
        _threadedView = config.threadedView;
        _quoteOriginal = config.quoteOriginal;
        _replyHeaderTemplate.text = config.replyHeaderTemplate;
        _replyDatePattern = config.replyDateFormat;
        _replyTimePattern = config.replyTimeFormat;
        _replyLinePrefix.text = config.replyLinePrefix;
        _replyPlainPosition = config.replyPlainPosition;
        _replyQuoteMode = config.replyQuoteMode;
        _composeUseRichText = config.composeUseRichText;
        _matrixChatUseRichText = config.matrixChatUseRichText;
      });
    } catch (e, st) {
      appLogStderr('settings: loadConfig failed: $e\n$st');
      if (!mounted) {
        return;
      }
      setState(() {
        _settingsReady = false;
        _loadError = e.toString();
      });
    }
  }

  @override
  void dispose() {
    _replyHeaderTemplate.dispose();
    _replyLinePrefix.dispose();
    super.dispose();
  }

  void _replaceConfig(AppSettingsConfig next) {
    setState(() => _config = next);
    ref.read(settingsRevisionProvider.notifier).state++;
    unawaited(_reloadRustSessionAccounts());
  }

  Future<void> _reloadRustSessionAccounts() async {
    try {
      final String path = await _api.configXmlPath();
      await frbSessionReloadAccounts(configXmlPath: path);
    } catch (e, st) {
      appLogStderr('frbSessionReloadAccounts failed: $e\n$st');
    }
  }

  /// Persists global preferences (not account CRUD — that happens in account detail).
  Future<void> _persistAppPreferences() async {
    if (!_settingsReady) {
      return;
    }
    final AppSettingsConfig config = _config.copyWith(
      useKeychain: _useKeychain,
      loadRemoteImages: _loadRemoteImages,
      threadedView: _threadedView,
      quoteOriginal: _quoteOriginal,
      replyHeaderTemplate: _replyHeaderTemplate.text,
      replyDateFormat: _replyDatePattern,
      replyTimeFormat: _replyTimePattern,
      replyLinePrefix: _replyLinePrefix.text.isEmpty ? '> ' : _replyLinePrefix.text,
      replyQuoteMode: _replyQuoteMode,
      replyPlainPosition: _replyPlainPosition,
      resourcePolicy: _loadRemoteImages ? 'allow-remote' : 'block-remote',
      composeUseRichText: _composeUseRichText,
      matrixChatUseRichText: _matrixChatUseRichText,
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
    if (!_settingsReady) {
      return Scaffold(
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
        ),
        body: Center(
          child: _loadError == null
              ? const CircularProgressIndicator()
              : Padding(
                  padding: const EdgeInsets.all(24),
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: <Widget>[
                      Text(
                        l10n.settingsLoadFailed,
                        textAlign: TextAlign.center,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 8),
                      SelectableText(
                        _loadError!,
                        textAlign: TextAlign.center,
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                      const SizedBox(height: 16),
                      FilledButton(
                        onPressed: _load,
                        child: Text(l10n.settingsLoadRetry),
                      ),
                    ],
                  ),
                ),
        ),
      );
    }
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
        SwitchListTile(
          value: _config.notifyNewMessages,
          title: Text(l10n.settingsNotifyNewMessages),
          subtitle: Text(l10n.settingsNotifyNewMessagesSubtitle),
          onChanged: (bool value) async {
            setState(() => _config = _config.copyWith(notifyNewMessages: value));
            await _persistAppPreferences();
          },
        ),
      ],
    );
  }

  List<String> _replyDatePatternChoices() {
    final List<String> p = List<String>.from(kReplyDateFormatPatterns);
    if (_replyDatePattern.isNotEmpty && !p.contains(_replyDatePattern)) {
      p.add(_replyDatePattern);
    }
    return p;
  }

  List<String> _replyTimePatternChoices() {
    final List<String> p = List<String>.from(kReplyTimeFormatPatterns);
    if (_replyTimePattern.isNotEmpty && !p.contains(_replyTimePattern)) {
      p.add(_replyTimePattern);
    }
    return p;
  }

  String _replyHeaderPreviewLine() {
    final Locale locale = Localizations.localeOf(context);
    final DateTime now = DateTime.now();
    final String date = formatReplyDate(now, _replyDatePattern, locale);
    final String time = formatReplyTime(now, _replyTimePattern, locale);
    return expandReplyHeaderTemplate(
      _replyHeaderTemplate.text,
      date,
      time,
      'alice@example.com',
    );
  }

  Widget _composingPane() {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final ThemeData theme = Theme.of(context);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Text(
          l10n.composingReplySection,
          style: theme.textTheme.titleSmall,
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          value: _quoteOriginal,
          title: Text(l10n.quoteOriginalOnReply),
          subtitle: Text(l10n.quoteOriginalOnReplySubtitle),
          onChanged: (bool value) async {
            setState(() {
              _quoteOriginal = value;
              _config = _config.copyWith(quoteOriginal: value);
            });
            await _persistAppPreferences();
          },
        ),
        if (_quoteOriginal) ...[
          const SizedBox(height: 4),
          DropdownButtonFormField<String>(
            // ignore: deprecated_member_use
            value: normalizeReplyPlainPosition(_replyPlainPosition) ==
                    'after_quote'
                ? 'after_quote'
                : 'before_quote',
            decoration: InputDecoration(
              labelText: l10n.replyPlainPositionLabel,
            ),
            items: <DropdownMenuItem<String>>[
              DropdownMenuItem<String>(
                value: 'before_quote',
                child: Text(l10n.replyPlainPositionBefore),
              ),
              DropdownMenuItem<String>(
                value: 'after_quote',
                child: Text(l10n.replyPlainPositionAfter),
              ),
            ],
            onChanged: (String? v) async {
              if (v == null) {
                return;
              }
              setState(() {
                _replyPlainPosition = v;
                _config = _config.copyWith(replyPlainPosition: v);
              });
              await _persistAppPreferences();
            },
          ),
          Padding(
            padding: const EdgeInsets.only(top: 4, bottom: 8),
            child: Text(
              l10n.replyPlainPositionSubtitle,
              style: theme.textTheme.bodySmall,
            ),
          ),
        ],
        TextField(
          controller: _replyHeaderTemplate,
          decoration: InputDecoration(
            labelText: l10n.replyHeaderTemplateLabel,
            helperText: l10n.replyHeaderTemplateHelp,
            helperMaxLines: 5,
          ),
          maxLines: 2,
          onChanged: (_) => setState(() {}),
          onEditingComplete: () async {
            setState(() {
              _config = _config.copyWith(
                replyHeaderTemplate: _replyHeaderTemplate.text,
              );
            });
            await _persistAppPreferences();
          },
        ),
        const SizedBox(height: 6),
        Text(
          '${l10n.replyHeaderPreviewLabel}: ${_replyHeaderPreviewLine()}',
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 12),
        DropdownButtonFormField<String>(
          // ignore: deprecated_member_use
          value: _replyDatePatternChoices().contains(_replyDatePattern)
              ? _replyDatePattern
              : _replyDatePatternChoices().first,
          decoration: InputDecoration(
            labelText: l10n.replyDateFormatLabel,
          ),
          items: _replyDatePatternChoices()
              .map(
                (String p) => DropdownMenuItem<String>(
                  value: p,
                  child: Text(replyDatePresetLabel(l10n, p)),
                ),
              )
              .toList(),
          onChanged: (String? v) async {
            if (v == null) {
              return;
            }
            setState(() {
              _replyDatePattern = v;
              _config = _config.copyWith(replyDateFormat: v);
            });
            await _persistAppPreferences();
          },
        ),
        const SizedBox(height: 8),
        DropdownButtonFormField<String>(
          // ignore: deprecated_member_use
          value: _replyTimePatternChoices().contains(_replyTimePattern)
              ? _replyTimePattern
              : _replyTimePatternChoices().first,
          decoration: InputDecoration(
            labelText: l10n.replyTimeFormatLabel,
          ),
          items: _replyTimePatternChoices()
              .map(
                (String p) => DropdownMenuItem<String>(
                  value: p,
                  child: Text(replyTimePresetLabel(l10n, p)),
                ),
              )
              .toList(),
          onChanged: (String? v) async {
            if (v == null) {
              return;
            }
            setState(() {
              _replyTimePattern = v;
              _config = _config.copyWith(replyTimeFormat: v);
            });
            await _persistAppPreferences();
          },
        ),
        const SizedBox(height: 8),
        TextField(
          controller: _replyLinePrefix,
          enabled: _quoteOriginal,
          decoration: InputDecoration(
            labelText: l10n.replyLinePrefixLabel,
            helperText: l10n.replyLinePrefixSubtitle,
            helperMaxLines: 4,
          ),
          onEditingComplete: () async {
            setState(() {
              _config = _config.copyWith(
                replyLinePrefix: _replyLinePrefix.text.isEmpty
                    ? '> '
                    : _replyLinePrefix.text,
              );
            });
            await _persistAppPreferences();
          },
        ),
        const SizedBox(height: 12),
        DropdownButtonFormField<String>(
          // ignore: deprecated_member_use
          value: _replyQuoteMode,
          decoration: InputDecoration(
            labelText: l10n.replyQuoteModeLabel,
          ),
          items: <DropdownMenuItem<String>>[
            DropdownMenuItem<String>(
              value: 'plain',
              child: Text(l10n.replyQuoteModePlain),
            ),
            DropdownMenuItem<String>(
              value: 'html_smtp',
              child: Text(l10n.replyQuoteModeHtmlSmtp),
            ),
          ],
          onChanged: (String? v) async {
            if (v == null) {
              return;
            }
            setState(() {
              _replyQuoteMode = v;
              _config = _config.copyWith(replyQuoteMode: v);
            });
            await _persistAppPreferences();
          },
        ),
        Padding(
          padding: const EdgeInsets.only(top: 4, bottom: 8),
          child: Text(
            l10n.replyQuoteModeHtmlSmtpSubtitle,
            style: theme.textTheme.bodySmall,
          ),
        ),
        SwitchListTile(
          value: _composeUseRichText,
          title: Text(l10n.settingsComposeRichText),
          subtitle: Text(l10n.settingsComposeRichTextSubtitle),
          onChanged: (bool value) async {
            setState(() {
              _composeUseRichText = value;
              _config = _config.copyWith(composeUseRichText: value);
            });
            await _persistAppPreferences();
          },
        ),
        SwitchListTile(
          value: _matrixChatUseRichText,
          title: Text(l10n.settingsMatrixChatRichText),
          subtitle: Text(l10n.settingsMatrixChatRichTextSubtitle),
          onChanged: (bool value) async {
            setState(() {
              _matrixChatUseRichText = value;
              _config = _config.copyWith(matrixChatUseRichText: value);
            });
            await _persistAppPreferences();
          },
        ),
        const Divider(height: 24),
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
