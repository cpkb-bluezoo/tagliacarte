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

import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

import '../util/process_log.dart';
import 'frb_api.dart';

/// Vendor segment for data paths (matches `tagliacarte_core::config::TAGLIACARTE_DATA_VENDOR`).
const String _kDataVendor = 'org.bluezoo';

/// Distinguishes "leave unchanged" from `null` in [AppSettingsConfig.copyWith] for optional strings.
const Object _kCopyWithUnsetOptionalString = Object();

bool _accountAttrsSeemComplete(Map<String, String> attrs, String backendType) {
  final String b = backendType.trim().toLowerCase();
  switch (b) {
    case 'imap':
    case 'exchange':
    case 'pop3':
    case 'nntp':
      return (attrs['host'] ?? '').isNotEmpty;
    case 'gmail':
      return (attrs['email'] ?? '').isNotEmpty;
    case 'maildir':
    case 'mbox':
      return (attrs['path'] ?? '').isNotEmpty;
    case 'nostr':
      return (attrs['npub'] ?? '').isNotEmpty;
    case 'matrix':
      return (attrs['homeserver'] ?? attrs['host'] ?? '').isNotEmpty;
    default:
      return true;
  }
}

/// Fills [attrs] (and optionally [lists]) from a legacy `storeUri` when modern fields are missing.
void _tryMergeLegacyStoreUri(
  Map<String, String> attrs,
  Map<String, List<String>> lists,
  String? storeUri,
  String backendType,
) {
  final String trimmed = storeUri?.trim() ?? '';
  if (trimmed.isEmpty) {
    return;
  }
  if (_accountAttrsSeemComplete(attrs, backendType)) {
    return;
  }
  final Uri? u = Uri.tryParse(trimmed);
  if (u == null) {
    return;
  }
  switch (u.scheme) {
    case 'maildir':
    case 'mbox':
      if (u.path.isNotEmpty) {
        attrs.putIfAbsent('path', () => u.path);
      }
      return;
    case 'imap':
    case 'imaps':
      final String? host = u.host.isNotEmpty ? u.host : null;
      if (host == null) {
        return;
      }
      attrs.putIfAbsent('host', () => host);
      final int port = u.hasPort ? u.port : (u.scheme == 'imaps' ? 993 : 143);
      attrs.putIfAbsent('port', () => '$port');
      final String sec = u.scheme == 'imaps' || port == 993
          ? 'tls'
          : ((u.queryParameters['security'] ?? '') == 'plain'
                ? 'plain'
                : 'starttls');
      attrs.putIfAbsent('security', () => sec);
      final String user = Uri.decodeComponent(u.userInfo);
      if (user.isNotEmpty) {
        attrs.putIfAbsent('username', () => user);
        attrs.putIfAbsent('email', () => user);
      }
      return;
    case 'pop3':
    case 'pop3s':
      final String? host = u.host.isNotEmpty ? u.host : null;
      if (host == null) {
        return;
      }
      attrs.putIfAbsent('host', () => host);
      attrs.putIfAbsent('port', () => '${u.hasPort ? u.port : 995}');
      attrs.putIfAbsent('security', () => 'tls');
      final String user = Uri.decodeComponent(u.userInfo);
      if (user.isNotEmpty) {
        attrs.putIfAbsent('username', () => user);
        attrs.putIfAbsent('email', () => user);
      }
      return;
    case 'nntp':
    case 'nntps':
      final String? host = u.host.isNotEmpty ? u.host : null;
      if (host == null) {
        return;
      }
      attrs.putIfAbsent('host', () => host);
      attrs.putIfAbsent('port', () => '${u.hasPort ? u.port : 563}');
      attrs.putIfAbsent('security', () => 'tls');
      final String user = Uri.decodeComponent(u.userInfo);
      if (user.isNotEmpty) {
        attrs.putIfAbsent('username', () => user);
        attrs.putIfAbsent('email', () => user);
      }
      return;
    case 'gmail':
      final String user = Uri.decodeComponent(u.userInfo);
      if (user.isNotEmpty) {
        attrs.putIfAbsent('email', () => user);
      }
      return;
    case 'graph':
      final String user = Uri.decodeComponent(u.userInfo);
      if (user.isNotEmpty) {
        attrs.putIfAbsent('email', () => user);
      }
      return;
    case 'nostr':
      if (trimmed.startsWith('nostr:store:')) {
        final String rest = trimmed.substring('nostr:store:'.length);
        final String idPart = rest.split(RegExp(r'[?#]')).first.trim();
        if (idPart.isNotEmpty) {
          attrs.putIfAbsent('npub', () => idPart);
        }
      } else if (trimmed.startsWith('nostr:')) {
        final String rest = trimmed.substring('nostr:'.length);
        if (!rest.startsWith('transport:')) {
          final String idPart = rest.split(RegExp(r'[?#]')).first.trim();
          if (idPart.isNotEmpty) {
            attrs.putIfAbsent('npub', () => idPart);
          }
        }
      }
      if ((lists['relayUrls'] ?? const <String>[]).isEmpty) {
        final String? r = u.queryParameters['relays'];
        if (r != null && r.isNotEmpty) {
          lists['relayUrls'] = r
              .split(RegExp(r'[,;\s]+'))
              .map((String s) => s.trim())
              .where((String s) => s.isNotEmpty)
              .toList();
        }
      }
      return;
    default:
      if (trimmed.startsWith('matrix:store:')) {
        final String rest = trimmed.substring('matrix:store:'.length);
        final int colon = rest.lastIndexOf(':');
        if (colon > 0) {
          final String hs = rest.substring(0, colon).trim();
          final String mx = rest.substring(colon + 1).trim();
          if (hs.isNotEmpty) {
            attrs.putIfAbsent('host', () => hs);
            attrs.putIfAbsent('homeserver', () => hs);
          }
          if (mx.isNotEmpty) {
            attrs.putIfAbsent('username', () => mx);
          }
        }
      }
  }
}

