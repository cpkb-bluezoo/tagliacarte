import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../l10n/app_localizations.dart';
import '../providers/app_state.dart';
import '../providers/calendar_view_prefs.dart';
import '../rust/frb_api.dart';
import '../rust/frb_api/frb_calendar.dart';
import '../rust/tagliacarte_api.dart';
import '../widgets/lucide_icon.dart';

class CalendarScreen extends ConsumerStatefulWidget {
  const CalendarScreen({super.key});

  @override
  ConsumerState<CalendarScreen> createState() => _CalendarScreenState();
}

class _CalendarScreenState extends ConsumerState<CalendarScreen> {
  late Future<List<FrbCalendarDescriptor>> _calendarsFuture;
  late Future<FrbCalendarEventRangeResult> _eventsFuture;
  final Set<String> _selectedCalendars = <String>{};
  DateTime _anchorDate = DateTime.now();

  CalendarViewMode get _mode => ref.read(calendarViewModeProvider);

  @override
  void initState() {
    super.initState();
    _calendarsFuture = frbCalendarListCalendars();
    _eventsFuture = _loadEvents();
  }

  Future<FrbCalendarEventRangeResult> _loadEvents() {
    final DateTime localStart = _rangeStartForMode(_anchorDate, _mode);
    final DateTime localEnd = _rangeEndForMode(_anchorDate, _mode);
    return frbCalendarListEventsRange(
      startUtcMs: localStart.toUtc().millisecondsSinceEpoch,
      endUtcMs: localEnd.toUtc().millisecondsSinceEpoch,
      calendarIds: _selectedCalendars.toList(),
    );
  }

  void _refreshEvents() {
    setState(() {
      _eventsFuture = _loadEvents();
    });
  }

  Future<void> _setMode(CalendarViewMode mode) async {
    await ref.read(calendarViewModeProvider.notifier).setMode(mode);
    _refreshEvents();
  }

  void _shiftAnchor(int direction) {
    setState(() {
      switch (_mode) {
        case CalendarViewMode.day:
          _anchorDate = _anchorDate.add(Duration(days: direction));
          break;
        case CalendarViewMode.week:
          _anchorDate = _anchorDate.add(Duration(days: 7 * direction));
          break;
        case CalendarViewMode.month:
          _anchorDate = DateTime(_anchorDate.year, _anchorDate.month + direction, 1);
          break;
        case CalendarViewMode.year:
          _anchorDate = DateTime(_anchorDate.year + direction, 1, 1);
          break;
      }
    });
    _refreshEvents();
  }

  void _jumpToToday() {
    setState(() {
      _anchorDate = DateTime.now();
    });
    _refreshEvents();
  }

