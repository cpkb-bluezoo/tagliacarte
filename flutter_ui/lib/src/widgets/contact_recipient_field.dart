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
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../rust/frb_api/frb_contacts.dart';

/// Comma-separated recipient field: debounced contact completion, inline grey
/// suffix for the active segment, Tab to insert the full formatted address.
class ContactRecipientField extends StatefulWidget {
  const ContactRecipientField({
    super.key,
    required this.controller,
    required this.focusNode,
    required this.labelText,
    this.debounceMs = 150,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final String labelText;
  final int debounceMs;

  @override
  State<ContactRecipientField> createState() => _ContactRecipientFieldState();
}

class _ContactRecipientFieldState extends State<ContactRecipientField> {
  Timer? _debounce;
  String _ghostSuffix = '';
  String _fullAddress = '';

  String _querySegment(String full) {
    final int comma = full.lastIndexOf(',');
    final String tail = comma < 0 ? full : full.substring(comma + 1);
    return tail.trim();
  }

  /// Text before the last comma (including comma), or empty if none.
  String _prefixBeforeSegment(String full) {
    final int comma = full.lastIndexOf(',');
    if (comma < 0) {
      return '';
    }
    return full.substring(0, comma + 1);
  }

  /// Raw text of the last segment (after last comma), including leading spaces.
  String _rawSegment(String full) {
    final int comma = full.lastIndexOf(',');
    if (comma < 0) {
      return full;
    }
    return full.substring(comma + 1);
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

  bool get _canComplete {
    final String tail = _querySegment(widget.controller.text);
    if (tail.isEmpty || _fullAddress.isEmpty) {
      return false;
    }
    if (_ghostSuffix.isNotEmpty) {
      return true;
    }
    return tail.toLowerCase() != _fullAddress.toLowerCase();
  }

  void _applyTabCompletion() {
    if (!_canComplete) {
      return;
    }
    _replaceTail(widget.controller.text, _fullAddress);
    setState(() {
      _ghostSuffix = '';
      _fullAddress = '';
    });
  }

  void _scheduleFetch() {
    _debounce?.cancel();
    _debounce = Timer(Duration(milliseconds: widget.debounceMs), _fetchCompletion);
  }

  Future<void> _fetchCompletion() async {
    final String tail = _querySegment(widget.controller.text);
    if (tail.isEmpty) {
      if (mounted) {
        setState(() {
          _ghostSuffix = '';
          _fullAddress = '';
        });
      }
      return;
    }
    try {
      final FrbRecipientCompletion? r =
          await frbContactsRecipientCompletion(prefix: tail);
      if (!mounted) {
        return;
      }
      final String tail2 = _querySegment(widget.controller.text);
      if (tail2 != tail) {
        return;
      }
      setState(() {
        if (r == null) {
          _ghostSuffix = '';
          _fullAddress = '';
        } else {
          _fullAddress = r.fullAddress;
          _ghostSuffix = r.ghostSuffix;
        }
      });
    } catch (_) {
      if (mounted) {
        setState(() {
          _ghostSuffix = '';
          _fullAddress = '';
        });
      }
    }
  }

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onControllerChanged);
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onControllerChanged);
    _debounce?.cancel();
    super.dispose();
  }

  void _onControllerChanged() {
    _scheduleFetch();
  }

  @override
  Widget build(BuildContext context) {
    final TextTheme theme = Theme.of(context).textTheme;
    final TextStyle baseStyle =
        theme.bodyLarge ?? const TextStyle(fontSize: 16, height: 1.4);
    final ColorScheme cs = Theme.of(context).colorScheme;
    final TextStyle ghostStyle = baseStyle.copyWith(
      color: cs.onSurface.withValues(alpha: 0.38),
    );

    final String full = widget.controller.text;
    final String beforeSeg = _prefixBeforeSegment(full);
    final String rawSeg = _rawSegment(full);

    return Focus(
      onKeyEvent: (FocusNode node, KeyEvent event) {
        if (event is! KeyDownEvent) {
          return KeyEventResult.ignored;
        }
        if (event.logicalKey == LogicalKeyboardKey.tab) {
          if (_canComplete) {
            _applyTabCompletion();
            return KeyEventResult.handled;
          }
        }
        return KeyEventResult.ignored;
      },
      child: Stack(
        alignment: Alignment.topLeft,
        children: <Widget>[
          ExcludeSemantics(
            child: Padding(
              padding: EdgeInsets.zero,
              child: RichText(
                textAlign: TextAlign.left,
                text: TextSpan(
                  style: baseStyle,
                  children: <InlineSpan>[
                    TextSpan(text: beforeSeg),
                    TextSpan(text: rawSeg),
                    TextSpan(text: _ghostSuffix, style: ghostStyle),
                  ],
                ),
                maxLines: 4,
              ),
            ),
          ),
          TextField(
            controller: widget.controller,
            focusNode: widget.focusNode,
            style: baseStyle.copyWith(color: Colors.transparent),
            cursorColor: cs.onSurface,
            minLines: 1,
            maxLines: 4,
            decoration: InputDecoration(
              labelText: widget.labelText,
            ),
          ),
        ],
      ),
    );
  }
}