Map<String, String> _attrsFromAccountJson(Map<String, dynamic> json) {
  final Map<String, String> out = <String, String>{};
  final dynamic attrs = json['attrs'];
  if (attrs is Map) {
    attrs.forEach((dynamic k, dynamic v) {
      if (k is String && v != null) {
        out[k] = v.toString();
      }
    });
  }
  void legacy(String key) {
    if (out.containsKey(key)) {
      return;
    }
    final dynamic v = json[key];
    if (v == null) {
      return;
    }
    out[key] = v is num ? v.toString() : v.toString();
  }

  legacy('username');
  legacy('host');
  legacy('port');
  legacy('security');
  legacy('path');
  legacy('email');
  legacy('transportUri');
  legacy('imapIdleMinIdleSeconds');
  legacy('defaultFrom');
  return out;
}

Map<String, List<String>> _listsFromAccountJson(Map<String, dynamic> json) {
  final Map<String, List<String>> out = <String, List<String>>{};
  final dynamic lists = json['lists'];
  if (lists is Map) {
    lists.forEach((dynamic k, dynamic v) {
      if (k is String && v is List<dynamic>) {
        out[k] = v.map((dynamic e) => e.toString()).toList();
      }
    });
  }
  final dynamic tid = json['transportIds'];
  if (tid is List<dynamic> && !out.containsKey('transportIds')) {
    out['transportIds'] = tid.map((dynamic e) => e.toString()).toList();
  }
  final dynamic relays = json['relayUrls'];
  if (relays is List<dynamic> && !out.containsKey('relayUrls')) {
    out['relayUrls'] = relays.map((dynamic e) => e.toString()).toList();
  }
  return out;
}

