// The reading pane: the open message's envelope and attachments, and its
// body in a sandboxed frame.
//
// The frame is created once and kept. Moving or recreating an iframe reloads
// it, so everything else in the pane is rebuilt in place and the frame's src
// changes only when a different message is opened.

import { state, changed } from './store.js';
import { call } from './wire.js';
import { h, setChildren } from './dom.js';
import { t } from './i18n.js';
import { getLocal } from './localprefs.js';
import { openCompose, composeSendKind, looksLikeDrafts } from './compose/view.js';
import { newRequestId, waitForEvent, cancel } from './requests.js';
import { abandonRestore } from './restore.js';
import { rememberLocation } from './location.js';
import { toast } from './toast.js';
import { confirmModal } from './modal.js';

let seq = 0;

// Remote images are loaded only when the person has asked: the setting, or the
// button for this one message. The page does not fetch them; asking for the
// body with images=1 makes the back end serve them through its proxy.
export function imagesAllowed(m) {
  return !!(m.images || (state.config && state.config.loadRemoteImages));
}

export function bodyUrl(m) {
  return '/mail/' + [m.accountId, m.folder, m.id].map(encodeURIComponent).join('/') + '/body' +
    (imagesAllowed(m) ? '?images=1' : '');
}

// How many remote images each message's body holds, as the back end counted them
// when the body was last loaded. Kept so that a message opened again shows its
// offer to load them at once, without waiting for the frame to ask for the body.
const remoteCounts = new Map();

function placeKey(accountId, folder, id) { return [accountId, folder, id].join('\n'); }

// `opts.restoring`: this is the message the user left open, which may have gone
// since; it is then dropped quietly instead of shown as an error.
export function openMessage(summary, opts) {
  const list = state.list;
  if (!list) { return; }
  abandonRestore();
  const m = {
    requestId: 'm' + (++seq),
    accountId: list.accountId,
    folder: list.folder,
    id: summary.id,
    summary: summary,
    headers: null,
    attachments: [],
    html: false,
    images: false,
    remoteCount: remoteCounts.get(placeKey(list.accountId, list.folder, summary.id)) || 0,
    signature: null,
    opened: false,
    restoring: !!(opts && opts.restoring),
    error: null,
    link: null
  };
  state.message = m;
  state.selectedMessage = summary.id;
  rememberLocation();
  call('session.openMessage', {
    accountId: m.accountId,
    folder: m.folder,
    messageId: m.id,
    requestId: m.requestId
  }, function (err) {
    if (err && state.message === m) {
      if (m.restoring) { dropRestored(m); return; }
      m.error = err.message;
      m.opened = true;
      changed();
    }
  });
  if (!summary.isRead) {
    call('session.markRead', { accountId: m.accountId, folder: m.folder, messageId: m.id }, function () {});
  }
  changed();
}

// The message that was open last time is not there any more.
function dropRestored(m) {
  if (state.message !== m) { return; }
  state.message = null;
  state.selectedMessage = null;
  rememberLocation();
  changed();
}

// messageHeaders, messageAttachment, messageSignature and messageOpened belong to one
// request; linkClicked belongs to whatever message is open; messageRemote is said
// by the body when the frame loads it, by message.
export function onReaderEvent(ev) {
  if (ev.type === 'messageRemote') {
    if (remoteCounts.size > 500) { remoteCounts.clear(); }
    remoteCounts.set(placeKey(ev.accountId, ev.folder, ev.messageId), ev.count);
    const open = state.message;
    if (open && open.accountId === ev.accountId && open.folder === ev.folder && open.id === ev.messageId) {
      open.remoteCount = ev.count;
    }
    return;
  }
  const m = state.message;
  if (!m) { return; }
  if (ev.type === 'linkClicked') {
    m.link = ev.url;
    return;
  }
  if (ev.requestId !== m.requestId) { return; }
  if (ev.type === 'messageHeaders') {
    m.headers = ev;
  } else if (ev.type === 'messageBody') {
    m.html = !!ev.html;
  } else if (ev.type === 'messageSignature') {
    m.signature = ev.status;
  } else if (ev.type === 'messageAttachment') {
    m.attachments.push(ev);
  } else if (ev.type === 'messageOpened') {
    if (ev.error && m.restoring) { dropRestored(m); return; }
    m.opened = true;
    if (ev.error) { m.error = ev.error; }
  }
}

