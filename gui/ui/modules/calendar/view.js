// The calendar: a view of the rail's calendar button, in four scales (a day, a
// week, a month, a year), with the calendars as a row of toggles, and the events of
// what is shown. The events of a range arrive as a sequence the back end sends
// after being asked (calendarEventsStarted, calendarEvent, calendarEventsDone, all
// with the request id); one for a range that has been moved on from is dropped.
//
// Showing or hiding a calendar needs no new request: every calendar's events are
// fetched and the page leaves out the hidden ones.

import { state, changed } from '../store.js';
import { call } from '../wire.js';
import { h, setChildren } from '../dom.js';
import { t } from '../i18n.js';
import { toast } from '../toast.js';
import { openModal, confirmModal } from '../modal.js';
import { setLocal } from '../localprefs.js';
import { hueOf } from '../avatar.js';
import { newRequestId, waitForEvent } from '../requests.js';
import * as D from './dates.js';
import { openEventEditor, calendarName } from './editor.js';

let seq = 0;

const root = h('section', { class: 'calendar' });
export function calendarElement() { return root; }

// ── asking ──

function requestEvents() {
  const c = state.calendar;
  const [start, end] = D.fetchRange(c.mode, c.anchor);
  const requestId = 'c' + (++seq);
  c.request = requestId;
  c.loading = true;
  c.error = null;
  call('calendar.events', { requestId: requestId, startMs: start, endMs: end, calendarIds: [] }, function (err) {
    if (err && c.request === requestId) {
      c.loading = false;
      c.error = err.message;
      changed();
    }
  });
}

function reload() {
  call('calendar.load', undefined, function (err) {
    if (err) { state.calendar.error = err.message; changed(); }
  });
  requestEvents();
}

// The rail's calendar button.
export function openCalendar() {
  state.view = 'calendar';
  reload();
  changed();
}

// calendarsStarted / calendarFound / calendarsLoaded, and the events of a range.
export function onCalendarEvent(ev) {
  const c = state.calendar;
  switch (ev.type) {
    case 'calendarsStarted':
      c.calendarsPending = [];
      break;
    case 'calendarFound':
      if (c.calendarsPending) { c.calendarsPending.push(ev); }
      break;
    case 'calendarsLoaded':
      if (c.calendarsPending) {
        c.calendars = c.calendarsPending;
        c.calendarsPending = null;
      }
      break;
    case 'calendarEventsStarted':
      if (ev.requestId === c.request) { c.eventsPending = []; }
      break;
    case 'calendarEvent':
      if (ev.requestId === c.request && c.eventsPending) { c.eventsPending.push(ev); }
      break;
    case 'calendarEventsDone':
      if (ev.requestId === c.request) {
        c.events = c.eventsPending || [];
        c.eventsPending = null;
        c.loading = false;
        c.error = ev.error || null;
      }
      break;
    default:
      break;
  }
}

// ── what the viewer chose ──

function setMode(mode) {
  state.calendar.mode = mode;
  setLocal('calendarMode', mode);
  requestEvents();
  changed();
}

function move(direction) {
  const c = state.calendar;
  c.anchor = D.shiftAnchor(c.mode, c.anchor, direction);
  requestEvents();
  changed();
}

function today() {
  state.calendar.anchor = new Date();
  requestEvents();
  changed();
}

function toggleCalendar(id) {
  const c = state.calendar;
  c.hidden = c.hidden.indexOf(id) >= 0 ? c.hidden.filter(function (x) { return x !== id; }) : c.hidden.concat(id);
  setLocal('calendarHidden', c.hidden);
  changed();
}

