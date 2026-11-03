// What an account looks like for each backend type: the form's starting
// values, the checks made before saving, and the attrs and lists written.
//
// These are pure functions of plain data so that they can be checked without
// a window. They follow what the previous interface wrote to config.xml, key
// for key, including what it removed: a key that does not apply to a type is
// deleted so that changing nothing but the type's rules never leaves strays.

export const TYPES = [
  { id: 'imap', label: 'IMAP' },
  { id: 'gmail', label: 'Gmail' },
  { id: 'exchange', label: 'Exchange' },
  { id: 'pop3', label: 'POP3' },
  { id: 'maildir', label: 'Maildir' },
  { id: 'mbox', label: 'mbox' },
  { id: 'nntp', label: 'NNTP' },
  { id: 'nostr', label: 'Nostr' },
  { id: 'matrix', label: 'Matrix' }
];

const ALIASES = { imaps: 'imap', graph: 'exchange', nntps: 'nntp' };

// Config files hold lower case; older ones may use an alias.
export function normaliseType(raw) {
  const t = String(raw || '').trim().toLowerCase();
  return ALIASES[t] || t;
}

export function typeLabel(raw) {
  const id = normaliseType(raw);
  const found = TYPES.find(function (x) { return x.id === id; });
  return found ? found.label : String(raw || '');
}

// Mail that is sent through a transport the user picks, versus accounts that
// send for themselves or cannot send.
export function requiresTransport(type) {
  return ['imap', 'pop3', 'maildir', 'mbox'].indexOf(normaliseType(type)) >= 0;
}

export function isLocalStore(type) {
  const t = normaliseType(type);
  return t === 'maildir' || t === 'mbox';
}

export function hasTcpServer(type) {
  return ['imap', 'pop3', 'nntp'].indexOf(normaliseType(type)) >= 0;
}

export const DEFAULT_RELAYS = ['wss://relay.damus.io', 'wss://nos.lol'];
export const DEFAULT_PORT = { imap: '993', pop3: '995', nntp: '563' };

// "@user:server" into the user id and the homeserver URL it implies.
export function parseMxid(raw) {
  const m = /^@([^:@]+):(.+)$/.exec(String(raw || '').trim());
  if (!m) { return null; }
  const domain = m[2].trim();
  if (!domain) { return null; }
  return { userId: String(raw).trim(), homeserverUrl: domain.indexOf('://') >= 0 ? domain : 'https://' + domain };
}

// An MXID for a stored account. Older configs keep a bare user name and a
// homeserver URL, from which the id is rebuilt.
export function mxidFor(account) {
  const a = account.attrs || {};
  const u = String(a.username || a.email || '').trim();
  if (/^@[^:]+:.+/.test(u)) { return u; }
  const hs = String(a.homeserver || a.host || '').trim();
  if (!u) { return ''; }
  try {
    const host = new URL(hs.indexOf('://') >= 0 ? hs : 'https://' + hs).hostname;
    return host ? '@' + u + ':' + host : u;
  } catch (e) {
    return u;
  }
}

// A local store's path as the back end keeps it: one leading slash. Windows
// drive and UNC paths are left as they are.
export function localPath(raw) {
  const p = String(raw || '').trim();
  if (/^[A-Za-z]:[\\/]/.test(p) || p.startsWith('\\\\')) { return p; }
  const rest = p.replace(/^\/+/, '');
  return rest === '' ? '/' : '/' + rest;
}

function lower(v) { return String(v || '').trim().toLowerCase(); }

