/*
 * main.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'app.dart';
import 'src/providers/new_mail_notification_service.dart';
import 'src/widgets/account_selection_sync.dart';
import 'src/rust/frb_api.dart';
import 'src/rust/frb_generated.dart';

/// macOS App Sandbox blocks loading dylibs from arbitrary paths (e.g. workspace
/// `target/release`). The Xcode "Copy Rust dylib" phase places the library in
/// `Tagliacarte.app/Contents/Frameworks/`; load that when present.
String? _bundledRustDylibPathMacos() {
  if (!Platform.isMacOS) {
    return null;
  }
  final String exe = Platform.resolvedExecutable;
  // .../Tagliacarte.app/Contents/MacOS/Tagliacarte
  final String frameworksDylib =
      '${File(exe).parent.parent.path}/Frameworks/libtagliacarte_app.dylib';
  final File f = File(frameworksDylib);
  return f.existsSync() ? f.path : null;
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  const String explicitRustLibPath = String.fromEnvironment(
    'TAGLIACARTE_RUST_LIB',
  );
  final String? bundled = _bundledRustDylibPathMacos();
  if (bundled != null) {
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(bundled),
    );
  } else if (explicitRustLibPath.isNotEmpty) {
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(explicitRustLibPath),
    );
  } else {
    await RustLib.init();
  }
  await NewMailNotificationService.instance.init();
  // WebView cannot present the ephemeral client cert yet; loopback TLS still encrypts the mail body.
  await frbMailBodySetTlsRequireClientCert(require: false);
  runApp(
    const ProviderScope(
      child: AccountSelectionSync(
        child: TagliacarteApp(),
      ),
    ),
  );
}