// Refreshes the calendars of the accounts and of the sources added by address.
export function syncCalendars() {
  const c = state.calendar;
  if (c.syncing) { return; }
  c.syncing = true;
  const requestId = newRequestId('y');
  waitForEvent(requestId, function (ev) {
    c.syncing = false;
    if (ev.error) {
      toast(t('calendarLoadFailed', { error: ev.error }));
    } else {
      toast(t('calendarSyncComplete', { calendarCount: ev.calendars, eventCount: ev.events }));
      if (ev.failed > 0) { toast(t('webCalendarSyncFailed', { count: ev.failed, first: ev.firstFailure || '' })); }
    }
    reload();
    changed();
  });
  call('calendar.sync', { requestId: requestId }, function (err) {
    if (err) {
      c.syncing = false;
      toast(t('calendarLoadFailed', { error: err.message }));
      changed();
    }
  });
  changed();
}

// ── the events ──

function calendarOf(id) {
  return state.calendar.calendars.find(function (c) { return c.id === id; }) || { id: id, name: id };
}

function colourOf(ev) { return D.calendarColour(calendarOf(ev.calendarId), hueOf); }

// The events the viewer has not hidden the calendar of.
function visibleEvents() {
  const c = state.calendar;
  return c.events.filter(function (e) { return c.hidden.indexOf(e.calendarId) < 0; });
}

const dateLong = new Intl.DateTimeFormat(undefined, { dateStyle: 'long' });
const dateLongUtc = new Intl.DateTimeFormat(undefined, { dateStyle: 'long', timeZone: 'UTC' });
const timeShort = new Intl.DateTimeFormat(undefined, { timeStyle: 'short' });

function whenOf(ev) {
  if (ev.allDay) {
    const lastDay = D.lastDayOf(ev.startMs, ev.endMs);
    return lastDay > ev.startMs
      ? dateLongUtc.format(new Date(ev.startMs)) + ' – ' + dateLongUtc.format(new Date(lastDay))
      : dateLongUtc.format(new Date(ev.startMs));
  }
  const s = new Date(ev.startMs);
  const e = new Date(ev.endMs);
  if (ev.endMs === ev.startMs) { return dateLong.format(s) + ' ' + timeShort.format(s); }
  return D.sameDay(s, e)
    ? dateLong.format(s) + ' ' + timeShort.format(s) + ' – ' + timeShort.format(e)
    : dateLong.format(s) + ' ' + timeShort.format(s) + ' – ' + dateLong.format(e) + ' ' + timeShort.format(e);
}

function eventTitle(ev) { return ev.title || t('webCalendarUntitled'); }

function editEvent(ev) {
  openEventEditor({ event: ev, calendars: state.calendar.calendars, onSaved: requestEvents, onDelete: deleteEvent });
}

function newEvent(day) {
  openEventEditor({ day: day, calendars: state.calendar.calendars, onSaved: requestEvents });
}

function deleteEvent(ev) {
  confirmModal({
    title: t('webCalendarDeleteEvent'),
    body: t('webCalendarDeleteConfirm', { title: eventTitle(ev) }),
    ok: t('delete'),
    danger: true
  }, function (yes) {
    if (!yes) { return; }
    call('calendar.delete', { eventId: ev.id }, function (err) {
      if (err) { toast(err.message); }
      requestEvents();
    });
  });
}

function roundButton(icon, label, onClick, extra) {
  return h('button', { type: 'button', class: 'round-btn ' + (extra || ''), title: label, 'aria-label': label, onClick: onClick },
    h('span', { class: 'icon icon-' + icon, 'aria-hidden': 'true' }));
}

// An event as a row: its calendar's colour, what and when, and how to change it.
// `after` runs first for a row that is in a dialog, which then gets out of the way.
function eventRow(ev, after) {
  const run = function (fn) { return function () { if (after) { after(); } fn(ev); }; };
  const row = h('div', { class: 'cal-event', 'data-event': ev.id },
    h('span', { class: 'cal-bar' }),
    h('div', { class: 'cal-event-text' },
      h('strong', { text: eventTitle(ev) }),
      h('span', { class: 'dim', text: whenOf(ev) }),
      ev.location ? h('span', { class: 'dim', text: ev.location }) : null,
      ev.repeats ? h('span', { class: 'dim', text: t('webCalendarRepeats') }) : null),
    roundButton('square-pen', t('webCalendarEditEvent'), run(editEvent), 'cal-edit'),
    roundButton('trash-2', t('webCalendarDeleteEvent'), run(deleteEvent), 'cal-delete'));
  row.style.setProperty('--c', colourOf(ev));
  return row;
}

