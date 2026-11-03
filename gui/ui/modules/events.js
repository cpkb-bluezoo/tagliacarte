// Session events into state.

import { state, changed } from './store.js';
import { onListEvent, onFlagsChanged } from './messages.js';
import { selectAccount } from './accounts.js';
import { onReaderEvent } from './reader.js';
import { onPathPicked } from './dialogs.js';
import { resolve as resolveRequest, resolveEvent } from './requests.js';
import { onMenuAction } from './menu.js';
import { startupSelection } from './restore.js';
import { onCalendarEvent } from './calendar/view.js';
import { onSourceEvent } from './settings/calendars.js';
import { maybePromptForCredentials } from './credentials.js';
import { filesDropped, filesDragging } from './compose/view.js';

export function ensureAccount(id) {
  if (!state.accounts[id]) {
    state.accounts[id] = {
      id: id,
      label: '',
      backendType: '',
      storeKind: '',
      connection: 'connecting',
      message: null,
      folders: [],
      delimiter: null,
      unread: {},
      names: {},
      folderError: null,
      attrs: {},
      lists: {},
      lastFolder: null,
      lastError: null
    };
  }
  return state.accounts[id];
}

// The configuration arrives as configStarted, one event per account and
// transport, then configLoaded, which is authoritative: whatever was not
// seen again no longer exists.
function configEvent(ev) {
  switch (ev.type) {
    case 'configStarted':
      state.configSeen = { accounts: [], transports: new Set() };
      break;
    case 'configAccount': {
      const a = ensureAccount(ev.id);
      a.label = ev.label;
      a.backendType = ev.backendType;
      a.avatarUrl = ev.avatarUrl || '';
      a.attrs = ev.attrs || {};
      a.lists = ev.lists || {};
      a.lastFolder = ev.lastFolder || null;
      a.lastMessageId = ev.lastMessageId || null;
      if (state.configSeen) { state.configSeen.accounts.push(ev.id); }
      break;
    }
    case 'configTransport':
      state.transports[ev.id] = ev;
      if (state.configSeen) { state.configSeen.transports.add(ev.id); }
      break;
    case 'configLoaded': {
      const seen = state.configSeen;
      state.config = ev;
      if (seen) {
        state.accountOrder = seen.accounts;
        for (const id of Object.keys(state.accounts)) {
          if (!seen.accounts.includes(id)) { delete state.accounts[id]; }
        }
        for (const id of Object.keys(state.transports)) {
          if (!seen.transports.has(id)) { delete state.transports[id]; }
        }
      }
      state.configSeen = null;
      if (!state.accountOrder.includes(state.selectedAccount)) {
        const wanted = ev.selectedStoreId;
        // Repairing the selection must not pull the user out of the settings
        // they are editing.
        const view = state.view;
        // The first account chosen at start is also returned to its saved folder and message.
        selectAccount(state.accountOrder.includes(wanted) ? wanted : state.accountOrder[0] || null, { restoring: startupSelection() });
        state.view = view;
      }
      break;
    }
    default:
      break;
  }
}

// The address book arrives as contactsStarted, one event per repository,
// group, group target, group member and contact, then contactsLoaded; the
// previous copy is replaced only at the end, so the page never shows half.
function contactsEvent(ev) {
  const c = state.contacts;
  // The rest of a sequence whose start this page never saw (it reloaded
  // part way through) is dropped; the next load starts again.
  if (ev.type !== 'contactsStarted' && !state.contactsPending && /^contacts(Repository|Group|Target|Member|Contact|Loaded)$/.test(ev.type)) { return; }
  switch (ev.type) {
    case 'contactsStarted':
      state.contactsPending = { loaded: true, repositories: [], groups: [], targets: [], members: {}, contacts: [], links: c.links };
      break;
    case 'contactsRepository': state.contactsPending.repositories.push(ev); break;
    case 'contactsGroup': state.contactsPending.groups.push(ev); break;
    case 'contactsTarget': state.contactsPending.targets.push(ev); break;
    case 'contactsMember':
      (state.contactsPending.members[ev.groupId] = state.contactsPending.members[ev.groupId] || []).push(ev);
      break;
    case 'contactsContact': state.contactsPending.contacts.push(ev); break;
    case 'contactsLoaded':
      state.contacts = state.contactsPending;
      state.contactsPending = null;
      break;
    case 'contactsRepoLink': {
      const slot = c.links[ev.contactId] = c.links[ev.contactId] || { rows: [], done: false };
      slot.rows.push(ev);
      break;
    }
    case 'contactsRepoLinksDone': {
      const slot = c.links[ev.contactId] = c.links[ev.contactId] || { rows: [], done: false };
      slot.done = true;
      break;
    }
    default:
      break;
  }
}

