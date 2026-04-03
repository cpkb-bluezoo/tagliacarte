/*
 * compose_screen.dart
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

import '../l10n/app_localizations.dart';

class ComposeScreen extends StatefulWidget {
  const ComposeScreen({super.key});

  @override
  State<ComposeScreen> createState() => _ComposeScreenState();
}

class _ComposeScreenState extends State<ComposeScreen> {
  final _to = TextEditingController();
  final _cc = TextEditingController();
  final _bcc = TextEditingController();
  final _subject = TextEditingController();
  final _body = TextEditingController();

  @override
  void dispose() {
    _to.dispose();
    _cc.dispose();
    _bcc.dispose();
    _subject.dispose();
    _body.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.compose),
        actions: [
          TextButton(
            onPressed: () {},
            child: Text(l10n.send),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(12),
        children: [
          TextField(
            controller: _to,
            decoration: InputDecoration(labelText: l10n.fieldTo),
          ),
          TextField(
            controller: _cc,
            decoration: InputDecoration(labelText: l10n.fieldCc),
          ),
          TextField(
            controller: _bcc,
            decoration: InputDecoration(labelText: l10n.fieldBcc),
          ),
          TextField(
            controller: _subject,
            decoration: InputDecoration(labelText: l10n.fieldSubject),
          ),
          const SizedBox(height: 8),
          TextField(
            controller: _body,
            minLines: 12,
            maxLines: 24,
            decoration: InputDecoration(
              labelText: l10n.fieldBody,
              border: const OutlineInputBorder(),
            ),
          ),
        ],
      ),
    );
  }
}
