/*
 * session_state.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * Consumes Rust [frbSessionStart] events into per-account mail metadata (folders, unreads).
 */

import 'dart:async';

import 'package:flutter/foundation.dart' show immutable;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../rust/frb_api.dart';
import '../rust/session/commands.dart';
import '../rust/session/events.dart';
import '../rust/tagliacarte_api.dart';
import '../models/subscription_folder_row.dart';
import '../util/mail_account_policy.dart';
import '../util/process_log.dart';
import 'app_state.dart';
import 'nostr_peer_labels.dart';

/// Fan-out for windowed message-list events (see [messageListSessionEventStream]).
final StreamController<AppEvent> _messageListSessionEventController =
    StreamController<AppEvent>.broadcast();

/// `messageListWindowStarted` / `messageListRowFound` / `messageListWindowComplete` from Rust.
Stream<AppEvent> get messageListSessionEventStream =>
    _messageListSessionEventController.stream;

void _emitMessageListSessionEvent(AppEvent m) {
  if (!_messageListSessionEventController.isClosed) {
    _messageListSessionEventController.add(m);
  }
}

/// [nostrProfileUpdated] for chat/message rows and folder labels.
final StreamController<AppEvent> _nostrProfileSessionEventController =
    StreamController<AppEvent>.broadcast();

Stream<AppEvent> get nostrProfileSessionEventStream =>
    _nostrProfileSessionEventController.stream;

