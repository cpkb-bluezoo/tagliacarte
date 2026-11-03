// The compose view: new messages, replies, forwards, continued drafts and
// newsgroup posts.
//
// The form is built once per message and kept (rebuilding it would lose what
// is being typed), so this module drives it directly instead of re-rendering
// from state. What it does with a message (quoting, recipients, threading) is
// in reply.js; where bytes go (attachments are files, sent in pieces) is in
// the back end.

import { state, changed } from '../store.js';
import { call, upload } from '../wire.js';
import { h, setChildren } from '../dom.js';
import { t } from '../i18n.js';
import { toast } from '../toast.js';
import { confirmModal, formModal } from '../modal.js';
import { pickPath } from '../dialogs.js';
import { newRequestId, waitFor, waitForEvent, cancel } from '../requests.js';
import {
  smtpErrorWantsCredentials, wantsGoogleOAuth, promptTransportCredentials, promptGoogleSignIn
} from '../credprompt.js';
import { normaliseType } from '../settings/accountschema.js';
import { transportTitle } from '../settings/transports.js';
import * as R from './reply.js';
import { makeRichEditor } from './rich.js';
import { recipientField } from './recipients.js';

const root = h('section', { class: 'compose' });
let session = null;

export function composeElement() { return root; }
export function composeIsOpen() { return session !== null; }

// How a message from this account is sent, or null if it cannot be from here.
// Accounts that read from a server or file send through a transport the user
// chose; Gmail and news have their own way; chat and Exchange are not mail
// compose.
export function composeSendKind(account) {
  if (!account) { return null; }
  const type = normaliseType(account.backendType);
  if (type === 'gmail') { return 'gmail'; }
  if (type === 'nntp') { return 'nntp'; }
  if (['imap', 'pop3', 'maildir', 'mbox'].indexOf(type) >= 0) {
    return transportsOf(account).length ? 'smtp' : null;
  }
  return null;
}

function transportsOf(account) {
  return ((account.lists || {}).transportIds || []).map(function (id) { return state.transports[id]; }).filter(Boolean);
}

export function looksLikeDrafts(account, folder) {
  const named = (account.attrs || {}).imapDraftsFolderName;
  return folder === named || String(folder || '').toLowerCase() === 'drafts';
}

function sizeLabel(n) {
  if (n < 1024) { return n + ' B'; }
  if (n < 1024 * 1024) { return (n / 1024).toFixed(1) + ' KB'; }
  return (n / (1024 * 1024)).toFixed(1) + ' MB';
}

function shortName(name) {
  const chars = Array.from(name);
  return chars.length > 24 ? chars.slice(0, 21).join('') + '…' : name;
}

function escapeHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// intent: { accountId, kind: 'new' | 'reply' | 'replyAll' | 'forward' | 'draft', folder, messageId }
export function openCompose(intent) {
  const account = state.accounts[intent.accountId];
  const kind = composeSendKind(account);
  if (!kind) { toast(t('composeNeedTransportTooltip')); return; }
  if (session) {
    // One message at a time: go back to the one in progress.
    state.view = 'compose';
    changed();
    return;
  }
  session = build(account, kind, intent);
  state.view = 'compose';
  changed();
}

export function closeCompose() {
  if (!session) { return; }
  session.dispose();
  session = null;
  setChildren(root, []);
  if (state.view === 'compose') { state.view = 'mail'; }
  changed();
}

// Files are being dragged over the window: show where they would land.
export function filesDragging(active) {
  if (session) { session.hint(active); }
}

// Files dropped on the window (their paths, from the shell).
export function filesDropped(paths) {
  if (session && state.view === 'compose') { paths.forEach(session.attachPath); }
}