  Future<void> _syncProviderAccounts() async {
    final l10n = AppLocalizations.of(context);
    final cfg = ref.read(accountsConfigProvider).valueOrNull;
    final accounts = cfg?.accounts ?? const <AppAccount>[];
    final now = DateTime.now().toUtc();
    final startMs = now.subtract(const Duration(days: 14)).millisecondsSinceEpoch;
    final endMs = now.add(const Duration(days: 120)).millisecondsSinceEpoch;
    int totalCalendars = 0;
    int totalEvents = 0;
    for (final a in accounts) {
      final t = a.backendType.trim().toLowerCase();
      final provider = a.attrs['oauthProvider']?.trim().toLowerCase() ?? '';
      final bool isProvider = t == 'gmail' || t == 'graph' || t == 'exchange' || provider == 'google' || provider == 'microsoft';
      if (!isProvider) {
        continue;
      }
      try {
        final res = await frbCalendarSyncProviderAccount(
          accountId: a.id,
          startUtcMs: startMs,
          endUtcMs: endMs,
        );
        totalCalendars += res.fetchedCount.toInt();
        totalEvents += res.importedCount.toInt();
      } catch (_) {}
    }
    if (!mounted) {
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(l10n.calendarSyncComplete(totalCalendars, totalEvents))),
    );
    setState(() {
      _calendarsFuture = frbCalendarListCalendars();
      _eventsFuture = _loadEvents();
    });
  }

  Future<void> _openCreateDialog(List<FrbCalendarDescriptor> calendars) async {
    if (calendars.isEmpty) {
      if (!mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('No calendar available. Connect/sync first.')),
      );
      return;
    }
    final bool created = await showDialog<bool>(
          context: context,
          builder: (BuildContext context) => _EventEditorDialog(
            calendars: calendars,
            initialStart: DateTime.now(),
          ),
        ) ??
        false;
    if (created) {
      _refreshEvents();
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final localeTag = Localizations.localeOf(context).toLanguageTag();
    final DateFormat rangeFmt = DateFormat.yMMMMd(localeTag);
    final DateFormat monthTitleFmt = DateFormat.yMMMM(localeTag);
    final range = (
      start: _rangeStartForMode(_anchorDate, _mode),
      end: _rangeEndForMode(_anchorDate, _mode),
    );
    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(l10n.calendarTitle),
            Text(
              '${rangeFmt.format(range.start)} - ${rangeFmt.format(range.end.subtract(const Duration(days: 1)))}',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
        actions: <Widget>[
          IconButton(
            tooltip: 'Previous',
            onPressed: () => _shiftAnchor(-1),
            icon: const Icon(Icons.chevron_left),
          ),
          IconButton(
            tooltip: 'Today',
            onPressed: _jumpToToday,
            icon: const Icon(Icons.today),
          ),
          IconButton(
            tooltip: 'Next',
            onPressed: () => _shiftAnchor(1),
            icon: const Icon(Icons.chevron_right),
          ),
          IconButton(
            tooltip: 'Connect calendars',
            onPressed: () => Navigator.of(context).pushNamed('/settings'),
            icon: const LucideIcon(LucideIcons.settings, size: 20),
          ),
          IconButton(
            tooltip: l10n.calendarSyncProviderTooltip,
            onPressed: _syncProviderAccounts,
            icon: const Icon(Icons.sync_rounded),
          ),
        ],
      ),
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 8, 12, 0),
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                SegmentedButton<CalendarViewMode>(
                  segments: const <ButtonSegment<CalendarViewMode>>[
                    ButtonSegment<CalendarViewMode>(value: CalendarViewMode.day, label: Text('Day')),
                    ButtonSegment<CalendarViewMode>(value: CalendarViewMode.week, label: Text('Week')),
                    ButtonSegment<CalendarViewMode>(value: CalendarViewMode.month, label: Text('Month')),
                    ButtonSegment<CalendarViewMode>(value: CalendarViewMode.year, label: Text('Year')),
                  ],
                  selected: <CalendarViewMode>{_mode},
                  onSelectionChanged: (Set<CalendarViewMode> selected) => _setMode(selected.first),
                ),
                Text(monthTitleFmt.format(_anchorDate), style: Theme.of(context).textTheme.titleMedium),
              ],
            ),
          ),
          FutureBuilder<List<FrbCalendarDescriptor>>(
            future: _calendarsFuture,
            builder: (BuildContext context, AsyncSnapshot<List<FrbCalendarDescriptor>> snap) {
              final items = snap.data ?? const <FrbCalendarDescriptor>[];
              if (items.isEmpty) {
                return const SizedBox.shrink();
              }
              return SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                child: Row(
                  children: items.map((FrbCalendarDescriptor c) {
                    final bool selected = _selectedCalendars.isEmpty || _selectedCalendars.contains(c.calendarId);
                    return Padding(
                      padding: const EdgeInsets.only(right: 8),
                      child: FilterChip(
                        label: Text(c.name),
                        selected: selected,
                        onSelected: (bool v) {
                          setState(() {
                            if (_selectedCalendars.isEmpty && !v) {
                              _selectedCalendars.addAll(items.map((e) => e.calendarId));
                              _selectedCalendars.remove(c.calendarId);
                            } else if (v) {
                              _selectedCalendars.add(c.calendarId);
                            } else {
                              _selectedCalendars.remove(c.calendarId);
                            }
                          });
                          _refreshEvents();
                        },
                      ),
                    );
                  }).toList(),
                ),
              );
            },
          ),
          Expanded(
            child: FutureBuilder<FrbCalendarEventRangeResult>(
              future: _eventsFuture,
              builder: (BuildContext context, AsyncSnapshot<FrbCalendarEventRangeResult> snap) {
                if (snap.connectionState == ConnectionState.waiting) {
                  return const Center(child: CircularProgressIndicator());
                }
                if (snap.hasError) {
                  return Center(child: Text(l10n.calendarLoadFailed(snap.error.toString())));
                }
                final events = snap.data?.events ?? const <FrbCalendarEvent>[];
                return FutureBuilder<List<FrbCalendarDescriptor>>(
                  future: _calendarsFuture,
                  builder: (context, calSnap) {
                    final calendars = calSnap.data ?? const <FrbCalendarDescriptor>[];
                    final calById = <String, FrbCalendarDescriptor>{for (final c in calendars) c.calendarId: c};
                    switch (_mode) {
                      case CalendarViewMode.day:
                        return _DayView(date: _anchorDate, events: events, calendarById: calById, onMutate: _refreshEvents);
                      case CalendarViewMode.week:
                        return _WeekView(anchorDate: _anchorDate, events: events, calendarById: calById, onMutate: _refreshEvents);
                      case CalendarViewMode.month:
                        return _MonthView(anchorDate: _anchorDate, events: events, calendarById: calById, onMutate: _refreshEvents);
                      case CalendarViewMode.year:
                        return _YearView(anchorDate: _anchorDate, events: events, calendarById: calById, onMutate: _refreshEvents);
                    }
                  },
                );
              },
            ),
          ),
        ],
      ),
      floatingActionButton: FutureBuilder<List<FrbCalendarDescriptor>>(
        future: _calendarsFuture,
        builder: (BuildContext context, AsyncSnapshot<List<FrbCalendarDescriptor>> snap) {
          return FloatingActionButton(
            tooltip: 'Add event',
            onPressed: () => _openCreateDialog(snap.data ?? const <FrbCalendarDescriptor>[]),
            child: const LucideIcon(LucideIcons.circlePlus, size: 22),
          );
        },
      ),
    );
  }
}

