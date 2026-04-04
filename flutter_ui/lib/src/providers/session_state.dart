/*
 * session_state.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * Consumes Rust [frbSessionStart] events into per-account mail metadata (folders, unreads).
 */

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../util/native_mail_uri.dart';
import 'app_state.dart';

/// Per-account folder list + unread counts + connection info from the Rust session.
@immutable
class AccountMailModel {
  const AccountMailModel({
    this.folders = const <String>[],
    this.unreadByFolder = const <String, int>{},
    this.hierarchyDelimiter,
    this.connection = MailConnectionState.idle,
    this.connectionMessage,
  });

  final List<String> folders;
  final Map<String, int> unreadByFolder;
  final String? hierarchyDelimiter;
  final MailConnectionState connection;
  final String? connectionMessage;

  AccountMailModel copyWith({
    List<String>? folders,
    Map<String, int>? unreadByFolder,
    String? hierarchyDelimiter,
    MailConnectionState? connection,
    String? connectionMessage,
  }) {
    return AccountMailModel(
      folders: folders ?? this.folders,
      unreadByFolder: unreadByFolder ?? this.unreadByFolder,
      hierarchyDelimiter: hierarchyDelimiter ?? this.hierarchyDelimiter,
      connection: connection ?? this.connection,
      connectionMessage: connectionMessage ?? this.connectionMessage,
    );
  }
}

enum MailConnectionState {
  idle,
  connecting,
  connected,
  disconnected,
  error,
}

/// Map of account id → model from the Rust app session.
class AccountMailModelsNotifier extends StateNotifier<Map<String, AccountMailModel>> {
  AccountMailModelsNotifier(this.ref) : super(const <String, AccountMailModel>{}) {
    unawaited(_subscribe());
  }

  final Ref ref;
  StreamSubscription<String>? _sub;
  bool _started = false;

  Future<void> _subscribe() async {
    if (_started) {
      return;
    }
    _started = true;
    try {
      final String path = await ref.read(tagliacarteApiProvider).configXmlPath();
      _sub = frbSessionStart(configXmlPath: path).listen(
        _onJson,
        onError: (Object e, StackTrace st) {
          debugPrint('mail session stream error: $e\n$st');
        },
      );
    } catch (e, st) {
      debugPrint('mail session start failed: $e\n$st');
    }
  }

  void _onJson(String raw) {
    try {
      final Map<String, dynamic> m =
          jsonDecode(raw) as Map<String, dynamic>;
      final String? t = m['type'] as String?;
      switch (t) {
        case 'accountConnectionChanged':
          _onConnection(m);
          break;
        case 'folderListUpdated':
          _onFolderList(m);
          break;
        case 'messageFlagsChanged':
        case 'commandResult':
          break;
      }
    } catch (e, st) {
      debugPrint('mail session event parse failed: $e\n$st raw=$raw');
    }
  }

  void _onConnection(Map<String, dynamic> m) {
    final String? id = m['accountId'] as String?;
    if (id == null) {
      return;
    }
    final String cs =
        (m['connectionState'] as String? ?? '').toLowerCase().trim();
    final String? msg = m['message'] as String?;
    MailConnectionState st;
    switch (cs) {
      case 'connecting':
        st = MailConnectionState.connecting;
        break;
      case 'connected':
        st = MailConnectionState.connected;
        break;
      case 'error':
        st = MailConnectionState.error;
        break;
      case 'disconnected':
        st = MailConnectionState.disconnected;
        break;
      default:
        st = MailConnectionState.idle;
        break;
    }
    final AccountMailModel prev = state[id] ?? const AccountMailModel();
    state = <String, AccountMailModel>{
      ...state,
      id: prev.copyWith(
        connection: st,
        connectionMessage: msg,
      ),
    };
  }

  void _onFolderList(Map<String, dynamic> m) {
    final String? id = m['accountId'] as String?;
    if (id == null) {
      return;
    }
    final List<String> folders = (m['folders'] as List<dynamic>?)
            ?.map((dynamic e) => e as String)
            .toList() ??
        const <String>[];
    final Map<String, int> unread = <String, int>{};
    final dynamic ur = m['unreadByFolder'];
    if (ur is Map) {
      ur.forEach((dynamic k, dynamic v) {
        unread[k as String] = (v as num).toInt();
      });
    }
    final String? delim = m['hierarchyDelimiter'] as String?;
    applyFolderListFromSession(
      id,
      folders: folders,
      unreadByFolder: unread,
      hierarchyDelimiter: delim,
    );
  }

  /// Same shape as [ _onFolderList ] but for a direct `frb_list_mail_folders` result
  /// (e.g. before the session stream has emitted for this account).
  void applyFolderListFromListCall({
    required String accountId,
    required List<String> folders,
    Map<String, int> unreadByFolder = const <String, int>{},
    String? hierarchyDelimiter,
  }) {
    applyFolderListFromSession(
      accountId,
      folders: folders,
      unreadByFolder: unreadByFolder,
      hierarchyDelimiter: hierarchyDelimiter,
    );
  }

