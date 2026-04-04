/*
 * new_mail_notification_service.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter_local_notifications/flutter_local_notifications.dart';

/// Background / minimized alerts when the app is not in the foreground.
class NewMailNotificationService {
  NewMailNotificationService._();
  static final NewMailNotificationService instance = NewMailNotificationService._();

  final FlutterLocalNotificationsPlugin _plugin = FlutterLocalNotificationsPlugin();
  bool _inited = false;
  int _nextId = 0;

  Future<void> init() async {
    if (_inited) {
      return;
    }
    const AndroidInitializationSettings android = AndroidInitializationSettings(
      '@mipmap/ic_launcher',
    );
    const DarwinInitializationSettings darwin = DarwinInitializationSettings();
    await _plugin.initialize(
      const InitializationSettings(
        android: android,
        iOS: darwin,
        macOS: darwin,
      ),
    );
    _inited = true;
  }

  Future<void> showOsNotification({
    required String title,
    required String body,
  }) async {
    await init();
    _nextId++;
    await _plugin.show(
      _nextId,
      title,
      body,
      const NotificationDetails(
        android: AndroidNotificationDetails(
          'tagliacarte_new_mail',
          'New mail',
          channelDescription: 'New messages detected on IMAP',
          importance: Importance.defaultImportance,
          priority: Priority.defaultPriority,
        ),
        iOS: DarwinNotificationDetails(),
        macOS: DarwinNotificationDetails(),
      ),
    );
  }
}