DateTime _rangeStartForMode(DateTime anchor, CalendarViewMode mode) {
  final day = DateTime(anchor.year, anchor.month, anchor.day);
  switch (mode) {
    case CalendarViewMode.day:
      return day;
    case CalendarViewMode.week:
      return day.subtract(Duration(days: day.weekday - DateTime.monday));
    case CalendarViewMode.month:
      return DateTime(anchor.year, anchor.month, 1);
    case CalendarViewMode.year:
      return DateTime(anchor.year, 1, 1);
  }
}

DateTime _rangeEndForMode(DateTime anchor, CalendarViewMode mode) {
  final start = _rangeStartForMode(anchor, mode);
  switch (mode) {
    case CalendarViewMode.day:
      return start.add(const Duration(days: 1));
    case CalendarViewMode.week:
      return start.add(const Duration(days: 7));
    case CalendarViewMode.month:
      return DateTime(start.year, start.month + 1, 1);
    case CalendarViewMode.year:
      return DateTime(start.year + 1, 1, 1);
  }
}

Color _calendarColor(FrbCalendarDescriptor? calendar, BuildContext context) {
  final fallback = Theme.of(context).colorScheme.primary;
  final colorHex = calendar?.colorHex;
  if (colorHex == null || colorHex.trim().isEmpty) {
    return fallback;
  }
  final value = colorHex.replaceAll('#', '').trim();
  if (value.length != 6 && value.length != 8) {
    return fallback;
  }
  final parsed = int.tryParse(value, radix: 16);
  if (parsed == null) {
    return fallback;
  }
  if (value.length == 6) {
    return Color(0xFF000000 | parsed);
  }
  return Color(parsed);
}

DateTime _eventStartLocal(FrbCalendarEvent ev) => DateTime.fromMillisecondsSinceEpoch(ev.startsAtUtcMs.toInt(), isUtc: true).toLocal();
DateTime _eventEndLocal(FrbCalendarEvent ev) => DateTime.fromMillisecondsSinceEpoch(ev.endsAtUtcMs.toInt(), isUtc: true).toLocal();
bool _sameDay(DateTime a, DateTime b) => a.year == b.year && a.month == b.month && a.day == b.day;

bool _intersectsDay(FrbCalendarEvent ev, DateTime day) {
  final start = _eventStartLocal(ev);
  final end = _eventEndLocal(ev);
  final begin = DateTime(day.year, day.month, day.day);
  final finish = begin.add(const Duration(days: 1));
  return end.isAfter(begin) && start.isBefore(finish);
}

List<FrbCalendarEvent> _eventsForDay(List<FrbCalendarEvent> events, DateTime day) {
  final out = events.where((ev) => _intersectsDay(ev, day)).toList();
  out.sort((a, b) => _eventStartLocal(a).compareTo(_eventStartLocal(b)));
  return out;
}

class _DayView extends StatelessWidget {
  const _DayView({required this.date, required this.events, required this.calendarById, required this.onMutate});
  final DateTime date;
  final List<FrbCalendarEvent> events;
  final Map<String, FrbCalendarDescriptor> calendarById;
  final VoidCallback onMutate;

