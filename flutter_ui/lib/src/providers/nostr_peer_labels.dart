/*
 * nostr_peer_labels.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * Display labels for Nostr DM folder ids (hex pubkeys): profile name / nip05 / npub bech32.
 */

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../rust/frb_api.dart';
import '../rust/session/events.dart';
import '../rust/tagliacarte_api.dart';
import 'app_state.dart';

class NostrPeerLabelsNotifier extends Notifier<Map<String, String>> {
  @override
  Map<String, String> build() => const <String, String>{};

  /// Apply [nostrProfileUpdated] session event (display name / nip05 / npub).
  void applyProfileEventApp(AppEvent e) {
    e.whenOrNull(
      nostrProfileUpdated:
          (
            String accountId,
            String pubkeyHex,
            String npub,
            String? displayName,
            String? nip05,
            String? picture,
          ) {
            final String k = pubkeyHex.trim().toLowerCase();
            final String label = composeNostrProfileLabelParts(
              displayName: displayName,
              nip05: nip05,
              npub: npub,
            );
            if (label.isEmpty) {
              return;
            }
            state = <String, String>{...state, k: label};
          },
    );
  }

  /// Bech32 npub for hex folders not yet enriched (async, non-blocking).
  Future<void> primeNpubLabels(Iterable<String> folders) async {
    for (final String f in folders) {
      final String t = f.trim().toLowerCase();
      if (t.length != 64 || !RegExp(r'^[0-9a-f]+$').hasMatch(t)) {
        continue;
      }
      if (state.containsKey(t)) {
        continue;
      }
      try {
        final String npub = await frbNostrHexToNpub(hexPubkey: t);
        if (npub.isNotEmpty) {
          state = <String, String>{...state, t: npub};
        }
      } catch (_) {}
    }
  }
}

/// Fallback order: display name → nip05 → npub (matches Rust).
String composeNostrProfileLabelParts({
  String? displayName,
  String? nip05,
  String? npub,
}) {
  final String dn = displayName?.trim() ?? '';
  if (dn.isNotEmpty) {
    return dn;
  }
  final String n5 = nip05?.trim() ?? '';
  if (n5.isNotEmpty) {
    return n5;
  }
  return npub?.trim() ?? '';
}

final nostrPeerLabelsProvider =
    NotifierProvider<NostrPeerLabelsNotifier, Map<String, String>>(
  NostrPeerLabelsNotifier.new,
);

/// Prime npub labels when folder list updates for Nostr accounts.
///
/// [sessionSaysNostr] is set from [AccountMailModel.storeKind] so we still prime when
/// [accountsConfigProvider] is not ready yet (e.g. `folderListUpdated` before config load).
void primeNostrFolderLabelsIfNeeded(
  Ref ref,
  String accountId,
  List<String> folders, {
  bool sessionSaysNostr = false,
}) {
  bool nostr = sessionSaysNostr;
  if (!nostr) {
    final AppSettingsConfig? cfg = ref.read(accountsConfigProvider).valueOrNull;
    for (final AppAccount a in cfg?.accounts ?? const <AppAccount>[]) {
      if (a.id == accountId && a.backendType.toLowerCase().trim() == 'nostr') {
        nostr = true;
        break;
      }
    }
  }
  if (!nostr) {
    return;
  }
  unawaited(
    ref.read(nostrPeerLabelsProvider.notifier).primeNpubLabels(folders),
  );
}
