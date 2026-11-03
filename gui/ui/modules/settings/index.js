// The settings view: a row of tabs and one page at a time. A tab's page is
// built the first time it is shown and then kept (see kit.js for why).

import { state, changed } from '../store.js';
import { h } from '../dom.js';
import { t } from '../i18n.js';
import * as accounts from './accounts.js';
import * as transports from './transports.js';
import * as contacts from './contacts.js';
import * as calendars from './calendars.js';
import * as security from './security.js';
import * as viewing from './viewing.js';
import * as composing from './composing.js';
import * as about from './about.js';

const TABS = [
  { id: 'accounts', label: 'settingsTabAccounts', module: accounts },
  { id: 'transports', label: 'settingsTabOutgoing', module: transports },
  { id: 'security', label: 'settingsTabSecurity', module: security },
  { id: 'viewing', label: 'settingsTabViewing', module: viewing },
  { id: 'composing', label: 'settingsTabComposing', module: composing },
  { id: 'contacts', label: 'settingsTabContacts', module: contacts },
  { id: 'calendars', label: 'webTabCalendars', module: calendars },
  { id: 'about', label: 'settingsTabAbout', module: about }
];

const root = h('section', { class: 'settings' });
const tabBar = h('div', { class: 'tabs', role: 'tablist' });
const panel = h('div', { class: 'tabpanel', role: 'tabpanel' });
const loadFailed = h('p', { class: 'notice' });
root.append(h('h1', { class: 'settings-title' }), tabBar, loadFailed, panel);

const pages = {};
let builtTabs = false;

export function settingsElement() { return root; }

function showTab(id) {
  state.settingsTab = id;
  changed();
}

export function updateSettings() {
  if (!builtTabs) {
    root.querySelector('.settings-title').textContent = t('settings');
    for (const tab of TABS) {
      tabBar.append(h('button', {
        role: 'tab',
        'data-tab': tab.id,
        onClick: function () { showTab(tab.id); }
      }, t(tab.label)));
    }
    builtTabs = true;
  }
  const active = TABS.find(function (x) { return x.id === state.settingsTab; }) || TABS[0];
  for (const b of tabBar.children) {
    b.setAttribute('aria-selected', b.dataset.tab === active.id ? 'true' : 'false');
  }
  // Nothing may be saved over a configuration that has not loaded.
  loadFailed.textContent = state.error || '';
  panel.hidden = !state.config;
  if (!state.config) { return; }

  if (!pages[active.id]) { pages[active.id] = active.module.build(); }
  const page = pages[active.id];
  if (panel.firstChild !== page.root) { panel.replaceChildren(page.root); }
  if (page.refresh) { page.refresh(); }
}