  @override
  Widget build(BuildContext context) {
    return _EventList(events: _eventsForDay(events, date), calendarById: calendarById, onMutate: onMutate);
  }
}

class _WeekView extends StatelessWidget {
  const _WeekView({required this.anchorDate, required this.events, required this.calendarById, required this.onMutate});
  final DateTime anchorDate;
  final List<FrbCalendarEvent> events;
  final Map<String, FrbCalendarDescriptor> calendarById;
  final VoidCallback onMutate;

  @override
  Widget build(BuildContext context) {
    final start = anchorDate.subtract(Duration(days: anchorDate.weekday - DateTime.monday));
    final localeTag = Localizations.localeOf(context).toLanguageTag();
    final dayFmt = DateFormat.E(localeTag);
    final dateFmt = DateFormat.Md(localeTag);
    return ListView.builder(
      itemCount: 7,
      itemBuilder: (BuildContext context, int index) {
        final day = start.add(Duration(days: index));
        final dayEvents = _eventsForDay(events, day);
        return ExpansionTile(
          title: Text('${dayFmt.format(day)} ${dateFmt.format(day)}'),
          subtitle: Text('${dayEvents.length} event(s)'),
          initiallyExpanded: _sameDay(day, DateTime.now()),
          children: <Widget>[
            if (dayEvents.isEmpty) const ListTile(dense: true, title: Text('No events')) else ...dayEvents.map((ev) => _EventTile(event: ev, color: _calendarColor(calendarById[ev.calendarId], context), onMutate: onMutate)),
          ],
        );
      },
    );
  }
}

class _MonthView extends StatelessWidget {
  const _MonthView({required this.anchorDate, required this.events, required this.calendarById, required this.onMutate});
  final DateTime anchorDate;
  final List<FrbCalendarEvent> events;
  final Map<String, FrbCalendarDescriptor> calendarById;
  final VoidCallback onMutate;