void _emitNostrProfileSessionEvent(AppEvent m) {
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
    this.folderDisplayLabels = const <String, String>{},
    this.subscriptionAvailable = const <SubscriptionFolderRow>[],
    this.hierarchyDelimiter,
    this.connection = MailConnectionState.idle,
    this.connectionMessage,
    this.storeKind = 'email',
  });

  final List<String> folders;
  final Map<String, int> unreadByFolder;
  final Map<String, String> folderDisplayLabels;
  final List<SubscriptionFolderRow> subscriptionAvailable;
  final String? hierarchyDelimiter;
  final MailConnectionState connection;
  final String? connectionMessage;
  /// `email` | `nostr` | `matrix` from Rust session events.
  final String storeKind;

  AccountMailModel copyWith({
    List<String>? folders,
    Map<String, int>? unreadByFolder,
    Map<String, String>? folderDisplayLabels,
    List<SubscriptionFolderRow>? subscriptionAvailable,
    String? hierarchyDelimiter,
    MailConnectionState? connection,
    String? connectionMessage,
    String? storeKind,
  }) {
    return AccountMailModel(
      folders: folders ?? this.folders,
      unreadByFolder: unreadByFolder ?? this.unreadByFolder,
      folderDisplayLabels: folderDisplayLabels ?? this.folderDisplayLabels,
      subscriptionAvailable: subscriptionAvailable ?? this.subscriptionAvailable,
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
          final Set<String> validIds =
              cfg.accounts.map((AppAccount a) => a.id).toSet();
          final Map<String, AccountMailModel> pruned =
              Map<String, AccountMailModel>.from(state);
          pruned.removeWhere((String k, _) => !validIds.contains(k));
          if (pruned.length != state.length) {
            state = pruned;
          }
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
  StreamSubscription<AppEvent>? _sub;
  bool _started = false;

  Future<void> _subscribe() async {
    if (_started) {
      return;
    }
    _started = true;
    try {
      final String path = await ref.read(tagliacarteApiProvider).configXmlPath();
      _sub = frbSessionStart(configXmlPath: path).listen(
        _onAppEvent,
        onError: (Object e, StackTrace st) {
          appLogStderr('mail session stream error: $e\n$st');
        },
      );
    } catch (e, st) {
      appLogStderr('mail session start failed: $e\n$st');
    }
  }

  void _onAppEvent(AppEvent e) {
    try {
      e.when(
        accountConnectionChanged:
            (String accountId, String storeKind, String connectionState, String? message) {
          _onConnection(accountId, connectionState, message, storeKind);
        },
        folderFound:
            (String accountId, String folderName, int unread) {
          _onFolderFound(accountId, folderName, unread);
        },
        folderListUpdated:
            (
              String accountId,
              List<String> folders,
              String? hierarchyDelimiter,
              Map<String, int> unreadByFolder,
              Map<String, String> folderDisplayNames,
              List<SubscriptionAvailableRow>? subscriptionAvailable,
            ) {
          _onFolderList(
            accountId,
            folders,
            unreadByFolder,
            hierarchyDelimiter,
            folderDisplayNames,
            subscriptionAvailable,
          );
        },
        messageFlagsChanged:
            (String accountId, String folder, String messageId, bool isRead) {
          // Reserved for future UI (read state sync).
        },
        messageListWindowStarted:
            (
              String requestId,
              String accountId,
              String folderName,
              String messageListSort,
              BigInt total,
              BigInt startIndex,
              String listStrategy,
              int rowCount,
              bool listReady,
            ) {
          _emitMessageListSessionEvent(e);
        },
        messageListRowFound:
            (
              String requestId,
              String accountId,
              String folderName,
              String messageListSort,
              BigInt rank,
              MessageListRowSummary summary,
            ) {
          _emitMessageListSessionEvent(e);
        },
        messageListWindowComplete:
            (
              String requestId,
              String accountId,
              String folderName,
              String messageListSort,
              String? error,
            ) {
          _emitMessageListSessionEvent(e);
        },
        commandResult: (String? requestId, bool ok, String? error) {},
        nostrProfileUpdated:
            (
              String accountId,
              String pubkeyHex,
              String npub,
              String? displayName,
              String? nip05,
              String? picture,
            ) {
          ref.read(nostrPeerLabelsProvider.notifier).applyProfileEventApp(e);
          _emitNostrProfileSessionEvent(e);
        },
      );
    } catch (e, st) {
      appLogStderr('mail session event handling failed: $e\n$st');
    }
  }

  void _onConnection(
    String id,
    String connectionStateRaw,
    String? message,
    String storeKindRaw,
  ) {
    final String cs = connectionStateRaw.toLowerCase().trim();
    final String? msg = message;
    final String storeKind = storeKindRaw.toLowerCase().trim();
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
              folderDisplayLabels: const <String, String>{},
              subscriptionAvailable: const <SubscriptionFolderRow>[],
            )
          : prev.copyWith(
              connection: st,
              connectionMessage: msg,
              storeKind: storeKind.isEmpty ? null : storeKind,
            ),
    };
  }

  void _onFolderFound(String id, String name, int unread) {
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

  void _onFolderList(
    String id,
    List<String> folders,
    Map<String, int> unreadByFolder,
    String? hierarchyDelimiter,
    Map<String, String> folderDisplayNames,
    List<SubscriptionAvailableRow>? subscriptionAvailableRaw,
  ) {
    final Map<String, String> displayLabels = <String, String>{};
    folderDisplayNames.forEach((String k, String v) {
      if (v.trim().isNotEmpty) {
        displayLabels[k.trim().toLowerCase()] = v.trim();
      }
    });
    final List<SubscriptionFolderRow> subAvail = subscriptionAvailableRaw
            ?.map(
              (SubscriptionAvailableRow r) => SubscriptionFolderRow(
                id: r.id,
                isSubscribed: r.isSubscribed,
                displayName: r.displayName,
                unread: r.unread,
                allowUnsubscribe: r.allowUnsubscribe,
              ),
            )
            .toList() ??
        const <SubscriptionFolderRow>[];
    applyFolderListFromSession(
      id,
      folders: folders,
      unreadByFolder: unreadByFolder,
      hierarchyDelimiter: hierarchyDelimiter,
      folderDisplayLabels: displayLabels,
      subscriptionAvailable: subAvail,
    );
  }

  /// Same shape as [ _onFolderList ] but for a direct `frb_list_mail_folders` result
  /// (e.g. before the session stream has emitted for this account).
  void applyFolderListFromListCall({
    required String accountId,
    required List<String> folders,
    Map<String, int> unreadByFolder = const <String, int>{},
    Map<String, String> folderDisplayLabels = const <String, String>{},
    List<SubscriptionFolderRow> subscriptionAvailable =
        const <SubscriptionFolderRow>[],
    String? hierarchyDelimiter,
  }) {
    applyFolderListFromSession(
      accountId,
      folders: folders,
      unreadByFolder: unreadByFolder,
      hierarchyDelimiter: hierarchyDelimiter,
      folderDisplayLabels: folderDisplayLabels,
      subscriptionAvailable: subscriptionAvailable,
    );
  }

  void applyFolderListFromSession(
    String accountId, {
    required List<String> folders,
    Map<String, int> unreadByFolder = const <String, int>{},
    Map<String, String> folderDisplayLabels = const <String, String>{},
    List<SubscriptionFolderRow> subscriptionAvailable =
        const <SubscriptionFolderRow>[],
    String? hierarchyDelimiter,
  }) {
    final AccountMailModel prev = state[accountId] ?? const AccountMailModel();
    state = <String, AccountMailModel>{
      ...state,
      accountId: prev.copyWith(
        folders: folders,
        unreadByFolder: unreadByFolder,
        folderDisplayLabels: folderDisplayLabels,
        subscriptionAvailable: subscriptionAvailable,
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
    folderDisplayLabels: m.folderDisplayLabels,
    subscriptionAvailable: m.subscriptionAvailable,
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
    command: AppCommand.markRead(
      accountId: accountId,
      folder: folder,
      messageId: messageId,
    ),
  );
}

Future<void> sessionRefreshFolders({required String accountId}) {
  return frbSessionCommand(
    command: AppCommand.refreshFolders(accountId: accountId),
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
  int? visibleFirstRank,
  int? visibleLastRank,
}) {
  return frbSessionCommand(
    command: AppCommand.listMessagesWindow(
      accountId: accountId,
      folderName: folderName,
      startIndex: BigInt.from(startIndex),
      limit: BigInt.from(limit),
      messageListSort: messageListSort,
      requestId: requestId,
      listReady: listReady,
      visibleFirstRank: visibleFirstRank == null
          ? null
          : BigInt.from(visibleFirstRank),
      visibleLastRank:
          visibleLastRank == null ? null : BigInt.from(visibleLastRank),
    ),
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
    command: AppCommand.transferMessages(
      sourceAccountId: sourceAccountId,
      sourceFolder: sourceFolder,
      destAccountId: destAccountId,
      destFolder: destFolder,
      messageIds: messageIds,
      isMove: isMove,
    ),
  );
}
