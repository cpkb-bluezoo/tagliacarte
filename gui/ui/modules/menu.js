// The native menu bar's side of the page: its words (from the ARB strings, in
// the language the page shows), which message actions can be used now, and what
// to do when one of the page's own items is chosen. Rust owns the menu's
// structure and shortcuts; see gui/src/menu.rs.

import { state, changed } from './store.js';
import { call } from './wire.js';
import { t } from './i18n.js';
import { toast } from './toast.js';
import { composeSendKind, openCompose } from './compose/view.js';
import { messageActions, runMessageAction } from './reader.js';

// menu item id -> [ARB key, parameters]
const APP = { app: 'Tagliacarte' };
export const LABELS = {
  about: ['webMenuAbout', APP],
  settings: ['webMenuSettings'],
  services: ['webMenuServices'],
  hide: ['webMenuHide', APP],
  hideOthers: ['webMenuHideOthers'],
  showAll: ['webMenuShowAll'],
  quit: ['webMenuQuit', APP],
  menuFile: ['webMenuFile'],
  newMessage: ['webMenuNewMessage'],
  getNewMail: ['webMenuGetNewMail'],
  closeWindow: ['webMenuCloseWindow'],
  menuEdit: ['webMenuEdit'],
  undo: ['webFmtUndo'],
  redo: ['webFmtRedo'],
  cut: ['webMenuCut'],
  copy: ['messageActionCopy'],
  paste: ['webMenuPaste'],
  selectAll: ['webMenuSelectAll'],
  menuView: ['webMenuView'],
  fullscreen: ['webMenuFullscreen'],
  menuMessage: ['messageTitle'],
  reply: ['messageActionReply'],
  replyAll: ['messageActionReplyAll'],
  forward: ['messageActionForward'],
  menuWindow: ['webMenuWindow'],
  minimize: ['webMenuMinimize'],
  zoom: ['webMenuZoom'],
  bringAllToFront: ['webMenuBringAllToFront'],
  menuHelp: ['webMenuHelp']
};

// The menu's words in the page's language. The menu starts in English, so this
// only has to be sent once the strings are loaded.
export function sendMenuLabels(cb) {
  const ids = Object.keys(LABELS);
  call('menu.labels', {
    ids: ids,
    labels: ids.map(function (id) { return t(LABELS[id][0], LABELS[id][1]); })
  }, cb || function () {});
}

// Which of the menu's message actions can be used for what is on screen.
export function enabledMenuItems() {
  const ids = [];
  if (state.view === 'mail' && state.selectedAccount) { ids.push('getNewMail'); }
  if (state.view === 'mail' && state.message) {
    for (const a of messageActions(state.message)) {
      if (a !== 'draft') { ids.push(a); }
    }
  }
  return ids;
}

let sent = null;

// Tells the menu what can be used, when that has changed.
export function syncMenu() {
  const ids = enabledMenuItems();
  const key = ids.join(',');
  if (key === sent) { return; }
  sent = key;
  call('menu.enable', { ids: ids }, function () {});
}

// An account to start a new message from: the open one, else the first that can send.
function sender() {
  const open = state.accounts[state.selectedAccount];
  if (open && composeSendKind(open)) { return open.id; }
  const id = state.accountOrder.find(function (a) { return composeSendKind(state.accounts[a]); });
  return id || null;
}

// The page's own items, chosen in the menu (a menuAction event).
export function onMenuAction(ev) {
  switch (ev.id) {
    case 'settings':
      state.view = 'settings';
      changed();
      break;
    case 'newMessage': {
      const id = sender();
      if (id) { openCompose({ accountId: id, kind: 'new' }); } else { toast(t('composeNeedTransportTooltip')); }
      break;
    }
    case 'getNewMail':
      if (state.selectedAccount) { call('session.refreshFolders', { accountId: state.selectedAccount }, function () {}); }
      break;
    case 'reply':
    case 'replyAll':
    case 'forward':
      runMessageAction(ev.id);
      break;
    default:
      break;
  }
}