/// Persistent settings API: **on disk** everything is `config.xml` (Flutter ↔ Rust still
/// uses JSON over flutter_rust_bridge for [FrbConfig] payloads).
///
/// **Primary file:** `{data_dir}/config.xml` where `data_dir` matches `tagliacarte_core::config::tagliacarte_data_dir`:
/// **macOS** `~/Library/Application Support/org.bluezoo/tagliacarte`, **Linux**
/// `$XDG_DATA_HOME|~/.local/share/org.bluezoo/tagliacarte`, **Windows**
/// `%APPDATA%\\org.bluezoo\\tagliacarte`, **iOS/Android** [getApplicationSupportDirectory] from
/// `path_provider` then `tagliacarte/` (app-scoped). Override: `TAGLIACARTE_DATA_DIR` /
/// `TAGLIACARTE_CONFIG_DIR`.
///
/// The **terminal UI** uses the same tree (single data directory, not split XDG config/cache).
///
/// Rust may still merge **stores** from an auxiliary `config.xml` when the primary file has
/// no stores (see `app/src/frb_api/mod.rs`). Legacy `config.json` in the same folder is
/// migrated once to XML and then deleted.
///
/// **Credentials** (passwords / OAuth) use the keychain or `{data_dir}/credentials`
/// depending on settings — not `config.xml`.
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
    this.defaultFrom = '',
    this.dsnNotify = 'failure',
    this.oauthProvider = '',
  });

  final String id;
  final String transportType;
  final String displayName;
  final String host;
  final int port;

  /// Symbolic: `starttls`, `tls`, `plain`, … (see ARCHITECTURE.md).
  final String security;

  /// Default From header for compose when this transport is selected.
  final String defaultFrom;

  /// Comma-separated: `never`, `failure`, `success`, `delay`.
  final String dsnNotify;

  /// Optional XOAUTH2 provider policy (`google`, `microsoft`, ...). Empty => password auth.
  final String oauthProvider;

  factory AppTransport.fromJson(Map<String, dynamic> json) => AppTransport(
    id: json['id'] as String,
    transportType: json['transportType'] as String? ?? 'smtp',
    displayName: json['displayName'] as String? ?? json['id'] as String,
    host: json['host'] as String? ?? '',
    port: (json['port'] as num?)?.toInt() ?? 587,
    security: json['security'] as String? ?? 'starttls',
    defaultFrom: json['defaultFrom'] as String? ?? '',
    dsnNotify: json['dsnNotify'] as String? ?? 'failure',
    oauthProvider: json['oauthProvider'] as String? ?? '',
  );

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'transportType': transportType,
    'displayName': displayName,
    'host': host,
    'port': port,
    'security': security,
    'defaultFrom': defaultFrom,
    'dsnNotify': dsnNotify,
    if (oauthProvider.trim().isNotEmpty) 'oauthProvider': oauthProvider.trim(),
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
    if (lower == 'gmail') {
      return 'Gmail';
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
    int port = 465,
    String security = 'tls',
    String defaultFrom = '',
    String dsnNotify = 'failure',
    String oauthProvider = '',
  }) {
    return AppTransport(
      id: id,
      transportType: 'smtp',
      displayName: displayName,
      host: host,
      port: port,
      security: security,
      defaultFrom: defaultFrom,
      dsnNotify: dsnNotify,
      oauthProvider: oauthProvider,
    );
  }
}

class AppAccount {
  AppAccount({
    required this.id,
    required this.label,
    required this.backendType,
    this.avatarUrl,
    this.lastFolder,
    this.lastMessageId,
    Map<String, String>? attrs,
    Map<String, List<String>>? lists,
  }) : attrs = attrs ?? <String, String>{},
       lists = lists ?? <String, List<String>>{};

  /// Stable store id (`s1`, …) for credentials and XML; may equal legacy URI.
  final String id;

  /// Display name for the account strip (see `docs/ui-decisions.md`).
  final String label;
  final String backendType;

  /// Backend-specific scalars (`host`, `username`, `npub`, `nip05`, `path`, …).
  final Map<String, String> attrs;

  /// Backend-specific lists (`transportIds`, `relayUrls`, …).
  final Map<String, List<String>> lists;

  /// Ordered outgoing transport ids (first = default). Empty disables send for
  /// backends that need external SMTP.
  List<String> get transportIds => lists['transportIds'] ?? const <String>[];

  /// Nostr relay URLs from [lists].
  List<String> get relayUrls => lists['relayUrls'] ?? const <String>[];

  /// HTTP(S) URL or local file path (IO platforms) for strip avatar.
  final String? avatarUrl;

  /// Last opened folder for this store (persisted under `<store>` in config XML).
  final String? lastFolder;

  /// Last selected message id within [lastFolder] for this store.
  final String? lastMessageId;

  /// One-line connection summary for settings / debugging (from [attrs], not a persisted URI).
  String get connectionSummary {
    final String b = backendType.trim().toLowerCase();
    final String host = attrs['host'] ?? '';
    final String port = attrs['port'] ?? '';
    final String path = attrs['path'] ?? '';
    final String npub = attrs['npub'] ?? '';
    if (b == 'imap' ||
        b == 'pop3' ||
        b == 'nntp' ||
        b == 'gmail' ||
        b == 'exchange') {
      if (host.isNotEmpty) {
        return port.isNotEmpty ? '$host:$port' : host;
      }
    }
    if ((b == 'maildir' || b == 'mbox') && path.isNotEmpty) {
      return path;
    }
    if (b == 'nostr') {
      if (npub.length > 24) {
        return '${npub.substring(0, 12)}…';
      }
      return npub.isEmpty ? label : npub;
    }
    if (b == 'matrix') {
      final String hs = attrs['homeserver'] ?? host;
      final String u = attrs['username'] ?? attrs['email'] ?? '';
      if (hs.isNotEmpty && u.isNotEmpty) {
        return '$u @ $hs';
      }
      return hs.isNotEmpty ? hs : label;
    }
    return label;
  }

