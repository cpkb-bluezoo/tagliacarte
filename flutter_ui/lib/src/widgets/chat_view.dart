/*
 * chat_view.dart
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
import 'package:intl/intl.dart';

import '../l10n/app_localizations.dart';
import '../models/message_row.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../rust/frb_api.dart';
import '../rust/session/commands.dart';
import '../rust/tagliacarte_api.dart';
import '../util/mail_account_policy.dart';
import '../util/mailbox_format.dart';
import 'attachment_cards.dart';
import 'lucide_icon.dart';
import 'rich_message_body_editor.dart';

/// Conversation timeline for Nostr/Matrix (same [folderMailboxListProvider] as mail list).
class ChatView extends ConsumerStatefulWidget {
  const ChatView({
    super.key,
    required this.folderParams,
  });

  final SessionFolderParams folderParams;

  @override
  ConsumerState<ChatView> createState() => _ChatViewState();
}

class _ChatViewState extends ConsumerState<ChatView> {
  final TextEditingController _input = TextEditingController();
  List<PickedAttachmentFile> _attachments = <PickedAttachmentFile>[];
  String _matrixRichPlain = '';
  String _matrixRichHtml = '';
  int _matrixRichEditorGeneration = 0;

  @override
  void dispose() {
    _input.dispose();
    super.dispose();
  }

  Future<void> _pickAttachments() async {
    final List<PickedAttachmentFile> picked = await pickAttachmentFiles();
    if (!mounted || picked.isEmpty) {
      return;
    }
    setState(() {
      _attachments = List<PickedAttachmentFile>.from(_attachments)..addAll(picked);
    });
  }

  Future<void> _onSend() async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (_attachments.isNotEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.chatAttachmentsNotSentInChat)),
      );
      return;
    }
    final AppSettingsConfig? cfg =
        ref.read(accountsConfigProvider).valueOrNull;
    AppAccount? acc;
    if (cfg != null) {
      for (final AppAccount a in cfg.accounts) {
        if (a.id == widget.folderParams.accountId) {
          acc = a;
          break;
        }
      }
    }
    final bool matrixRich = acc != null &&
        isMatrixMailboxBackend(acc) &&
        (cfg?.matrixChatUseRichText ?? false);
    final String text = matrixRich
        ? _matrixRichPlain.trim()
        : _input.text.trim();
    if (text.isEmpty) {
      return;
    }
    _input.clear();
    try {
      await frbSessionCommand(
        command: AppCommand.sendChatMessage(
          accountId: widget.folderParams.accountId,
          folder: widget.folderParams.folderName,
          text: text,
          bodyHtml: matrixRich && _matrixRichHtml.trim().isNotEmpty
              ? _matrixRichHtml.trim()
              : null,
        ),
      );
      if (mounted) {
        if (matrixRich) {
          setState(() => _matrixRichEditorGeneration++);
        }
        ref.invalidate(folderMailboxListProvider(widget.folderParams));
      }
    } catch (e) {
      if (mounted) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.operationFailed(e.toString()))),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final FolderListVm vm =
        ref.watch(folderMailboxListProvider(widget.folderParams));
    final ColorScheme scheme = Theme.of(context).colorScheme;

    if (vm.error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Text(l10n.operationFailed(vm.error.toString())),
        ),
      );
    }

    if (vm.totalCount == 0 && vm.ready) {
      return Column(
        children: [
          Expanded(
            child: Center(
              child: Text(
                l10n.noMessages,
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
            ),
          ),
          _composer(context, l10n, scheme),
        ],
      );
    }

    if (vm.totalCount == 0) {
      return const Center(child: CircularProgressIndicator());
    }

    return Column(
      children: [
        Expanded(
          child: ListView.builder(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 12),
            itemCount: vm.totalCount,
            itemBuilder: (BuildContext context, int i) {
              final MessageListRow? row = vm.rowAtDataIndex(i);
              if (row == null) {
                return const Padding(
                  padding: EdgeInsets.symmetric(vertical: 6),
                  child: Center(
                    child: SizedBox(
                      width: 22,
                      height: 22,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ),
                );
              }
              // Nostr maps body preview into [MessageListRow.subject].
              final String body =
                  row.subject.trim().isEmpty ? '…' : row.subject;
              return Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: scheme.surfaceContainerHigh,
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 10,
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Expanded(
                                child: Text(
                                  messageListSenderLine(row.from),
                                  style: Theme.of(context)
                                      .textTheme
                                      .labelMedium
                                      ?.copyWith(
                                        fontWeight: FontWeight.w600,
                                        color: scheme.primary,
                                      ),
                                ),
                              ),
                              Text(
                                DateFormat.yMMMd().add_Hm().format(
                                      row.date.toLocal(),
                                    ),
                                style: Theme.of(context)
                                    .textTheme
                                    .labelSmall
                                    ?.copyWith(
                                      color: scheme.onSurfaceVariant,
                                    ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 4),
                          Text(
                            body,
                            style: Theme.of(context).textTheme.bodyMedium,
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              );
            },
          ),
        ),
        _composer(context, l10n, scheme),
      ],
    );
  }

  Widget _composer(
    BuildContext context,
    AppLocalizations l10n,
    ColorScheme scheme,
  ) {
    final AppSettingsConfig? cfg =
        ref.watch(accountsConfigProvider).valueOrNull;
    AppAccount? acc;
    if (cfg != null) {
      for (final AppAccount a in cfg.accounts) {
        if (a.id == widget.folderParams.accountId) {
          acc = a;
          break;
        }
      }
    }
    final bool matrixRich = acc != null &&
        isMatrixMailboxBackend(acc) &&
        (cfg?.matrixChatUseRichText ?? false);

    final ThemeData theme = Theme.of(context);
    return Material(
      elevation: 2,
      color: scheme.surface,
      child: Padding(
        padding: const EdgeInsets.all(8),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            if (matrixRich)
              SizedBox(
                height: 160,
                child: RichMessageBodyEditor(
                  key: ValueKey<int>(_matrixRichEditorGeneration),
                  onChanged: (RichTextBodySnapshot s) {
                    setState(() {
                      _matrixRichPlain = s.plain;
                      _matrixRichHtml = s.html;
                    });
                  },
                ),
              )
            else
              TextField(
                controller: _input,
                minLines: 1,
                maxLines: 4,
                decoration: InputDecoration(
                  hintText: l10n.chatHintTypeMessage,
                  border: const OutlineInputBorder(),
                  isDense: true,
                ),
                onSubmitted: (_) => _onSend(),
              ),
            if (_attachments.isNotEmpty) ...<Widget>[
              const SizedBox(height: 8),
              Text(
                l10n.attachments,
                style: theme.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
              const SizedBox(height: 8),
              AttachmentCardsGrid(
                children: _attachments.asMap().entries.map((
                  MapEntry<int, PickedAttachmentFile> e,
                ) {
                  final int i = e.key;
                  final PickedAttachmentFile a = e.value;
                  return AttachmentDisplayCard(
                    filename: a.filename,
                    subtitle: attachmentSizeLabel(a.sizeBytes),
                    trailing: IconButton(
                      tooltip: l10n.composeRemoveAttachment,
                      icon: const LucideIcon(LucideIcons.x, size: 20),
                      onPressed: () {
                        setState(() {
                          _attachments = List<PickedAttachmentFile>.from(_attachments)
                            ..removeAt(i);
                        });
                      },
                    ),
                  );
                }).toList(),
              ),
            ],
            const SizedBox(height: 8),
            Row(
              children: <Widget>[
                const Spacer(),
                IconButton(
                  tooltip: l10n.attach,
                  onPressed: _pickAttachments,
                  icon: const LucideIcon(LucideIcons.paperclip),
                ),
                IconButton(
                  tooltip: l10n.composeTooltip,
                  onPressed: _onSend,
                  icon: const LucideIcon(LucideIcons.send),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
