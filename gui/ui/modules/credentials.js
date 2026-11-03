// A login that goes missing while the app runs (a password removed, a token
// revoked, an account added from another device's config) shows up as an
// error from the session. This asks for it, once, and tries again.

import { state } from './store.js';
import { call } from './wire.js';
import {
  isMissingImapCredentials, isMissingNostrCredentials, imapStyleCredentialKind,
  promptStoreCredentials, promptGoogleSignIn, promptNostrSecret, loginOptions
} from './credprompt.js';
import { normaliseType, isLocalStore } from './settings/accountschema.js';

const asking = new Set();
const declined = new Set();

// Choosing the account again is a request to try again.
export function forgetDeclined(accountId) { declined.delete(accountId); }

// Asks only for the account the person is looking at. An error for another
// account is remembered (account.lastError) and acted on when they select it,
// so a start-up with several accounts does not open a dialog for each, or for
// one that was only briefly unavailable.
export function maybePromptForCredentials(accountId, message) {
  const account = state.accounts[accountId];
  if (!account || asking.has(accountId) || declined.has(accountId)) { return; }
  if (accountId !== state.selectedAccount || state.view !== 'mail') { return; }
  const type = normaliseType(account.backendType);
  if (isLocalStore(type)) { return; }

  const finished = function (ok) {
    asking.delete(accountId);
    if (ok) {
      call('session.refreshFolders', { accountId: accountId }, function () {});
    } else {
      declined.add(accountId);
    }
  };

  if (type === 'nostr') {
    if (!isMissingNostrCredentials(message)) { return; }
    asking.add(accountId);
    promptNostrSecret(accountId, function (npub) { finished(!!npub); }, message);
    return;
  }
  if (!isMissingImapCredentials(message)) { return; }
  asking.add(accountId);
  if (imapStyleCredentialKind(type, message, account.attrs) === 'google') {
    promptGoogleSignIn('store', accountId, account.label, finished, message);
  } else {
    promptStoreCredentials(account, Object.assign({ reason: message }, loginOptions(account)), finished);
  }
}
