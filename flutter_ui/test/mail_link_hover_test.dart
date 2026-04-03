/*
 * mail_link_hover_test.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter_test/flutter_test.dart';
import 'package:tagliacarte_ui/src/util/mail_link_hover.dart';

void main() {
  test('truncateUrlForStatusBar keeps short URLs', () {
    expect(truncateUrlForStatusBar('https://a.com/x'), 'https://a.com/x');
  });

  test('truncateUrlForStatusBar truncates end only', () {
    final String long = 'https://example.com/${'p' * 120}';
    final String out = truncateUrlForStatusBar(long, maxChars: 40);
    expect(out.length, 40);
    expect(out.endsWith('…'), isTrue);
    expect(out.startsWith('https://example.com/'), isTrue);
  });

  test('linkTextMisrepresentsHttpHref when text URL differs from href', () {
    expect(
      linkTextMisrepresentsHttpHref(
        'https://evil.example/phish',
        'https://bank.example/secure',
      ),
      isTrue,
    );
  });

  test('linkTextMisrepresentsHttpHref false when text matches href', () {
    expect(
      linkTextMisrepresentsHttpHref(
        'https://same.example/path?q=1',
        'https://same.example/path?q=1',
      ),
      isFalse,
    );
  });

  test('linkTextMisrepresentsHttpHref false for non-URL link text', () {
    expect(
      linkTextMisrepresentsHttpHref(
        'https://evil.com',
        'Click here to verify',
      ),
      isFalse,
    );
  });

  test('www display text normalizes to compare', () {
    expect(
      linkTextMisrepresentsHttpHref(
        'https://other.com',
        'www.bank.com',
      ),
      isTrue,
    );
  });
}
