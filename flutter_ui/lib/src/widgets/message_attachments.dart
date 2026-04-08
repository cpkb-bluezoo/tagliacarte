/*
 * message_attachments.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../l10n/app_localizations.dart';
import '../providers/app_state.dart';
import '../providers/mail_sync.dart';
import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import '../util/mail_account_policy.dart';
import 'attachment_cards.dart';
import 'lucide_icon.dart';

class MessageAttachmentsBlock extends ConsumerStatefulWidget {
  const MessageAttachmentsBlock({
    super.key,
    required this.attachments,
    this.fetchParams,
  });

  final List<MailAttachmentDetail> attachments;
  final MailMessageDetailParams? fetchParams;

  @override
  ConsumerState<MessageAttachmentsBlock> createState() =>
      _MessageAttachmentsBlockState();
}

class _MessageAttachmentsBlockState
    extends ConsumerState<MessageAttachmentsBlock> {
  int? _busyIndex;

  String _label(MailAttachmentDetail a) {
    if (a.filename != null && a.filename!.trim().isNotEmpty) {
      return a.filename!.trim();
    }
    return a.contentType;
  }

  Future<void> _saveBytes(BuildContext context, String name, List<int> bytes) async {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final String? path = await FilePicker.platform.saveFile(
      dialogTitle: l10n.saveAttachment,
      fileName: name,
    );
    if (path == null || !context.mounted) {
      return;
    }
    try {
      final File f = File(path);
      await f.writeAsBytes(bytes, flush: true);
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.savedToPath(path))),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(l10n.saveFailed(e.toString())),
          ),
        );
      }
    }
  }

  Future<void> _onSaveTap(int index, MailAttachmentDetail a) async {
    if (a.inlineData != null) {
      await _saveBytes(context, _label(a), a.inlineData!);
      return;
    }
    final MailMessageDetailParams? p = widget.fetchParams;
    final String? sec = a.imapSection;
    if (p == null || sec == null || sec.isEmpty) {
      if (mounted) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.cannotDownloadAttachment)),
        );
      }
      return;
    }
    final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
    AppAccount? acc;
    for (final AppAccount x in cfg?.accounts ?? const <AppAccount>[]) {
      if (x.id == p.accountId) {
        acc = x;
        break;
      }
    }
    if (acc == null || !isImapStyleMailboxBackend(acc)) {
      if (mounted) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.cannotDownloadAttachment)),
        );
      }
      return;
    }
    setState(() => _busyIndex = index);
    try {
      final String resp = await frbFetchFolderMessagePart(
        accountId: acc.id,
        folderName: p.folderName,
        messageId: p.messageId,
        imapSection: sec,
        transferEncoding:
            a.transferEncoding.trim().isEmpty ? '8BIT' : a.transferEncoding,
      );
      if (!mounted) {
        return;
      }
      final Map<String, dynamic> m = resp.isEmpty
          ? <String, dynamic>{}
          : (jsonDecode(resp) as Map<String, dynamic>);
      final String? b64 = m['bytesBase64'] as String?;
      if (b64 == null || b64.isEmpty) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.emptyAttachmentData)),
        );
        return;
      }
      final List<int> bytes = base64.decode(b64);
      await _saveBytes(context, _label(a), bytes);
    } catch (e) {
      if (mounted) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.downloadFailed(e.toString()))),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _busyIndex = null);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.attachments.isEmpty) {
      return const SizedBox.shrink();
    }
    final AppLocalizations l10n = AppLocalizations.of(context);
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.attachments,
            style: theme.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 8),
          AttachmentCardsGrid(
            children: widget.attachments.asMap().entries.map((
              MapEntry<int, MailAttachmentDetail> e,
            ) {
              final int i = e.key;
              final MailAttachmentDetail a = e.value;
              final bool busy = _busyIndex == i;
              final bool canSave =
                  a.inlineData != null ||
                  (widget.fetchParams != null &&
                      a.imapSection != null &&
                      a.imapSection!.isNotEmpty);
              return AttachmentDisplayCard(
                filename: _label(a),
                subtitle: '${a.contentType} · ${attachmentSizeLabel(a.sizeBytes)}',
                trailing: canSave
                    ? busy
                        ? const Padding(
                            padding: EdgeInsets.all(10),
                            child: SizedBox(
                              width: 24,
                              height: 24,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            ),
                          )
                        : IconButton(
                            tooltip: l10n.saveVerb,
                            icon: const LucideIcon(LucideIcons.download),
                            onPressed: () => _onSaveTap(i, a),
                          )
                    : null,
              );
            }).toList(),
          ),
        ],
      ),
    );
  }
}