export function apply(ev) {
  switch (ev.type) {
    case 'contactsStarted':
    case 'contactsRepository':
    case 'contactsGroup':
    case 'contactsTarget':
    case 'contactsMember':
    case 'contactsContact':
    case 'contactsLoaded':
    case 'contactsRepoLink':
    case 'contactsRepoLinksDone':
      contactsEvent(ev);
      break;
    case 'contactsSyncDone':
    case 'composeSource':
    case 'composeDraftSaved':
    case 'attachmentSaved':
      resolveEvent(ev);
      break;
    case 'menuAction':
      onMenuAction(ev);
      break;
    case 'calendarsStarted':
    case 'calendarFound':
    case 'calendarsLoaded':
    case 'calendarEventsStarted':
    case 'calendarEvent':
    case 'calendarEventsDone':
      onCalendarEvent(ev);
      break;
    case 'calendarSourcesStarted':
    case 'calendarSource':
    case 'calendarSourcesLoaded':
      onSourceEvent(ev);
      break;
    case 'calendarSynced':
    case 'calendarImported':
      resolveEvent(ev);
      break;
    case 'windowTitle':
      // Sent in the self-test only: the title the native window has.
      state.nativeTitle = ev.title;
      break;
    case 'filesDropped':
      filesDropped(ev.paths || []);
      break;
    case 'filesDragging':
      filesDragging(!!ev.active);
      break;
    case 'configStarted':
    case 'configAccount':
    case 'configTransport':
    case 'configLoaded':
      configEvent(ev);
      break;
    case 'accountConnectionChanged': {
      const a = ensureAccount(ev.accountId);
      a.storeKind = ev.storeKind;
      a.connection = ev.connectionState;
      a.message = ev.message || null;
      a.lastError = ev.connectionState === 'error' ? ev.message || '' : null;
      if (ev.connectionState === 'error') { maybePromptForCredentials(ev.accountId, ev.message); }
      break;
    }
    case 'folderFound': {
      const a = ensureAccount(ev.accountId);
      if (!a.folders.includes(ev.folderName)) { a.folders.push(ev.folderName); }
      a.unread[ev.folderName] = ev.unread;
      break;
    }
    case 'folderListUpdated': {
      // The authoritative list: anything not in it no longer exists.
      const a = ensureAccount(ev.accountId);
      a.folders = ev.folders;
      a.delimiter = ev.hierarchyDelimiter || null;
      a.unread = ev.unreadByFolder || {};
      a.names = ev.folderDisplayNames || {};
      a.folderError = null;
      break;
    }
    case 'folderListFailed':
      ensureAccount(ev.accountId).folderError = ev.message;
      ensureAccount(ev.accountId).lastError = ev.message;
      maybePromptForCredentials(ev.accountId, ev.message);
      break;
    case 'messageFlagsChanged':
      if (onFlagsChanged(ev)) {
        const a = state.accounts[ev.accountId];
        if (a && a.unread[ev.folder] > 0) { a.unread[ev.folder] -= 1; }
      }
      break;
    case 'oauthFinished':
    case 'composeSent':
    case 'transportVerified':
    case 'nostrProfileSynced':
    case 'nostrProfilePublished':
      resolveRequest(ev);
      break;
    case 'pathPicked':
      onPathPicked(ev);
      break;
    case 'messageHeaders':
    case 'messageBody':
    case 'messageSignature':
    case 'messageRemote':
    case 'messageAttachment':
    case 'messageOpened':
    case 'linkClicked':
      onReaderEvent(ev);
      break;
    case 'messageListWindowStarted':
    case 'messageListRowFound':
    case 'messageListWindowComplete':
      onListEvent(ev);
      break;
    default:
      break;
  }
  changed();
}
