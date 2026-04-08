/*
 * rich_message_body_editor.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/material.dart';
import 'package:flutter_quill/flutter_quill.dart';
import 'package:flutter_quill/quill_delta.dart';
import 'package:flutter_quill_delta_from_html/flutter_quill_delta_from_html.dart';
import 'package:vsc_quill_delta_to_html/vsc_quill_delta_to_html.dart';

import '../util/compose_reply.dart' show sanitizeOutboundRichHtml;

/// Latest sanitized HTML and plain-text export from [RichMessageBodyEditor].
@immutable
class RichTextBodySnapshot {
  const RichTextBodySnapshot({
    required this.html,
    required this.plain,
  });

  final String html;
  final String plain;
}

/// Matches empty paragraphs we emit before/after quoted blocks (reply seed).
final RegExp _kLeadingEmptyParagraph = RegExp(
  r'<p>\s*<br\s*/?>\s*</p>',
  caseSensitive: false,
);

/// Strips leading empty `<p><br></p>` runs and re-encodes them as raw newline
/// inserts so [HtmlToDelta] does not pick up stray spaces from HTML text nodes.
({Document document, int leadingBlankLines}) _parseInitialHtmlSeed(
  String? html,
) {
  final String? h0 = html?.trim();
  if (h0 == null || h0.isEmpty) {
    return (document: Document(), leadingBlankLines: 0);
  }
  try {
    final RegExp leadSeq = RegExp(
      '^(?:${_kLeadingEmptyParagraph.pattern}\\s*)+',
      caseSensitive: false,
    );
    final Match? lead = leadSeq.firstMatch(h0);
    String rest = h0;
    int blankPrefixLines = 0;
    if (lead != null) {
      blankPrefixLines =
          _kLeadingEmptyParagraph.allMatches(lead.group(0)!).length;
      rest = h0.substring(lead.end).trimLeft();
    }
    final Delta body = rest.isEmpty ? Delta() : HtmlToDelta().convert(rest);
    if (blankPrefixLines == 0) {
      return (document: Document.fromDelta(body), leadingBlankLines: 0);
    }
    final Delta prefix = Delta();
    for (int i = 0; i < blankPrefixLines; i++) {
      prefix.insert('\n');
    }
    if (body.isEmpty) {
      return (document: Document.fromDelta(prefix), leadingBlankLines: blankPrefixLines);
    }
    return (
      document: Document.fromDelta(prefix.concat(body)),
      leadingBlankLines: blankPrefixLines,
    );
  } catch (_) {
    return (
      document: Document.fromJson(<dynamic>[
        <String, dynamic>{'insert': '$h0\n'},
      ]),
      leadingBlankLines: 0,
    );
  }
}

/// Converts the current Quill [document] to outbound HTML (sanitized) and plain text.
RichTextBodySnapshot exportRichTextBodySnapshot(Document document) {
  final Delta delta = document.toDelta();
  final List<Map<String, dynamic>> ops = delta.toJson();
  final String rawHtml = QuillDeltaToHtmlConverter(ops).convert();
  final String html = sanitizeOutboundRichHtml(rawHtml);
  String plain = document.toPlainText();
  plain = plain.replaceAll('\u00a0', ' ').trimRight();
  return RichTextBodySnapshot(html: html, plain: plain);
}

/// Rich body editor (flutter_quill) for email and Matrix; not used for Nostr / NNTP.
class RichMessageBodyEditor extends StatefulWidget {
  const RichMessageBodyEditor({
    super.key,
    this.initialHtml,
    required this.onChanged,
    this.readOnly = false,
    this.initialCaretAtEnd = false,
  });

  final String? initialHtml;
  final ValueChanged<RichTextBodySnapshot> onChanged;
  final bool readOnly;

  /// After load, move the caret to the end (e.g. reply-after-quoted seed).
  final bool initialCaretAtEnd;

  @override
  State<RichMessageBodyEditor> createState() => _RichMessageBodyEditorState();
}

class _RichMessageBodyEditorState extends State<RichMessageBodyEditor> {
  late final QuillController _controller;
  late final FocusNode _focus;
  late final ScrollController _scroll;
  late final int _leadingBlankLinesFromSeed;

  @override
  void initState() {
    super.initState();
    _focus = FocusNode();
    _scroll = ScrollController();
    final ({Document document, int leadingBlankLines}) seed =
        _parseInitialHtmlSeed(widget.initialHtml);
    _leadingBlankLinesFromSeed = seed.leadingBlankLines;
    _controller = QuillController(
      document: seed.document,
      selection: const TextSelection.collapsed(offset: 0),
      readOnly: widget.readOnly,
    );
    _controller.addListener(_emit);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      if (widget.initialCaretAtEnd) {
        _controller.moveCursorToEnd();
      } else if (_leadingBlankLinesFromSeed > 0) {
        _controller.moveCursorToPosition(0);
      }
      _emit();
    });
  }

  @override
  void didUpdateWidget(covariant RichMessageBodyEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.readOnly != widget.readOnly) {
      _controller.readOnly = widget.readOnly;
    }
  }

  void _emit() {
    widget.onChanged(exportRichTextBodySnapshot(_controller.document));
  }

  @override
  void dispose() {
    _controller.removeListener(_emit);
    _controller.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        QuillSimpleToolbar(
          controller: _controller,
          config: const QuillSimpleToolbarConfig(
            showFontFamily: false,
            showFontSize: false,
            showUnderLineButton: false,
            showStrikeThrough: false,
            showColorButton: false,
            showBackgroundColorButton: false,
            showClearFormat: true,
            showAlignmentButtons: false,
            showHeaderStyle: false,
            showListNumbers: false,
            showListBullets: false,
            showListCheck: false,
            showLink: false,
            showSearchButton: false,
            showSubscript: false,
            showSuperscript: false,
            showIndent: false,
            multiRowsDisplay: true,
          ),
        ),
        Expanded(
          child: QuillEditor(
            controller: _controller,
            focusNode: _focus,
            scrollController: _scroll,
            config: const QuillEditorConfig(),
          ),
        ),
      ],
    );
  }
}