  factory AppAccount.fromJson(Map<String, dynamic> json) {
    final Map<String, String> a = _attrsFromAccountJson(json);
    final Map<String, List<String>> l = _listsFromAccountJson(json);
    final String bt = json['backendType'] as String;
    _tryMergeLegacyStoreUri(a, l, json['storeUri'] as String?, bt);
    return AppAccount(
      id: json['id'] as String,
      label: json['label'] as String,
      backendType: bt,
      avatarUrl: json['avatarUrl'] as String?,
      lastFolder: json['lastFolder'] as String?,
      lastMessageId: json['lastMessageId'] as String?,
      attrs: a,
      lists: l,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'label': label,
    'backendType': backendType,
    if (avatarUrl != null) 'avatarUrl': avatarUrl,
    if (lastFolder != null) 'lastFolder': lastFolder,
    if (lastMessageId != null) 'lastMessageId': lastMessageId,
    if (attrs.isNotEmpty) 'attrs': attrs,
    if (lists.isNotEmpty) 'lists': lists,
  };

  AppAccount copyWith({
    String? id,
    String? label,
    String? backendType,
    String? avatarUrl,
    Object? lastFolder = _kCopyWithUnsetOptionalString,
    Object? lastMessageId = _kCopyWithUnsetOptionalString,
    Map<String, String>? attrs,
    Map<String, List<String>>? lists,
  }) => AppAccount(
    id: id ?? this.id,
    label: label ?? this.label,
    backendType: backendType ?? this.backendType,
    avatarUrl: avatarUrl ?? this.avatarUrl,
    lastFolder: identical(lastFolder, _kCopyWithUnsetOptionalString)
        ? this.lastFolder
        : lastFolder as String?,
    lastMessageId: identical(lastMessageId, _kCopyWithUnsetOptionalString)
        ? this.lastMessageId
        : lastMessageId as String?,
    attrs: attrs == null
        ? Map<String, String>.from(this.attrs)
        : Map<String, String>.from(attrs),
    lists: lists == null
        ? {
            for (final MapEntry<String, List<String>> e in this.lists.entries)
              e.key: List<String>.from(e.value),
          }
        : {
            for (final MapEntry<String, List<String>> e in lists.entries)
              e.key: List<String>.from(e.value),
          },
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
    required this.replyHeaderTemplate,
    required this.replyDateFormat,
    required this.replyTimeFormat,
    required this.replyLinePrefix,
    required this.replyQuoteMode,
    required this.replyPlainPosition,
    required this.messageListSort,
    required this.notifyNewMessages,
    required this.composeUseRichText,
    required this.matrixChatUseRichText,
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
  final String replyHeaderTemplate;
  final String replyDateFormat;
  final String replyTimeFormat;
  final String replyLinePrefix;

  /// `plain` (default) or `html_smtp` (append original HTML in multipart for SMTP).
  final String replyQuoteMode;

  /// Rich compose: `before_quote` or `after_quote` for generated text/plain ordering.
  final String replyPlainPosition;

  /// Symbolic sort: `from_asc`, `date_desc`, etc.
  final String messageListSort;

  /// Toasts / local notifications when new mail arrives (after baseline sync).
  final bool notifyNewMessages;

  /// Rich Quill editor on email compose (non-NNTP).
  final bool composeUseRichText;

  /// Rich Quill editor in Matrix chat composer.
  final bool matrixChatUseRichText;

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
    replyHeaderTemplate: r'On $date at $time, $sender wrote:',
    replyDateFormat: '',
    replyTimeFormat: '',
    replyLinePrefix: '> ',
    replyQuoteMode: 'plain',
    replyPlainPosition: 'before_quote',
    messageListSort: 'date_desc',
    notifyNewMessages: false,
    composeUseRichText: false,
    matrixChatUseRichText: false,
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
      replyHeaderTemplate:
          json['replyHeaderTemplate'] as String? ??
          r'On $date at $time, $sender wrote:',
      replyDateFormat: json['replyDateFormat'] as String? ?? '',
      replyTimeFormat: json['replyTimeFormat'] as String? ?? '',
      replyLinePrefix: json['replyLinePrefix'] as String? ?? '> ',
      replyQuoteMode: json['replyQuoteMode'] as String? ?? 'plain',
      replyPlainPosition:
          json['replyPlainPosition'] as String? ?? 'before_quote',
      messageListSort: json['messageListSort'] as String? ?? 'date_desc',
      notifyNewMessages: json['notifyNewMessages'] as bool? ?? false,
      composeUseRichText: json['composeUseRichText'] as bool? ?? false,
      matrixChatUseRichText: json['matrixChatUseRichText'] as bool? ?? false,
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
    'replyHeaderTemplate': replyHeaderTemplate,
    'replyDateFormat': replyDateFormat,
    'replyTimeFormat': replyTimeFormat,
    'replyLinePrefix': replyLinePrefix,
    'replyQuoteMode': replyQuoteMode,
    'replyPlainPosition': replyPlainPosition,
    'messageListSort': messageListSort,
    'notifyNewMessages': notifyNewMessages,
    'composeUseRichText': composeUseRichText,
    'matrixChatUseRichText': matrixChatUseRichText,
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
    String? replyHeaderTemplate,
    String? replyDateFormat,
    String? replyTimeFormat,
    String? replyLinePrefix,
    String? replyQuoteMode,
    String? replyPlainPosition,
    String? messageListSort,
    bool? notifyNewMessages,
    bool? composeUseRichText,
    bool? matrixChatUseRichText,
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
    replyHeaderTemplate: replyHeaderTemplate ?? this.replyHeaderTemplate,
    replyDateFormat: replyDateFormat ?? this.replyDateFormat,
    replyTimeFormat: replyTimeFormat ?? this.replyTimeFormat,
    replyLinePrefix: replyLinePrefix ?? this.replyLinePrefix,
    replyQuoteMode: replyQuoteMode ?? this.replyQuoteMode,
    replyPlainPosition: replyPlainPosition ?? this.replyPlainPosition,
    messageListSort: messageListSort ?? this.messageListSort,
    notifyNewMessages: notifyNewMessages ?? this.notifyNewMessages,
    composeUseRichText: composeUseRichText ?? this.composeUseRichText,
    matrixChatUseRichText: matrixChatUseRichText ?? this.matrixChatUseRichText,
  );
}

AppAccount _appAccountFromFrb(FrbAccount a) {
  return AppAccount(
    id: a.id,
    label: a.label,
    backendType: a.backendType,
    avatarUrl: a.avatarUrl,
    lastFolder: a.lastFolder,
    lastMessageId: a.lastMessageId,
    attrs: Map<String, String>.from(a.attrs),
    lists: <String, List<String>>{
      for (final MapEntry<String, List<String>> e in a.lists.entries)
        e.key: List<String>.from(e.value),
    },
  );
}

AppTransport _appTransportFromFrb(FrbTransport t) {
  return AppTransport(
    id: t.id,
    transportType: t.transportType,
    displayName: t.displayName,
    host: t.host,
    port: t.port,
    security: t.security,
    defaultFrom: t.defaultFrom,
    dsnNotify: t.dsnNotify,
    oauthProvider: t.oauthProvider,
  );
}

AppSettingsConfig _appSettingsFromFrb(FrbConfig f) {
  return AppSettingsConfig(
    accounts: f.accounts.map(_appAccountFromFrb).toList(),
    transports: f.transports.map(_appTransportFromFrb).toList(),
    selectedStoreId: f.selectedStoreId,
    dateFormat: f.dateFormat,
    resourcePolicy: f.resourcePolicy,
    useKeychain: f.useKeychain,
    loadRemoteImages: f.loadRemoteImages,
    threadedView: f.threadedView,
    quoteOriginal: f.quoteOriginal,
    replyHeaderTemplate: f.replyHeaderTemplate,
    replyDateFormat: f.replyDateFormat,
    replyTimeFormat: f.replyTimeFormat,
    replyLinePrefix: f.replyLinePrefix,
    replyQuoteMode: f.replyQuoteMode,
    replyPlainPosition: f.replyPlainPosition,
    messageListSort: f.messageListSort,
    notifyNewMessages: f.notifyNewMessages,
    composeUseRichText: f.composeUseRichText,
    matrixChatUseRichText: f.matrixChatUseRichText,
  );
}

FrbAccount _frbAccountFromApp(AppAccount a) {
  return FrbAccount(
    id: a.id,
    label: a.label,
    backendType: a.backendType,
    avatarUrl: a.avatarUrl,
    lastFolder: a.lastFolder,
    lastMessageId: a.lastMessageId,
    attrs: Map<String, String>.from(a.attrs),
    lists: <String, List<String>>{
      for (final MapEntry<String, List<String>> e in a.lists.entries)
        e.key: List<String>.from(e.value),
    },
  );
}

FrbTransport _frbTransportFromApp(AppTransport t) {
  return FrbTransport(
    id: t.id,
    transportType: t.transportType,
    displayName: t.displayName,
    host: t.host,
    port: t.port,
    security: t.security,
    defaultFrom: t.defaultFrom,
    dsnNotify: t.dsnNotify,
    oauthProvider: t.oauthProvider,
  );
}

FrbConfig _frbConfigFromApp(AppSettingsConfig c) {
  return FrbConfig(
    accounts: c.accounts.map(_frbAccountFromApp).toList(),
    transports: c.transports.map(_frbTransportFromApp).toList(),
    selectedStoreId: c.selectedStoreId,
    dateFormat: c.dateFormat,
    resourcePolicy: c.resourcePolicy,
    useKeychain: c.useKeychain,
    loadRemoteImages: c.loadRemoteImages,
    threadedView: c.threadedView,
    quoteOriginal: c.quoteOriginal,
    replyHeaderTemplate: c.replyHeaderTemplate,
    replyDateFormat: c.replyDateFormat,
    replyTimeFormat: c.replyTimeFormat,
    replyLinePrefix: c.replyLinePrefix,
    replyQuoteMode: c.replyQuoteMode,
    replyPlainPosition: c.replyPlainPosition,
    messageListSort: c.messageListSort,
    notifyNewMessages: c.notifyNewMessages,
    composeUseRichText: c.composeUseRichText,
    matrixChatUseRichText: c.matrixChatUseRichText,
  );
}

class TagliacarteApi {
  const TagliacarteApi();

