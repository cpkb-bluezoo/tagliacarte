/*
 * new_mail_merge_test.dart
 * Copyright (C) 2026 Chris Burdess
 */

import 'package:flutter_test/flutter_test.dart';
import 'package:tagliacarte_ui/src/util/new_mail_merge.dart';

void main() {
  test('no notify before baseline', () {
    final ({bool shouldNotify, int countHint}) r = detectNewMailAfterMerge(
      listReady: true,
      baselineEstablished: false,
      previousRefTotal: 0,
      previousKnownIds: <String>{},
      mergedTotal: 5,
      mergedIdsFromSlots: <String>{'a'},
    );
    expect(r.shouldNotify, false);
    expect(r.countHint, 0);
  });

  test('notify when total increases', () {
    final ({bool shouldNotify, int countHint}) r = detectNewMailAfterMerge(
      listReady: true,
      baselineEstablished: true,
      previousRefTotal: 10,
      previousKnownIds: <String>{'x'},
      mergedTotal: 12,
      mergedIdsFromSlots: <String>{'x', 'y'},
    );
    expect(r.shouldNotify, true);
    expect(r.countHint, 2);
  });

  test('no notify when unchanged', () {
    final ({bool shouldNotify, int countHint}) r = detectNewMailAfterMerge(
      listReady: true,
      baselineEstablished: true,
      previousRefTotal: 3,
      previousKnownIds: <String>{'a', 'b'},
      mergedTotal: 3,
      mergedIdsFromSlots: <String>{'a', 'b'},
    );
    expect(r.shouldNotify, false);
    expect(r.countHint, 0);
  });

  test('notify when new id appears without total change', () {
    final ({bool shouldNotify, int countHint}) r = detectNewMailAfterMerge(
      listReady: true,
      baselineEstablished: true,
      previousRefTotal: 2,
      previousKnownIds: <String>{'a'},
      mergedTotal: 2,
      mergedIdsFromSlots: <String>{'a', 'b'},
    );
    expect(r.shouldNotify, true);
    expect(r.countHint, 1);
  });
}
