/*
 * app_test.dart
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

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'package:tagliacarte_ui/app.dart';
import 'package:tagliacarte_ui/src/providers/app_state.dart';
import 'package:tagliacarte_ui/src/rust/tagliacarte_api.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('Home screen bootstraps', (tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          accountsConfigProvider.overrideWith((Ref ref) async {
            return AppSettingsConfig.defaults().copyWith(
              accounts: <AppAccount>[
                AppAccount(
                  id: 'maildir://test',
                  label: 'Test',
                  backendType: 'maildir',
                  storeUri: 'maildir://test',
                ),
              ],
            );
          }),
        ],
        child: const TagliacarteApp(),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.textContaining('Tagliacarte'), findsWidgets);
  });
}
