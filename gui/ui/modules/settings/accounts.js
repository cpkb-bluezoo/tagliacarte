// The Accounts tab: the list, and an editor whose fields depend on the type.

import { state } from '../store.js';
import { call } from '../wire.js';
import { h, setChildren } from '../dom.js';
import { t } from '../i18n.js';
import { toast } from '../toast.js';
import { copyText } from '../clipboard.js';
import { confirmModal, openModal, formModal } from '../modal.js';
import { pickPath } from '../dialogs.js';
import { waitFor } from '../requests.js';
import { promptStoreCredentials, promptGoogleSignIn, promptNostrSecret, loginOptions } from '../credprompt.js';
import { makeEditor, editorBar } from './editor.js';
import { securityOptions, oauthOptions, transportTitle, transportKind } from './transports.js';
import * as S from './accountschema.js';
import { accountAvatar } from '../avatar.js';

function randomId() { return 's' + Date.now() + Math.floor(Math.random() * 1000); }

function trashOptions(deleteLabelKey) {
  return [
    { value: 'Move to Trash', label: t('deleteModeMoveToTrash') },
    { value: deleteLabelKey === 'maildir' ? 'Delete immediately' : 'Mark Deleted',
      label: t(deleteLabelKey === 'maildir' ? 'deleteModeDeleteImmediately' : 'deleteModeMarkDeleted') }
  ];
}

// After a save the account needs a login if the vault has none. Maildir and
// mbox never do. Nostr also pulls its profile and relays, and publishes ours
// unless the secret was only just linked (then there is nothing to publish yet).
function afterSave(account, linkedSecret) {
  const type = S.normaliseType(account.backendType);
  if (S.isLocalStore(type)) { return; }
  const id = account.id;
  const refresh = function () { call('session.refreshFolders', { accountId: id }, function () {}); };

  const signedIn = function (cb) {
    call('credential.hasStorePassword', { accountId: id }, function (err, has) { cb(!err && has === true); });
  };

  const askLogin = function (done) {
    if (type === 'nostr') {
      promptNostrSecret(id, function (npub) { done(!!npub); });
    } else if (type === 'gmail' || (type === 'imap' && account.attrs.oauthProvider === 'google')) {
      promptGoogleSignIn('store', id, account.label, done);
    } else {
      const opts = loginOptions(account);
      promptStoreCredentials(account, opts, done);
    }
  };

  signedIn(function (has) {
    const proceed = function () {
      refresh();
      if (type !== 'nostr') { return; }
      call('nostr.syncProfile', { accountId: id }, function () {});
      waitFor(id, function () {
        if (!linkedSecret) { call('nostr.publishProfile', { accountId: id }, function () {}); }
      });
    };
    if (has) { proceed(); } else { askLogin(function (ok) { if (ok) { proceed(); } }); }
  });
}

