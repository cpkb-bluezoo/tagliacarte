// Where the user is (the account, the folder in it, the message in that folder)
// is saved to config.xml as they move, so that the next start can return there.
//
// A save follows a pause in moving about, so that stepping through a list with
// the keyboard is one write and not one per row, and a place that has not
// changed is not sent again. An account chosen without a folder says only the
// account: that account's own last folder and message stay as they were.

import { state } from './store.js';
import { call } from './wire.js';

const PAUSE = 250;
let timer = null;
let sent = '';

// Something the user did moved them; save where they are once things settle.
export function rememberLocation() {
  clearTimeout(timer);
  timer = setTimeout(flushLocation, PAUSE);
}

// Saves now (also when the window is going away).
export function flushLocation() {
  clearTimeout(timer);
  timer = null;
  const id = state.selectedAccount;
  if (!id) { return; }
  const args = { accountId: id };
  if (state.selectedFolder) {
    args.folder = state.selectedFolder;
    if (state.message && state.message.id) { args.messageId = state.message.id; }
  }
  const key = JSON.stringify(args);
  if (key === sent) { return; }
  sent = key;
  call('location.save', args, function () {});
}
