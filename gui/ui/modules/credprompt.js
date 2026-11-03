// Asking the person for credentials, and deciding when to.

import { call } from './wire.js';
import { t } from './i18n.js';
import { formModal, actionModal } from './modal.js';
import { newRequestId, waitFor } from './requests.js';
import { normaliseType } from './settings/accountschema.js';

// The wording of server errors is the only signal that a login is missing, so
// these match text, as the Flutter UI did.

const NETWORK_ERRORS = [
  'lookup address', 'no such host', 'name or service not known', 'connection refused',
  'network is unreachable', 'host is down', 'connection timed out', 'connection reset'
];

const CREDENTIAL_ERRORS = [
  'credential required for smtp transport',
  'no saved smtp credential for transport',
  'no saved credential for this account',
  'no smtp username in saved credentials',
  'no saved oauth credential for transport',
  'auth failed:'
];

// Whether an SMTP error means "ask for a login" rather than "the server is unreachable".
export function smtpErrorWantsCredentials(message) {
  const m = String(message || '').toLowerCase();
  if (NETWORK_ERRORS.some(function (s) { return m.indexOf(s) >= 0; })) { return false; }
  if (m.indexOf('nodename') >= 0 && m.indexOf('servname') >= 0) { return false; }
  return CREDENTIAL_ERRORS.some(function (s) { return m.indexOf(s) >= 0; });
}

// The AUTH mechanisms the server advertised, from "server EHLO AUTH: PLAIN, XOAUTH2".
export function smtpAdvertisedAuth(message) {
  const m = /server EHLO AUTH:\s*([A-Za-z0-9\-_, ]+)/.exec(String(message || ''));
  if (!m) { return []; }
  return m[1].split(/[ ,]+/).filter(Boolean).map(function (s) { return s.toUpperCase(); });
}

export function wantsGoogleOAuth(transport, message) {
  return String(transport.oauthProvider || '').toLowerCase() === 'google' &&
    smtpAdvertisedAuth(message).indexOf('XOAUTH2') >= 0;
}

// Mail-server errors carry the capability list after this marker.
const IMAP_CAPS_MARKER = 'TAGLIACARTE_IMAP_CAPS:';

// Password or Google sign-in for a mail account. An account configured for
// Google (or a Gmail account) gets Google's sign-in; Microsoft has no browser
// sign-in here, so it gets a password box. Otherwise the server's first
// advertised AUTH mechanism decides: XOAUTH2 means Google, the plain ones a
// password.
export function imapStyleCredentialKind(backendType, message, attrs) {
  const type = String(backendType || '').toLowerCase();
  if (type === 'gmail') { return 'google'; }
  if (type === 'imap' || type === 'imaps') {
    const provider = String((attrs || {}).oauthProvider || '').toLowerCase();
    if (provider === 'google') { return 'google'; }
    if (provider === 'microsoft') { return 'password'; }
    const text = String(message || '');
    const at = text.indexOf(IMAP_CAPS_MARKER);
    if (at >= 0) {
      const caps = text.slice(at + IMAP_CAPS_MARKER.length).split('\n')[0].split(',')
        .map(function (c) { return c.trim().toUpperCase(); });
      for (const cap of caps) {
        if (!cap.startsWith('AUTH=')) { continue; }
        const mech = cap.slice(5);
        if (mech === 'XOAUTH2') { return 'google'; }
        if (['PLAIN', 'LOGIN', 'CRAM-MD5', 'SCRAM-SHA-256'].indexOf(mech) >= 0) { return 'password'; }
      }
    }
  }
  return 'password';
}

// The text the session reports when a mail account has no usable login.
export function isMissingImapCredentials(message) {
  const m = String(message || '');
  return m.indexOf('no saved password for IMAP account') >= 0 ||
    m.indexOf('Gmail: add credentials') >= 0 ||
    m.indexOf('credential required for') >= 0 ||
    m.indexOf('NeedsCredential') >= 0;
}

export function isMissingNostrCredentials(message) {
  const m = String(message || '');
  return m.indexOf('no saved credential for this account') >= 0 ||
    m.indexOf('no secret key') >= 0 ||
    m.indexOf('NeedsCredential') >= 0;
}

