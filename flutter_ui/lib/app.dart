/*
 * app.dart
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

import 'package:flutter/material.dart';
import 'package:flutter_quill/flutter_quill.dart';

import 'src/l10n/app_localizations.dart';
import 'src/screens/compose_screen.dart';
import 'src/screens/home_screen.dart';
import 'src/screens/settings_screen.dart';
import 'src/theme/app_theme.dart';

class TagliacarteApp extends StatelessWidget {
  const TagliacarteApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Tagliacarte',
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      localizationsDelegates: <LocalizationsDelegate<dynamic>>[
        ...AppLocalizations.localizationsDelegates,
        FlutterQuillLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      routes: {
        '/': (_) => const HomeScreen(),
        '/settings': (BuildContext context) {
          final Object? raw = ModalRoute.of(context)?.settings.arguments;
          final int tab = raw is int ? raw : 0;
          return SettingsScreen(initialTabIndex: tab.clamp(0, 6));
        },
        '/compose': (BuildContext context) {
          final Object? raw = ModalRoute.of(context)?.settings.arguments;
          return ComposeScreen(
            intent: raw is ComposeIntent ? raw : null,
          );
        },
      },
      initialRoute: '/',
    );
  }
}
