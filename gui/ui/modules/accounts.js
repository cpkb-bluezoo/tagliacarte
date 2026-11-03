// The account rail.

import { state, changed } from './store.js';
import { call } from './wire.js';
import { h } from './dom.js';
import { forgetDeclined, maybePromptForCredentials } from './credentials.js';
import { composeIsOpen } from './compose/view.js';
import { t } from './i18n.js';
import { accountAvatar } from './avatar.js';
import { openCalendar } from './calendar/view.js';
import { beginRestore, abandonRestore } from './restore.js';
import { rememberLocation } from './location.js';

const CONNECTION_KEYS = {
  connecting: 'webConnecting',
  connected: 'webConnected',
  disconnected: 'webDisconnected',
  error: 'webError'
};

// `opts.restoring` is the first choice at start-up: the saved folder and message
// of that account are then returned to; any other choice is the user's own.
export function selectAccount(id, opts) {
  state.view = 'mail';
  if (id) { forgetDeclined(id); }
  if (state.selectedAccount === id) { changed(); return; }
  state.selectedAccount = id;
  state.selectedFolder = null;
  state.selectedMessage = null;
  state.list = null;
  state.message = null;
  const acc = state.accounts[id];
  if (acc && acc.folders.length === 0) {
    call('session.refreshFolders', { accountId: id }, function () {});
  }
  if (opts && opts.restoring) {
    beginRestore(id);
  } else {
    abandonRestore();
    rememberLocation();
  }
  changed();
  // An error that arrived while another account was in view is dealt with now.
  if (acc && acc.lastError) { maybePromptForCredentials(id, acc.lastError); }
}

export function railContent() {
  const accounts = state.accountOrder.map(function (id) {
    const acc = state.accounts[id];
    const label = acc.label || id;
    return h('button', {
      class: 'account account-avatar',
      'data-account': id,
      'aria-pressed': state.selectedAccount === id ? 'true' : 'false',
      'aria-label': label,
      title: label + ' (' + t(CONNECTION_KEYS[acc.connection] || 'webError') + ')',
      onClick: function () { selectAccount(id); }
    },
    accountAvatar(acc, 'rail', 40),
    h('span', { class: 'dot ' + acc.connection }));
  });
  // Calendar and Settings sit at the foot of the rail.
  const calendar = h('button', {
    class: 'account calendar-btn',
    'aria-pressed': state.view === 'calendar' ? 'true' : 'false',
    'aria-label': t('calendarTitle'),
    title: t('calendarTitle'),
    onClick: openCalendar
  }, h('span', { class: 'icon icon-calendar' }));
  const settings = h('button', {
    class: 'account settings-btn',
    'aria-pressed': state.view === 'settings' ? 'true' : 'false',
    'aria-label': t('settings'),
    title: t('settings'),
    onClick: function () {
      state.view = 'settings';
      changed();
    }
  }, h('span', { class: 'icon icon-settings' }));
  // A message being written stays open while other views are used; this returns to it.
  const draft = composeIsOpen() ? h('button', {
    class: 'account compose-resume' + (state.view === 'compose' ? ' current' : ''),
    'aria-pressed': state.view === 'compose' ? 'true' : 'false',
    'aria-label': t('webComposeInProgress'),
    title: t('webComposeInProgress'),
    onClick: function () { state.view = 'compose'; changed(); }
  }, '\u270e') : null;
  return [...accounts, draft, calendar, settings];
}
