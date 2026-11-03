// The Outgoing tab: SMTP transports. A list, and an editor for one transport.

import { state } from '../store.js';
import { call } from '../wire.js';
import { h, setChildren } from '../dom.js';
import { t } from '../i18n.js';
import { toast } from '../toast.js';
import { confirmModal } from '../modal.js';
import { newRequestId, waitFor, cancel } from '../requests.js';
import {
  smtpErrorWantsCredentials, wantsGoogleOAuth, promptTransportCredentials, promptGoogleSignIn
} from '../credprompt.js';
import { makeEditor, editorBar } from './editor.js';

export function securityOptions(implicitTlsKey) {
  return [
    { value: 'tls', label: t(implicitTlsKey) },
    { value: 'starttls', label: t('mailSecurityStarttls') },
    { value: 'plain', label: t('mailSecurityNoEncryption') }
  ];
}

export function oauthOptions() {
  return [
    { value: '', label: t('webOauthNone') },
    { value: 'google', label: t('webOauthGoogle') },
    { value: 'microsoft', label: t('webOauthMicrosoft') }
  ];
}

const DSN = ['never', 'failure', 'success', 'delay', 'failure,success'];
const DSN_LABELS = {
  never: 'dsnNever', failure: 'dsnFailure', success: 'dsnSuccess', delay: 'dsnDelay',
  'failure,success': 'dsnFailureAndSuccess'
};

export function transportTitle(tr) {
  return tr.displayName || tr.host || t('composeOutgoingTransport');
}

export function transportKind(tr) {
  return String(tr.transportType).toLowerCase() === 'gmail' ? t('transportKindGmail') : t('transportKindSmtp');
}

// Saves the transport has no way to know the server is fine, so after saving
// it logs in once. A missing login asks for one and tries again.
function verify(transport, cb) {
  const requestId = newRequestId('v');
  waitFor(requestId, function (err) {
    if (!err) { cb(true, null); return; }
    if (!smtpErrorWantsCredentials(err)) { cb(false, err); return; }
    const again = function (ok) {
      if (!ok) { cb(false, t('composeSendCancelledNoSmtpCredentials')); return; }
      verify(transport, cb);
    };
    if (wantsGoogleOAuth(transport, err)) {
      promptGoogleSignIn('transport', transport.id, transport.displayName, again);
    } else {
      promptTransportCredentials(transport, again);
    }
  });
  call('transport.verify', { id: transport.id, requestId: requestId }, function (err) {
    if (err) {
      cancel(requestId);
      cb(false, err.message);
    }
  });
}