export function build() {
  const root = h('div', { class: 'settings-page' });
  let editing = false;

  function showList() {
    editing = false;
    const accounts = state.accountOrder.map(function (id) { return state.accounts[id]; });
    setChildren(root, [
      h('h3', { text: t('accountsListTitle') }),
      h('p', { class: 'hint', text: t('accountsListSubtitle') }),
      accounts.map(function (a) {
        return h('div', { class: 'item', 'data-account': a.id },
          h('button', { class: 'item-main', onClick: function () { showEditor(a.backendType, a); } },
            h('span', { class: 'item-line' }, accountAvatar(a, 'list', 34), h('span', {}, h('strong', { text: a.label || a.id }),
              h('span', { class: 'dim block', text: S.typeLabel(a.backendType) })))),
          h('button', {
            class: 'danger', title: t('deleteTooltip'), 'aria-label': t('deleteTooltip'),
            onClick: function () { remove(a); }
          }, '×'));
      }),
      h('div', { class: 'item' }, h('button', { class: 'item-main add', onClick: chooseType }, '+ ' + t('addAccount'))),
      accounts.length === 0 ? h('p', { class: 'hint', text: t('noAccountsYet') }) : null
    ]);
  }

  function chooseType() {
    openModal({
      title: t('accountTypeDialogTitle'),
      content: function (ctl) {
        return h('div', { class: 'choices' }, S.TYPES.map(function (ty) {
          return h('button', {
            type: 'button', class: 'choice', 'data-type': ty.id,
            onClick: function (e) { e.preventDefault(); ctl.close(ty.id); }
          }, ty.label);
        }));
      },
      actions: [{ label: t('cancel'), run: function (c) { c.close(null); } }]
    }, function (type) { if (type) { showEditor(type, null); } });
  }

  function remove(a) {
    confirmModal({
      title: t('removeAccountTitle'),
      body: t('removeAccountBody', { label: a.label || a.id }),
      ok: t('remove'),
      danger: true
    }, function (yes) {
      if (!yes) { return; }
      call('account.remove', { id: a.id }, function (err) {
        toast(err ? t('operationFailed', { error: err.message }) : t('removedAccount', { label: a.label || a.id }));
      });
    });
  }

  function showEditor(rawType, existing) {
    editing = true;
    const type = S.normaliseType(rawType);
    const isNew = !existing;
    const accountId = existing ? existing.id : randomId();
    const ed = makeEditor(S.initialDraft(type, existing));
    const d = ed.draft;
    const message = h('p', { class: 'error', role: 'alert' });
    let linked = false;

    ed.readonly({ label: t('accountTypeLabel'), value: S.typeLabel(type), hint: t('accountTypeHelper') });
    ed.text('label', { label: t('accountNameLabel') });
    ed.path('avatarUrl', {
      label: t('avatarUrlLabel'),
      hint: t('avatarUrlHelper'),
      browse: function (cb) { pickPath({ title: t('avatarUrlLabel') }, cb); }
    });

    const serverFields = function (sectionKey, hostKey, portHint, tlsKey) {
      ed.heading(t(sectionKey));
      ed.text('host', { label: t(hostKey), placeholder: type === 'imap' ? 'imap.example.com' : type === 'pop3' ? 'pop.example.com' : 'news.example.com' });
      ed.number('port', { label: t('portLabel'), hint: t(portHint) });
      ed.select('security', { label: t('securityLabel'), options: securityOptions(tlsKey) });
    };

    switch (type) {
      case 'imap':
        serverFields('imapServerSection', 'hostLabel', 'portHelperImap', 'mailSecurityImplicitTlsImap');
        ed.select('oauthProvider', { label: t('webOauthProviderLabel'), hint: t('webOauthHelperImap'), options: oauthOptions() });
        ed.number('idleMin', { label: t('accountImapMinIdleSecondsLabel'), hint: t('accountImapMinIdleSecondsHelper') });
        ed.select('deleteMode', { label: t('deleteModeLabel'), options: trashOptions('imap') });
        ed.text('trashFolder', { label: t('trashFolderNameLabel'), visible: function (x) { return x.deleteMode === 'Move to Trash'; } });
        ed.text('junkFolder', { label: t('junkFolderNameLabel') });
        ed.text('sentFolder', { label: t('webSentFolderName'), hint: t('webSentFolderHelper') });
        ed.text('draftsFolder', { label: t('webDraftsFolderName'), hint: t('webDraftsFolderHelper') });
        ed.text('mirrorSent', { label: t('webMirrorSent'), hint: t('webMirrorSentHelper') });
        ed.number('draftAutosave', { label: t('webDraftAutosave'), hint: t('webDraftAutosaveHelper') });
        break;
      case 'gmail':
        ed.text('email', { label: t('webGmailAddress'), hint: t('webGmailAddressHelper') });
        for (const [key, labelKey, def] of [['trashLabel', 'webGmailTrashLabel', 'TRASH'], ['spamLabel', 'webGmailSpamLabel', 'SPAM'],
          ['sentLabel', 'webGmailSentLabel', 'SENT'], ['draftLabel', 'webGmailDraftLabel', 'DRAFT'], ['inboxLabel', 'webGmailInboxLabel', 'INBOX']]) {
          ed.text(key, { label: t(labelKey), hint: t('webDefaultValue', { value: def }) });
        }
        break;
      case 'exchange':
        ed.text('trashFolder', { label: t('trashFolderNameLabel'), hint: t('exchangeTrashFolderHelper') });
        ed.text('junkFolder', { label: t('junkFolderNameLabel'), hint: t('exchangeJunkFolderHelper') });
        break;
      case 'pop3':
        serverFields('pop3ServerSection', 'hostLabel', 'portHelperPop3', 'mailSecurityImplicitTlsPop3');
        break;
      case 'nntp':
        serverFields('nntpServerSection', 'serverHostLabel', 'portHelperNntp', 'mailSecurityImplicitTlsNntp');
        ed.text('defaultFrom', { label: t('accountNntpDefaultFromLabel'), hint: t('accountNntpDefaultFromHelper') });
        break;
      case 'maildir':
      case 'mbox': {
        const maildir = type === 'maildir';
        ed.heading(t('localMailboxSection'));
        ed.path('path', {
          label: t(maildir ? 'pathMaildirRoot' : 'pathMboxFile'),
          hint: t(maildir ? 'helperMaildirPath' : 'helperMboxPath'),
          browse: function (cb) {
            pickPath({ folder: maildir, title: t(maildir ? 'chooseMaildirFolderTitle' : 'chooseMboxFileTitle') }, cb);
          }
        });
        if (maildir) {
          ed.select('deleteMode', { label: t('deleteModeLabel'), options: trashOptions('maildir') });
          ed.text('trashFolder', { label: t('trashFolderNameLabel'), visible: function (x) { return x.deleteMode === 'Move to Trash'; } });
          ed.text('junkFolder', { label: t('junkFolderNameLabel') });
        }
        break;
      }
      case 'nostr':
        ed.heading(t('nostrSection'));
        ed.text('nip05', { label: t('webNip05') });
        ed.node(npubField(d, ed));
        ed.node(relayEditor(ed));
        ed.node(h('button', {
          type: 'button',
          onClick: function () { createIdentity(); }
        }, t('nostrNewIdentityTooltip')));
        break;
      case 'matrix':
        ed.text('mxid', { label: t('accountMatrixUserIdLabel'), hint: t('accountMatrixMxidHelper'), placeholder: '@you:matrix.org' });
        break;
      default:
        break;
    }

    if (S.requiresTransport(type)) { ed.node(transportEditor(ed)); }
    if (existing) {
      ed.node(h('p', { class: 'hint', text: t('webConnectionLine', { summary: S.connectionSummary(existing) }) }));
    }

    // A secret key is made here, shown once, and stored for this account.
    function createIdentity() {
      call('nostr.generateKeypair', undefined, function (err, kp) {
        if (err || !kp || !kp.secretHex) { toast(t('webKeyGenFailed')); return; }
        openModal({
          title: t('webNostrKeyTitle'),
          content: h('div', {}, h('p', { text: t('webNostrKeyBody') }), h('code', { class: 'secret', text: kp.secretHex })),
          actions: [
            { label: t('cancel'), run: function (c) { c.close(false); } },
            {
              label: t('webNostrKeySave'),
              primary: true,
              run: function (c) {
                c.setBusy(true);
                call('nostr.linkSecret', { accountId: accountId, secret: kp.secretHex }, function (e2, r) {
                  if (e2) { c.setBusy(false); c.setError(e2.message); return; }
                  d.npub = r.npub;
                  linked = true;
                  ed.refresh();
                  c.close(true);
                });
              }
            }
          ]
        }, function (ok) { if (ok) { toast(t('webNostrIdentityReady')); } });
      });
    }

    const bar = editorBar({
      title: isNew ? t('accountDetailTitleNew', { type: S.typeLabel(type) }) : t('accountDetailTitleEdit', { label: existing.label }),
      onBack: function () {
        if (!ed.isDirty()) { showList(); return; }
        confirmModal({
          title: t('discardChangesTitle'), body: t('discardChangesBody'),
          ok: t('discard'), cancel: t('keepEditing'), danger: true
        }, function (yes) { if (yes) { showList(); } });
      },
      onSave: save
    });

    function save() {
      message.textContent = '';
      const problem = S.validate(type, d);
      if (problem) { toast(t(problem.key, problem.params)); return; }
      bar.setBusy(true);
      // A Nostr account without a key first asks for one.
      if (type === 'nostr' && !d.npub) {
        promptNostrSecret(accountId, function (npub) {
          if (!npub) { bar.setBusy(false); return; }
          d.npub = npub;
          linked = true;
          persist();
        });
        return;
      }
      persist();
    }

    function persist() {
      const account = S.toAccount(type, d, existing, accountId);
      call('account.save', account, function (err) {
        bar.setBusy(false);
        if (err) { message.textContent = err.message; return; }
        ed.markClean();
        toast(t('accountSaved'));
        showList();
        afterSave(account, linked);
      });
    }

    setChildren(root, [bar.el, ed.root, message]);
  }

  showList();
  return {
    root: root,
    refresh: function () { if (!editing) { showList(); } }
  };
}