export function initialDraft(rawType, existing) {
  const type = normaliseType(rawType);
  const a = existing ? existing.attrs || {} : {};
  const lists = existing ? existing.lists || {} : {};
  const d = { label: existing ? existing.label || '' : '', avatarUrl: existing ? existing.avatarUrl || '' : '' };
  const transports = (lists.transportIds || []).slice();

  switch (type) {
    case 'imap': {
      const had = !!a.host;
      Object.assign(d, {
        host: a.host || '',
        port: had ? a.port || (a.security === 'starttls' || a.security === 'plain' ? '143' : '993') : '993',
        security: had ? a.security || 'tls' : 'tls',
        oauthProvider: ['google', 'microsoft'].indexOf(lower(a.oauthProvider)) >= 0 ? lower(a.oauthProvider) : '',
        idleMin: a.imapIdleMinIdleSeconds || '',
        deleteMode: a.imapDeleteMode === 'Mark Deleted' ? 'Mark Deleted' : 'Move to Trash',
        trashFolder: a.imapTrashFolderName || '',
        junkFolder: a.imapJunkFolderName || '',
        sentFolder: a.imapSentFolderName || '',
        draftsFolder: a.imapDraftsFolderName || '',
        mirrorSent: a.imapMirrorSentIfMissing || '',
        draftAutosave: a.draftAutosaveSeconds || '',
        transportIds: transports
      });
      break;
    }
    case 'gmail':
      Object.assign(d, {
        email: a.email || '',
        trashLabel: a.gmailTrashLabelId || '',
        spamLabel: a.gmailSpamLabelId || '',
        sentLabel: a.gmailSentLabelId || 'SENT',
        draftLabel: a.gmailDraftLabelId || 'DRAFT',
        inboxLabel: a.gmailInboxLabelId || 'INBOX'
      });
      break;
    case 'exchange':
      Object.assign(d, { trashFolder: a.imapTrashFolderName || '', junkFolder: a.imapJunkFolderName || '' });
      break;
    case 'pop3':
    case 'nntp':
      Object.assign(d, {
        host: a.host || '',
        port: a.host ? a.port || DEFAULT_PORT[type] : DEFAULT_PORT[type],
        security: a.host ? a.security || 'tls' : 'tls'
      });
      if (type === 'nntp') { d.defaultFrom = a.defaultFrom || ''; } else { d.transportIds = transports; }
      break;
    case 'maildir':
      Object.assign(d, {
        path: a.path || '',
        deleteMode: a.maildirDeleteMode === 'Delete immediately' ? 'Delete immediately' : 'Move to Trash',
        trashFolder: a.maildirTrashFolderName || '',
        junkFolder: a.maildirJunkFolderName || '',
        transportIds: transports
      });
      break;
    case 'mbox':
      Object.assign(d, { path: a.path || '', transportIds: transports });
      break;
    case 'nostr': {
      const relays = (lists.relayUrls || []).filter(Boolean);
      Object.assign(d, { nip05: a.nip05 || '', npub: a.npub || '', relays: relays.length ? relays : DEFAULT_RELAYS.slice() });
      break;
    }
    case 'matrix':
      d.mxid = existing ? mxidFor(existing) : '';
      break;
    default:
      break;
  }
  return d;
}

// The first thing wrong with a draft, as { key, params } for the string table,
// or null. The order is the order the checks have always run in.
export function validate(rawType, d) {
  const type = normaliseType(rawType);
  const blank = function (v) { return String(v == null ? '' : v).trim() === ''; };
  if (blank(d.label)) { return { key: 'validationAccountNameRequired' }; }
  if (isLocalStore(type) && blank(d.path)) { return { key: 'validationLocalPathRequired' }; }
  if (type === 'matrix') {
    if (blank(d.mxid)) { return { key: 'validationMatrixUserIdRequired' }; }
    if (!parseMxid(d.mxid)) { return { key: 'validationMatrixMxidInvalid' }; }
  }
  if (type === 'nostr' && !d.relays.some(function (r) { return !blank(r); })) {
    return { key: 'nostrRelayUrlsRequired' };
  }
  if (type === 'gmail' && blank(d.email)) { return { key: 'validationEmailAddressRequired' }; }
  if (hasTcpServer(type)) {
    if (blank(d.host)) { return { key: 'validationHostRequired' }; }
    const port = String(d.port).trim();
    if (!/^\d+$/.test(port) || Number(port) < 1 || Number(port) > 65535) { return { key: 'validationPortRequired' }; }
  }
  if (type === 'imap' && !blank(d.idleMin)) {
    const n = String(d.idleMin).trim();
    if (!/^\d+$/.test(n) || Number(n) < 15 || Number(n) > 864000) { return { key: 'validationImapMinIdleSeconds' }; }
  }
  return null;
}

function put(attrs, key, value) {
  const v = String(value == null ? '' : value).trim();
  if (v === '') { delete attrs[key]; } else { attrs[key] = v; }
}

function drop(attrs, keys) { for (const k of keys) { delete attrs[k]; } }

const IMAP_ONLY = [
  'imapIdleMinIdleSeconds', 'imapDeleteMode', 'imapTrashFolderName', 'imapJunkFolderName',
  'imapSentFolderName', 'imapDraftsFolderName', 'imapMirrorSentIfMissing', 'draftAutosaveSeconds'
];

