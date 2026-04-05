/*
 * session_state.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * Consumes Rust [frbSessionStart] events into per-account mail metadata (folders, unreads).
 */

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart' show immutable;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../util/mail_account_policy.dart';
import '../util/process_log.dart';
import 'app_state.dart';
import 'nostr_peer_labels.dart';

/// Fan-out for windowed message-list events (see [messageListSessionEventStream]).
final StreamController<Map<String, dynamic>> _messageListSessionEventController =
    StreamController<Map<String, dynamic>>.broadcast();

/// `messageListWindowStarted` / `messageListRowFound` / `messageListWindowComplete` from Rust.
Stream<Map<String, dynamic>> get messageListSessionEventStream =>
    _messageListSessionEventController.stream;

void _emitMessageListSessionEvent(Map<String, dynamic> m) {
  if (!_messageListSessionEventController.isClosed) {
    _messageListSessionEventController.add(m);
  }
}

/// [nostrProfileUpdated] for chat/message rows and folder labels.
final StreamController<Map<String, dynamic>> _nostrProfileSessionEventController =
    StreamController<Map<String, dynamic>>.broadcast();

Stream<Map<String, dynamic>> get nostrProfileSessionEventStream =>
    _nostrProfileSessionEventController.stream;

void _emitNostrProfileSessionEvent(Map<String, dynamic> m) {
  if (!_nostrProfileSessionEventController.isClosed) {
    _nostrProfileSessionEventController.add(m);
  }
}

/// Per-account folder list + unread counts + connection info from the Rust session.
@immutable
class AccountMailModel {
  const AccountMailModel({
    this.folders = const <String>[],
    this.unreadByFolder = const <String, int>{},
    this.hierarchyDelimiter,
    this.connection = MailConnectionState.idle,
    this.connectionMessage,
    this.storeKind = 'email',
  });

  final List<String> folders;
  final Map<String, int> unreadByFolder;
  final String? hierarchyDelimiter;
  final MailConnectionState connection;
  final String? connectionMessage;
  /// `email` | `nostr` | `matrix` from Rust session events.
  final String storeKind;

  AccountMailModel copyWith({
    List<String>? folders,
    Map<String, int>? unreadByFolder,
    String? hierarchyDelimiter,
    MailConnectionState? connection,
    String? connectionMessage,
    String? storeKind,
  }) {
    return AccountMailModel(
      folders: folders ?? this.folders,
      unreadByFolder: unreadByFolder ?? this.unreadByFolder,
      hierarchyDelimiter: hierarchyDelimiter ?? this.hierarchyDelimiter,
      connection: connection ?? this.connection,
      connectionMessage: connectionMessage ?? this.connectionMessage,
      storeKind: storeKind ?? this.storeKind,
    );
  }

