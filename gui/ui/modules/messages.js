// The open folder's message list.
//
// The back end fills a list of `total` slots, oldest first, and a row's
// `rank` is its slot. A newest-first view therefore asks for one row to learn
// the total, then for the newest page.

import { call } from './wire.js';
import { state, changed } from './store.js';
import { h } from './dom.js';
import { t } from './i18n.js';
import { openMessage } from './reader.js';
import { abandonRestore } from './restore.js';
import { rememberLocation } from './location.js';

const SORT = 'date_desc';
const PAGE = 50;
let seq = 0;

// `opts.restoring` is the return to where the user was at start-up, which is
// not a new place to remember (the message in it is still to be opened).
export function openFolder(accountId, folder, opts) {
  if (!(opts && opts.restoring)) {
    abandonRestore();
    rememberLocation();
  }
  state.selectedFolder = folder;
  state.selectedMessage = null;
  state.message = null;
  state.list = {
    accountId: accountId,
    folder: folder,
    sort: SORT,
    total: 0,
    slots: [],
    ready: false,
    error: null,
    requests: {}
  };
  request(0, 1, false, true);
  changed();
}

function request(start, limit, listReady, probe) {
  const list = state.list;
  const requestId = 'w' + (++seq);
  list.requests[requestId] = { probe: probe, listReady: listReady };
  call('session.listMessagesWindow', {
    accountId: list.accountId,
    folderName: list.folder,
    startIndex: start,
    limit: limit,
    messageListSort: list.sort,
    requestId: requestId,
    listReady: listReady
  }, function (err) {
    if (err && state.list === list) {
      list.error = err.message;
      list.ready = true;
      changed();
    }
  });
}

// Called for messageListWindowStarted, messageListRowFound and
// messageListWindowComplete.
export function onListEvent(ev) {
  const list = state.list;
  if (!list || !list.requests[ev.requestId]) { return; }
  const req = list.requests[ev.requestId];

  if (ev.type === 'messageListWindowStarted') {
    list.total = ev.total;
    if (list.slots.length !== ev.total) {
      const next = new Array(ev.total).fill(null);
      for (let i = 0; i < Math.min(next.length, list.slots.length); i++) { next[i] = list.slots[i]; }
      list.slots = next;
    }
  } else if (ev.type === 'messageListRowFound') {
    if (ev.rank >= 0 && ev.rank < list.slots.length) { list.slots[ev.rank] = ev.summary; }
  } else if (ev.type === 'messageListWindowComplete') {
    delete list.requests[ev.requestId];
    if (ev.error) {
      list.error = ev.error;
      list.ready = true;
    } else if (req.probe) {
      if (list.total === 0) {
        list.ready = true;
      } else {
        const limit = Math.min(PAGE, list.total);
        request(list.total - limit, limit, true, false);
      }
    } else if (req.listReady) {
      list.ready = true;
    }
  }
}

// Returns true when a row went from unread to read here.
export function onFlagsChanged(ev) {
  const list = state.list;
  if (!list || list.accountId !== ev.accountId || list.folder !== ev.folder) { return false; }
  let turnedRead = false;
  for (const row of list.slots) {
    if (row && row.id === ev.messageId) {
      turnedRead = turnedRead || (!row.isRead && ev.isRead);
      row.isRead = ev.isRead;
    }
  }
  return turnedRead;
}

const dayFormat = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' });
const timeFormat = new Intl.DateTimeFormat(undefined, { timeStyle: 'short' });

function formatDate(ms) {
  if (ms == null) { return ''; }
  const d = new Date(ms);
  return d.toDateString() === new Date().toDateString() ? timeFormat.format(d) : dayFormat.format(d);
}

function row(summary) {
  return h('button', {
    class: 'message-row' + (summary.isRead ? '' : ' unread'),
    'data-id': summary.id,
    'aria-current': state.selectedMessage === summary.id ? 'true' : null,
    onClick: function () { openMessage(summary); }
  },
  h('span', { class: 'line' },
    h('span', { class: 'from', text: summary.from }),
    h('span', { class: 'date', text: formatDate(summary.dateMs) })),
  h('span', { class: 'subject', text: summary.subject || t('webNoSubject') }));
}

export function messagesContent() {
  const list = state.list;
  if (!list) { return [h('p', { class: 'empty', text: t('selectFolder') })]; }
  const out = [h('h2', { class: 'pane-title', text: list.folder })];
  if (list.error) { out.push(h('p', { class: 'notice', text: list.error })); }
  // Slots are oldest first and the view is newest first.
  const loaded = [];
  for (let i = list.slots.length - 1; i >= 0; i--) {
    if (list.slots[i]) { loaded.push(row(list.slots[i])); }
  }
  if (loaded.length) {
    out.push(...loaded);
  } else {
    out.push(h('p', { class: 'empty', text: list.ready ? t('noMessages') : t('webLoading') }));
  }
  return out;
}