function build(account, kind, intent) {
  const cfg = state.config || {};
  const nntp = kind === 'nntp';
  const smtp = kind === 'smtp';
  const imap = ['imap', 'imaps'].indexOf(normaliseType(account.backendType)) >= 0;
  const s = {
    attachments: [],
    dirty: false,
    unsavedDraft: false,
    sending: false,
    messageId: null,
    inReplyTo: null,
    references: null,
    originalHtml: null,
    draftUid: null,
    dsn: null,
    crypto: null,
    fromTouched: false,
    timers: [],
    pending: []
  };
  const rich = !nntp && !!cfg.composeUseRichText;

  const markDirty = function () { s.dirty = true; s.unsavedDraft = true; };

  // --- fields -------------------------------------------------------------
  const from = h('input', { type: 'text', 'data-field': 'from', autocomplete: 'off', spellcheck: 'false', 'aria-label': t('fieldFrom') });
  from.addEventListener('input', function () { s.fromTouched = true; markDirty(); });

  const transports = smtp ? transportsOf(account) : [];
  const transport = h('select', { 'data-field': 'transport', 'aria-label': t('composeOutgoingTransport') },
    transports.map(function (tr) { return h('option', { value: tr.id, text: transportTitle(tr) }); }));
  const currentTransport = function () { return transports.find(function (x) { return x.id === transport.value; }) || null; };

  const to = recipientField({ label: t('fieldTo'), name: 'to', onChange: markDirty });
  const cc = recipientField({ label: t('fieldCc'), name: 'cc', onChange: markDirty });
  const bcc = recipientField({ label: t('fieldBcc'), name: 'bcc', onChange: markDirty });
  const groups = h('input', { type: 'text', 'data-field': 'newsgroups', placeholder: 'comp.lang.rust, alt.test', 'aria-label': t('fieldNewsgroups') });
  groups.addEventListener('input', markDirty);
  const subject = h('input', { type: 'text', 'data-field': 'subject', 'aria-label': t('fieldSubject') });
  subject.addEventListener('input', markDirty);

  const editor = rich ? makeRichEditor({ onChange: markDirty, onFiles: function (files) { Array.from(files).forEach(attachBlob); } }) : null;
  const textarea = rich ? null : h('textarea', { 'data-field': 'body', 'aria-label': t('fieldBody') });
  if (textarea) { textarea.addEventListener('input', markDirty); }

  const attachmentsBox = h('div', { class: 'compose-attachments' });
  const status = h('p', { class: 'error', role: 'alert' });

  // --- seeding --------------------------------------------------------------
  const seedFrom = function () {
    if (s.fromTouched) { return; }
    let value = '';
    const tr = currentTransport();
    if (tr) { value = (tr.defaultFrom || '').trim(); }
    else if (nntp) { value = ((account.attrs || {}).defaultFrom || '').trim(); }
    else if (kind === 'gmail') { value = ((account.attrs || {}).email || '').trim(); }
    from.value = value;
  };
  seedFrom();
  transport.addEventListener('change', function () { seedFrom(); markDirty(); });

  // The user's own addresses, to leave out of "reply to all".
  const selfEmails = function () {
    const set = new Set();
    const add = function (v) { const e = R.extractEmail(v); if (e) { set.add(e.toLowerCase()); } };
    add((account.attrs || {}).defaultFrom);
    add((account.attrs || {}).email);
    transportsOf(account).forEach(function (tr) { add(tr.defaultFrom); });
    add(from.value);
    return Array.from(set);
  };

  const setBody = function (plain, html, caretAtEnd) {
    if (editor) { editor.setHtml(html != null ? html : escapeHtml(plain).replace(/\n/g, '<br>'), caretAtEnd); }
    else {
      textarea.value = plain;
      textarea.focus();
      const at = caretAtEnd ? plain.length : 0;
      textarea.setSelectionRange(at, at);
    }
  };

  const seedFromSource = function (ev) {
    if (ev.error) { toast(t('webComposeOriginalFailed', { error: ev.error })); return; }
    const source = {
      from: ev.from || [], to: ev.to || [], cc: ev.cc || [], subject: ev.subject || '', dateMs: ev.dateMs,
      messageId: ev.messageId, references: ev.references, bodyPlain: ev.bodyPlain, bodyHtml: ev.bodyHtml
    };
    const locale = document.documentElement.lang || undefined;
    const rk = intent.kind;

    if (rk === 'draft') {
      if (source.from.length) { from.value = R.formatAddress(source.from[0]); s.fromTouched = true; }
      to.set(source.to.map(R.formatAddress));
      cc.set(source.cc.map(R.formatAddress));
      subject.value = source.subject.trim();
      s.messageId = R.normaliseMessageId(source.messageId);
      const m = /^imaps?:\/\/.*\/(\d+)$/.exec(String(intent.messageId));
      s.draftUid = m ? Number(m[1]) : (/^\d+$/.test(String(intent.messageId)) ? Number(intent.messageId) : null);
      const html = String(source.bodyHtml || '').trim();
      setBody(source.bodyPlain || R.htmlToPlain(html), editor && html ? html : null, false);
      s.dirty = false;
      s.unsavedDraft = false;
      return;
    }

    if (nntp) {
      groups.value = intent.folder || '';
      subject.value = R.replySubject(source.subject);
      const id = String(source.messageId || '').trim();
      if (id) { s.inReplyTo = id; s.references = id; }
      const body = R.quotedPlain(source, cfg, locale);
      setBody(body, null, !R.aboveQuote(cfg));
      return;
    }

    const rec = R.replyRecipients(rk, source, selfEmails());
    to.set(rec.to);
    cc.set(rec.cc);
    subject.value = rk === 'forward' ? R.forwardSubject(source.subject) : R.replySubject(source.subject);
    const th = R.threading(rk, source);
    s.inReplyTo = th.inReplyTo;
    s.references = th.references;

    if (cfg.quoteOriginal !== false) {
      if (editor) { setBody('', R.quotedRichSeed(source, cfg, locale), !R.aboveQuote(cfg)); }
      else { setBody(R.quotedPlain(source, cfg, locale), null, !R.aboveQuote(cfg)); }
      const wantsHtml = String(cfg.replyQuoteMode || '').trim().toLowerCase() === 'html_smtp';
      s.originalHtml = wantsHtml && String(source.bodyHtml || '').trim() ? source.bodyHtml : null;
    } else {
      // Not quoted: the original travels as a message attached to the new one.
      attachBlob(new File([R.syntheticEml(source)], 'original.eml', { type: 'message/rfc822' }));
    }
  };

  if (intent.kind && intent.kind !== 'new') {
    const requestId = newRequestId('cs');
    s.pending.push(requestId);
    waitForEvent(requestId, function (ev) { seedFromSource(ev); });
    call('compose.source', { accountId: intent.accountId, folder: intent.folder, messageId: intent.messageId, requestId: requestId },
      function (err) { if (err) { cancel(requestId); toast(t('webComposeOriginalFailed', { error: err.message })); } });
  }

  // --- attachments ------------------------------------------------------------
  const renderAttachments = function () {
    attachmentsBox.hidden = s.attachments.length === 0;
    setChildren(attachmentsBox, [
      h('strong', { class: 'attachments-title', text: t('attachments') }),
      h('div', { class: 'attachment-cards' }, s.attachments.map(function (a) {
        return h('div', { class: 'attachment-card', 'data-attachment': a.id },
          h('span', { class: 'name', title: a.name, text: shortName(a.name) }),
          h('span', { class: 'size', text: sizeLabel(a.size) }),
          h('button', {
            type: 'button', class: 'round-btn small', title: t('composeRemoveAttachment'), 'aria-label': t('composeRemoveAttachment'),
            onClick: function () {
              s.attachments = s.attachments.filter(function (x) { return x.id !== a.id; });
              call('compose.removeAttachment', { id: a.id }, function () {});
              markDirty();
              renderAttachments();
            }
          }, h('span', { class: 'icon icon-x', 'aria-hidden': 'true' })));
      }))
    ]);
  };
  renderAttachments();

  const added = function (info) { s.attachments.push(info); markDirty(); renderAttachments(); };

  // A file named by path: nothing is copied or read until the message is written.
  const attachPath = function (path) {
    call('compose.attachPath', { path: path }, function (err, info) {
      if (err) { toast(t('webComposeAttachFailed', { name: path.split(/[\\/]/).pop(), error: err.message })); } else { added(info); }
    });
  };

  // Bytes that have no path (a pasted image, the original as .eml).
  const attachBlob = function (file) {
    if (file.size > 64 * 1024 * 1024) { toast(t('webAttachmentTooBig')); return; }
    upload(file.name || 'attachment', file.type, file, function (err, info) {
      if (err) { toast(t('webComposeAttachFailed', { name: file.name, error: err.message })); } else { added(info); }
    });
  };

  const chooseFile = function () {
    pickPath({ mode: 'file', title: t('webPickAttachments') }, function (path) { if (path) { attachPath(path); } });
  };

  // --- what is sent --------------------------------------------------------------
  const htmlSmtp = String(cfg.replyQuoteMode || '').trim().toLowerCase() === 'html_smtp';

  // The two forms of the body. In rich mode the text form carries the quote with
  // each line prefixed; an original kept as separate HTML goes after the
  // person's own words, and is not repeated inside them.
  const bodies = function () {
    if (!editor) {
      const plain = textarea.value;
      const html = s.originalHtml && htmlSmtp
        ? '<div style="white-space:pre-wrap">' + escapeHtml(plain).replace(/\n/g, '<br>\n') + '</div><hr/><div>' + R.sanitiseRich(s.originalHtml) + '</div>'
        : null;
      return { plain: plain, html: html };
    }
    const html = editor.html();
    const plain = editor.plain(cfg);
    if (s.originalHtml && htmlSmtp) {
      const doc = new DOMParser().parseFromString('<body>' + html, 'text/html');
      const quote = doc.body.querySelector('div.' + R.QUOTED_CLASS);
      if (quote) { quote.remove(); }
      const mine = doc.body.innerHTML.trim();
      return { plain: plain, html: (mine ? '<div>' + mine + '</div>' : '') + '<hr/><div>' + R.sanitiseRich(s.originalHtml) + '</div>' };
    }
    return { plain: plain, html: editor.isEmpty() ? null : html.trim() || null };
  };

  const ensureMessageId = function (cb) {
    if (s.messageId || nntp || from.value.trim() === '') { cb(); return; }
    call('compose.messageId', { from: from.value.trim() }, function (err, r) {
      if (!err && r) { s.messageId = r.messageId; }
      cb();
    });
  };

  const payload = function () {
    const b = bodies();
    return {
      kind: kind,
      accountId: account.id,
      transportId: smtp ? transport.value : undefined,
      from: from.value.trim(),
      to: nntp ? undefined : to.list(),
      cc: nntp ? undefined : cc.list(),
      bcc: nntp ? undefined : bcc.list(),
      newsgroups: nntp ? groups.value.split(/[\s,]+/).filter(Boolean) : undefined,
      subject: subject.value.trim(),
      bodyPlain: b.plain,
      bodyHtml: nntp ? null : b.html,
      attachments: s.attachments.map(function (a) { return a.id; }),
      dsn: s.dsn || undefined,
      crypto: s.crypto || undefined,
      inReplyTo: s.inReplyTo,
      references: s.references,
      messageId: s.messageId
    };
  };

  // --- send ---------------------------------------------------------------------
  const setSending = function (on) {
    s.sending = on;
    sendButton.disabled = on;
    sendButton.textContent = on ? t('webSending') : t('send');
  };

  // Sends, and when the server wants a login that is missing, asks for it and
  // sends again with the same message id.
  const attemptSend = function (body) {
    const requestId = newRequestId('cm');
    s.pending.push(requestId);
    waitFor(requestId, function (err) {
      if (!err) {
        toast(t('composeSendSucceeded'));
        s.dirty = false;
        closeCompose();
        return;
      }
      const tr = currentTransport();
      if (!smtp || !tr || !smtpErrorWantsCredentials(err)) {
        setSending(false);
        status.textContent = err;
        toast(err);
        return;
      }
      const retry = function (ok) {
        if (!ok) { setSending(false); toast(t('composeSendCancelledNoSmtpCredentials')); return; }
        attemptSend(body);
      };
      if (wantsGoogleOAuth(tr, err)) { promptGoogleSignIn('transport', tr.id, tr.displayName, retry); }
      else {
        const hint = /<([^<>]+)>/.exec(body.from);
        promptTransportCredentials(Object.assign({}, tr, { defaultFrom: hint ? hint[1] : body.from.split(',')[0] }), retry);
      }
    });
    call('compose.send', Object.assign({ requestId: requestId }, body), function (err) {
      if (err) { cancel(requestId); setSending(false); status.textContent = err.message; toast(err.message); }
    });
  };

  const send = function () {
    if (s.sending) { return; }
    status.textContent = '';
    if (from.value.trim() === '') { toast(t('composeMissingFrom')); return; }
    if (nntp) {
      if (groups.value.split(/[\s,]+/).filter(Boolean).length === 0) { toast(t('composeMissingNewsgroups')); return; }
    } else if (to.list().length === 0) {
      toast(t('composeMissingTo'));
      return;
    }
    setSending(true);
    ensureMessageId(function () { attemptSend(payload()); });
  };

  // --- drafts ------------------------------------------------------------------------
  const saveDraft = function (manual) {
    if (!imap || s.sending || from.value.trim() === '') { return; }
    if (!manual && !s.unsavedDraft) { return; }
    ensureMessageId(function () {
      const requestId = newRequestId('cd');
      s.pending.push(requestId);
      s.unsavedDraft = false;
      waitForEvent(requestId, function (ev) {
        if (ev.error) {
          s.unsavedDraft = true;
          if (manual) { toast(t('webDraftFailed', { error: ev.error })); }
        } else {
          s.draftUid = ev.uid != null ? ev.uid : null;
          if (manual) { toast(t('webDraftSaved')); }
        }
      });
      const body = payload();
      call('compose.saveDraft', Object.assign({ requestId: requestId, replaceUid: s.draftUid != null ? s.draftUid : undefined }, body),
        function (err) { if (err) { cancel(requestId); s.unsavedDraft = true; if (manual) { toast(t('webDraftFailed', { error: err.message })); } } });
    });
  };

  const seconds = Number((account.attrs || {}).draftAutosaveSeconds);
  if (imap && seconds > 0) { s.timers.push(setInterval(function () { saveDraft(false); }, seconds * 1000)); }

  // --- dialogs and the bar --------------------------------------------------------------
  const chooseDsn = function () {
    const tr = currentTransport();
    if (!tr) { return; }
    const labels = { never: 'dsnNever', failure: 'dsnFailure', success: 'dsnSuccess', delay: 'dsnDelay', 'failure,success': 'dsnFailureAndSuccess' };
    formModal({
      title: t('dsnLabel'),
      subtitle: t('dsnUseTransportDefault') + ': ' + (tr.dsnNotify || 'failure'),
      fields: [{
        key: 'dsn', label: t('dsnNotifyLabel'), type: 'select', value: s.dsn || '__transport__',
        options: [{ value: '__transport__', label: t('dsnUseTransportDefault') }].concat(
          Object.keys(labels).map(function (v) { return { value: v, label: t(labels[v]) }; }))
      }],
      ok: 'OK',
      submit: function (v, ctl) { s.dsn = v.dsn === '__transport__' ? null : v.dsn; ctl.close(true); }
    }, function () {});
  };

  const chooseCrypto = function () {
    formModal({
      title: t('composeCryptoTitle'),
      fields: [{
        key: 'mode', label: t('composeCryptoLabel'), type: 'select', value: s.crypto || 'none',
        options: [
          { value: 'none', label: t('composeCryptoNone') }, { value: 'sign', label: t('composeCryptoSign') },
          { value: 'encrypt', label: t('composeCryptoEncrypt') }, { value: 'sign_encrypt', label: t('composeCryptoSignEncrypt') }
        ]
      }],
      ok: 'OK',
      submit: function (v, ctl) { s.crypto = v.mode === 'none' ? null : v.mode; ctl.close(true); }
    }, function () {});
  };

  const discard = function () {
    if (!s.dirty && !s.unsavedDraft) { closeCompose(); return; }
    confirmModal({
      title: t('discardChangesTitle'), body: t('discardChangesBody'), ok: t('discard'), cancel: t('keepEditing'), danger: true
    }, function (yes) { if (yes) { closeCompose(); } });
  };

  // Every action is a round Lucide icon, named by its tooltip.
  const button = function (icon, label, action, onClick, extra) {
    return h('button', Object.assign({
      type: 'button', class: 'round-btn', title: label, 'aria-label': label, 'data-action': action, onClick: onClick
    }, extra || {}), h('span', { class: 'icon icon-' + icon, 'aria-hidden': 'true' }));
  };
  const sendButton = button('send', t('send'), 'send', send, { class: 'round-btn primary' });
  const bar = h('div', { class: 'compose-bar' },
    h('h2', { class: 'compose-title', text: t('compose') }),
    smtp ? button('bell', t('dsnLabel'), 'dsn', chooseDsn) : null,
    !nntp ? button('shield', t('composeCryptoLabel'), 'crypto', chooseCrypto) : null,
    button('paperclip', t('attach'), 'attach', chooseFile),
    imap ? button('save', t('webSaveDraft'), 'save-draft', function () { saveDraft(true); }) : null,
    button('trash-2', t('discard'), 'discard', discard),
    sendButton);

  // --- layout --------------------------------------------------------------------------------
  const row = function (label, control) {
    return h('label', { class: 'compose-row' }, h('span', { class: 'label', text: label }), control);
  };
  const dropHint = h('div', { class: 'drop-hint', text: t('webDropToAttach'), hidden: true });
  const nodes = [
    bar,
    h('div', { class: 'compose-form' },
      nntp ? h('p', { class: 'hint', text: t('composeNntpPostingBlurb') })
        : smtp ? row(t('composeOutgoingTransport'), transport)
          : null,
      row(t('fieldFrom'), from),
      nntp ? row(t('fieldNewsgroups'), groups) : [to.el, cc.el, bcc.el],
      row(t('fieldSubject'), subject),
      h('div', { class: 'compose-body' }, editor ? editor.el : textarea),
      attachmentsBox,
      status),
    dropHint
  ];
  setChildren(root, nodes);
  (editor ? editor.area : textarea).addEventListener('dragover', function () { dropHint.hidden = false; });

  // A new message starts with the cursor where the first thing to write is.
  setTimeout(function () {
    if (intent.kind && intent.kind !== 'new') { return; }
    (from.value.trim() === '' ? from : nntp ? groups : to.input).focus();
  }, 0);

  return {
    attachPath: attachPath,
    hint: function (on) { dropHint.hidden = !on || state.view !== 'compose'; },
    dispose: function () {
      s.timers.forEach(clearInterval);
      s.pending.forEach(cancel);
      // The files held for this message go with it.
      s.attachments.forEach(function (a) { call('compose.removeAttachment', { id: a.id }, function () {}); });
    }
  };
}
