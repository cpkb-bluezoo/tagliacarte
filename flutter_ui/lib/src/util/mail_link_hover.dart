/*
 * mail_link_hover.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

/// Max characters for the HTML link hover status line; truncate **end** only.
const int kMaxMailLinkStatusUrlChars = 96;

String truncateUrlForStatusBar(
  String url, {
  int maxChars = kMaxMailLinkStatusUrlChars,
}) {
  final String t = url.trim();
  if (t.length <= maxChars) {
    return t;
  }
  const String ell = '…';
  final int keep = maxChars - ell.length;
  if (keep <= 0) {
    return ell;
  }
  return '${t.substring(0, keep)}$ell';
}

/// Strip default ports and normalize host/path/query for http(s) equality checks.
String? normalizeHttpUrlForComparison(String raw) {
  final Uri? u = Uri.tryParse(raw.trim());
  if (u == null) {
    return null;
  }
  if (u.scheme != 'http' && u.scheme != 'https') {
    return null;
  }
  final String host = u.host.toLowerCase();
  if (host.isEmpty) {
    return null;
  }
  String path = u.path;
  if (path.isEmpty) {
    path = '/';
  }
  final String q = u.hasQuery ? '?${u.query}' : '';
  final int defaultPort = u.scheme == 'https' ? 443 : 80;
  final String port =
      u.hasPort && u.port != defaultPort ? ':${u.port}' : '';
  return '${u.scheme}://$host$port$path$q';
}

/// If [raw] looks like a URL users might believe they are visiting (http(s), www…, or domain.tld/…).
Uri? parseDisplayedTextAsLikelyHttpUrl(String raw) {
  String s = raw.trim();
  if (s.isEmpty) {
    return null;
  }
  if (s.length >= 2 && s.startsWith('<') && s.endsWith('>')) {
    s = s.substring(1, s.length - 1).trim();
  }
  Uri? u = Uri.tryParse(s);
  if (u != null && u.hasScheme) {
    if (u.scheme == 'http' || u.scheme == 'https') {
      return u;
    }
    return null;
  }
  if (RegExp(r'^www\.', caseSensitive: false).hasMatch(s)) {
    u = Uri.tryParse('https://$s');
    if (u != null && u.hasAuthority && u.host.isNotEmpty) {
      return u;
    }
  }
  if (!s.contains(' ') && s.contains('.') && !s.startsWith('/')) {
    u = Uri.tryParse('https://$s');
    if (u != null &&
        u.hasAuthority &&
        u.host.contains('.') &&
        !u.host.startsWith('.')) {
      return u;
    }
  }
  return null;
}

/// True when visible link text parses as an http(s) URL and differs from [href] after normalization.
bool linkTextMisrepresentsHttpHref(String href, String linkText) {
  final Uri? textUri = parseDisplayedTextAsLikelyHttpUrl(linkText);
  if (textUri == null) {
    return false;
  }
  final String? hrefNorm = normalizeHttpUrlForComparison(href);
  final String? textNorm = normalizeHttpUrlForComparison(textUri.toString());
  if (hrefNorm == null || textNorm == null) {
    return false;
  }
  return hrefNorm != textNorm;
}