const root = h('section', { class: 'reading' });
const placeholder = h('p', { class: 'empty' });
// The actions and the signature mark float over the corners of the headers; the
// headers are laid out to leave room for them.
const actions = h('div', { class: 'msg-actions' });
const sign = h('span', { class: 'msg-sign', role: 'img' });
const head = h('header', { class: 'msg-head' });
const imagesbar = h('div', { class: 'linkbar imagesbar' });
const linkbar = h('div', { class: 'linkbar' });
const frame = h('iframe', { class: 'msg-body', sandbox: 'allow-popups', title: 'Message' });
// Attachments come last, at the foot of the message.
const attachments = h('div', { class: 'attachments' });
root.append(placeholder, actions, sign, head, imagesbar, linkbar, frame, attachments);

export function readerElement() { return root; }

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: 'full', timeStyle: 'short' });

function address(a) { return a.name ? a.name + ' <' + a.email + '>' : a.email; }

// The back end shows the save dialog and streams the attachment to the file
// that is chosen; the page hears only how it ended.
function saveAttachment(m, a) {
  const name = a.filename || a.contentType;
  const requestId = newRequestId('s');
  waitForEvent(requestId, function (ev) {
    if (ev.cancelled) { return; }
    toast(ev.error ? t('downloadFailed', { error: ev.error }) : t('webAttachmentSaved', { name: name }));
  });
  call('attachment.save', {
    requestId: requestId,
    accountId: m.accountId,
    folder: m.folder,
    messageId: m.id,
    index: a.index,
    fileName: a.filename || ''
  }, function (err) {
    if (err) {
      cancel(requestId);
      toast(t('downloadFailed', { error: err.message }));
    }
  });
}

// Reads an invitation attachment into the calendar for invitations, once the person agrees.
function addToCalendar(m, a) {
  confirmModal({
    title: t('calendarInviteAddTitle'),
    body: t('calendarInviteImportPrompt'),
    ok: t('calendarInviteAddAction')
  }, function (yes) {
    if (!yes) { return; }
    const requestId = newRequestId('i');
    waitForEvent(requestId, function (ev) {
      toast(ev.error ? t('calendarInviteImportFailed', { error: ev.error }) : t('calendarInviteImportDone', { count: ev.count }));
    });
    call('calendar.importInvite', {
      requestId: requestId,
      accountId: m.accountId,
      folder: m.folder,
      messageId: m.id,
      index: a.index
    }, function (err) {
      if (err) {
        cancel(requestId);
        toast(t('calendarInviteImportFailed', { error: err.message }));
      }
    });
  });
}

function formatSize(n) {
  if (n < 1024) { return n + ' B'; }
  if (n < 1024 * 1024) { return Math.round(n / 1024) + ' KB'; }
  return (n / (1024 * 1024)).toFixed(1) + ' MB';
}

// The list row already knows From, Date and Subject; the envelope adds To and
// Cc. A field the envelope leaves empty keeps the list's value rather than
// blanking it.
function headerRows(m) {
  const hd = m.headers;
  const from = hd && hd.from.length ? hd.from.map(address).join(', ') : m.summary.from;
  const date = hd && hd.dateMs != null ? hd.dateMs : m.summary.dateMs;
  const rows = [['headerFrom', from]];
  // "Minimal headers" hides Cc only.
  if (hd && hd.cc.length && !getLocal('headersMinimal', true)) { rows.push(['headerCc', hd.cc.map(address).join(', ')]); }
  if (date != null) { rows.push(['headerDate', dateFormat.format(new Date(date))]); }
  return rows.map(function (r) {
    return h('div', { class: 'hdr' },
      h('span', { class: 'label', text: t(r[0]) }),
      h('span', { class: 'value', text: r[1] }));
  });
}

function linkBar(m) {
  return [
    h('span', { text: t('webOpenLinkQuestion') }),
    h('code', { class: 'url', text: m.link }),
    h('button', {
      class: 'primary',
      onClick: function () {
        const url = m.link;
        m.link = null;
        call('app.openUrl', { url: url }, function (err) {
          if (err) { m.error = err.message; }
          changed();
        });
        changed();
      }
    }, t('webOpenLink')),
    h('button', { onClick: function () { m.link = null; changed(); } }, t('cancel'))
  ];
}

// What can be done with the message: reply, reply to all, forward. News can
// only be replied to; a message in Drafts is edited rather than replied to.
export function messageActions(m) {
  const account = state.accounts[m.accountId];
  const kind = composeSendKind(account);
  if (!kind) { return []; }
  if (kind === 'smtp' && looksLikeDrafts(account, m.folder)) { return ['draft']; }
  if (kind === 'nntp') { return ['reply']; }
  return ['reply', 'replyAll', 'forward'];
}