function eventList(events, after) {
  return events.length === 0
    ? h('p', { class: 'empty', text: t('webCalendarNoEvents') })
    : h('div', { class: 'cal-list' }, events.map(function (e) { return eventRow(e, after); }));
}

// ── the four scales ──

const dayName = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
const dayShort = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric', month: 'numeric' });
const monthName = new Intl.DateTimeFormat(undefined, { month: 'long' });
const dayFull = new Intl.DateTimeFormat(undefined, { dateStyle: 'full' });

function dayView(events) {
  return eventList(D.eventsOn(events, state.calendar.anchor));
}

function weekView(events) {
  const c = state.calendar;
  const now = new Date();
  return h('div', { class: 'cal-week' }, D.weekDays(c.anchor).map(function (day) {
    const key = day.toDateString();
    const list = D.eventsOn(events, day);
    const open = key in c.openDays ? c.openDays[key] : D.sameDay(day, now);
    const details = h('details', { class: 'cal-weekday', 'data-day': D.toLocalInput(day.getTime()).slice(0, 10) },
      h('summary', {},
        h('span', { class: 'cal-weekday-name', text: dayShort.format(day) }),
        h('span', { class: 'dim', text: t('webCalendarEvents', { count: list.length }) })),
      eventList(list));
    if (open) { details.setAttribute('open', ''); }
    details.addEventListener('toggle', function () { c.openDays[key] = details.open; });
    return details;
  }));
}

function openDay(day, events) {
  const list = D.eventsOn(events, day);
  if (list.length === 0) { newEvent(day); return; }
  openModal({
    title: dayFull.format(day),
    content: function (ctl) { return eventList(list, function () { ctl.close(false); }); },
    actions: [
      { label: t('webCalendarAddEvent'), run: function (ctl) { ctl.close(false); newEvent(day); } },
      { label: t('cancel'), primary: true, run: function (ctl) { ctl.close(false); } }
    ]
  });
}

function monthView(events) {
  const c = state.calendar;
  const now = new Date();
  const heads = D.weekDays(new Date(2026, 0, 5)).map(function (d) { return h('div', { class: 'cal-head', text: dayName.format(d) }); });
  const cells = D.monthGrid(c.anchor).map(function (day) {
    const list = D.eventsOn(events, day);
    const inMonth = day.getMonth() === c.anchor.getMonth();
    const cell = h('button', {
      type: 'button',
      class: 'cal-cell' + (inMonth ? '' : ' outside') + (D.sameDay(day, now) ? ' today' : ''),
      'aria-label': dayFull.format(day) + ', ' + t('webCalendarEvents', { count: list.length }),
      'data-day': D.toLocalInput(day.getTime()).slice(0, 10),
      onClick: function () { openDay(day, events); }
    },
    h('span', { class: 'cal-num', text: String(day.getDate()) }),
    list.slice(0, 3).map(function (e) {
      const chip = h('span', { class: 'cal-chip', text: eventTitle(e) });
      chip.style.setProperty('--c', colourOf(e));
      return chip;
    }),
    list.length > 3 ? h('span', { class: 'cal-more', text: t('webCalendarMore', { count: list.length - 3 }) }) : null);
    return cell;
  });
  return h('div', { class: 'cal-month' }, h('div', { class: 'cal-heads' }, heads), h('div', { class: 'cal-grid' }, cells));
}

