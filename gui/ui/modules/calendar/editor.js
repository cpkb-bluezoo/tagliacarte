// The dialog for making or changing an event. The back end checks it again and
// has the last word; this only saves a round trip for the common mistakes.

import { call } from '../wire.js';
import { h } from '../dom.js';
import { t } from '../i18n.js';
import { openModal } from '../modal.js';
import * as D from './dates.js';

// A calendar's name for people: the one that is always there is Local, in the viewer's language.
export function calendarName(c) {
  return c.id === 'local' ? t('webCalendarLocal') : c.name;
}

// Splits what was typed into addresses: commas, semicolons and spaces all separate.
export function parseInvitees(text) {
  return String(text || '').split(/[\s,;]+/).map(function (s) { return s.trim(); }).filter(Boolean);
}

// o: { event (to change one), day (a Date, for a new one), calendars, onSaved(), onDelete(event) }
export function openEventEditor(o) {
  const ev = o.event || null;
  const now = new Date();
  const first = ev ? ev.startMs : D.newEventStart(o.day || now, now);
  const last = ev ? ev.endMs : first + 3600000;
  let allDay = ev ? ev.allDay : false;

  const field = function (label, control) {
    return h('label', { class: 'cal-field' }, h('span', { class: 'label', text: label }), control);
  };
  const calendar = h('select', { 'data-field': 'calendar' }, o.calendars.map(function (c) {
    return h('option', { value: c.id, text: calendarName(c) });
  }));
  calendar.value = ev && o.calendars.some(function (c) { return c.id === ev.calendarId; }) ? ev.calendarId : (o.calendars.some(function (c) { return c.id === 'local'; }) ? 'local' : o.calendars[0].id);
  const title = h('input', { type: 'text', 'data-field': 'title', value: ev ? ev.title : '' });
  const allDayBox = h('input', { type: 'checkbox', 'data-field': 'allDay' });
  allDayBox.checked = allDay;
  const starts = h('input', { 'data-field': 'starts' });
  const ends = h('input', { 'data-field': 'ends' });
  const location = h('input', { type: 'text', 'data-field': 'location', value: ev && ev.location ? ev.location : '' });
  const description = h('textarea', { 'data-field': 'description', rows: '3' });
  description.value = ev && ev.description ? ev.description : '';
  const invitees = h('input', { type: 'text', 'data-field': 'invitees', placeholder: t('webCalendarInviteesHint'), value: ev ? ev.attendees.join(', ') : '' });

  // The two date fields hold dates for an all-day event, and dates with times otherwise.
  const show = function (startMs, endMs) {
    starts.type = allDay ? 'date' : 'datetime-local';
    ends.type = allDay ? 'date' : 'datetime-local';
    starts.value = allDay ? D.toDateInput(startMs) : D.toLocalInput(startMs);
    ends.value = allDay ? D.toDateInput(D.lastDayOf(startMs, endMs)) : D.toLocalInput(endMs);
  };
  show(first, last);

  allDayBox.addEventListener('change', function () {
    const was = allDay;
    allDay = allDayBox.checked;
    if (was === allDay) { return; }
    const s = was ? D.fromDateInput(starts.value) : D.fromLocalInput(starts.value);
    if (s === null) { show(first, last); return; }
    if (allDay) {
      // The day it was on, here; the end is the same day.
      const d = new Date(s);
      const day = Date.UTC(d.getFullYear(), d.getMonth(), d.getDate());
      show(day, D.dayAfter(day));
    } else {
      const d = new Date(s);
      const nine = new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate(), 9).getTime();
      show(nine, nine + 3600000);
    }
  });

  const content = h('div', { class: 'cal-form' },
    field(t('webCalendarFieldCalendar'), calendar),
    field(t('webCalendarFieldTitle'), title),
    h('label', { class: 'cal-field cal-check' }, allDayBox, h('span', { text: t('webCalendarFieldAllDay') })),
    field(t('webCalendarFieldStarts'), starts),
    field(t('webCalendarFieldEnds'), ends),
    field(t('webCalendarFieldLocation'), location),
    field(t('webCalendarFieldDescription'), description),
    field(t('webCalendarFieldInvitees'), invitees));

  const save = function (ctl) {
    const name = title.value.trim();
    if (!name) { ctl.setError(t('webCalendarNeedTitle')); return; }
    let startMs;
    let endMs;
    if (allDay) {
      startMs = D.fromDateInput(starts.value);
      const lastDay = D.fromDateInput(ends.value);
      if (startMs === null || lastDay === null) { ctl.setError(t('webCalendarNeedDates')); return; }
      endMs = D.dayAfter(lastDay);
      if (endMs <= startMs) { ctl.setError(t('webCalendarEndBeforeStart')); return; }
    } else {
      startMs = D.fromLocalInput(starts.value);
      endMs = D.fromLocalInput(ends.value);
      if (startMs === null || endMs === null) { ctl.setError(t('webCalendarNeedDates')); return; }
      if (endMs < startMs) { ctl.setError(t('webCalendarEndBeforeStart')); return; }
    }
    const args = {
      calendarId: calendar.value,
      title: name,
      description: description.value,
      location: location.value,
      startMs: startMs,
      endMs: endMs,
      allDay: allDay,
      attendees: parseInvitees(invitees.value)
    };
    if (ev) { args.eventId = ev.id; }
    ctl.setBusy(true);
    call('calendar.save', args, function (err) {
      ctl.setBusy(false);
      if (err) { ctl.setError(err.message); return; }
      ctl.close(true);
      if (o.onSaved) { o.onSaved(); }
    });
  };

  // A repeating event is shown, not changed: saving one occurrence would move the series.
  if (ev && ev.locked) {
    content.querySelectorAll('[data-field]').forEach(function (f) { f.disabled = true; });
    return openModal({
      title: t('webCalendarViewEvent'),
      note: t('webCalendarRepeatingReadOnly'),
      content: content,
      actions: [{ label: t('webClose'), primary: false, run: function (c) { c.close(false); } }]
    });
  }

  const actions = [{ label: t('cancel'), run: function (c) { c.close(false); } }];
  if (ev && o.onDelete) {
    actions.push({ label: t('delete'), danger: true, run: function (c) { c.close(false); o.onDelete(ev); } });
  }
  actions.push({ label: t('save'), primary: true, run: save });
  return openModal({ title: ev ? t('webCalendarEditEvent') : t('webCalendarAddEvent'), content: content, actions: actions });
}