// The account to save. Unknown keys of the existing account are kept.
export function toAccount(rawType, d, existing, id) {
  const type = normaliseType(rawType);
  const attrs = Object.assign({}, existing ? existing.attrs : {});
  const lists = {};
  for (const k of Object.keys(existing ? existing.lists || {} : {})) { lists[k] = existing.lists[k].slice(); }
  const server = function () {
    attrs.host = d.host.trim();
    attrs.port = String(d.port).trim();
    attrs.security = d.security;
  };

  switch (type) {
    case 'imap':
      server();
      put(attrs, 'oauthProvider', d.oauthProvider);
      put(attrs, 'imapIdleMinIdleSeconds', d.idleMin);
      attrs.imapDeleteMode = d.deleteMode;
      if (d.deleteMode === 'Move to Trash') { put(attrs, 'imapTrashFolderName', d.trashFolder); } else { delete attrs.imapTrashFolderName; }
      put(attrs, 'imapJunkFolderName', d.junkFolder);
      put(attrs, 'imapSentFolderName', d.sentFolder);
      put(attrs, 'imapDraftsFolderName', d.draftsFolder);
      put(attrs, 'imapMirrorSentIfMissing', d.mirrorSent);
      put(attrs, 'draftAutosaveSeconds', d.draftAutosave);
      drop(attrs, ['username', 'email']);
      lists.transportIds = d.transportIds.slice();
      break;
    case 'gmail':
      attrs.email = d.email.trim();
      attrs.gmailTrashLabelId = d.trashLabel.trim() || 'TRASH';
      attrs.gmailSpamLabelId = d.spamLabel.trim() || 'SPAM';
      attrs.gmailSentLabelId = d.sentLabel.trim() || 'SENT';
      attrs.gmailDraftLabelId = d.draftLabel.trim() || 'DRAFT';
      attrs.gmailInboxLabelId = d.inboxLabel.trim() || 'INBOX';
      attrs.oauthProvider = 'google';
      drop(attrs, ['username', 'host', 'port', 'security'].concat(IMAP_ONLY));
      delete lists.transportIds;
      break;
    case 'exchange':
      put(attrs, 'imapTrashFolderName', d.trashFolder);
      put(attrs, 'imapJunkFolderName', d.junkFolder);
      attrs.oauthProvider = 'microsoft';
      drop(attrs, ['email', 'username']);
      delete lists.transportIds;
      break;
    case 'pop3':
      server();
      drop(attrs, ['username', 'email', 'oauthProvider']);
      lists.transportIds = d.transportIds.slice();
      break;
    case 'nntp':
      server();
      put(attrs, 'defaultFrom', d.defaultFrom);
      drop(attrs, ['username', 'email', 'oauthProvider']);
      delete lists.transportIds;
      break;
    case 'maildir':
      attrs.path = localPath(d.path);
      attrs.maildirDeleteMode = d.deleteMode;
      if (d.deleteMode === 'Move to Trash') { put(attrs, 'maildirTrashFolderName', d.trashFolder); } else { delete attrs.maildirTrashFolderName; }
      put(attrs, 'maildirJunkFolderName', d.junkFolder);
      drop(attrs, ['username', 'email', 'oauthProvider']);
      lists.transportIds = d.transportIds.slice();
      break;
    case 'mbox':
      attrs.path = localPath(d.path);
      drop(attrs, ['username', 'email', 'oauthProvider', 'maildirDeleteMode', 'maildirTrashFolderName', 'maildirJunkFolderName']);
      lists.transportIds = d.transportIds.slice();
      break;
    case 'nostr':
      put(attrs, 'nip05', d.nip05);
      attrs.npub = d.npub;
      drop(attrs, ['username', 'email', 'host', 'port', 'security', 'oauthProvider']);
      lists.relayUrls = d.relays.map(function (r) { return r.trim(); }).filter(Boolean);
      delete lists.transportIds;
      break;
    case 'matrix': {
      const m = parseMxid(d.mxid);
      attrs.username = m.userId;
      attrs.homeserver = m.homeserverUrl;
      attrs.host = m.homeserverUrl;
      delete attrs.email;
      delete lists.transportIds;
      break;
    }
    default:
      break;
  }
  const account = { id: id || (existing ? existing.id : ''), label: d.label.trim(), backendType: type, attrs: attrs, lists: lists };
  // No picture is no key at all, which is how it is cleared.
  if (d.avatarUrl && d.avatarUrl.trim()) { account.avatarUrl = d.avatarUrl.trim(); }
  if (existing && existing.lastFolder) { account.lastFolder = existing.lastFolder; }
  if (existing && existing.lastMessageId) { account.lastMessageId = existing.lastMessageId; }
  return account;
}

// A short description of where an account connects, for its list row.
export function connectionSummary(account) {
  const a = account.attrs || {};
  switch (normaliseType(account.backendType)) {
    case 'imap': case 'pop3': case 'nntp': case 'gmail': case 'exchange':
      return a.host ? (a.port ? a.host + ':' + a.port : a.host) : a.email || '';
    case 'maildir': case 'mbox':
      return a.path || '';
    case 'nostr':
      return a.npub && a.npub.length > 24 ? a.npub.slice(0, 12) + '…' : a.npub || account.label;
    case 'matrix':
      return a.username ? a.username + (a.homeserver ? ' @ ' + a.homeserver : '') : '';
    default:
      return account.label;
  }
}