// Why the person is being asked, from the server's own words; long ones are cut.
function reasonNote(reason) {
  const m = String(reason || '').trim();
  if (!m) { return null; }
  return t('webSignInReason', { message: m.length > 300 ? m.slice(0, 300) + '\u2026' : m });
}

function signInFields(usernameHint) {
  return [
    { key: 'username', label: t('usernameLabel'), value: usernameHint || '' },
    { key: 'password', label: t('passwordLabel'), type: 'password' }
  ];
}

function needBoth(values) {
  return values.username.trim() !== '' && values.password !== '';
}

// IMAP, POP3, NNTP, Exchange and Matrix accounts. cb(true) if saved.
export function promptStoreCredentials(account, opts, cb) {
  formModal({
    title: opts.title || t('imapSignInTitle'),
    subtitle: account.label || account.id,
    note: reasonNote(opts.reason),
    fields: signInFields(opts.usernameHint),
    ok: t('save'),
    submit: function (values, ctl) {
      if (!needBoth(values)) { ctl.setError(t('enterUsernameAndPassword')); return; }
      call('credential.saveStore', {
        accountId: account.id, username: values.username.trim(), password: values.password
      }, function (err) {
        if (err) { ctl.setError(t('operationFailed', { error: err.message })); } else { ctl.close(true); }
      });
    }
  }, cb);
}

export function promptTransportCredentials(transport, cb) {
  formModal({
    title: t('smtpSignInTitle'),
    subtitle: t('smtpSignInSubtitle', { transportName: transport.displayName || transport.id, host: transport.host || '—' }),
    fields: signInFields(transport.defaultFrom),
    ok: t('save'),
    submit: function (values, ctl) {
      if (!needBoth(values)) { ctl.setError(t('enterUsernameAndPassword')); return; }
      call('credential.saveTransport', {
        transportId: transport.id, username: values.username.trim(), password: values.password
      }, function (err) {
        if (err) { ctl.setError(t('operationFailed', { error: err.message })); } else { ctl.close(true); }
      });
    }
  }, cb);
}

// Google sign-in in the system browser, for a mail account (kind "store") or
// an SMTP transport (kind "transport"). cb(true) once the sign-in finished.
export function promptGoogleSignIn(kind, id, label, cb, reason) {
  actionModal({
    title: t('gmailSignInTitle'),
    subtitle: label,
    note: reasonNote(reason),
    body: t('gmailSignInBody'),
    ok: t('gmailSignInBrowserButton'),
    work: function (ctl) {
      const requestId = newRequestId('o');
      waitFor(requestId, function (error) {
        if (error) { ctl.setError(t('operationFailed', { error: error })); } else { ctl.close(true); }
      });
      call('oauth.google', { kind: kind, id: id, requestId: requestId }, function (err) {
        if (err) { ctl.setError(t('operationFailed', { error: err.message })); }
      });
    }
  }, cb);
}

// Asks for an nsec or hex secret and links it to the account. cb(npub|null).
export function promptNostrSecret(accountId, cb, reason) {
  let npub = null;
  formModal({
    title: t('webNostrSecretTitle'),
    subtitle: t('webNostrSecretBody'),
    note: reasonNote(reason),
    fields: [{ key: 'secret', label: t('webNostrSecretField'), type: 'password' }],
    ok: t('save'),
    submit: function (values, ctl) {
      if (values.secret.trim() === '') { ctl.setError(t('webNostrSecretRequired')); return; }
      call('nostr.linkSecret', { accountId: accountId, secret: values.secret.trim() }, function (err, result) {
        if (err) { ctl.setError(err.message); return; }
        npub = result.npub;
        ctl.close(true);
      });
    }
  }, function (ok) { cb(ok ? npub : null); });
}

// Sign-in dialog title and username suggestion for a mail account. A Matrix
// user name is the part of the id between "@" and ":".
export function loginOptions(account) {
  if (normaliseType(account.backendType) === 'matrix') {
    return {
      title: t('matrixSignInTitle'),
      usernameHint: String((account.attrs || {}).username || '').replace(/^@/, '').replace(/:.*$/, '')
    };
  }
  return {};
}
