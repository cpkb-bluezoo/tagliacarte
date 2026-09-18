import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

enum CalendarViewMode { day, week, month, year }

const String _kCalendarViewMode = 'tagliacarte.calendar.view_mode';

final calendarViewModeProvider = StateNotifierProvider<CalendarViewModeNotifier, CalendarViewMode>(
  (Ref ref) => CalendarViewModeNotifier(),
);

class CalendarViewModeNotifier extends StateNotifier<CalendarViewMode> {
  CalendarViewModeNotifier() : super(CalendarViewMode.month) {
    _load();
  }

  Future<void> _load() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_kCalendarViewMode);
    state = switch (raw) {
      'day' => CalendarViewMode.day,
      'week' => CalendarViewMode.week,
      'year' => CalendarViewMode.year,
      _ => CalendarViewMode.month,
    };
  }

  Future<void> setMode(CalendarViewMode mode) async {
    final prefs = await SharedPreferences.getInstance();
    final raw = switch (mode) {
      CalendarViewMode.day => 'day',
      CalendarViewMode.week => 'week',
      CalendarViewMode.month => 'month',
      CalendarViewMode.year => 'year',
    };
    await prefs.setString(_kCalendarViewMode, raw);
    state = mode;
  }
}
