/*
 * contact_recipient_field.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

import 'dart:convert';

import 'package:flutter/material.dart';
import '../rust/frb_api/frb_contacts.dart';

/// Comma-separated recipient field with debounced contact search autocomplete.
class ContactRecipientField extends StatefulWidget {
  const ContactRecipientField({
    super.key,
    required this.controller,
    required this.focusNode,
    required this.labelText,
    this.debounceMs = 200,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final String labelText;
  final int debounceMs;

  @override
  State<ContactRecipientField> createState() => _ContactRecipientFieldState();
}

class _ContactRecipientFieldState extends State<ContactRecipientField> {
  String _querySegment(String full) {
    final int comma = full.lastIndexOf(',');
    final String tail = comma < 0 ? full : full.substring(comma + 1);
    return tail.trim();
  }

  void _replaceTail(String full, String replacement) {
    final int comma = full.lastIndexOf(',');
    if (comma < 0) {
      widget.controller.text = replacement;
    } else {
      widget.controller.text =
          '${full.substring(0, comma + 1)} $replacement'.trimLeft();
    }
    widget.controller.selection = TextSelection.collapsed(
      offset: widget.controller.text.length,
    );
  }

  @override
  Widget build(BuildContext context) {
    return RawAutocomplete<String>(
      textEditingController: widget.controller,
      focusNode: widget.focusNode,
      displayStringForOption: (String x) => x,
      optionsBuilder: (TextEditingValue value) async {
        final String q = _querySegment(value.text);
        if (q.isEmpty) {
          return const <String>[];
        }
        await Future<void>.delayed(Duration(milliseconds: widget.debounceMs));
        if (!mounted) {
          return const <String>[];
        }
        final String q2 = _querySegment(widget.controller.text);
        if (q2 != q) {
          return const <String>[];
        }
        try {
          final String j = await frbContactsSearch(
            query: q,
            limit: 40,
          );
          final List<dynamic> arr = jsonDecode(j) as List<dynamic>;
          final List<String> opts = <String>[];
          for (final dynamic e in arr) {
            if (e is! Map<String, dynamic>) {
              continue;
            }
            final String name = (e['displayName'] as String? ?? '').trim();
            final List<dynamic> emails =
                e['emails'] as List<dynamic>? ?? const <dynamic>[];
            final String em = emails.isNotEmpty ? emails.first as String : '';
            if (em.isEmpty) {
              continue;
            }
            if (name.isEmpty) {
              opts.add(em);
            } else {
              opts.add('$name <$em>');
            }
          }
          return opts;
        } catch (_) {
          return const <String>[];
        }
      },
      onSelected: (String selection) {
        _replaceTail(widget.controller.text, selection);
      },
      fieldViewBuilder:
          (BuildContext context, TextEditingController c, FocusNode fn, _) {
        return TextField(
          controller: c,
          focusNode: fn,
          decoration: InputDecoration(labelText: widget.labelText),
          minLines: 1,
          maxLines: 4,
        );
      },
      optionsViewBuilder: (BuildContext context, void Function(String) onSel,
          Iterable<String> options) {
        final List<String> list = options.toList();
        if (list.isEmpty) {
          return const SizedBox.shrink();
        }
        return Align(
          alignment: Alignment.topLeft,
          child: Material(
            elevation: 4,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 200, maxWidth: 400),
              child: ListView.builder(
                padding: EdgeInsets.zero,
                shrinkWrap: true,
                itemCount: list.length,
                itemBuilder: (BuildContext context, int i) {
                  final String opt = list[i];
                  return ListTile(
                    dense: true,
                    title: Text(opt),
                    onTap: () => onSel(opt),
                  );
                },
              ),
            ),
          ),
        );
      },
    );
  }
}