const ACTION_LABELS = { draft: 'webEditDraft', reply: 'messageActionReply', replyAll: 'messageActionReplyAll', forward: 'messageActionForward' };
// The same Lucide pictures as the Flutter interface used.
const ACTION_ICONS = { draft: 'square-pen', reply: 'reply', replyAll: 'reply-all', forward: 'forward' };

// What the signature mark says, by verdict: the icon and the tooltip.
const SIGNATURE = {
  valid: { icon: 'badge-check', tip: 'messageSignatureVerifiedTooltip' },
  invalid: { icon: 'badge-x', tip: 'messageSignatureInvalidTooltip' },
  unknown: { icon: 'circle-help', tip: 'messageSignatureUnknownTooltip' }
};

// Starts composing from the open message: one of `messageActions`.
export function runMessageAction(kind) {
  const m = state.message;
  if (!m || messageActions(m).indexOf(kind) < 0) { return false; }
  openCompose({ accountId: m.accountId, kind: kind, folder: m.folder, messageId: m.id });
  return true;
}

function actionButtons(m) {
  return messageActions(m).map(function (action) {
    const label = t(ACTION_LABELS[action]);
    return h('button', {
      type: 'button',
      class: 'round-btn',
      'data-action': action,
      title: label,
      'aria-label': label,
      onClick: function () { runMessageAction(action); }
    }, h('span', { class: 'icon icon-' + ACTION_ICONS[action], 'aria-hidden': 'true' }));
  });
}

export function updateReader() {
  const m = state.message;
  placeholder.hidden = !!m;
  placeholder.textContent = state.error || t('selectMessage');
  head.hidden = !m;
  actions.hidden = !m;
  frame.hidden = !m;
  if (!m) {
    attachments.hidden = true;
    linkbar.hidden = true;
    imagesbar.hidden = true;
    sign.hidden = true;
    return;
  }

  setChildren(actions, actionButtons(m));

  // A verdict only for a message that is signed and whose sender has a key.
  const verdict = SIGNATURE[m.signature];
  sign.hidden = !verdict;
  sign.className = 'msg-sign' + (verdict ? ' sign-' + m.signature : '');
  setChildren(sign, verdict ? [h('span', { class: 'icon icon-' + verdict.icon, 'aria-hidden': 'true' })] : []);
  if (verdict) {
    sign.title = t(verdict.tip);
    sign.setAttribute('aria-label', t(verdict.tip));
  }

  const subject = (m.headers && m.headers.subject) || m.summary.subject || t('webNoSubject');
  setChildren(head, [
    h('h2', { class: 'subject', text: subject }),
    headerRows(m),
    m.error ? h('p', { class: 'notice', text: m.error }) : null
  ]);

  const files = m.attachments.filter(function (a) { return !a.contentId; });
  attachments.hidden = files.length === 0;
  setChildren(attachments, files.map(function (a) {
    const save = h('button', {
      class: 'attachment',
      type: 'button',
      title: t('saveAttachment'),
      'data-index': String(a.index),
      onClick: function () { saveAttachment(m, a); }
    }, a.filename || a.contentType, h('span', { class: 'size', text: formatSize(a.size) }));
    if (!a.invite) { return save; }
    // An invitation can also be added to the calendar.
    return h('span', { class: 'attachment-group' }, save, h('button', {
      class: 'attachment add-to-calendar',
      type: 'button',
      title: t('webCalendarAddToCalendar'),
      'aria-label': t('webCalendarAddToCalendar'),
      'data-index': String(a.index),
      onClick: function () { addToCalendar(m, a); }
    }, h('span', { class: 'icon icon-calendar', 'aria-hidden': 'true' })));
  }));

  // Blocked by default; offered only when the body really refers to remote images.
  const offer = m.remoteCount > 0 && !imagesAllowed(m);
  imagesbar.hidden = !offer;
  setChildren(imagesbar, offer ? [
    h('span', { class: 'url', text: t('remoteImagesBlocked') }),
    h('button', { type: 'button', class: 'load-images', onClick: function () { m.images = true; changed(); } }, t('loadImages'))
  ] : []);

  linkbar.hidden = !m.link;
  setChildren(linkbar, m.link ? linkBar(m) : []);

  const url = bodyUrl(m);
  if (frame.dataset.url !== url) {
    frame.dataset.url = url;
    frame.src = url;
  }
}
