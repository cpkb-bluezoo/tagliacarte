/*
 * subscription_folder_row.dart
 * Copyright (C) 2026 Chris Burdess
 */

import 'package:flutter/foundation.dart';

@immutable
class SubscriptionFolderRow {
  const SubscriptionFolderRow({
    required this.id,
    required this.isSubscribed,
    this.displayName,
    this.unread,
    required this.allowUnsubscribe,
  });

  final String id;
  final bool isSubscribed;
  final String? displayName;
  final int? unread;
  final bool allowUnsubscribe;

  static SubscriptionFolderRow? tryParse(Map<String, dynamic> m) {
    final String? id = m['id'] as String?;
    if (id == null || id.isEmpty) {
      return null;
    }
    return SubscriptionFolderRow(
      id: id,
      isSubscribed: m['isSubscribed'] as bool? ?? false,
      displayName: m['displayName'] as String?,
      unread: (m['unread'] as num?)?.toInt(),
      allowUnsubscribe: m['allowUnsubscribe'] as bool? ?? false,
    );
  }
}