  void applyFolderListFromSession(
    String accountId, {
    required List<String> folders,
    Map<String, int> unreadByFolder = const <String, int>{},
    String? hierarchyDelimiter,
  }) {
    final AccountMailModel prev = state[accountId] ?? const AccountMailModel();
    state = <String, AccountMailModel>{
      ...state,
      accountId: prev.copyWith(
        folders: folders,
        unreadByFolder: unreadByFolder,
        hierarchyDelimiter: hierarchyDelimiter,
      ),
    };
  }

  @override
  void dispose() {
    unawaited(_sub?.cancel() ?? Future<void>.value());
    super.dispose();
  }
}

final accountMailModelsProvider =
    StateNotifierProvider<AccountMailModelsNotifier, Map<String, AccountMailModel>>(
  AccountMailModelsNotifier.new,
);

/// Folders + unreads for the currently selected account (Rust session or non-native list).
final foldersProvider = Provider<MailFoldersState>((Ref ref) {
  ref.watch(accountMailModelsProvider);
  final String? id = ref.watch(selectedAccountIdProvider);
  if (id == null) {
    return const MailFoldersState();
  }
  final AppSettingsConfig? cfg = ref.watch(accountsConfigProvider).valueOrNull;
  AppAccount? account;
  for (final AppAccount a in cfg?.accounts ?? const <AppAccount>[]) {
    if (a.id == id) {
      account = a;
      break;
    }
  }
  if (account == null) {
    return const MailFoldersState();
  }
  if (!isNativeMailStoreUri(account.storeUri)) {
    return MailFoldersState(folders: ref.watch(nonNativeFolderListProvider));
  }
  final AccountMailModel? m = ref.watch(accountMailModelsProvider)[id];
  if (m == null) {
    return const MailFoldersState();
  }
  return MailFoldersState(
    folders: m.folders,
    unreadByFolder: m.unreadByFolder,
  );
});

/// IMAP hierarchy delimiter for the selected account.
final folderHierarchyDelimiterProvider = Provider<String?>((Ref ref) {
  final String? id = ref.watch(selectedAccountIdProvider);
  if (id == null) {
    return null;
  }
  return ref.watch(accountMailModelsProvider)[id]?.hierarchyDelimiter;
});

/// Sum of unreads across all folders per account (for account rail).
final storeTotalUnreadByAccountProvider = Provider<Map<String, int>>((Ref ref) {
  final Map<String, AccountMailModel> m = ref.watch(accountMailModelsProvider);
  final Map<String, int> out = <String, int>{};
  for (final MapEntry<String, AccountMailModel> e in m.entries) {
    int sum = 0;
    for (final int u in e.value.unreadByFolder.values) {
      sum += u;
    }
    if (sum > 0) {
      out[e.key] = sum;
    }
  }
  return out;
});

/// INBOX unread per native account (dock badge).
final nativeAccountInboxUnreadProvider = Provider<Map<String, int>>((Ref ref) {
  final Map<String, AccountMailModel> m = ref.watch(accountMailModelsProvider);
  final Map<String, int> out = <String, int>{};
  for (final MapEntry<String, AccountMailModel> e in m.entries) {
    for (final MapEntry<String, int> u in e.value.unreadByFolder.entries) {
      if (u.key.toUpperCase() == 'INBOX' && u.value > 0) {
        out[e.key] = u.value;
        break;
      }
    }
  }
  return out;
});

/// Sum of INBOX unreads over native accounts (macOS dock badge).
final nativeTotalInboxUnreadProvider = Provider<int>((Ref ref) {
  final Map<String, int> m = ref.watch(nativeAccountInboxUnreadProvider);
  int t = 0;
  for (final int v in m.values) {
    t += v;
  }
  return t;
});

Future<void> sessionMarkRead({
  required String accountId,
  required String folder,
  required String messageId,
}) {
  return frbSessionCommand(
    commandJson: jsonEncode(<String, dynamic>{
      'type': 'markRead',
      'accountId': accountId,
      'folder': folder,
      'messageId': messageId,
    }),
  );
}

Future<void> sessionRefreshFolders({required String accountId}) {
  return frbSessionCommand(
    commandJson: jsonEncode(<String, dynamic>{
      'type': 'refreshFolders',
      'accountId': accountId,
    }),
  );
}

Future<void> sessionTransferMessages({
  required String sourceAccountId,
  required String sourceFolder,
  required String destAccountId,
  required String destFolder,
  required List<String> messageIds,
  required bool isMove,
}) {
  return frbSessionCommand(
    commandJson: jsonEncode(<String, dynamic>{
      'type': 'transferMessages',
      'sourceAccountId': sourceAccountId,
      'sourceFolder': sourceFolder,
      'destAccountId': destAccountId,
      'destFolder': destFolder,
      'messageIds': messageIds,
      'isMove': isMove,
    }),
  );
}
