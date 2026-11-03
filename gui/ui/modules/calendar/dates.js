// Calendar arithmetic, with no page in it. Weeks start on Monday, as they did in
// the Flutter calendar.
//
// Times are milliseconds in UTC, as the back end keeps them. A timed event is
// shown where it falls in the viewer's own day. An all-day event is a date, held
// as UTC midnight of its first day with the end at the midnight after its last, so
// it is read by its UTC date: read through local time it would slip a day in any
// zone west of UTC.

export const MODES = ['day', 'week', 'month', 'year'];

const DAY = 24 * 60 * 60 * 1000;

// A date's own midnight, locally.
export function startOfDay(d) {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

export function addDays(d, n) {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

export function sameDay(a, b) {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

// The Monday on or before a date.
export function mondayOf(d) {
  const day = startOfDay(d);
  return addDays(day, -((day.getDay() + 6) % 7));
}

// The first day of what the view shows, and the first day after it.
export function rangeStart(mode, anchor) {
  const day = startOfDay(anchor);
  switch (mode) {
    case 'day': return day;
    case 'week': return mondayOf(day);
    case 'month': return new Date(day.getFullYear(), day.getMonth(), 1);
    default: return new Date(day.getFullYear(), 0, 1);
  }
}

export function rangeEnd(mode, anchor) {
  const start = rangeStart(mode, anchor);
  switch (mode) {
    case 'day': return addDays(start, 1);
    case 'week': return addDays(start, 7);
    case 'month': return new Date(start.getFullYear(), start.getMonth() + 1, 1);
    default: return new Date(start.getFullYear() + 1, 0, 1);
  }
}

// One step back (-1) or forward (1) in the view's own unit.
export function shiftAnchor(mode, anchor, direction) {
  const d = startOfDay(anchor);
  switch (mode) {
    case 'day': return addDays(d, direction);
    case 'week': return addDays(d, 7 * direction);
    case 'month': return new Date(d.getFullYear(), d.getMonth() + direction, 1);
    default: return new Date(d.getFullYear() + direction, 0, 1);
  }
}

// The six weeks of a month's grid: 42 days from the Monday on or before the 1st.
export function monthGrid(anchor) {
  const first = mondayOf(new Date(anchor.getFullYear(), anchor.getMonth(), 1));
  const days = [];
  for (let i = 0; i < 42; i++) { days.push(addDays(first, i)); }
  return days;
}

export function weekDays(anchor) {
  const first = mondayOf(anchor);
  const days = [];
  for (let i = 0; i < 7; i++) { days.push(addDays(first, i)); }
  return days;
}

// What has to be fetched to draw the view: what it shows, and a day either side
// since an all-day event is a date and a timed one is in this zone's day.
// Returns [startMs, endMs].
export function fetchRange(mode, anchor) {
  let start = rangeStart(mode, anchor);
  let end = rangeEnd(mode, anchor);
  if (mode === 'month') {
    const grid = monthGrid(anchor);
    start = grid[0];
    end = addDays(grid[41], 1);
  }
  return [addDays(start, -1).getTime(), addDays(end, 1).getTime()];
}

// Whether an event falls on a day (a Date at its local midnight).
export function occursOn(ev, day) {
  if (ev.allDay) {
    const key = Date.UTC(day.getFullYear(), day.getMonth(), day.getDate());
    // An all-day event with no usable end is its one day.
    return ev.startMs <= key && (key < ev.endMs || key === ev.startMs);
  }
  const begin = startOfDay(day).getTime();
  const finish = addDays(day, 1).getTime();
  // A reminder, which starts and ends together, is on the day it starts.
  return ev.startMs < finish && (ev.endMs > begin || ev.startMs >= begin);
}

// The events on a day: those that last all day first, then by when they start.
export function eventsOn(events, day) {
  return events.filter(function (e) { return occursOn(e, day); }).sort(function (a, b) {
    if (a.allDay !== b.allDay) { return a.allDay ? -1 : 1; }
    return a.startMs - b.startMs || a.title.localeCompare(b.title);
  });
}

// Whether a month (a Date at its 1st) has a day with the event on it, for the year view.
export function occursInMonth(ev, month) {
  const next = new Date(month.getFullYear(), month.getMonth() + 1, 1);
  if (ev.allDay) {
    const from = Date.UTC(month.getFullYear(), month.getMonth(), 1);
    const to = Date.UTC(next.getFullYear(), next.getMonth(), 1);
    return ev.startMs < to && (ev.endMs > from || ev.startMs >= from);
  }
  return ev.startMs < next.getTime() && (ev.endMs > month.getTime() || ev.startMs >= month.getTime());
}

// ── what the editor's fields hold ──

function two(n) { return String(n).padStart(2, '0'); }

// 'YYYY-MM-DDTHH:mm' in the viewer's zone, for a datetime-local field.
export function toLocalInput(ms) {
  const d = new Date(ms);
  return d.getFullYear() + '-' + two(d.getMonth() + 1) + '-' + two(d.getDate()) + 'T' + two(d.getHours()) + ':' + two(d.getMinutes());
}

export function fromLocalInput(text) {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(text || '');
  if (!m) { return null; }
  return new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5]).getTime();
}

// 'YYYY-MM-DD' of a date held as UTC midnight, for a date field.
export function toDateInput(ms) {
  const d = new Date(ms);
  return d.getUTCFullYear() + '-' + two(d.getUTCMonth() + 1) + '-' + two(d.getUTCDate());
}

export function fromDateInput(text) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text || '');
  if (!m) { return null; }
  return Date.UTC(+m[1], +m[2] - 1, +m[3]);
}

// The editor shows an all-day event's last day, the store holds the midnight after it.
export function lastDayOf(startMs, endMs) {
  return Math.max(startMs, endMs - DAY);
}

export function dayAfter(ms) {
  return ms + DAY;
}

// When a new event on a day begins: the next whole hour if it is today, else nine in the morning.
export function newEventStart(day, now) {
  if (sameDay(day, now)) {
    return new Date(now.getFullYear(), now.getMonth(), now.getDate(), now.getHours() + 1).getTime();
  }
  return new Date(day.getFullYear(), day.getMonth(), day.getDate(), 9).getTime();
}

// A calendar's colour: its own if it has one, else one made from its id, so that
// each calendar looks the same every time.
export function calendarColour(calendar, hueOf) {
  const hex = calendar && calendar.colorHex ? String(calendar.colorHex).trim() : '';
  if (/^#?[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(hex)) {
    return '#' + hex.replace('#', '').slice(0, 6);
  }
  return 'hsl(' + hueOf(calendar ? calendar.id : '') + ' 55% 45%)';
}
