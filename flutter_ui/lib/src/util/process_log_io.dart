/*
 * process_log_io.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * VM / desktop / mobile: write to process stdout and stderr (not the Flutter tool debug console).
 */

import 'dart:io' show stderr, stdout;

void appLogStdout(String message) {
  stdout.writeln(message);
}

void appLogStderr(String message) {
  stderr.writeln(message);
}
