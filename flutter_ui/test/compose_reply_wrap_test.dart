/*
 * compose_reply_wrap_test.dart
 * Copyright (C) 2026 Chris Burdess
 */

import 'package:flutter_test/flutter_test.dart';
import 'package:tagliacarte_ui/src/util/compose_reply.dart';

void main() {
  group('prefixedWrappedPhysicalLines', () {
    test('wraps on last space within width', () {
      const String prefix = '> ';
      // max 80 with "> " => 78 content chars per line
      final String line = '${'a' * 40} ${'b' * 40}';
      final List<String> out =
          prefixedWrappedPhysicalLines(line, prefix).toList();
      expect(out.length, 2);
      expect(out[0].length, lessThanOrEqualTo(80));
      expect(out[1].length, lessThanOrEqualTo(80));
      expect(out[0], startsWith(prefix));
      expect(out[1], startsWith(prefix));
      expect(out[0].contains(' '), true);
    });

    test('extends past width when no space in first segment', () {
      const String prefix = '> ';
      final String word = 'x' * 90;
      final List<String> out =
          prefixedWrappedPhysicalLines('$word tail', prefix).toList();
      expect(out.length, greaterThanOrEqualTo(2));
      expect(out[0].length, greaterThan(80));
      expect(out[0], startsWith(prefix));
      expect(out[0], contains(word));
    });

    test('empty line yields prefix only', () {
      final List<String> out =
          prefixedWrappedPhysicalLines('', '> ').toList();
      expect(out, <String>['> ']);
    });
  });

  group('formatReplyHeaderAndQuotedBodyPlain', () {
    test('header without prefix; quoted body wrapped with prefix', () {
      final String s = formatReplyHeaderAndQuotedBodyPlain(
        headerLine: 'On Mon, you wrote:',
        quotedBody: '${'w' * 50} ${'z' * 50}',
        linePrefix: '> ',
      );
      final List<String> lines = s.split('\n');
      expect(lines.first, 'On Mon, you wrote:');
      expect(lines[1], startsWith('> '));
      expect(lines[1].length, lessThanOrEqualTo(80));
    });
  });
}