  bool get isConversationKind =>
      storeKind == 'nostr' || storeKind == 'matrix';
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
    ref.listen<AsyncValue<AppSettingsConfig>>(
      accountsConfigProvider,
      (AsyncValue<AppSettingsConfig>? previous, AsyncValue<AppSettingsConfig> next) {
        next.whenData((AppSettingsConfig cfg) {
          for (final AppAccount a in cfg.accounts) {
            if (a.backendType.toLowerCase().trim() != 'nostr') {
              continue;
            }
            final List<String> folders = state[a.id]?.folders ?? const <String>[];
            if (folders.isEmpty) {
              continue;
            }
            unawaited(
              ref.read(nostrPeerLabelsProvider.notifier).primeNpubLabels(folders),
            );
          }
        });
      },
    );
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
          appLogStderr('mail session stream error: $e\n$st');
        },
      );
    } catch (e, st) {
      appLogStderr('mail session start failed: $e\n$st');
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
        case 'folderFound':
          _onFolderFound(m);
          break;
        case 'folderListUpdated':
          _onFolderList(m);
          break;
        case 'messageListWindowStarted':
        case 'messageListRowFound':
        case 'messageListWindowComplete':
          _emitMessageListSessionEvent(m);
          break;
        case 'nostrProfileUpdated':
          ref.read(nostrPeerLabelsProvider.notifier).applyProfileEvent(m);
          _emitNostrProfileSessionEvent(m);
          break;
        case 'messageFlagsChanged':
        case 'commandResult':
          break;
      }
    } catch (e, st) {
      appLogStderr('mail session event parse failed: $e\n$st raw=$raw');
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
    final String storeKind =
        (m['storeKind'] as String? ?? 'email').toLowerCase().trim();
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
      id: st == MailConnectionState.error
          ? prev.copyWith(
              connection: st,
              connectionMessage: msg,
              storeKind: storeKind.isEmpty ? null : storeKind,
              folders: const <String>[],
              unreadByFolder: const <String, int>{},
            )
          : prev.copyWith(
              connection: st,
              connectionMessage: msg,
              storeKind: storeKind.isEmpty ? null : storeKind,
            ),
    };
  }

  void _onFolderFound(Map<String, dynamic> m) {
    final String? id = m['accountId'] as String?;
    final String? name = m['folderName'] as String?;
    if (id == null || name == null) {
      return;
    }
    final int unread = (m['unread'] as num?)?.toInt() ?? 0;
    final AccountMailModel prev = state[id] ?? const AccountMailModel();
    final List<String> folders = List<String>.from(prev.folders);
    final Map<String, int> unreadMap = Map<String, int>.from(prev.unreadByFolder);
    if (!folders.contains(name)) {
      folders.add(name);
    }
    unreadMap[name] = unread;
    state = <String, AccountMailModel>{
      ...state,
      id: prev.copyWith(
        folders: folders,
        unreadByFolder: unreadMap,
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
    primeNostrFolderLabelsIfNeeded(
      ref,
      accountId,
      folders,
      sessionSaysNostr: prev.storeKind == 'nostr',
    );
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

/// Session mail model for [selectedAccountIdProvider], if any.
final selectedAccountMailModelProvider = Provider<AccountMailModel?>((Ref ref) {
  final String? id = ref.watch(selectedAccountIdProvider);
  if (id == null) {
    return null;
  }
  return ref.watch(accountMailModelsProvider)[id];
});

/// Folders + unreads for the currently selected account (Rust session only).
final foldersProvider = Provider<MailFoldersState>((Ref ref) {
  final String? id = ref.watch(selectedAccountIdProvider);
  if (id == null) {
    return const MailFoldersState();
  }
  final AppSettingsConfig? cfg = ref.watch(accountsConfigProvider).valueOrNull;
  bool found = false;
  for (final AppAccount a in cfg?.accounts ?? const <AppAccount>[]) {
    if (a.id == id) {
      found = true;
      break;
    }
  }
  if (!found) {
    return const MailFoldersState();
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

/// INBOX unread per email account (dock badge; Nostr/Matrix omit).
final nativeAccountInboxUnreadProvider = Provider<Map<String, int>>((Ref ref) {
  final Map<String, AccountMailModel> m = ref.watch(accountMailModelsProvider);
  final Map<String, int> out = <String, int>{};
  for (final MapEntry<String, AccountMailModel> e in m.entries) {
    if (e.value.storeKind != 'email') {
      continue;
    }
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

/// Nostr/Matrix-style folder = chat; use conversation pane instead of mail list + detail.
///
/// Prefer session [AccountMailModel.storeKind] when set; also trust [AppAccount.backendType] from
/// config so we still use [ChatView] if `accountConnectionChanged` was missed (e.g. broadcast lag)
/// or [storeKind] never arrived before folder list.
final selectedAccountConversationModeProvider = Provider<bool>((Ref ref) {
  final String? id = ref.watch(selectedAccountIdProvider);
  if (id == null) {
    return false;
  }
  if (ref.watch(accountMailModelsProvider)[id]?.isConversationKind ?? false) {
    return true;
  }
  final AppSettingsConfig? cfg = ref.watch(accountsConfigProvider).valueOrNull;
  for (final AppAccount a in cfg?.accounts ?? const <AppAccount>[]) {
    if (a.id == id && isConversationBackend(a)) {
      return true;
    }
  }
  return false;
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

/// Non-blocking message window fetch; rows arrive on [messageListSessionEventStream].
Future<void> sessionListMessagesWindowCommand({
  required String accountId,
  required String folderName,
  required int startIndex,
  required int limit,
  required String messageListSort,
  required String requestId,
  required bool listReady,
}) {
  return frbSessionCommand(
    commandJson: jsonEncode(<String, dynamic>{
      'type': 'listMessagesWindow',
      'accountId': accountId,
      'folderName': folderName,
      'startIndex': startIndex,
      'limit': limit,
      'messageListSort': messageListSort,
      'requestId': requestId,
      'listReady': listReady,
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
