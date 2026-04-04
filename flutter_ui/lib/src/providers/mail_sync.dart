/*
 * mail_sync.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

import 'dart:convert';

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/message_row.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
export '../util/native_mail_uri.dart';

import '../util/native_mail_uri.dart';
import '../util/new_mail_merge.dart';
import 'app_state.dart';
import 'message_sort_persist.dart';
import 'session_state.dart';

/// Caches loopback mail body server init + per-store registration keys for WebView HTML URLs.
class MailBodyServerCache {
  MailBodyServerCache._();
  static bool _inited = false;
  static final Map<String, String> _storeKeyByAccount = <String, String>{};

  static Future<void> ensureInit() async {
    if (_inited) {
      return;
    }
    await frbMailBodyServerInit();
    _inited = true;
  }

  static Future<String> storeKeyForAccount(String accountId) async {
    await ensureInit();
    final String k = accountId.trim();
    final String? existing = _storeKeyByAccount[k];
    if (existing != null) {
      return existing;
    }
    final String sk =
        await frbSessionRegisterMailBodyStore(accountId: k);
    _storeKeyByAccount[k] = sk;
    return sk;
  }
}

bool isImapStoreUri(String uri) {
  return uri.startsWith('imap://') || uri.startsWith('imaps://');
}

/// FRB JSON uses camelCase; accept snake_case for older payloads.
int? _jsonDateMs(Map<String, dynamic> m) {
  final num? n = m['dateMs'] as num? ?? m['date_ms'] as num?;
  return n?.toInt();
}

/// Matches [frb_mail] error when no row exists for this store in credentials.
bool isMissingImapCredentialsError(Object e) {
  return e.toString().contains('no saved password for this IMAP account');
}

/// Message list / chat timeline: session resolves store + credentials from [accountId].
@immutable
class SessionFolderParams {
  const SessionFolderParams({
    required this.accountId,
    required this.folderName,
    required this.messageListSort,
  });

  final String accountId;
  final String folderName;
  /// Flutter `messageListSort` token (snake_case), e.g. [kMessageListSortDateDesc].
  final String messageListSort;

  @override
  bool operator ==(Object other) =>
      other is SessionFolderParams &&
      accountId == other.accountId &&
      folderName == other.folderName &&
      messageListSort == other.messageListSort;

  @override
  int get hashCode => Object.hash(accountId, folderName, messageListSort);
}

/// Sparse folder list: [slots] aligned to **oldest-first** mailbox order (index 0 = oldest).
@immutable
class FolderListVm {
  const FolderListVm({
    required this.totalCount,
    required this.slots,
    required this.ready,
    this.error,
  });

  final int totalCount;
  final List<MessageListRow?> slots;
  final bool ready;
  final Object? error;

  bool containsId(String id) {
    for (final MessageListRow? r in slots) {
      if (r?.id == id) {
        return true;
      }
    }
    return false;
  }

  MessageListRow? rowAtDataIndex(int i) {
    if (i < 0 || i >= slots.length) {
      return null;
    }
    return slots[i];
  }

  MessageListRow? rowById(String id) {
    for (final MessageListRow? r in slots) {
      if (r?.id == id) {
        return r;
      }
    }
    return null;
  }

  int? dataIndexOf(String id) {
    for (int i = 0; i < slots.length; i++) {
      if (slots[i]?.id == id) {
        return i;
      }
    }
    return null;
  }
}

MessageListRow _messageListRowFromSummaryJson(Map<String, dynamic> m) {
  final int? ms = _jsonDateMs(m);
  return MessageListRow(
    id: m['id'] as String,
    from: m['from'] as String? ?? '',
    subject: m['subject'] as String? ?? '',
    date: ms != null
        ? DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true).toLocal()
        : DateTime.fromMillisecondsSinceEpoch(0, isUtc: true),
    isRead: m['isRead'] as bool? ?? m['is_read'] as bool? ?? true,
    markedForDeletion: m['markedForDeletion'] as bool? ?? false,
  );
}

/// Windowed folder loading: fetches only the ranges needed for the viewport (+ prefetch margin).
final folderMailboxListProvider = NotifierProvider.autoDispose
    .family<FolderMailboxListNotifier, FolderListVm, SessionFolderParams>(
      FolderMailboxListNotifier.new,
    );

class FolderMailboxListNotifier
    extends AutoDisposeFamilyNotifier<FolderListVm, SessionFolderParams> {
  static const int kPageSize = 48;
  static const int kPrefetchExtra = 36;

  Future<void> _opQueue = Future<void>.value();
  final Set<String> _ensureInFlight = <String>{};

  bool _newMailBaselineDone = false;
  int _newMailRefTotal = 0;
  final Set<String> _newMailKnownIds = <String>{};

  @override
  FolderListVm build(SessionFolderParams arg) {
    ref.keepAlive();
    Future<void>.microtask(_bootstrap);
    return const FolderListVm(totalCount: 0, slots: <MessageListRow?>[], ready: false);
  }

  Future<void> _runQueued(Future<void> Function() op) {
    final Future<void> f = _opQueue.then((_) => op());
    _opQueue = f.catchError((Object _) {});
    return f;
  }

  Future<void> _bootstrap() async {
    await _runQueued(() async {
      await _fetchWindowImpl(0, kPageSize, listReady: false);
      final int t = state.totalCount;
      if (t > kPageSize) {
        await _fetchWindowImpl(t - kPageSize, kPageSize, listReady: true);
      } else {
        state = FolderListVm(
          totalCount: state.totalCount,
          slots: state.slots,
          ready: true,
          error: state.error,
        );
      }
    });
  }

  /// Fetch summaries for oldest-first indices `[startIndex, startIndex + limit)`.
  Future<void> fetchWindow(int startIndex, int limit) =>
      _runQueued(() => _fetchWindowImpl(startIndex, limit));

  Future<void> _fetchWindowImpl(
    int startIndex,
    int limit, {
    bool listReady = true,
  }) async {
    final SessionFolderParams p = arg;
    if (limit <= 0) {
      return;
    }
    try {
      final String jsonStr = await frbSessionListMessagesWindow(
        accountId: p.accountId,
        folderName: p.folderName,
        startIndex: startIndex,
        limit: limit,
        messageListSort: p.messageListSort,
      );
      final Map<String, dynamic> decoded =
          jsonDecode(jsonStr) as Map<String, dynamic>;
      final int total = (decoded['total'] as num).toInt();
      final int si = (decoded['startIndex'] as num).toInt();
      final List<dynamic> raw =
          decoded['messages'] as List<dynamic>? ?? <dynamic>[];
      final List<MessageListRow> rows = raw.map((dynamic e) {
        return _messageListRowFromSummaryJson(e as Map<String, dynamic>);
      }).toList();
      state = _mergeInto(
        state,
        total: total,
        startIndex: si,
        rows: rows,
        listReady: listReady,
      );
      _afterFolderMerge(afterMerge: state, listReady: listReady);
    } catch (e, st) {
      state = FolderListVm(
        totalCount: state.totalCount,
        slots: state.slots,
        ready: true,
        error: e,
      );
      debugPrint('folderMailboxList fetch failed: $e\n$st');
    }
  }

  FolderListVm _mergeInto(
    FolderListVm prev, {
    required int total,
    required int startIndex,
    required List<MessageListRow> rows,
    bool listReady = true,
  }) {
    List<MessageListRow?> next;
    if (prev.slots.length != total) {
      next = List<MessageListRow?>.filled(total, null);
      for (int i = 0; i < prev.slots.length && i < total; i++) {
        next[i] = prev.slots[i];
      }
    } else {
      next = List<MessageListRow?>.from(prev.slots);
    }
    for (int i = 0; i < rows.length; i++) {
      final int ix = startIndex + i;
      if (ix >= 0 && ix < next.length) {
        next[ix] = rows[i];
      }
    }
    return FolderListVm(totalCount: total, slots: next, ready: listReady);
  }

  void _afterFolderMerge({
    required FolderListVm afterMerge,
    required bool listReady,
  }) {
    if (afterMerge.error != null || !listReady) {
      return;
    }
    final SessionFolderParams p = arg;
    final String? sk =
        ref.read(accountMailModelsProvider)[p.accountId]?.storeKind;
    if (sk != null && sk != 'email') {
      return;
    }
    final Set<String> idsNow = <String>{
      for (final MessageListRow? r in afterMerge.slots)
        if (r != null) r.id,
    };
    if (!_newMailBaselineDone) {
      _newMailBaselineDone = true;
      _newMailRefTotal = afterMerge.totalCount;
      _newMailKnownIds
        ..clear()
        ..addAll(idsNow);
      return;
    }
    final int prevRefTotal = _newMailRefTotal;
    final Set<String> prevKnownIds = Set<String>.from(_newMailKnownIds);
    final ({bool shouldNotify, int countHint}) detected = detectNewMailAfterMerge(
      listReady: true,
      baselineEstablished: true,
      previousRefTotal: prevRefTotal,
      previousKnownIds: prevKnownIds,
      mergedTotal: afterMerge.totalCount,
      mergedIdsFromSlots: idsNow,
    );
    if (afterMerge.totalCount < prevRefTotal) {
      _newMailKnownIds.retainWhere(idsNow.contains);
    }
    _newMailRefTotal = afterMerge.totalCount;
    _newMailKnownIds.addAll(idsNow);
    if (!detected.shouldNotify) {
      return;
    }
    final bool totalIncreased = afterMerge.totalCount > prevRefTotal;
    if (!totalIncreased) {
      bool anyUnreadNew = false;
      for (final String id in idsNow) {
        if (!prevKnownIds.contains(id)) {
          final MessageListRow? r = afterMerge.rowById(id);
          if (r != null && !r.isRead) {
            anyUnreadNew = true;
            break;
          }
        }
      }
      if (!anyUnreadNew) {
        return;
      }
    }
    final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
    if (cfg?.notifyNewMessages != true) {
      return;
    }
    String accountLabel = '';
    if (cfg != null) {
      for (final AppAccount a in cfg.accounts) {
        if (a.id == p.accountId) {
          accountLabel = a.label;
          break;
        }
      }
    }
    ref.read(newMailToastSignalProvider.notifier).state = NewMailToastSignal(
      accountLabel: accountLabel.isNotEmpty ? accountLabel : p.folderName,
      folderName: p.folderName,
      countHint: detected.countHint,
    );
  }

  /// Load any missing rows in **[dataLo, dataHi]** (inclusive), expanded by [kPrefetchExtra].
  Future<void> requestVisibleDataRange(int dataLo, int dataHi) async {
    final int t = state.totalCount;
    if (t <= 0) {
      return;
    }
    int a = (dataLo - kPrefetchExtra).clamp(0, t - 1);
    int b = (dataHi + kPrefetchExtra).clamp(0, t - 1);
    if (a > b) {
      return;
    }
    bool anyMissing = false;
    for (int i = a; i <= b; i++) {
      if (state.slots[i] == null) {
        anyMissing = true;
        break;
      }
    }
    if (!anyMissing) {
      return;
    }
    final int span = b - a + 1;
    final int lim = span.clamp(1, kPageSize * 3);
    await fetchWindow(a, lim);
  }

  /// Scan pages until [id] is loaded (recovery when selection is not in cache).
  Future<void> ensureMessageIdLoaded(String id) async {
    if (state.containsId(id) || state.totalCount == 0) {
      return;
    }
    if (_ensureInFlight.contains(id)) {
      return;
    }
    _ensureInFlight.add(id);
    try {
      for (int s = 0; s < state.totalCount; s += kPageSize) {
        await fetchWindow(s, kPageSize);
        if (state.containsId(id)) {
          return;
        }
      }
    } finally {
      _ensureInFlight.remove(id);
    }
  }

  void markMessageRead(String id) {
    final List<MessageListRow?> next = List<MessageListRow?>.from(state.slots);
    bool changed = false;
    for (int i = 0; i < next.length; i++) {
      final MessageListRow? r = next[i];
      if (r != null && r.id == id && !r.isRead) {
        next[i] = MessageListRow(
          id: r.id,
          from: r.from,
          subject: r.subject,
          date: r.date,
          isRead: true,
          markedForDeletion: r.markedForDeletion,
        );
        changed = true;
      }
    }
    if (changed) {
      state = FolderListVm(
        totalCount: state.totalCount,
        slots: next,
        ready: state.ready,
        error: state.error,
      );
    }
  }
}

/// Same [SessionFolderParams] family key as [folderMailboxListProvider] / message list.
SessionFolderParams sessionFolderParamsMatchingList(
  WidgetRef ref,
  MailMessageDetailParams detail,
) {
  final MessageSortField field = ref.read(messageSortFieldProvider);
  final bool asc = ref.read(messageSortAscendingProvider);
  return SessionFolderParams(
    accountId: detail.accountId,
    folderName: detail.folderName,
    messageListSort: messageListSortSymbolic(field, asc),
  );
}

/// After message body loads in the detail pane, set \\Seen on the server and refresh local state.
Future<void> markMessageReadAfterDetailLoaded(
  WidgetRef ref,
  MailMessageDetailParams detail, {
  String? accountIdOverride,
}) async {
  final String accId =
      accountIdOverride ?? detail.accountId;
  final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
  AppAccount? account;
  for (final AppAccount a in cfg?.accounts ?? const <AppAccount>[]) {
    if (a.id == accId) {
      account = a;
      break;
    }
  }
  if (account == null || !isSessionBackedStoreUri(account.storeUri)) {
    return;
  }
  final SessionFolderParams fp = sessionFolderParamsMatchingList(ref, detail);
  // mbox has no \\Seen on disk; summaries are always read. Skip FRB store_flags.
  if (account.storeUri.startsWith('mbox:')) {
    ref.read(folderMailboxListProvider(fp).notifier).markMessageRead(detail.messageId);
    return;
  }
  final FolderListVm vm = ref.read(folderMailboxListProvider(fp));
  final MessageListRow? row = vm.rowById(detail.messageId);
  if (row != null && row.isRead) {
    return;
  }

  unawaited(
    sessionMarkRead(
      accountId: accId,
      folder: detail.folderName,
      messageId: detail.messageId,
    ),
  );
  ref.read(folderMailboxListProvider(fp).notifier).markMessageRead(detail.messageId);
}

@immutable
class MailMessageDetailParams {
  const MailMessageDetailParams({
    required this.accountId,
    required this.folderName,
    required this.messageId,
  });

  final String accountId;
  final String folderName;
  final String messageId;

  @override
  bool operator ==(Object other) =>
      other is MailMessageDetailParams &&
      accountId == other.accountId &&
      folderName == other.folderName &&
      messageId == other.messageId;

  @override
  int get hashCode => Object.hash(accountId, folderName, messageId);
}

@immutable
class MailAttachmentDetail {
  const MailAttachmentDetail({
    this.filename,
    required this.contentType,
    required this.sizeBytes,
    required this.transferEncoding,
    this.imapSection,
    this.contentId,
    this.inlineData,
  });

  final String? filename;
  final String contentType;
  final int sizeBytes;
  final String transferEncoding;
  final String? imapSection;
  final String? contentId;
  final Uint8List? inlineData;

  static MailAttachmentDetail? tryParse(Object? e) {
    if (e is! Map<String, dynamic>) {
      return null;
    }
    final Map<String, dynamic> m = e;
    final String? sec =
        m['imapSection'] as String? ?? m['imap_section'] as String?;
    final String? cid =
        m['contentId'] as String? ?? m['content_id'] as String?;
    final String? b64 =
        m['dataBase64'] as String? ?? m['data_base64'] as String?;
    Uint8List? data;
    if (b64 != null && b64.isNotEmpty) {
      try {
        data = base64Decode(b64);
      } catch (_) {
        data = null;
      }
    }
    return MailAttachmentDetail(
      filename: m['filename'] as String?,
      contentType: m['contentType'] as String? ??
          m['content_type'] as String? ??
          'application/octet-stream',
      sizeBytes: (m['sizeBytes'] as num?)?.toInt() ??
          (m['size_bytes'] as num?)?.toInt() ??
          0,
      transferEncoding: m['transferEncoding'] as String? ??
          m['transfer_encoding'] as String? ??
          '',
      imapSection: sec,
      contentId: cid,
      inlineData: data,
    );
  }
}

@immutable
class MailMessageDetailView {
  const MailMessageDetailView({
    required this.subject,
    required this.fromRaw,
    required this.toRaw,
    this.ccRaw,
    this.dateMs,
    this.bodyPlain,
    this.bodyHtml,
    this.attachments = const [],
    this.mailBodyStoreKey,
  });

  final String subject;
  final String fromRaw;
  final String toRaw;
  final String? ccRaw;

  /// UTC epoch ms from the envelope; formatted in the UI from the system locale.
  final int? dateMs;
  final String? bodyPlain;
  final String? bodyHtml;
  final List<MailAttachmentDetail> attachments;

  /// When set, HTML body is shown via loopback HTTPS WebView ([frbMailBodyMessageUrl]).
  final String? mailBodyStoreKey;

  factory MailMessageDetailView.fromJson(Map<String, dynamic> m) {
    final List<dynamic>? raw = m['attachments'] as List<dynamic>?;
    final List<MailAttachmentDetail> atts = raw == null
        ? const []
        : raw
            .map(MailAttachmentDetail.tryParse)
            .whereType<MailAttachmentDetail>()
            .toList();
    return MailMessageDetailView(
      subject: m['subject'] as String? ?? '',
      fromRaw: m['from'] as String? ?? '',
      toRaw: m['to'] as String? ?? '',
      ccRaw: m['cc'] as String?,
      dateMs: _jsonDateMs(m),
      bodyPlain: m['bodyPlain'] as String? ?? m['body_plain'] as String?,
      bodyHtml: m['bodyHtml'] as String? ?? m['body_html'] as String?,
      attachments: atts,
    );
  }
}

final mailMessageDetailProvider = FutureProvider.autoDispose
    .family<MailMessageDetailView, MailMessageDetailParams>((Ref ref, p) async {
      final json = await frbSessionGetFolderMessage(
        accountId: p.accountId,
        folderName: p.folderName,
        messageId: p.messageId,
      );
      final MailMessageDetailView view = MailMessageDetailView.fromJson(
        jsonDecode(json) as Map<String, dynamic>,
      );
      final String? html = view.bodyHtml?.trim();
      final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
      AppAccount? acc;
      for (final AppAccount a in cfg?.accounts ?? const <AppAccount>[]) {
        if (a.id == p.accountId) {
          acc = a;
          break;
        }
      }
      if (html != null &&
          html.isNotEmpty &&
          acc != null &&
          isNativeMailStoreUri(acc.storeUri)) {
        try {
          final String sk =
              await MailBodyServerCache.storeKeyForAccount(p.accountId);
          return MailMessageDetailView(
            subject: view.subject,
            fromRaw: view.fromRaw,
            toRaw: view.toRaw,
            ccRaw: view.ccRaw,
            dateMs: view.dateMs,
            bodyPlain: view.bodyPlain,
            bodyHtml: view.bodyHtml,
            attachments: view.attachments,
            mailBodyStoreKey: sk,
          );
        } catch (_) {
          return view;
        }
      }
      return view;
    });