  @override
  Widget build(BuildContext context) {
    final monthStart = DateTime(anchorDate.year, anchorDate.month, 1);
    final gridStart = monthStart.subtract(Duration(days: monthStart.weekday - DateTime.monday));
    final localeTag = Localizations.localeOf(context).toLanguageTag();
    final headFmt = DateFormat.E(localeTag);
    final weekDays = List<Widget>.generate(7, (i) => Center(child: Text(headFmt.format(DateTime(2026, 1, 5 + i)), style: Theme.of(context).textTheme.labelMedium)));
    return Column(
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          child: Row(children: weekDays.map((w) => Expanded(child: w)).toList()),
        ),
        Expanded(
          child: GridView.builder(
            padding: const EdgeInsets.all(8),
            gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
              crossAxisCount: 7,
              childAspectRatio: 1.1,
              crossAxisSpacing: 6,
              mainAxisSpacing: 6,
            ),
            itemCount: 42,
            itemBuilder: (BuildContext context, int index) {
              final day = gridStart.add(Duration(days: index));
              final dayEvents = _eventsForDay(events, day);
              final inMonth = day.month == anchorDate.month;
              return InkWell(
                onTap: dayEvents.isEmpty
                    ? null
                    : () => showModalBottomSheet<void>(
                          context: context,
                          builder: (ctx) => SafeArea(child: _EventList(events: dayEvents, calendarById: calendarById, onMutate: onMutate)),
                        ),
                child: Container(
                  padding: const EdgeInsets.all(6),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: _sameDay(day, DateTime.now()) ? Theme.of(context).colorScheme.primary : Theme.of(context).dividerColor),
                    color: inMonth ? Theme.of(context).colorScheme.surface : Theme.of(context).colorScheme.surfaceContainerLowest,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text('${day.day}', style: Theme.of(context).textTheme.labelLarge?.copyWith(color: inMonth ? null : Theme.of(context).disabledColor)),
                      const SizedBox(height: 4),
                      ...dayEvents.take(3).map((ev) {
                        final color = _calendarColor(calendarById[ev.calendarId], context);
                        return Container(
                          margin: const EdgeInsets.only(bottom: 2),
                          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                          decoration: BoxDecoration(color: color.withAlpha(43), borderRadius: BorderRadius.circular(4)),
                          child: Text(ev.title.isEmpty ? '(Untitled)' : ev.title, maxLines: 1, overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.labelSmall),
                        );
                      }),
                      if (dayEvents.length > 3) Text('+${dayEvents.length - 3} more', style: Theme.of(context).textTheme.labelSmall),
                    ],
                  ),
                ),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _YearView extends StatelessWidget {
  const _YearView({required this.anchorDate, required this.events, required this.calendarById, required this.onMutate});
  final DateTime anchorDate;
  final List<FrbCalendarEvent> events;
  final Map<String, FrbCalendarDescriptor> calendarById;
  final VoidCallback onMutate;

  @override
  Widget build(BuildContext context) {
    final localeTag = Localizations.localeOf(context).toLanguageTag();
    final monthFmt = DateFormat.MMMM(localeTag);
    return GridView.builder(
      padding: const EdgeInsets.all(12),
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 3,
        mainAxisSpacing: 10,
        crossAxisSpacing: 10,
        childAspectRatio: 1.8,
      ),
      itemCount: 12,
      itemBuilder: (BuildContext context, int index) {
        final month = DateTime(anchorDate.year, index + 1, 1);
        final monthEnd = DateTime(anchorDate.year, index + 2, 1);
        final monthEvents = events.where((ev) {
          final start = _eventStartLocal(ev);
          final end = _eventEndLocal(ev);
          return end.isAfter(month) && start.isBefore(monthEnd);
        }).toList();
        return Card(
          child: Padding(
            padding: const EdgeInsets.all(10),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(monthFmt.format(month), style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 8),
                Text('${monthEvents.length} event(s)'),
                const Spacer(),
                Wrap(
                  spacing: 4,
                  runSpacing: 4,
                  children: monthEvents.take(6).map((ev) {
                    final c = _calendarColor(calendarById[ev.calendarId], context);
                    return Container(width: 8, height: 8, decoration: BoxDecoration(color: c, shape: BoxShape.circle));
                  }).toList(),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _EventList extends StatelessWidget {
  const _EventList({required this.events, required this.calendarById, required this.onMutate});
  final List<FrbCalendarEvent> events;
  final Map<String, FrbCalendarDescriptor> calendarById;
  final VoidCallback onMutate;

  @override
  Widget build(BuildContext context) {
    if (events.isEmpty) {
      return const Center(child: Text('No events'));
    }
    return ListView.builder(
      itemCount: events.length,
      itemBuilder: (BuildContext context, int index) {
        final ev = events[index];
        return _EventTile(event: ev, color: _calendarColor(calendarById[ev.calendarId], context), onMutate: onMutate);
      },
    );
  }
}

class _EventTile extends StatelessWidget {
  const _EventTile({required this.event, required this.color, required this.onMutate});
  final FrbCalendarEvent event;
  final Color color;
  final VoidCallback onMutate;

  @override
  Widget build(BuildContext context) {
    final localeTag = Localizations.localeOf(context).toLanguageTag();
    final dateFmt = DateFormat.yMMMMd(localeTag);
    final timeFmt = DateFormat.Hm(localeTag);
    final start = _eventStartLocal(event);
    final end = _eventEndLocal(event);
    final subtitle = event.allDay ? dateFmt.format(start) : '${dateFmt.format(start)} ${timeFmt.format(start)} - ${timeFmt.format(end)}';
    return ListTile(
      leading: Container(width: 12, height: 40, decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(8))),
      title: Text(event.title.isEmpty ? '(Untitled event)' : event.title),
      subtitle: Text(subtitle),
      trailing: PopupMenuButton<String>(
        onSelected: (String action) async {
          if (action == 'reschedule') {
            await _importEventMutation(
              context: context,
              calendarId: event.calendarId,
              uid: event.uid,
              sequence: event.sequence.toInt() + 1,
              title: event.title,
              start: start.add(const Duration(hours: 1)),
              end: end.add(const Duration(hours: 1)),
              attendees: event.attendees,
              status: event.status ?? 'CONFIRMED',
            );
          } else if (action == 'invite') {
            await _importEventMutation(
              context: context,
              calendarId: event.calendarId,
              uid: event.uid,
              sequence: event.sequence.toInt() + 1,
              title: event.title,
              start: start,
              end: end,
              attendees: <String>[...event.attendees, 'new.person@example.com'],
              status: event.status ?? 'CONFIRMED',
            );
          } else if (action == 'delete') {
            await _importEventMutation(
              context: context,
              calendarId: event.calendarId,
              uid: event.uid,
              sequence: event.sequence.toInt() + 1,
              title: event.title,
              start: start,
              end: end,
              attendees: event.attendees,
              status: 'CANCELLED',
            );
          }
          onMutate();
        },
        itemBuilder: (BuildContext context) => const <PopupMenuEntry<String>>[
          PopupMenuItem<String>(value: 'reschedule', child: Text('Reschedule (+1h)')),
          PopupMenuItem<String>(value: 'invite', child: Text('Invite people')),
          PopupMenuItem<String>(value: 'delete', child: Text('Delete')),
        ],
      ),
    );
  }
}

class _EventEditorDialog extends StatefulWidget {
  const _EventEditorDialog({required this.calendars, required this.initialStart});
  final List<FrbCalendarDescriptor> calendars;
  final DateTime initialStart;

  @override
  State<_EventEditorDialog> createState() => _EventEditorDialogState();
}

class _EventEditorDialogState extends State<_EventEditorDialog> {
  late final TextEditingController _title;
  late final TextEditingController _attendees;
  late DateTime _start;
  late DateTime _end;
  late String _calendarId;

  @override
  void initState() {
    super.initState();
    _title = TextEditingController();
    _attendees = TextEditingController();
    _start = widget.initialStart;
    _end = widget.initialStart.add(const Duration(hours: 1));
    _calendarId = widget.calendars.first.calendarId;
  }

  @override
  void dispose() {
    _title.dispose();
    _attendees.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return AlertDialog(
      title: const Text('Add event'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            DropdownButtonFormField<String>(
              initialValue: _calendarId,
              items: widget.calendars.map((c) => DropdownMenuItem<String>(value: c.calendarId, child: Text(c.name))).toList(),
              onChanged: (v) {
                if (v != null) {
                  setState(() {
                    _calendarId = v;
                  });
                }
              },
              decoration: const InputDecoration(labelText: 'Calendar'),
            ),
            TextField(controller: _title, decoration: const InputDecoration(labelText: 'Title')),
            TextField(
              controller: _attendees,
              decoration: const InputDecoration(labelText: 'Invitees', hintText: 'Comma-separated emails'),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(onPressed: () => Navigator.of(context).pop(false), child: Text(l10n.cancel)),
        FilledButton(
          onPressed: () async {
            final title = _title.text.trim();
            if (title.isEmpty) {
              return;
            }
            final attendees = _attendees.text.split(',').map((s) => s.trim()).where((s) => s.isNotEmpty).toList();
            await _importEventMutation(
              context: context,
              calendarId: _calendarId,
              uid: 'tc-${DateTime.now().microsecondsSinceEpoch}',
              sequence: 0,
              title: title,
              start: _start,
              end: _end,
              attendees: attendees,
              status: 'CONFIRMED',
            );
            if (!context.mounted) {
              return;
            }
            Navigator.of(context).pop(true);
          },
          child: Text(l10n.create),
        ),
      ],
    );
  }
}

Future<void> _importEventMutation({
  required BuildContext context,
  required String calendarId,
  required String uid,
  required int sequence,
  required String title,
  required DateTime start,
  required DateTime end,
  required List<String> attendees,
  required String status,
}) async {
  final payload = _buildSingleEventIcs(
    uid: uid,
    sequence: sequence,
    title: title,
    start: start,
    end: end,
    attendees: attendees,
    status: status,
  );
  try {
    await frbCalendarImportIcs(calendarId: calendarId, icsPayload: payload, sourceHint: 'ui_edit');
  } catch (e) {
    if (!context.mounted) {
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Calendar update failed: $e')));
  }
}

String _buildSingleEventIcs({
  required String uid,
  required int sequence,
  required String title,
  required DateTime start,
  required DateTime end,
  required List<String> attendees,
  required String status,
}) {
  String fmt(DateTime d) => DateFormat("yyyyMMdd'T'HHmmss'Z'").format(d.toUtc());
  final attendeeLines = attendees.map((a) => 'ATTENDEE;CN=$a:mailto:$a').join('\r\n');
  return [
    'BEGIN:VCALENDAR',
    'VERSION:2.0',
    'PRODID:-//Tagliacarte//EN',
    'BEGIN:VEVENT',
    'UID:$uid',
    'SEQUENCE:$sequence',
    'SUMMARY:${title.replaceAll('\n', ' ')}',
    'DTSTART:${fmt(start)}',
    'DTEND:${fmt(end)}',
    'STATUS:$status',
    if (attendeeLines.isNotEmpty) attendeeLines,
    'END:VEVENT',
    'END:VCALENDAR',
    '',
  ].join('\r\n');
}
