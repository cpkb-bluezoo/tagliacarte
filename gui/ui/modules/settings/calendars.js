// The Calendars tab: calendars added by address (CalDAV), each signed in to with
// nothing, a user name and password, or one of the user's own accounts (OAuth).
// A password goes to the back end and is never shown again; the list says only
// whether one is kept. The calendars themselves are shown in the calendar view.

import { state } from '../store.js';
import { call } from '../wire.js';
import { h, setChildren } from '../dom.js';
import { t } from '../i18n.js';
import { toast } from '../toast.js';
import { confirmModal, formModal } from '../modal.js';
import { syncCalendars } from '../calendar/view.js';

// calendarSourcesStarted / calendarSource / calendarSourcesLoaded.
export function onSourceEvent(ev) {
  const s = state.calendarSources;
  switch (ev.type) {
    case 'calendarSourcesStarted':
      s.pending = [];
      break;
    case 'calendarSource':
      if (s.pending) { s.pending.push(ev); }
      break;
    case 'calendarSourcesLoaded':
      if (s.pending) {
        s.items = s.pending;
        s.pending = null;
        s.loaded = true;
      }
      break;
    default:
      break;
  }
}

// The accounts a calendar can borrow a sign-in from: those that sign in with Google.
function googleAccounts() {
  return state.accountOrder.map(function (id) { return state.accounts[id]; }).filter(function (a) {
    return a && (String(a.backendType).toLowerCase() === 'gmail' || String((a.attrs || {}).oauthProvider || '').toLowerCase() === 'google');
  });
}

function accountLabel(id) {
  const a = state.accounts[id];
  return a ? (a.label || a.id) : id;
}

export function build() {
  const root = h('div', { class: 'settings-page calendar-sources' });
  let started = false;

  const load = function () {
    call('calendar.sources', undefined, function (err) { if (err) { toast(t('operationFailed', { error: err.message })); } });
  };

  function sourceDialog(existing) {
    const isNew = !existing;
    const accounts = googleAccounts();
    const kind = existing ? existing.authKind : 'none';
    formModal({
      title: isNew ? t('webAddCalendarSource') : t('webEditCalendarSource'),
      note: t('webCalendarSourceNote'),
      fields: [
        { key: 'name', label: t('webCalendarSourceName'), value: existing ? existing.name : '' },
        { key: 'url', label: t('webCalendarSourceUrl'), value: existing ? existing.url : '', placeholder: 'https://dav.example.com/calendars/me/work/' },
        {
          key: 'authKind', type: 'select', label: t('webCalendarSignIn'), value: kind,
          options: [
            { value: 'none', label: t('webCalendarSignInNone') },
            { value: 'basic', label: t('webCalendarSignInPassword') },
            { value: 'account', label: t('webCalendarSignInAccount') }
          ]
        },
        { key: 'username', label: t('webCalendarSourceUsername'), value: existing ? existing.username || '' : '', showWhen: { key: 'authKind', is: ['basic'] } },
        {
          key: 'password', type: 'password', showWhen: { key: 'authKind', is: ['basic'] },
          label: existing && existing.hasPassword ? t('webCalendarSourcePasswordKeep') : t('webCalendarSourcePassword')
        },
        {
          key: 'accountId', type: 'select', label: t('webCalendarSourceAccount'), value: existing ? existing.accountId || '' : '',
          showWhen: { key: 'authKind', is: ['account'] },
          options: accounts.length
            ? accounts.map(function (a) { return { value: a.id, label: a.label || a.id }; })
            : [{ value: '', label: t('webCalendarNoGoogleAccount') }]
        },
        { key: 'enabled', type: 'checkbox', label: t('webEnabled'), value: existing ? existing.enabled : true }
      ],
      ok: isNew ? t('webAdd') : t('save'),
      submit: function (v, ctl) {
        call('calendar.sourceSave', {
          id: existing ? existing.id : undefined, name: v.name, url: v.url, authKind: v.authKind,
          username: v.username, password: v.password, accountId: v.accountId, enabled: v.enabled
        }, function (err) { if (err) { ctl.setError(err.message); } else { ctl.close(true); } });
      }
    }, function (ok) {
      if (!ok) { return; }
      load();
      // What was added is wanted at once.
      syncCalendars();
    });
  }

  function remove(src) {
    confirmModal({ title: t('webDeleteCalendarSource'), body: src.name + ' - ' + t('webDeleteCalendarSourceBody'), ok: t('delete'), danger: true }, function (yes) {
      if (!yes) { return; }
      call('calendar.sourceDelete', { id: src.id }, function (err) {
        if (err) { toast(t('operationFailed', { error: err.message })); return; }
        load();
      });
    });
  }

  function describe(src) {
    const how = src.authKind === 'basic'
      ? src.username + (src.hasPassword ? '' : ' (' + t('webCalendarNoPassword') + ')')
      : src.authKind === 'account' ? accountLabel(src.accountId) : t('webCalendarSignInNone');
    return [src.url, how, src.enabled ? '' : t('webOff')].filter(Boolean).join(' · ');
  }

  function render() {
    const s = state.calendarSources;
    if (!s.loaded) { setChildren(root, [h('p', { class: 'hint', text: t('webLoadingTargets') })]); return; }
    setChildren(root, [
      h('p', { class: 'hint', text: t('webCalendarSourcesHint') }),
      s.items.length === 0 ? h('p', { class: 'hint', text: t('webCalendarNoSources') }) : s.items.map(function (src) {
        return h('div', { class: 'item', 'data-source': src.id },
          h('div', { class: 'item-main static' },
            h('strong', { text: src.name }),
            h('span', { class: 'dim', text: describe(src) })),
          h('div', { class: 'row-actions' },
            h('button', { type: 'button', 'data-action': 'edit-source', onClick: function () { sourceDialog(src); } }, t('webEdit')),
            h('button', { type: 'button', class: 'danger', 'data-action': 'delete-source', onClick: function () { remove(src); } }, t('delete'))));
      }),
      h('div', { class: 'row-actions bottom' },
        h('button', { type: 'button', 'data-action': 'add-source', onClick: function () { sourceDialog(null); } }, '+ ' + t('webAddCalendarSource')),
        s.items.length ? h('button', { type: 'button', 'data-action': 'sync-sources', onClick: syncCalendars }, t('webCalendarSyncNow')) : null)
    ]);
  }

  return {
    root: root,
    refresh: function () {
      if (!started) { started = true; load(); }
      render();
    }
  };
}
