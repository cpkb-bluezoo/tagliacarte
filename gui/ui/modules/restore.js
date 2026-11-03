// On start, the page returns to the folder and message the user left open.
//
// What was saved is the account's last folder and last message. The folder list
// and the message list arrive as events some time after the window opens, so
// this is a small plan that is advanced whenever the page's state changes: wait
// for the folder to be listed, open it, wait for its list, then open the
// message. The user's own first move cancels it, and anything that is no longer
// there (a folder that was removed, a message that was deleted) is skipped
// without a word.

import { state, changed } from './store.js';
import { openFolder } from './messages.js';
import { openMessage } from './reader.js';

// The self-test starts from nothing and turns this on for the test of it.
let enabled = !window.__SELFTEST__;
let first = true;
let plan = null;

export function allowRestore(on) {
  enabled = on;
  first = on;
  plan = null;
}

// True once, for the account chosen when the page first has its configuration.
export function startupSelection() {
  const was = first;
  first = false;
  return was;
}

export function beginRestore(accountId) {
  const a = state.accounts[accountId];
  plan = enabled && a && a.lastFolder
    ? { accountId: accountId, folder: a.lastFolder, messageId: a.lastMessageId || null, stage: 'folder' }
    : null;
  if (plan) { changed(); }
}

// The user went somewhere themselves.
export function abandonRestore() { plan = null; }

export function restoring() { return plan !== null; }

function scrollSelectedIntoView() {
  const row = document.querySelector('.message-row[aria-current="true"]');
  if (row) { row.scrollIntoView({ block: 'nearest' }); }
}

// Advances the plan; called after every change of state.
export function restoreStep() {
  if (!plan) { return; }
  if (state.selectedAccount !== plan.accountId) { plan = null; return; }

  if (plan.stage === 'folder') {
    if (state.selectedFolder !== null) { plan = null; return; }
    const a = state.accounts[plan.accountId];
    if (!a || a.folders.indexOf(plan.folder) < 0) { return; }
    plan.stage = 'message';
    openFolder(plan.accountId, plan.folder, { restoring: true });
    return;
  }

  const list = state.list;
  if (!list || list.accountId !== plan.accountId || list.folder !== plan.folder) { plan = null; return; }
  if (!list.ready) { return; }
  const wanted = plan.messageId;
  plan = null;
  if (!wanted || state.message) { return; }
  // The list holds the newest messages; one further back is opened by its id.
  const row = list.slots.find(function (r) { return r && r.id === wanted; });
  openMessage(row || { id: wanted, from: '', subject: '', dateMs: null, isRead: true }, { restoring: true });
  setTimeout(scrollSelectedIntoView, 60);
}
