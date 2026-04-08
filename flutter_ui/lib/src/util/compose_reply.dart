/*
 * compose_reply.dart
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
import 'package:html/dom.dart' as h;
import 'package:html/parser.dart' as html_parser;
import 'package:intl/intl.dart';

import '../providers/mail_sync.dart';
import '../rust/tagliacarte_api.dart';

/// Wrapper class for the seeded reply quote in rich compose (plain-text reordering + identification).
const String kQuotedMessageReplyClassName = 'quoted-message-reply';

/// Visible text from HTML bodies (for quoting when [MailMessageDetailView.bodyPlain] is empty).
String htmlToPlainText(String? html) {
  if (html == null) {
    return '';
  }
  final String t = html.trim();
  if (t.isEmpty) {
    return '';
  }
  final h.Document doc = html_parser.parse(t);
  final String? bodyText = doc.body?.text;
  if (bodyText == null) {
    return '';
  }
  return bodyText
      .replaceAll('\r\n', '\n')
      .replaceAll('\r', '\n')
      .replaceAll(RegExp(r'[ \t]+\n'), '\n')
      .replaceAll(RegExp(r'\n{3,}'), '\n\n')
      .trim();
}

/// Prefer plain body; otherwise derive from HTML.
String plainBodyForQuote(MailMessageDetailView view) {
  final String? p = view.bodyPlain?.trim();
  if (p != null && p.isNotEmpty) {
    return p;
  }
  return htmlToPlainText(view.bodyHtml);
}

String _senderForTemplate(MailMessageDetailView view) {
  final String f = view.fromRaw.trim();
  if (f.isNotEmpty) {
    return f;
  }
  return '(unknown)';
}

String expandReplyHeaderTemplate(
  String template,
  String date,
  String time,
  String sender,
) {
  return template
      .replaceAll(r'$date', date)
      .replaceAll(r'$time', time)
      .replaceAll(r'$sender', sender);
}

String formatReplyDate(DateTime local, String pattern, Locale locale) {
  final String lc = locale.toString();
  if (pattern.trim().isEmpty) {
    return DateFormat.yMMMMEEEEd(lc).format(local);
  }
  return DateFormat(pattern, lc).format(local);
}

String formatReplyTime(DateTime local, String pattern, Locale locale) {
  final String lc = locale.toString();
  if (pattern.trim().isEmpty) {
    return DateFormat.jm(lc).format(local);
  }
  return DateFormat(pattern, lc).format(local);
}

/// True only for breakable ASCII whitespace. U+00A0 NBSP and similar must **not** match — they are
/// non-breaking and must not be used as wrap points.
bool _isWhitespaceGrapheme(String g) {
  if (g.isEmpty) {
    return false;
  }
  if (g == ' ' || g == '\t') {
    return true;
  }
  if (g == '\n' || g == '\r' || g == '\f' || g == '\v') {
    return true;
  }
  return false;
}

Characters _trimLeadingWhitespaceChars(Characters c) {
  final String t = c.toString().trimLeft();
  return t.isEmpty ? Characters('') : t.characters;
}

/// One logical line of the *quoted original* (no prefix), as one or more physical lines
/// each starting with [linePrefix], each at most [maxPrefixedLineLength] **characters**
/// (grapheme clusters) long.
///
/// Wraps at the last whitespace within the first [maxContent] character run of each
/// segment; if there is no whitespace in that run, extends until the next whitespace
/// (or end of line), so a single long token may exceed [maxPrefixedLineLength].
Iterable<String> prefixedWrappedPhysicalLines(
  String originalLine,
  String linePrefix, {
  int maxPrefixedLineLength = 80,
}) sync* {
  final int pLen = linePrefix.characters.length;
  if (maxPrefixedLineLength <= pLen) {
    yield '$linePrefix$originalLine';
    return;
  }
  final int maxContent = maxPrefixedLineLength - pLen;
  Characters rest = originalLine.characters;
  rest = _trimLeadingWhitespaceChars(rest);
  if (rest.isEmpty) {
    yield linePrefix;
    return;
  }
  while (rest.isNotEmpty) {
    if (rest.length <= maxContent) {
      yield '$linePrefix$rest';
      return;
    }
    final Characters win = rest.take(maxContent);
    int? lastWsIdx;
    int i = 0;
    for (final String g in win) {
      if (_isWhitespaceGrapheme(g)) {
        lastWsIdx = i;
      }
      i++;
    }
    if (lastWsIdx != null && lastWsIdx > 0) {
      final int at = lastWsIdx;
      final String segment = rest.take(at).toString().trimRight();
      yield '$linePrefix$segment';
      rest = rest.skip(at + 1);
      rest = _trimLeadingWhitespaceChars(rest);
      continue;
    }
    int? wsAtOrAfter;
    int j = 0;
    for (final String g in rest) {
      if (j >= maxContent && _isWhitespaceGrapheme(g)) {
        wsAtOrAfter = j;
        break;
      }
      j++;
    }
    if (wsAtOrAfter == null) {
      yield '$linePrefix$rest';
      return;
    }
    final String longSeg = rest.take(wsAtOrAfter).toString().trimRight();
    yield '$linePrefix$longSeg';
    rest = rest.skip(wsAtOrAfter + 1);
    rest = _trimLeadingWhitespaceChars(rest);
  }
}

/// Reply header line plus quoted body lines with [linePrefix] and wrapping (see
/// [prefixedWrappedPhysicalLines]).
String formatReplyHeaderAndQuotedBodyPlain({
  required String headerLine,
  required String quotedBody,
  required String linePrefix,
  int maxPrefixedLineLength = 80,
}) {
  final StringBuffer buf = StringBuffer();
  final String h = headerLine.trimRight();
  if (h.isNotEmpty) {
    buf.writeln(h);
  }
  final String body = quotedBody.replaceAll('\r\n', '\n').replaceAll('\r', '\n');
  if (body.trim().isEmpty) {
    return buf.toString().trimRight();
  }
  for (final String line in body.split('\n')) {
    for (final String pl in prefixedWrappedPhysicalLines(
      line,
      linePrefix,
      maxPrefixedLineLength: maxPrefixedLineLength,
    )) {
      buf.writeln(pl);
    }
  }
  return buf.toString().trimRight();
}

/// Quoted original as plain text (prefix on each line), after [headerLine].
///
/// When [replyBeforeQuoted] is true, two blank lines precede [headerLine]
/// (caret at start). When false, two blank lines follow the quoted lines
/// (caret at end).
String buildQuotedPlainBlock({
  required String headerLine,
  required String linePrefix,
  required String plainBody,
  required bool replyBeforeQuoted,
}) {
  final String plain = plainBody.trimRight();
  final StringBuffer buf = StringBuffer();
  if (replyBeforeQuoted) {
    buf.writeln();
    buf.writeln();
  } else {
    buf.writeln();
  }
  buf.writeln(headerLine);
  if (plain.isEmpty) {
    if (replyBeforeQuoted) {
      buf.writeln();
    } else {
      buf.writeln();
      buf.writeln();
    }
    return buf.toString();
  }
  for (final String line in plain.split('\n')) {
    for (final String pl in prefixedWrappedPhysicalLines(line, linePrefix)) {
      buf.writeln(pl);
    }
  }
  if (replyBeforeQuoted) {
    buf.writeln();
  } else {
    buf.writeln();
    buf.writeln();
  }
  return buf.toString();
}

/// Full reply/forward quote block using global compose preferences.
String quotedReplyBodyForConfig(
  MailMessageDetailView view,
  AppSettingsConfig cfg,
  Locale locale,
) {
  final int? ms = view.dateMs;
  final DateTime when = ms != null
      ? DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true).toLocal()
      : DateTime.now();
  final String date = formatReplyDate(when, cfg.replyDateFormat, locale);
  final String time = formatReplyTime(when, cfg.replyTimeFormat, locale);
  final String sender = _senderForTemplate(view);
  final String header = expandReplyHeaderTemplate(
    cfg.replyHeaderTemplate,
    date,
    time,
    sender,
  );
  final String plain = plainBodyForQuote(view);
  final bool replyBefore =
      normalizeReplyPlainPosition(cfg.replyPlainPosition) == 'before_quote';
  return buildQuotedPlainBlock(
    headerLine: header,
    linePrefix: cfg.replyLinePrefix,
    plainBody: plain,
    replyBeforeQuoted: replyBefore,
  );
}

/// Rich compose: HTML seed with reply header, blockquote of original body, and [kQuotedMessageReplyClassName] wrapper.
String buildQuotedRichHtmlSeed({
  required MailMessageDetailView view,
  required AppSettingsConfig cfg,
  required Locale locale,
}) {
  final int? ms = view.dateMs;
  final DateTime when = ms != null
      ? DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true).toLocal()
      : DateTime.now();
  final String date = formatReplyDate(when, cfg.replyDateFormat, locale);
  final String time = formatReplyTime(when, cfg.replyTimeFormat, locale);
  final String sender = _senderForTemplate(view);
  final String header = expandReplyHeaderTemplate(
    cfg.replyHeaderTemplate,
    date,
    time,
    sender,
  );
  final String headerHtml = '<p>${_htmlEscapeForEmail(header)}</p>';
  final String? rawHtml = view.bodyHtml?.trim();
  final String innerQuote;
  if (rawHtml != null && rawHtml.isNotEmpty) {
    innerQuote = sanitizeOutboundRichHtml(rawHtml);
  } else {
    final String plain = plainBodyForQuote(view);
    innerQuote = _htmlEscapeForEmail(plain).replaceAll('\n', '<br>\n');
  }
  final String quoted =
      '<div class="$kQuotedMessageReplyClassName">$headerHtml'
      '<blockquote>$innerQuote</blockquote></div>';
  final bool replyBefore =
      normalizeReplyPlainPosition(cfg.replyPlainPosition) == 'before_quote';
  if (replyBefore) {
    return '<p><br></p><p><br></p>$quoted';
  }
  return '$quoted<p><br></p><p><br></p>';
}

String normalizeReplyPlainPosition(String raw) {
  if (raw.trim().toLowerCase() == 'after_quote') {
    return 'after_quote';
  }
  return 'before_quote';
}

/// Whether [sanitizedRichHtml] still contains the reply-quote wrapper (after [sanitizeOutboundRichHtml]).
bool richHtmlContainsQuotedMessageMarker(String sanitizedRichHtml) {
  final String t = sanitizedRichHtml.trim();
  if (t.isEmpty) {
    return false;
  }
  final h.Document doc =
      html_parser.parse('<div id="__tc_root">$t</div>');
  final h.Element? root = doc.getElementById('__tc_root');
  return root?.querySelector('div.$kQuotedMessageReplyClassName') != null;
}

/// Builds `text/plain` for SMTP when rich HTML contains [kQuotedMessageReplyClassName]; otherwise [quillPlainFallback].
///
/// The quoted block is taken from the same structure as [buildQuotedRichHtmlSeed]: first `<p>` is
/// the reply header (no `>` prefix); `<blockquote>` is the original body, prefixed and wrapped per
/// [replyLinePrefix] and [maxQuotedPrefixedLineLength].
String buildOrderedReplyPlainFromSanitizedRichHtml({
  required String sanitizedRichHtml,
  required String quillPlainFallback,
  required String replyPlainPosition,
  String replyLinePrefix = '> ',
  int maxQuotedPrefixedLineLength = 80,
}) {
  final String t = sanitizedRichHtml.trim();
  if (t.isEmpty) {
    return quillPlainFallback;
  }
  final h.Document doc =
      html_parser.parse('<div id="__tc_root">$t</div>');
  final h.Element? wrap = doc.getElementById('__tc_root');
  if (wrap == null) {
    return quillPlainFallback;
  }
  final h.Element? quoted =
      wrap.querySelector('div.$kQuotedMessageReplyClassName');
  if (quoted == null) {
    return quillPlainFallback;
  }
  final String quotedPlainFormatted = _quotedDivToPrefixedPlain(
    quoted,
    replyLinePrefix,
    maxQuotedPrefixedLineLength,
  );
  quoted.remove();
  final String userPlain = htmlToPlainText(wrap.innerHtml).trim();
  if (userPlain.isEmpty && quotedPlainFormatted.isEmpty) {
    return quillPlainFallback;
  }
  final bool after =
      normalizeReplyPlainPosition(replyPlainPosition) == 'after_quote';
  final String first = after ? quotedPlainFormatted : userPlain;
  final String second = after ? userPlain : quotedPlainFormatted;
  if (first.isEmpty) {
    return second;
  }
  if (second.isEmpty) {
    return first;
  }
  return '$first\n\n$second';
}

/// Plain text for [quotedDiv] (`<p>` header + `<blockquote>` body with prefix/wrap).
String _quotedDivToPrefixedPlain(
  h.Element quotedDiv,
  String linePrefix,
  int maxPrefixedLineLength,
) {
  final h.Element? p = quotedDiv.querySelector('p');
  final h.Element? bq = quotedDiv.querySelector('blockquote');
  if (p != null && bq != null) {
    final String header = htmlToPlainText(p.innerHtml).trim();
    final String body = htmlToPlainText(bq.innerHtml).trimRight();
    return formatReplyHeaderAndQuotedBodyPlain(
      headerLine: header,
      quotedBody: body,
      linePrefix: linePrefix,
      maxPrefixedLineLength: maxPrefixedLineLength,
    );
  }
  final String full = htmlToPlainText(quotedDiv.innerHtml).trim();
  if (full.isEmpty) {
    return '';
  }
  final List<String> lines = full.split(RegExp(r'\r\n|\n|\r'));
  if (lines.length <= 1) {
    return formatReplyHeaderAndQuotedBodyPlain(
      headerLine: full,
      quotedBody: '',
      linePrefix: linePrefix,
      maxPrefixedLineLength: maxPrefixedLineLength,
    );
  }
  return formatReplyHeaderAndQuotedBodyPlain(
    headerLine: lines.first.trimRight(),
    quotedBody: lines.sublist(1).join('\n'),
    linePrefix: linePrefix,
    maxPrefixedLineLength: maxPrefixedLineLength,
  );
}

String _htmlEscapeForEmail(String s) {
  return s
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;');
}

/// Optional second body for `multipart/alternative` when preserving HTML (SMTP only).
String? smtpHtmlAlternativeBody({
  required String fullPlainComposeBody,
  required String? originalMessageHtml,
}) {
  final String? raw = originalMessageHtml?.trim();
  if (raw == null || raw.isEmpty) {
    return null;
  }
  final String esc =
      _htmlEscapeForEmail(fullPlainComposeBody).replaceAll('\n', '<br>\n');
  return '<div style="white-space:pre-wrap">$esc</div>'
      '<hr/>'
      '<div>$raw</div>';
}

/// Same as [smtpHtmlAlternativeBody] but the user-authored top section is already HTML (from Quill).
String? smtpHtmlAlternativeBodyFromRichHtml({
  required String userHtml,
  required String? originalMessageHtml,
}) {
  final String? raw = originalMessageHtml?.trim();
  if (raw == null || raw.isEmpty) {
    return null;
  }
  final String u = userHtml.trim();
  if (u.isEmpty) {
    return '<hr/><div>$raw</div>';
  }
  return '<div>$u</div><hr/><div>$raw</div>';
}

/// Seed rich editor from a plain-text reply quote (escaped, line breaks preserved).
String plainTextToSimpleHtmlBlock(String plain) {
  final String esc =
      _htmlEscapeForEmail(plain).replaceAll('\n', '<br>\n');
  return '<div style="white-space:pre-wrap">$esc</div>';
}

bool isReplyQuoteModeHtmlSmtp(AppSettingsConfig cfg) {
  return cfg.replyQuoteMode.trim().toLowerCase() == 'html_smtp';
}

/// Strip scripts, event handlers, and other risky constructs from user-authored HTML before send.
String sanitizeOutboundRichHtml(String html) {
  final String t = html.trim();
  if (t.isEmpty) {
    return '';
  }
  final h.DocumentFragment frag = html_parser.parseFragment(t);
  for (final h.Node c in List<h.Node>.from(frag.nodes)) {
    _sanitizeOutboundRichHtmlNode(c);
  }
  return frag.outerHtml;
}

const Set<String> _kRichHtmlAllowedTags = <String>{
  'p',
  'br',
  'div',
  'span',
  'strong',
  'b',
  'em',
  'i',
  'u',
  's',
  'strike',
  'del',
  'code',
  'pre',
  'blockquote',
  'a',
  'ul',
  'ol',
  'li',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'hr',
};

void _sanitizeOutboundRichHtmlNode(h.Node node) {
  if (node is h.Element) {
    final String name = (node.localName ?? '').toLowerCase();
    if (name == 'script' ||
        name == 'style' ||
        name == 'iframe' ||
        name == 'object' ||
        name == 'embed') {
      node.remove();
      return;
    }
    if (name.isNotEmpty && !_kRichHtmlAllowedTags.contains(name)) {
      node.replaceWith(h.Text(node.text));
      return;
    }
    if (name == 'div') {
      final String? cls = node.attributes['class']?.trim();
      node.attributes.clear();
      if (cls == kQuotedMessageReplyClassName) {
        node.attributes['class'] = kQuotedMessageReplyClassName;
      }
    } else {
      final List<Object> attrKeys = List<Object>.from(node.attributes.keys);
      for (final Object key in attrKeys) {
        final String an = key.toString().toLowerCase();
        if (an.startsWith('on')) {
          node.attributes.remove(key);
          continue;
        }
        if (an == 'href' && name == 'a') {
          final String? v = node.attributes[key]?.trim().toLowerCase();
          if (v == null ||
              !(v.startsWith('http://') ||
                  v.startsWith('https://') ||
                  v.startsWith('mailto:'))) {
            node.attributes.remove(key);
          }
          continue;
        }
        if (an != 'href') {
          node.attributes.remove(key);
        }
      }
    }
  }
  if (node is h.Element || node is h.DocumentFragment) {
    for (final h.Node c in List<h.Node>.from(node.nodes)) {
      _sanitizeOutboundRichHtmlNode(c);
    }
  }
}