export function build() {
  const root = h('div', { class: 'settings-page' });
  let editing = false;

  function showList() {
    editing = false;
    const list = Object.values(state.transports);
    setChildren(root, [
      h('h3', { text: t('outgoingListTitle') }),
      h('p', { class: 'hint', text: t('outgoingListSubtitle') }),
      list.map(function (tr) {
        return h('div', { class: 'item', 'data-transport': tr.id },
          h('button', { class: 'item-main', onClick: function () { showEditor(tr); } },
            h('strong', { text: transportTitle(tr) }), h('span', { class: 'dim', text: transportKind(tr) })),
          h('button', {
            class: 'danger', title: t('deleteTooltip'), 'aria-label': t('deleteTooltip'),
            onClick: function () { remove(tr); }
          }, '×'));
      }),
      h('div', { class: 'item' }, h('button', { class: 'item-main add', onClick: function () { showEditor(null); } }, '+ ' + t('addTransport'))),
      list.length === 0 ? h('p', { class: 'hint', text: t('noTransportsYet') }) : null
    ]);
  }

  function remove(tr) {
    confirmModal({
      title: t('removeTransportTitle'),
      body: t('removeTransportBody', { name: transportTitle(tr) }),
      ok: t('remove'),
      danger: true
    }, function (yes) {
      if (!yes) { return; }
      call('transport.remove', { id: tr.id }, function (err) {
        toast(err ? t('operationFailed', { error: err.message }) : t('removedTransport', { name: transportTitle(tr) }));
      });
    });
  }

  function showEditor(existing) {
    editing = true;
    const isNew = !existing;
    const dsn = existing && DSN.indexOf(existing.dsnNotify) >= 0 ? existing.dsnNotify : 'failure';
    const oauth = existing && ['google', 'microsoft'].indexOf(existing.oauthProvider) >= 0 ? existing.oauthProvider : '';
    const ed = makeEditor({
      id: existing ? existing.id : '',
      displayName: existing ? existing.displayName : '',
      host: existing ? existing.host : '',
      port: String(existing && existing.port ? existing.port : 465),
      security: existing ? existing.security || 'tls' : 'tls',
      oauthProvider: oauth,
      defaultFrom: existing ? existing.defaultFrom : '',
      dsnNotify: dsn
    });
    const message = h('p', { class: 'error', role: 'alert' });

    ed.readonly({ label: t('transportKindLabel'), value: t('transportKindSmtp'), hint: t('transportTypeFixedHelper') });
    ed.text('displayName', { label: t('accountNameLabel') });
    ed.text('host', { label: t('smtpHostLabel'), placeholder: 'smtp.example.com' });
    ed.number('port', { label: t('portLabel'), hint: t('smtpPortHelper') });
    ed.select('security', { label: t('securityLabel'), options: securityOptions('mailSecurityImplicitTlsSmtp') });
    ed.select('oauthProvider', { label: t('webOauthProviderLabel'), hint: t('webOauthHelperSmtp'), options: oauthOptions() });
    ed.text('defaultFrom', { label: t('defaultFromLabel'), hint: t('defaultFromHelper') });
    ed.select('dsnNotify', {
      label: t('dsnNotifyLabel'),
      options: DSN.map(function (v) { return { value: v, label: t(DSN_LABELS[v]) }; })
    });

    const bar = editorBar({
      title: t(isNew ? 'newTransport' : 'editTransport'),
      onBack: function () {
        if (!ed.isDirty()) { showList(); return; }
        confirmModal({
          title: t('discardChangesTitle'), body: t('discardChangesBody'),
          ok: t('discard'), cancel: t('keepEditing'), danger: true
        }, function (yes) { if (yes) { showList(); } });
      },
      onSave: function () { save(ed, bar, message); }
    });
    setChildren(root, [bar.el, ed.root, message]);
  }

  function save(ed, bar, message) {
    message.textContent = '';
    const d = ed.draft;
    if (d.displayName.trim() === '' || d.host.trim() === '') { toast(t('transportDisplayHostRequired')); return; }
    const port = Number(d.port.trim());
    if (!/^\d+$/.test(d.port.trim()) || port < 1 || port > 65535) { toast(t('validationPortRequired')); return; }

    bar.setBusy(true);
    call('transport.save', {
      id: d.id, displayName: d.displayName.trim(), host: d.host.trim(), port: port,
      security: d.security, oauthProvider: d.oauthProvider, defaultFrom: d.defaultFrom.trim(), dsnNotify: d.dsnNotify
    }, function (err, result) {
      if (err) {
        bar.setBusy(false);
        message.textContent = err.message;
        return;
      }
      d.id = result.id;
      ed.markClean();
      verify({
        id: d.id, displayName: d.displayName.trim(), host: d.host.trim(),
        defaultFrom: d.defaultFrom.trim(), oauthProvider: d.oauthProvider
      }, function (ok, why) {
        bar.setBusy(false);
        if (ok) {
          toast(t('transportSavedAndVerified'));
          showList();
        } else {
          message.textContent = why || '';
          toast(t('transportSavedVerifyPending'));
        }
      });
    });
  }

  showList();
  return {
    root: root,
    // The list follows the configuration; an open editor is left alone.
    refresh: function () { if (!editing) { showList(); } }
  };
}
