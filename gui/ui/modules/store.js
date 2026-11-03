import { getLocal } from './localprefs.js';

// All UI state in one object; render functions read it, event handling and
// user actions mutate it and call changed(). Renders are coalesced into one
// per turn of the event loop.

export const state = {
  // Settings from the last configLoaded event.
  config: null,
  transports: {},
  // Accounts and transports seen since configStarted; configLoaded drops the rest.
  configSeen: null,
  accountOrder: [],
  // id -> { id, label, backendType, storeKind, connection, message,
  //         folders, delimiter, unread, names }
  accounts: {},
  // 'mail' or 'settings'
  view: 'mail',
  // The open settings tab.
  settingsTab: 'accounts',
  selectedAccount: null,
  selectedFolder: null,
  selectedMessage: null,
  // The open folder's message list; see messages.js.
  list: null,
  // The address book (settings, Contacts tab); see events.js.
  contacts: { loaded: false, repositories: [], groups: [], targets: [], members: {}, contacts: [], links: {} },
  contactsPending: null,
  // The open message: envelope, attachments and the link awaiting a decision; see reader.js.
  message: null,
  // The calendar (the rail's calendar button); see calendar/view.js. `hidden` is the
  // calendars the viewer turned off; the events are those of the range being shown.
  calendar: {
    mode: getLocal('calendarMode', 'month'),
    anchor: new Date(),
    calendars: [],
    calendarsPending: null,
    hidden: getLocal('calendarHidden', []),
    events: [],
    eventsPending: null,
    request: null,
    loading: false,
    error: null,
    syncing: false,
    openDays: {}
  },
  error: null
};

const subscribers = [];
let queued = false;

export function subscribe(fn) { subscribers.push(fn); }

export function changed() {
  if (queued) { return; }
  queued = true;
  queueMicrotask(function () {
    queued = false;
    for (const fn of subscribers) { fn(state); }
  });
}
