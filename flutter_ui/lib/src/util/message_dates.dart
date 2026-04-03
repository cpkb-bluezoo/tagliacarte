/*
 * message_dates.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'package:flutter/widgets.dart';
import 'package:intl/intl.dart';

/// Short date+time for message list rows (locale-driven).
String formatMessageListRowDate(DateTime utcOrLocal, Locale locale) {
  final DateTime local = utcOrLocal.toLocal();
  return DateFormat.yMd(locale.toLanguageTag()).add_jm().format(local);
}

/// Long date+time for message detail header (locale-driven).
String formatMessageDetailHeaderDate(DateTime utcOrLocal, Locale locale) {
  final DateTime local = utcOrLocal.toLocal();
  return DateFormat.yMMMMEEEEd(locale.toLanguageTag()).add_jm().format(local);
}
