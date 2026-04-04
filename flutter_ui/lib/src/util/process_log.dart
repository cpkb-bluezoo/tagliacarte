/*
 * process_log.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * Frontend-agnostic logging: prefer process stdout/stderr so embedders and future UIs see output
 * without relying on the Flutter IDE debug console.
 */

import 'process_log_io.dart' if (dart.library.html) 'process_log_stub.dart' as process_log_impl;

void appLogStdout(String message) => process_log_impl.appLogStdout(message);

void appLogStderr(String message) => process_log_impl.appLogStderr(message);
