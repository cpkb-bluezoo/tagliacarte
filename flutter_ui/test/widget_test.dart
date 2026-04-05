// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

/*
 * widget_test.dart
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

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tagliacarte_ui/app.dart';
import 'package:tagliacarte_ui/src/models/mail_pending_transfer.dart';
import 'package:tagliacarte_ui/src/providers/app_state.dart';
import 'package:tagliacarte_ui/src/rust/tagliacarte_api.dart';
import 'package:tagliacarte_ui/src/screens/home_screen.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('mailPendingTransferProvider replaces previous tag', () {
    final ProviderContainer container = ProviderContainer();
    addTearDown(container.dispose);
    final MailPendingTransfer first = MailPendingTransfer(
      kind: MailPendingTransferKind.moveOp,
      sourceAccountId: 'acc1',
      sourceFolder: 'INBOX',
      messageIds: <String>['1'],
    );
    container.read(mailPendingTransferProvider.notifier).state = first;
    expect(container.read(mailPendingTransferProvider), first);
    final MailPendingTransfer second = MailPendingTransfer(
      kind: MailPendingTransferKind.copyOp,
      sourceAccountId: 'acc1',
      sourceFolder: 'Sent',
      messageIds: <String>['2', '3'],
    );
    container.read(mailPendingTransferProvider.notifier).state = second;
    expect(container.read(mailPendingTransferProvider)?.kind,
        MailPendingTransferKind.copyOp);
    expect(container.read(mailPendingTransferProvider)?.messageIds,
        <String>['2', '3']);
  });

  testWidgets('App boots to home with mock account', (WidgetTester tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          accountsConfigProvider.overrideWith((Ref ref) async {
            return AppSettingsConfig.defaults().copyWith(
              accounts: <AppAccount>[
                AppAccount(
                  id: 's_test',
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
    expect(find.byType(HomeScreen), findsOneWidget);
  });
}