// The npub, read-only, with a button that copies it.
function npubField(d, ed) {
  const value = h('div', { class: 'readonly mono' });
  const el = h('div', { class: 'field' },
    h('span', { class: 'label', text: t('webNpubReadonly') }),
    h('div', { class: 'row' }, value, h('button', {
      type: 'button',
      onClick: function () {
        if (!d.npub) { return; }
        copyText(d.npub, t('webCopiedNpub'));
      }
    }, t('webCopyNpub'))));
  const show = function () { value.textContent = d.npub || '—'; };
  show();
  ed.onChange(show);
  return el;
}

// Relay URLs: a row each, at least one, and a row for the next.
function relayEditor(ed) {
  const d = ed.draft;
  const box = h('div', { class: 'field' });
  const pending = h('input', { type: 'text', placeholder: t('relayAddFieldHint'), spellcheck: 'false' });
  const addPending = function () {
    const v = pending.value.trim();
    if (v) { d.relays.push(v); pending.value = ''; render(); ed.refresh(); }
  };
  pending.addEventListener('keydown', function (e) { if (e.key === 'Enter') { e.preventDefault(); addPending(); } });
  function render() {
    setChildren(box, [
      h('span', { class: 'label', text: t('relayUrlsLabel') }),
      h('p', { class: 'hint', text: t('relayUrlsHelper') }),
      d.relays.map(function (url, i) {
        const input = h('input', { type: 'text', value: url, placeholder: 'wss://...', spellcheck: 'false' });
        input.addEventListener('input', function () { d.relays[i] = input.value; ed.refresh(); });
        return h('div', { class: 'relay-row' }, input,
          d.relays.length > 1 ? h('button', {
            type: 'button', title: t('relayRemoveTooltip'), 'aria-label': t('relayRemoveTooltip'),
            onClick: function () { d.relays.splice(i, 1); render(); ed.refresh(); }
          }, '×') : null);
      }),
      h('div', { class: 'relay-row' }, pending,
        h('button', { type: 'button', title: t('relayAddTooltip'), 'aria-label': t('relayAddTooltip'), onClick: addPending }, '+'))
    ]);
  }
  render();
  return box;
}