function yearView(events) {
  const c = state.calendar;
  const cards = [];
  for (let m = 0; m < 12; m++) {
    const first = new Date(c.anchor.getFullYear(), m, 1);
    const list = events.filter(function (e) { return D.occursInMonth(e, first); });
    const dots = list.slice(0, 6).map(function (e) {
      const dot = h('span', { class: 'cal-dot' });
      dot.style.setProperty('--c', colourOf(e));
      return dot;
    });
    cards.push(h('button', {
      type: 'button',
      class: 'cal-year-card',
      'data-month': String(m + 1),
      'aria-label': t('webCalendarMonthSummary', { month: monthName.format(first), count: list.length }),
      // A month, to look at its days.
      onClick: function () { c.anchor = first; setMode('month'); }
    }, h('strong', { text: monthName.format(first) }), h('span', { class: 'dim', text: t('webCalendarEvents', { count: list.length }) }), h('span', { class: 'cal-dots' }, dots)));
  }
  return h('div', { class: 'cal-year' }, cards);
}

// ── the title of what is shown ──

function titleText() {
  const c = state.calendar;
  const start = D.rangeStart(c.mode, c.anchor);
  switch (c.mode) {
    case 'day': return dayFull.format(start);
    case 'week': return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).formatRange(start, D.addDays(D.rangeEnd('week', c.anchor), -1));
    case 'month': return new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' }).format(start);
    default: return String(start.getFullYear());
  }
}

const MODE_LABELS = { day: 'webCalendarDay', week: 'webCalendarWeek', month: 'webCalendarMonth', year: 'webCalendarYear' };

// Drawn again whenever the state changes while the calendar is shown.
export function updateCalendar() {
  const c = state.calendar;
  const events = visibleEvents();
  const top = root.scrollTop;

  const bar = h('header', { class: 'cal-bar-top' },
    h('div', { class: 'cal-nav' },
      roundButton('chevron-left', t('webCalendarPrevious'), function () { move(-1); }, 'cal-prev'),
      h('button', { type: 'button', class: 'cal-today', onClick: today }, t('webCalendarToday')),
      roundButton('chevron-right', t('webCalendarNext'), function () { move(1); }, 'cal-next')),
    h('h2', { class: 'cal-title', text: titleText() }),
    h('div', { class: 'cal-modes', role: 'group' }, D.MODES.map(function (mode) {
      return h('button', {
        type: 'button',
        class: 'cal-mode',
        'data-mode': mode,
        'aria-pressed': c.mode === mode ? 'true' : 'false',
        onClick: function () { setMode(mode); }
      }, t(MODE_LABELS[mode]));
    })),
    h('span', { class: 'cal-spacer' }),
    roundButton('refresh-cw', t('calendarSyncProviderTooltip'), syncCalendars, 'cal-sync' + (c.syncing ? ' busy' : '')),
    h('button', { type: 'button', class: 'primary cal-add', onClick: function () { newEvent(c.anchor); } },
      h('span', { class: 'icon icon-circle-plus', 'aria-hidden': 'true' }), t('webCalendarAddEvent')));

  const chips = c.calendars.length === 0 ? null : h('div', { class: 'cal-calendars' }, c.calendars.map(function (cal) {
    const shown = c.hidden.indexOf(cal.id) < 0;
    const chip = h('button', {
      type: 'button',
      class: 'cal-toggle',
      'data-calendar': cal.id,
      'aria-pressed': shown ? 'true' : 'false',
      title: t('webCalendarShowCalendar'),
      onClick: function () { toggleCalendar(cal.id); }
    }, h('span', { class: 'cal-dot' }), calendarName(cal));
    chip.style.setProperty('--c', D.calendarColour({ id: cal.id, colorHex: cal.colorHex }, hueOf));
    return chip;
  }));

  let body;
  if (c.error) {
    body = h('p', { class: 'notice', text: t('calendarLoadFailed', { error: c.error }) });
  } else {
    body = ({ day: dayView, week: weekView, month: monthView, year: yearView })[c.mode](events);
  }
  setChildren(root, [bar, chips, h('div', { class: 'cal-body' }, body)]);
  root.scrollTop = top;
}

