// The window's title, as the page's own document.title: what is open, then the
// application. The shell mirrors it to the window, so nothing here is special
// to the native side, and the same title serves a browser tab.

import { state } from './store.js';
import { t } from './i18n.js';
import { folderLabel } from './folders.js';

const APP = 'Tagliacarte';

// parts: { view, account, folder, unread, messageSubject, composeSubject, settings, compose, calendar }
//   view: 'mail' | 'settings' | 'compose' | 'calendar'
//   account: the account's name, folder: the open folder's name, unread: its unread count,
//   messageSubject: the open message's subject (empty if none is open),
//   composeSubject: what has been typed as the subject of the message being written.
// settings and compose are those words, in the page's language.
export function windowTitle(p) {
  const tail = [];
  if (p.view === 'settings') {
    tail.push(p.settings);
  } else if (p.view === 'calendar') {
    tail.push(p.calendar);
  } else if (p.view === 'compose') {
    tail.push(p.composeSubject || p.compose);
  } else if (p.messageSubject) {
    tail.push(p.messageSubject);
    if (p.folder) { tail.push(p.folder); }
  } else if (p.folder) {
    tail.push(p.unread > 0 ? p.folder + ' (' + p.unread + ')' : p.folder);
    if (p.account) { tail.push(p.account); }
  } else if (p.account) {
    tail.push(p.account);
  }
  return tail.concat(APP).join(' - ');
}

// The title for what the page is showing now.
export function currentTitle() {
  const acc = state.accounts[state.selectedAccount];
  const m = state.message;
  const field = document.querySelector('[data-field="subject"]');
  return windowTitle({
    view: state.view,
    account: acc ? (acc.label || acc.id) : '',
    folder: acc && state.selectedFolder ? folderLabel(acc, state.selectedFolder) : '',
    unread: acc && state.selectedFolder ? (acc.unread[state.selectedFolder] || 0) : 0,
    messageSubject: m ? ((m.headers && m.headers.subject) || (m.summary && m.summary.subject) || t('webNoSubject')) : '',
    composeSubject: field ? field.value.trim() : '',
    settings: t('settings'),
    compose: t('compose'),
    calendar: t('calendarTitle')
  });
}

let shown = '';

// Sets document.title if it differs.
export function syncTitle() {
  const title = currentTitle();
  if (title !== shown) {
    shown = title;
    document.title = title;
  }
}
