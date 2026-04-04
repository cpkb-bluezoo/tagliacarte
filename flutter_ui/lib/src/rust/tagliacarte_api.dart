/*
 * tagliacarte_api.dart
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

import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';

import 'frb_api.dart';

/// Distinguishes "leave unchanged" from `null` in [AppSettingsConfig.copyWith] for optional strings.
const Object _kCopyWithUnsetOptionalString = Object();

/// Persistent settings API: **on disk** everything is `config.xml` (Flutter ↔ Rust still
/// uses JSON over flutter_rust_bridge for [FrbConfig] payloads).
///
/// **Primary file (all desktop platforms):** [getApplicationSupportDirectory] from
/// `path_provider`, then `tagliacarte/config.xml`. On **macOS** that is typically:
/// `~/Library/Application Support/<bundle-id>/tagliacarte/config.xml`
/// (bundle id from `macos/Runner/Configs/AppInfo.xcconfig`, e.g. `org.bluezoo.tagliacarte`).
///
/// Rust may still merge **stores** from `TAGLIACARTE_CONFIG_DIR` or `~/.tagliacarte/config.xml`
/// when the app config has no stores (see `app/src/frb_api/mod.rs`). Legacy `config.json`
/// in the same folder is migrated once to XML and then deleted.
///
/// **Credentials** (passwords / OAuth) use the keychain or `~/.tagliacarte/credentials`
/// depending on settings — not this file.
///
/// A few **view toggles** (inline detail, minimal headers) use `shared_preferences`
/// (on macOS under the app’s preferences domain), not `config.xml`.
///
/// Outgoing transport (SMTP, etc.), persisted in config JSON / XML.
class AppTransport {
  AppTransport({
    required this.id,
    required this.transportType,
    required this.displayName,
    required this.host,
    required this.port,
    required this.security,
    required this.transportUri,
  });

  final String id;
  final String transportType;
  final String displayName;
  final String host;
  final int port;

  /// Symbolic: `starttls`, `tls`, `plain`, … (see ARCHITECTURE.md).
  final String security;
  final String transportUri;

  factory AppTransport.fromJson(Map<String, dynamic> json) => AppTransport(
    id: json['id'] as String,
    transportType: json['transportType'] as String? ?? 'smtp',
    displayName: json['displayName'] as String? ?? json['id'] as String,
    host: json['host'] as String? ?? '',
    port: (json['port'] as num?)?.toInt() ?? 587,
    security: json['security'] as String? ?? 'starttls',
    transportUri: json['transportUri'] as String? ?? '',
  );

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'transportType': transportType,
    'displayName': displayName,
    'host': host,
    'port': port,
    'security': security,
    'transportUri': transportUri,
  };

  /// Matches [tagliacarte_core::uri::smtp_transport_uri]: port 465 → smtps.
  static String deriveSmtpUri(String host, int port) {
    final String scheme = port == 465 ? 'smtps' : 'smtp';
    return '$scheme://$host:$port';
  }

  /// Primary line for list rows (like account [AppAccount.label]); no URL, no [id].
  String get primaryListTitle {
    if (displayName.isNotEmpty) {
      return displayName;
    }
    if (host.isNotEmpty) {
      return host;
    }
    return 'Outgoing transport';
  }

  /// Subtitle for list rows (e.g. `SMTP`); extensible when more [transportType]s exist.
  String get typeDisplayLabel {
    final String raw = transportType.trim();
    if (raw.isEmpty) {
      return 'Outgoing';
    }
    final String lower = raw.toLowerCase();
    if (lower == 'smtp') {
      return 'SMTP';
    }
    if (raw.length == raw.toUpperCase().length && raw.length <= 8) {
      return raw.toUpperCase();
    }
    return raw[0].toUpperCase() + raw.substring(1).toLowerCase();
  }

  /// One-line label for dialogs/snackbars; same as [primaryListTitle].
  String get uiListLabel => primaryListTitle;

  factory AppTransport.smtpDraft({
    required String id,
    required String displayName,
    required String host,
    int port = 587,
    String security = 'starttls',
  }) {
    return AppTransport(
      id: id,
      transportType: 'smtp',
      displayName: displayName,
      host: host,
      port: port,
      security: security,
      transportUri: deriveSmtpUri(host, port),
    );
  }
}

class AppAccount {
  AppAccount({
    required this.id,
    required this.label,
    required this.backendType,
    required this.storeUri,
    this.transportIds = const <String>[],
    this.transportUri,
    this.username,
    this.host,
    this.port,
    this.security,
    this.path,
    this.email,
    this.avatarUrl,
    this.lastFolder,
    this.lastMessageId,
    this.imapIdleMinIdleSeconds,
  });

  /// Stable store id (`s1`, …) for credentials and XML; may equal legacy URI.
  final String id;

  /// Display name for the account strip (see `docs/ui-decisions.md`).
  final String label;
  final String backendType;
  final String storeUri;

  /// Ordered outgoing transport ids (first = default). Empty disables send for
  /// backends that need external SMTP.
  final List<String> transportIds;
  final String? transportUri;

  /// Username / email from account setup; used for initials when [label] is empty.
  final String? email;

  /// Optional structured fields when not inferring store from [storeUri] alone.
  final String? username;
  final String? host;
  final int? port;
  final String? security;
  final String? path;

  /// HTTP(S) URL or local file path (IO platforms) for strip avatar.
  final String? avatarUrl;

  /// Last opened folder for this store (persisted under `<store>` in config XML).
  final String? lastFolder;

  /// Last selected message id within [lastFolder] for this store.
  final String? lastMessageId;

  /// Minimum seconds of IMAP connection quiet before IDLE; `null` → default 120.
  final int? imapIdleMinIdleSeconds;

  factory AppAccount.fromJson(Map<String, dynamic> json) => AppAccount(
    id: json['id'] as String,
    label: json['label'] as String,
    backendType: json['backendType'] as String,
    storeUri: json['storeUri'] as String,
    transportIds:
        (json['transportIds'] as List<dynamic>?)
            ?.map((dynamic e) => e.toString())
            .toList() ??
        const <String>[],
    transportUri: json['transportUri'] as String?,
    username: json['username'] as String?,
    host: json['host'] as String?,
    port: (json['port'] as num?)?.toInt(),
    security: json['security'] as String?,
    path: json['path'] as String?,
    email: json['email'] as String?,
    avatarUrl: json['avatarUrl'] as String?,
    lastFolder: json['lastFolder'] as String?,
    lastMessageId: json['lastMessageId'] as String?,
    imapIdleMinIdleSeconds: (json['imapIdleMinIdleSeconds'] as num?)?.toInt(),
  );

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'label': label,
    'backendType': backendType,
    'storeUri': storeUri,
    'transportIds': transportIds,
    if (transportUri != null) 'transportUri': transportUri,
    if (username != null) 'username': username,
    if (host != null) 'host': host,
    if (port != null) 'port': port,
    if (security != null) 'security': security,
    if (path != null) 'path': path,
    if (email != null) 'email': email,
    if (avatarUrl != null) 'avatarUrl': avatarUrl,
    if (lastFolder != null) 'lastFolder': lastFolder,
    if (lastMessageId != null) 'lastMessageId': lastMessageId,
    if (imapIdleMinIdleSeconds != null)
      'imapIdleMinIdleSeconds': imapIdleMinIdleSeconds,
  };

  AppAccount copyWith({
    String? id,
    String? label,
    String? backendType,
    String? storeUri,
    List<String>? transportIds,
    String? transportUri,
    String? username,
    String? host,
    int? port,
    String? security,
    String? path,
    String? email,
    String? avatarUrl,
    Object? lastFolder = _kCopyWithUnsetOptionalString,
    Object? lastMessageId = _kCopyWithUnsetOptionalString,
    int? imapIdleMinIdleSeconds,
  }) => AppAccount(
    id: id ?? this.id,
    label: label ?? this.label,
    backendType: backendType ?? this.backendType,
    storeUri: storeUri ?? this.storeUri,
    transportIds: transportIds ?? this.transportIds,
    transportUri: transportUri ?? this.transportUri,
    username: username ?? this.username,
    host: host ?? this.host,
    port: port ?? this.port,
    security: security ?? this.security,
    path: path ?? this.path,
    email: email ?? this.email,
    avatarUrl: avatarUrl ?? this.avatarUrl,
    lastFolder: identical(lastFolder, _kCopyWithUnsetOptionalString)
        ? this.lastFolder
        : lastFolder as String?,
    lastMessageId: identical(lastMessageId, _kCopyWithUnsetOptionalString)
        ? this.lastMessageId
        : lastMessageId as String?,
    imapIdleMinIdleSeconds:
        imapIdleMinIdleSeconds ?? this.imapIdleMinIdleSeconds,
  );
}

class AppSettingsConfig {
  AppSettingsConfig({
    required this.accounts,
    this.transports = const <AppTransport>[],
    this.selectedStoreId,
    required this.dateFormat,
    required this.resourcePolicy,
    required this.useKeychain,
    required this.loadRemoteImages,
    required this.threadedView,
    required this.quoteOriginal,
    required this.deleteMode,
    required this.trashFolderName,
    required this.messageListSort,
    required this.notifyNewMessages,
  });

  final List<AppAccount> accounts;
  final List<AppTransport> transports;
  final String? selectedStoreId;
  final String dateFormat;
  final String resourcePolicy;
  final bool useKeychain;
  final bool loadRemoteImages;
  final bool threadedView;
  final bool quoteOriginal;
  final String deleteMode;
  final String trashFolderName;

  /// Symbolic sort: `from_asc`, `date_desc`, etc.
  final String messageListSort;

  /// Toasts / local notifications when new mail arrives (after baseline sync).
  final bool notifyNewMessages;

  factory AppSettingsConfig.defaults() => AppSettingsConfig(
    accounts: <AppAccount>[],
    transports: <AppTransport>[],
    selectedStoreId: null,
    dateFormat: 'yyyy-MM-dd HH:mm',
    resourcePolicy: 'block-remote',
    useKeychain: true,
    loadRemoteImages: false,
    threadedView: true,
    quoteOriginal: true,
    deleteMode: 'Move to Trash',
    trashFolderName: 'Trash',
    messageListSort: 'date_desc',
    notifyNewMessages: false,
  );

  factory AppSettingsConfig.fromJson(Map<String, dynamic> json) {
    List<AppAccount> accounts =
        (json['accounts'] as List<dynamic>? ?? <dynamic>[])
            .map(
              (dynamic entry) =>
                  AppAccount.fromJson(entry as Map<String, dynamic>),
            )
            .toList();
    final String? selectedStoreId = json['selectedStoreId'] as String?;
    final String? legacyFolder = json['selectedFolder'] as String?;
    final String? legacyMessageId = json['selectedMessageId'] as String?;
    if (selectedStoreId != null &&
        (legacyFolder != null || legacyMessageId != null)) {
      final int i = accounts.indexWhere(
        (AppAccount a) => a.id == selectedStoreId,
      );
      if (i >= 0) {
        final AppAccount a = accounts[i];
        final bool mergeFolder = a.lastFolder == null && legacyFolder != null;
        final bool mergeMessage =
            a.lastMessageId == null && legacyMessageId != null;
        if (mergeFolder || mergeMessage) {
          accounts = List<AppAccount>.from(accounts);
          accounts[i] = a.copyWith(
            lastFolder: mergeFolder
                ? legacyFolder
                : _kCopyWithUnsetOptionalString,
            lastMessageId: mergeMessage
                ? legacyMessageId
                : _kCopyWithUnsetOptionalString,
          );
        }
      }
    }
    return AppSettingsConfig(
      accounts: accounts,
      transports: (json['transports'] as List<dynamic>? ?? <dynamic>[])
          .map(
            (dynamic entry) =>
                AppTransport.fromJson(entry as Map<String, dynamic>),
          )
          .toList(),
      selectedStoreId: selectedStoreId,
      dateFormat: json['dateFormat'] as String? ?? 'yyyy-MM-dd HH:mm',
      resourcePolicy: json['resourcePolicy'] as String? ?? 'block-remote',
      useKeychain: json['useKeychain'] as bool? ?? true,
      loadRemoteImages: json['loadRemoteImages'] as bool? ?? false,
      threadedView: json['threadedView'] as bool? ?? true,
      quoteOriginal: json['quoteOriginal'] as bool? ?? true,
      deleteMode: json['deleteMode'] as String? ?? 'Move to Trash',
      trashFolderName: json['trashFolderName'] as String? ?? 'Trash',
      messageListSort: json['messageListSort'] as String? ?? 'date_desc',
      notifyNewMessages: json['notifyNewMessages'] as bool? ?? false,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'accounts': accounts.map((AppAccount a) => a.toJson()).toList(),
    'transports': transports.map((AppTransport t) => t.toJson()).toList(),
    if (selectedStoreId != null) 'selectedStoreId': selectedStoreId,
    'dateFormat': dateFormat,
    'resourcePolicy': resourcePolicy,
    'useKeychain': useKeychain,
    'loadRemoteImages': loadRemoteImages,
    'threadedView': threadedView,
    'quoteOriginal': quoteOriginal,
    'deleteMode': deleteMode,
    'trashFolderName': trashFolderName,
    'messageListSort': messageListSort,
    'notifyNewMessages': notifyNewMessages,
  };

  AppSettingsConfig copyWith({
    List<AppAccount>? accounts,
    List<AppTransport>? transports,
    Object? selectedStoreId = _kCopyWithUnsetOptionalString,
    String? dateFormat,
    String? resourcePolicy,
    bool? useKeychain,
    bool? loadRemoteImages,
    bool? threadedView,
    bool? quoteOriginal,
    String? deleteMode,
    String? trashFolderName,
    String? messageListSort,
    bool? notifyNewMessages,
  }) => AppSettingsConfig(
    accounts: accounts ?? this.accounts,
    transports: transports ?? this.transports,
    selectedStoreId: identical(selectedStoreId, _kCopyWithUnsetOptionalString)
        ? this.selectedStoreId
        : selectedStoreId as String?,
    dateFormat: dateFormat ?? this.dateFormat,
    resourcePolicy: resourcePolicy ?? this.resourcePolicy,
    useKeychain: useKeychain ?? this.useKeychain,
    loadRemoteImages: loadRemoteImages ?? this.loadRemoteImages,
    threadedView: threadedView ?? this.threadedView,
    quoteOriginal: quoteOriginal ?? this.quoteOriginal,
    deleteMode: deleteMode ?? this.deleteMode,
    trashFolderName: trashFolderName ?? this.trashFolderName,
    messageListSort: messageListSort ?? this.messageListSort,
    notifyNewMessages: notifyNewMessages ?? this.notifyNewMessages,
  );
}

/// Vault key for IMAP/store credentials. Empty string lets native code fall back
/// to [AppAccount.storeUri] for legacy configs.
String storeCredentialKey(AppAccount account) => account.id;

class TagliacarteApi {
  const TagliacarteApi();

  /// Absolute path to `config.xml` (same file as [loadConfig] / [saveConfig]).
  Future<String> configXmlPath() => _configPath();

  Future<String> _configPath() async {
    final Directory base = await getApplicationSupportDirectory();
    return '${base.path}/tagliacarte/config.xml';
  }

  Future<AppSettingsConfig> loadConfig() async {
    final String path = await _configPath();
    // Rust reads config.xml at [path], returns JSON for Dart. Merges auxiliary
    // config.xml when needed (see app/src/frb_api/mod.rs).
    final String jsonValue = await frbLoadConfigJson(path: path);
    if (jsonValue.isEmpty) {
      if (kDebugMode) {
        debugPrint(
          'tagliacarte: loadConfig empty JSON; path=$path '
          '(XML merge still applies inside native load when xml exists)',
        );
      }
      return AppSettingsConfig.defaults();
    }
    final dynamic decoded = jsonDecode(jsonValue);
    final AppSettingsConfig config = AppSettingsConfig.fromJson(
      decoded as Map<String, dynamic>,
    );
    if (kDebugMode) {
      debugPrint(
        'tagliacarte: loadConfig path=$path '
        'accounts=${config.accounts.length}'
        '${Platform.isMacOS ? ' [macOS: under ~/Library/Application Support/]' : ''}',
      );
    }
    return config;
  }

  Future<void> saveConfig(AppSettingsConfig config) async {
    final String path = await _configPath();
    if (kDebugMode) {
      debugPrint(
        'tagliacarte: saveConfig path=$path'
        '${Platform.isMacOS ? ' [macOS: under ~/Library/Application Support/]' : ''}',
      );
    }
    await frbSaveConfigJson(
      path: path,
      configJson: jsonEncode(config.toJson()),
    );
  }

  Future<AppSettingsConfig> addOrUpdateAccount(
    AppSettingsConfig current,
    AppAccount account,
  ) async {
    final String path = await _configPath();
    final String updatedJson = await frbUpsertAccount(
      path: path,
      accountJson: jsonEncode(account.toJson()),
    );
    final dynamic decoded = jsonDecode(updatedJson);
    return AppSettingsConfig.fromJson(decoded as Map<String, dynamic>);
  }

  Future<AppSettingsConfig> removeAccount(
    AppSettingsConfig current,
    String accountId,
  ) async {
    final String path = await _configPath();
    final String updatedJson = await frbRemoveAccount(
      path: path,
      accountId: accountId,
    );
    final dynamic decoded = jsonDecode(updatedJson);
    return AppSettingsConfig.fromJson(decoded as Map<String, dynamic>);
  }
}
