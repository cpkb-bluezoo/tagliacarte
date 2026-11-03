import { state, subscribe, changed } from './store.js';
import { call, onEvent, start } from './wire.js';
import { apply } from './events.js';
import { railContent } from './accounts.js';
import { foldersContent } from './folders.js';
import { messagesContent } from './messages.js';
import { readerElement, updateReader } from './reader.js';
import { settingsElement, updateSettings } from './settings/index.js';
import { composeElement } from './compose/view.js';
import { calendarElement, updateCalendar } from './calendar/view.js';
import { loadLocale } from './i18n.js';
import { sendMenuLabels, syncMenu } from './menu.js';
import { syncTitle } from './title.js';
import { restoreStep } from './restore.js';
import { flushLocation } from './location.js';
import { h, setChildren } from './dom.js';
import { installPanes, updatePanes } from './panes.js';

const shell = document.getElementById('app');
const panes = {
  rail: h('section', { class: 'rail' }),
  folders: h('section', { class: 'folders' }),
  messages: h('section', { class: 'messages' })
};
shell.append(panes.rail, panes.folders, panes.messages, readerElement(), settingsElement(), composeElement(), calendarElement());
installPanes(shell, panes.rail);

// Replaces a pane's content, keeping its scroll position.
function fill(pane, nodes) {
  const top = pane.scrollTop;
  setChildren(pane, nodes);
  pane.scrollTop = top;
}

subscribe(function () {
  shell.classList.toggle('view-settings', state.view === 'settings');
  shell.classList.toggle('view-compose', state.view === 'compose');
  shell.classList.toggle('view-calendar', state.view === 'calendar');
  fill(panes.rail, railContent());
  fill(panes.folders, foldersContent());
  fill(panes.messages, messagesContent());
  if (state.view === 'settings') { updateSettings(); }
  if (state.view === 'calendar') { updateCalendar(); }
  updatePanes();
  updateReader();
  syncMenu();
  syncTitle();
  restoreStep();
});

// Where the user is goes to config.xml shortly after they move; make sure of it as the window goes.
window.addEventListener('pagehide', flushLocation);
document.addEventListener('visibilitychange', function () { if (document.hidden) { flushLocation(); } });

// A subject typed in the message being written is part of the window's title.
document.addEventListener('input', syncTitle);

loadLocale(function () {
  sendMenuLabels();
  // The configuration and the session both report as events. Their order
  // does not matter: an account is created the first time either mentions it.
  onEvent(apply);
  start();
  call('config.load', undefined, function (err) {
    if (err) {
      state.error = err.message;
      changed();
    }
  });
});

if (window.__SELFTEST__) {
  document.head.append(h('script', { type: 'module', src: '/modules/selftest.js' }));
}
