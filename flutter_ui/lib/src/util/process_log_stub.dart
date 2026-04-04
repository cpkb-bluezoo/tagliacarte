/*
 * process_log_stub.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * Web: no dart:io; browser console receives print output.
 */

void appLogStdout(String message) {
  // ignore: avoid_print — web has no dart:io stdout; console is the visible sink.
  print(message);
}

void appLogStderr(String message) {
  // ignore: avoid_print — web has no dart:io stderr; console is the visible sink.
  print(message);
}
