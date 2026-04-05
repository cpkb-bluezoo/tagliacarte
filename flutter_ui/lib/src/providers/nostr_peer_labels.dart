/*
 * nostr_peer_labels.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * Display labels for Nostr DM folder ids (hex pubkeys): profile name / nip05 / npub bech32.
 */

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../rust/frb_api.dart';
import '../rust/tagliacarte_api.dart';
import 'app_state.dart';

class NostrPeerLabelsNotifier extends Notifier<Map<String, String>> {
  @override
  Map<String, String> build() => const <String, String>{};

  /// Apply [nostrProfileUpdated] session event (display name / nip05 / npub).
  void applyProfileEvent(Map<String, dynamic> m) {
    final String? pk = m['pubkeyHex'] as String?;
    if (pk == null) {
      return;
    }
    final String k = pk.trim().toLowerCase();
    final String label = composeNostrProfileLabel(m);
    if (label.isEmpty) {
      return;
    }
    state = <String, String>{...state, k: label};
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
String composeNostrProfileLabel(Map<String, dynamic> m) {
  final String dn = (m['displayName'] as String?)?.trim() ?? '';
  if (dn.isNotEmpty) {
    return dn;
  }
  final String n5 = (m['nip05'] as String?)?.trim() ?? '';
  if (n5.isNotEmpty) {
    return n5;
  }
  return (m['npub'] as String?)?.trim() ?? '';
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
