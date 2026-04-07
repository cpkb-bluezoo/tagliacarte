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

/// Quoted original as plain text (prefix on each line), after [headerLine].
String buildQuotedPlainBlock({
  required String headerLine,
  required String linePrefix,
  required String plainBody,
}) {
  final String plain = plainBody.trimRight();
  final StringBuffer buf = StringBuffer();
  buf.writeln();
  buf.writeln(headerLine);
  if (plain.isEmpty) {
    buf.writeln();
    return buf.toString();
  }
  for (final String line in plain.split('\n')) {
    buf.writeln('$linePrefix$line');
  }
  buf.writeln();
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
  return buildQuotedPlainBlock(
    headerLine: header,
    linePrefix: cfg.replyLinePrefix,
    plainBody: plain,
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
  if (node is h.Element || node is h.DocumentFragment) {
    for (final h.Node c in List<h.Node>.from(node.nodes)) {
      _sanitizeOutboundRichHtmlNode(c);
    }
  }
}