  /// Absolute path to `config.xml` (same file as [loadConfig] / [saveConfig]).
  Future<String> configXmlPath() => _configPath();

  Future<String> _configPath() async {
    final String? home = Platform.environment['HOME'];
    if (Platform.isMacOS && home != null && home.isNotEmpty) {
      return p.join(
        home,
        'Library',
        'Application Support',
        _kDataVendor,
        'tagliacarte',
        'config.xml',
      );
    }
    if (Platform.isLinux && home != null && home.isNotEmpty) {
      final String? xdg = Platform.environment['XDG_DATA_HOME'];
      final String base = (xdg != null && xdg.isNotEmpty)
          ? xdg
          : p.join(home, '.local', 'share');
      return p.join(base, _kDataVendor, 'tagliacarte', 'config.xml');
    }
    if (Platform.isWindows) {
      final String? appdata = Platform.environment['APPDATA'];
      if (appdata != null && appdata.isNotEmpty) {
        return p.join(appdata, _kDataVendor, 'tagliacarte', 'config.xml');
      }
    }
    final Directory base = await getApplicationSupportDirectory();
    return p.join(base.path, 'tagliacarte', 'config.xml');
  }

  Future<AppSettingsConfig> loadConfig() async {
    final String path = await _configPath();
    final FrbConfig f = await frbLoadConfig(path: path);
    final AppSettingsConfig config = _appSettingsFromFrb(f);
    appLogStdout(
      'tagliacarte: loadConfig path=$path '
      'accounts=${config.accounts.length}'
      '${Platform.isMacOS ? ' [macOS: Application Support/org.bluezoo/tagliacarte]' : ''}',
    );
    return config;
  }

  Future<void> saveConfig(AppSettingsConfig config) async {
    final String path = await _configPath();
    appLogStdout(
      'tagliacarte: saveConfig path=$path'
      '${Platform.isMacOS ? ' [macOS: Application Support/org.bluezoo/tagliacarte]' : ''}',
    );
    await frbSaveConfig(path: path, cfg: _frbConfigFromApp(config));
  }

  Future<AppSettingsConfig> addOrUpdateAccount(
    AppSettingsConfig current,
    AppAccount account,
  ) async {
    final String path = await _configPath();
    final FrbConfig updated = await frbUpsertAccount(
      path: path,
      account: _frbAccountFromApp(account),
    );
    return _appSettingsFromFrb(updated);
  }

  Future<AppSettingsConfig> removeAccount(
    AppSettingsConfig current,
    String accountId,
  ) async {
    final String path = await _configPath();
    final FrbConfig updated = await frbRemoveAccount(
      path: path,
      accountId: accountId,
    );
    return _appSettingsFromFrb(updated);
  }
}
