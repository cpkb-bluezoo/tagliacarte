/*
 * message_list.dart
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

import 'dart:async' show Timer, unawaited;

import 'package:flutter/foundation.dart'
    show defaultTargetPlatform, kIsWeb, TargetPlatform;
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../models/mail_drag_data.dart';
import '../models/message_row.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../rust/tagliacarte_api.dart';
import '../util/mail_account_policy.dart';
import '../util/mailbox_format.dart';
import '../util/matrix_strings.dart';
import '../util/message_dates.dart';

/// Per visual row including trailing gap (viewport / jumpTo math).
/// Two text lines (from+date, subject); keep in sync with tile padding and typography.
const double kMessageListRowExtent = 64;
const double _kMessageListTileHeight = kMessageListRowExtent - 4;

class MessageList extends ConsumerStatefulWidget {
  const MessageList({
    super.key,
    required this.folderParams,
    required this.onOpen,
    this.enableDesktopMailDrag = false,
  });

  final SessionFolderParams folderParams;
  final ValueChanged<MessageListRow> onOpen;
  /// Desktop (macOS/Linux/Windows): drag selected row(s) to a folder for same-store transfer.
  final bool enableDesktopMailDrag;

  @override
  ConsumerState<MessageList> createState() => _MessageListState();
}

class _MessageListState extends ConsumerState<MessageList> {
  final ScrollController _scrollController = ScrollController();
  final GlobalKey _selectedRowKey = GlobalKey();
  Timer? _prefetchDebounce;

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);
    SchedulerBinding.instance.addPostFrameCallback((_) {
      _scrollToSelected(jump: true);
    });
  }

  @override
  void dispose() {
    _prefetchDebounce?.cancel();
    _scrollController.removeListener(_onScroll);
    _scrollController.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(MessageList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.folderParams != widget.folderParams) {
      SchedulerBinding.instance.addPostFrameCallback((_) {
        _scrollToSelected(jump: true);
      });
    }
  }

  void _onScroll() {
    if (!_scrollController.hasClients) {
      return;
    }
    _prefetchDebounce?.cancel();
    _prefetchDebounce = Timer(const Duration(milliseconds: 64), () {
      if (!mounted) {
        return;
      }
      final FolderListVm vm = ref.read(folderMailboxListProvider(widget.folderParams));
      if (vm.totalCount <= 0) {
        return;
      }
      final bool reverse = _reverseMailboxOrder(ref);
      final double o = _scrollController.offset;
      final double vp = _scrollController.position.viewportDimension;
      int firstVi = (o / kMessageListRowExtent).floor();
      int lastVi = ((o + vp) / kMessageListRowExtent).ceil();
      firstVi = firstVi.clamp(0, vm.totalCount - 1);
      lastVi = lastVi.clamp(0, vm.totalCount - 1);
      int dataLo = reverse ? vm.totalCount - 1 - lastVi : firstVi;
      int dataHi = reverse ? vm.totalCount - 1 - firstVi : lastVi;
      if (dataLo > dataHi) {
        final int t = dataLo;
        dataLo = dataHi;
        dataHi = t;
      }
      unawaited(
        ref
            .read(folderMailboxListProvider(widget.folderParams).notifier)
            .requestVisibleDataRange(dataLo, dataHi),
      );
    });
  }

  /// Descending sort: reverse list so the first row in ascending order appears at the bottom.
  bool _reverseMailboxOrder(WidgetRef ref) {
    final bool asc = ref.read(messageSortAscendingProvider);
    return !asc;
  }

  int _visualIndexForDataIndex(int di, int total, bool reverse) {
    return reverse ? (total - 1 - di) : di;
  }

  Future<void> _scrollToSelected({required bool jump}) async {
    if (!mounted) {
      return;
    }
    final bool multi = ref.read(mailMultiSelectActiveProvider);
    if (multi) {
      return;
    }
    final String? id = ref.read(selectedMessageProvider);
    if (id == null) {
      return;
    }
    final FolderListVm vm = ref.read(folderMailboxListProvider(widget.folderParams));
    if (vm.totalCount <= 0) {
      return;
    }
    int? di = vm.dataIndexOf(id);
    if (di == null) {
      await ref
          .read(folderMailboxListProvider(widget.folderParams).notifier)
          .ensureMessageIdLoaded(id);
      if (!mounted) {
        return;
      }
      di = ref.read(folderMailboxListProvider(widget.folderParams)).dataIndexOf(id);
    }
    if (di == null || !_scrollController.hasClients) {
      return;
    }
    final ScrollPosition pos = _scrollController.position;
    if (!pos.hasContentDimensions) {
      SchedulerBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          unawaited(_scrollToSelected(jump: jump));
        }
      });
      return;
    }
    final bool reverse = _reverseMailboxOrder(ref);
    final int vi = _visualIndexForDataIndex(di, vm.totalCount, reverse);
    final double target = (vi * kMessageListRowExtent).clamp(
      0.0,
      pos.maxScrollExtent,
    );
    if (jump) {
      _scrollController.jumpTo(target);
    } else {
      await _scrollController.animateTo(
        target,
        duration: const Duration(milliseconds: 220),
        curve: Curves.easeOutCubic,
      );
    }
    SchedulerBinding.instance.addPostFrameCallback((_) {
      _ensureSelectedVisible();
    });
  }

  void _ensureSelectedVisible() {
    if (!mounted) {
      return;
    }
    if (ref.read(mailMultiSelectActiveProvider)) {
      return;
    }
    final String? id = ref.read(selectedMessageProvider);
    if (id == null) {
      return;
    }
    final BuildContext? ctx = _selectedRowKey.currentContext;
    if (ctx == null) {
      return;
    }
    Scrollable.ensureVisible(
      ctx,
      alignment: 0.12,
      duration: const Duration(milliseconds: 200),
      curve: Curves.easeOutCubic,
    );
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final FolderListVm vm = ref.watch(folderMailboxListProvider(widget.folderParams));
    final bool multi = ref.watch(mailMultiSelectActiveProvider);
    final Set<String> selectedIds = ref.watch(mailSelectedIdsProvider);
    final String? primarySelected = ref.watch(selectedMessageProvider);
    final bool reverse = _reverseMailboxOrder(ref);
    final ColorScheme scheme = Theme.of(context).colorScheme;
    final Locale locale = Localizations.localeOf(context);
    final AppSettingsConfig? accCfg =
        ref.watch(accountsConfigProvider).valueOrNull;
    AppAccount? listAccount;
    if (accCfg != null) {
      for (final AppAccount a in accCfg.accounts) {
        if (a.id == widget.folderParams.accountId) {
          listAccount = a;
          break;
        }
      }
    }

    ref.listen<String?>(selectedMessageProvider, (String? previous, String? next) {
      if (previous == next) {
        return;
      }
      unawaited(_scrollToSelected(jump: false));
    });

    if (vm.error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Text(
            l10n.operationFailed(vm.error.toString()),
          ),
        ),
      );
    }

    if (vm.totalCount == 0 && vm.ready) {
      return Center(
        child: Text(
          l10n.noMessages,
          style: TextStyle(color: scheme.onSurfaceVariant),
        ),
      );
    }

    if (vm.totalCount == 0) {
      return const Center(child: CircularProgressIndicator());
    }

    return NotificationListener<ScrollNotification>(
      onNotification: (ScrollNotification n) {
        if (n is ScrollEndNotification) {
          _onScroll();
        }
        return false;
      },
      child: ListView.builder(
        controller: _scrollController,
        padding: const EdgeInsets.symmetric(vertical: 4),
        itemExtent: kMessageListRowExtent,
        itemCount: vm.totalCount,
        itemBuilder: (BuildContext context, int visualIndex) {
          final int di = reverse
              ? (vm.totalCount - 1 - visualIndex)
              : visualIndex;
          final MessageListRow? row = vm.rowAtDataIndex(di);

          if (row == null) {
            return Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: SizedBox(
                height: _kMessageListTileHeight,
                child: Material(
                  color: scheme.surfaceContainerLow,
                  borderRadius: BorderRadius.circular(8),
                  child: const Center(
                    child: SizedBox(
                      width: 22,
                      height: 22,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ),
                ),
              ),
            );
          }

          final bool isSelectedRow = multi
              ? selectedIds.contains(row.id)
              : primarySelected == row.id;

          Widget tile = Padding(
            padding: const EdgeInsets.only(bottom: 4),
            child: SizedBox(
              height: _kMessageListTileHeight,
              child: Material(
                key: !multi && primarySelected == row.id
                    ? _selectedRowKey
                    : ValueKey<String>(row.id),
                color: isSelectedRow
                    ? scheme.primaryContainer.withValues(alpha: 0.35)
                    : scheme.surfaceContainerLow,
                borderRadius: BorderRadius.circular(8),
                child: InkWell(
                  borderRadius: BorderRadius.circular(8),
                  onTap: () {
                    if (multi) {
                      ref.read(mailSelectedIdsProvider.notifier).toggle(row.id);
                    } else {
                      widget.onOpen(row);
                    }
                  },
                  onLongPress: () {
                    if (!multi) {
                      ref.read(mailMultiSelectActiveProvider.notifier).state =
                          true;
                      ref
                          .read(mailSelectedIdsProvider.notifier)
                          .enterWith(row.id);
                    }
                  },
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 6,
                    ),
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Row(
                          children: [
                            Expanded(
                              child: Text(
                                messageListSenderLine(row.from),
                                style: Theme.of(context)
                                    .textTheme
                                    .bodyMedium
                                    ?.copyWith(fontWeight: FontWeight.w600),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            Text(
                              formatMessageListRowDate(row.date, locale),
                              style: Theme.of(context)
                                  .textTheme
                                  .bodySmall
                                  ?.copyWith(color: scheme.onSurfaceVariant),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ],
                        ),
                        const SizedBox(height: 2),
                        Text(
                          matrixConversationPreviewText(
                            l10n,
                            listAccount,
                            row.subject,
                          ),
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                                fontWeight: row.isRead
                                    ? FontWeight.normal
                                    : FontWeight.bold,
                                decoration: row.markedForDeletion
                                    ? TextDecoration.lineThrough
                                    : null,
                                decorationColor: scheme.onSurface,
                              ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          );

          final bool desktopDrag = widget.enableDesktopMailDrag &&
              !kIsWeb &&
              (defaultTargetPlatform == TargetPlatform.macOS ||
                  defaultTargetPlatform == TargetPlatform.linux ||
                  defaultTargetPlatform == TargetPlatform.windows);
          if (desktopDrag && isSelectedRow) {
            final List<String> dragIds = multi
                ? selectedIds.toList()
                : <String>[row.id];
            final AppSettingsConfig? cfg =
                ref.read(accountsConfigProvider).valueOrNull;
            AppAccount? dragAccount;
            for (final AppAccount a in cfg?.accounts ?? const <AppAccount>[]) {
              if (a.id == widget.folderParams.accountId) {
                dragAccount = a;
                break;
              }
            }
            final bool canDragMail = dragAccount != null &&
                isEmailMailboxBackend(dragAccount);
            if (dragIds.isNotEmpty && canDragMail) {
              final AppLocalizations l10n = AppLocalizations.of(context);
              tile = Draggable<MailListDragPayload>(
                data: MailListDragPayload(
                  sourceAccountId: widget.folderParams.accountId,
                  sourceFolder: widget.folderParams.folderName,
                  messageIds: List<String>.from(dragIds),
                ),
                feedback: Material(
                  elevation: 6,
                  borderRadius: BorderRadius.circular(8),
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 300),
                    child: ListTile(
                      dense: true,
                      leading: const Icon(Icons.mail_outline),
                      title: Text(
                        dragIds.length == 1
                            ? l10n.multiSelectCount(1)
                            : l10n.multiSelectCount(dragIds.length),
                      ),
                    ),
                  ),
                ),
                childWhenDragging: Opacity(opacity: 0.4, child: tile),
                child: tile,
              );
            }
          }

          return tile;
        },
      ),
    );
  }
}