// The transports this account sends through, in order of preference.
function transportEditor(ed) {
  const d = ed.draft;
  const box = h('div', { class: 'field transports' });
  const move = function (i, by) {
    const j = i + by;
    if (j < 0 || j >= d.transportIds.length) { return; }
    const x = d.transportIds[i];
    d.transportIds[i] = d.transportIds[j];
    d.transportIds[j] = x;
    render();
    ed.refresh();
  };
  function addOne() {
    const candidates = Object.values(state.transports).filter(function (tr) { return d.transportIds.indexOf(tr.id) < 0; });
    if (candidates.length === 0) { toast(t('webNoTransportAvailable')); return; }
    openModal({
      title: t('addTransportDialogTitle'),
      content: function (ctl) {
        return h('div', { class: 'choices' }, candidates.map(function (tr) {
          return h('button', {
            type: 'button', class: 'choice', 'data-transport': tr.id,
            onClick: function (e) { e.preventDefault(); ctl.close(tr.id); }
          }, transportTitle(tr));
        }));
      },
      actions: [{ label: t('cancel'), run: function (c) { c.close(null); } }]
    }, function (id) {
      if (id) { d.transportIds.push(id); render(); ed.refresh(); }
    });
  }
  function render() {
    setChildren(box, [
      h('h3', { text: t('outgoingTransportsSection') }),
      h('p', { class: 'hint', text: t(d.transportIds.length ? 'transportsOrderHint' : 'noTransportsHintLinked') }),
      d.transportIds.map(function (id, i) {
        const tr = state.transports[id];
        return h('div', { class: 'transport-row', 'data-transport': id },
          h('div', { class: 'grow' }, h('strong', { text: tr ? transportTitle(tr) : t('unknownTransport') }),
            tr ? h('span', { class: 'dim block', text: transportKind(tr) }) : null),
          h('button', { type: 'button', title: t('moveUpTooltip'), 'aria-label': t('moveUpTooltip'), disabled: i === 0, onClick: function () { move(i, -1); } }, '↑'),
          h('button', { type: 'button', title: t('moveDownTooltip'), 'aria-label': t('moveDownTooltip'), disabled: i === d.transportIds.length - 1, onClick: function () { move(i, 1); } }, '↓'),
          h('button', {
            type: 'button', title: t('removeFromAccountTooltip'), 'aria-label': t('removeFromAccountTooltip'),
            onClick: function () { d.transportIds.splice(i, 1); render(); ed.refresh(); }
          }, '×'));
      }),
      h('button', { type: 'button', onClick: addOne }, t('addTransportToAccount'))
    ]);
  }
  render();
  return box;
}
