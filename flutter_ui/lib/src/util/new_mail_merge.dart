/*
 * new_mail_merge.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

/// Pure logic for "new mail" after a folder list merge (tests target this).
({bool shouldNotify, int countHint}) detectNewMailAfterMerge({
  required bool listReady,
  required bool baselineEstablished,
  required int previousRefTotal,
  required Set<String> previousKnownIds,
  required int mergedTotal,
  required Set<String> mergedIdsFromSlots,
}) {
  if (!listReady || !baselineEstablished) {
    return (shouldNotify: false, countHint: 0);
  }
  final bool totalIncreased = mergedTotal > previousRefTotal;
  int freshIds = 0;
  for (final String id in mergedIdsFromSlots) {
    if (!previousKnownIds.contains(id)) {
      freshIds++;
    }
  }
  if (!totalIncreased && freshIds == 0) {
    return (shouldNotify: false, countHint: 0);
  }
  final int hint = totalIncreased
      ? (mergedTotal - previousRefTotal).clamp(1, 1 << 20)
      : freshIds.clamp(1, 1 << 20);
  return (shouldNotify: true, countHint: hint);
}
