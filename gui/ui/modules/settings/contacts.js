// The Contacts tab: address-book repositories, groups, and the contacts
// themselves with where each may be synced.

import { state } from '../store.js';
import { call } from '../wire.js';
import { h, setChildren } from '../dom.js';
import { t } from '../i18n.js';
import { toast } from '../toast.js';
import { confirmModal, formModal } from '../modal.js';
import { pickPath } from '../dialogs.js';
import { newRequestId, waitForEvent } from '../requests.js';

const PAGE = 200;

export function build() {
  const root = h('div', { class: 'settings-page contacts' });
  const open = new Set();          // details elements the user has opened
  let shown = PAGE;
  let filter = '';
  let started = false;

  const load = function () { call('contacts.load', undefined, function (err) { if (err) { toast(t('operationFailed', { error: err.message })); } }); };
  const then = function (okMessage) {
    return function (err, result) {
      if (err) { toast(t('operationFailed', { error: err.message })); return; }
      if (okMessage) { toast(okMessage(result)); }
      load();
    };
  };

  function repositoryDialog(existing) {
    const isNew = !existing;
    formModal({
      title: isNew ? t('settingsContactsAddCarddavRepo') : t('settingsContactsEditRepository'),
      fields: [
        { key: 'name', label: t('settingsContactsRepoName'), value: existing ? existing.name : 'CardDAV' },
        { key: 'baseUrl', label: t('settingsContactsRepoUrl'), value: existing ? existing.baseUrl : '', placeholder: 'https://dav.example.com' },
        { key: 'collectionPath', label: t('settingsContactsCollectionPath'), value: existing ? existing.collectionPath : '',
          placeholder: '/remote.php/dav/addressbooks/users/me/personal/' },
        { key: 'enabled', type: 'checkbox', label: t('webEnabled'), value: existing ? existing.enabled : true },
        { key: 'defaultNewContact', type: 'checkbox', label: t('settingsContactsDefaultNewContact'), value: existing ? existing.defaultNewContact : false }
      ],
      ok: isNew ? t('webAdd') : t('save'),
      submit: function (v, ctl) {
        call('contacts.repoSave', {
          id: existing ? existing.id : undefined, name: v.name, baseUrl: v.baseUrl, collectionPath: v.collectionPath,
          enabled: v.enabled, defaultNewContact: v.defaultNewContact
        }, function (err) { if (err) { ctl.setError(err.message); } else { ctl.close(true); } });
      }
    }, function (ok) { if (ok) { load(); } });
  }

  function sync(repo, action) {
    const label = t(action === 'pull' ? 'settingsContactsCarddavPull' : 'settingsContactsCarddavPush');
    formModal({
      title: label,
      subtitle: repo.name,
      fields: [
        { key: 'username', label: t('settingsContactsUsername') },
        { key: 'password', label: t('settingsContactsPassword'), type: 'password' }
      ],
      ok: label,
      submit: function (v, ctl) {
        const requestId = newRequestId('c');
        waitForEvent(requestId, function (ev) {
          if (!ev.ok) { ctl.setError(ev.message); return; }
          toast(action === 'pull'
            ? t('webSyncSummaryPull', { message: ev.message, imported: ev.imported, fetched: ev.fetched })
            : t('webSyncSummaryPush', { message: ev.message, pushed: ev.pushed, failed: ev.failed }));
          ctl.close(true);
        });
        call('contacts.carddav', { action: action, id: repo.id, username: v.username, password: v.password, requestId: requestId },
          function (err) { if (err) { ctl.setError(err.message); } });
      }
    }, function (ok) { if (ok) { load(); } });
  }

  function deleteRepository(repo) {
    confirmModal({ title: t('contactsDeleteRepositoryTitle'), body: repo.name, ok: t('delete'), danger: true }, function (yes) {
      if (yes) { call('contacts.repoDelete', { id: repo.id }, then()); }
    });
  }

  function addMember(group) {
    const inGroup = new Set((state.contacts.members[group.id] || []).map(function (m) { return m.id; }));
    const candidates = state.contacts.contacts.filter(function (c) { return !inGroup.has(c.id); });
    if (candidates.length === 0) { toast(t('webNoContactsToAdd')); return; }
    formModal({
      title: t('webAddToGroup'),
      subtitle: group.name,
      fields: [{
        key: 'contact', label: t('webAddMember'), type: 'select',
        options: candidates.map(function (c) { return { value: String(c.id), label: c.displayName + (c.primaryEmail ? ' ' + c.primaryEmail : '') }; })
      }],
      ok: t('webAdd'),
      submit: function (v, ctl) {
        call('contacts.groupAddMember', { contactId: Number(v.contact), groupId: group.id },
          function (err) { if (err) { ctl.setError(err.message); } else { ctl.close(true); } });
      }
    }, function (ok) { if (ok) { load(); } });
  }

  function newGroup() {
    formModal({
      title: t('settingsContactsNewGroup'),
      fields: [{ key: 'name', label: t('settingsContactsGroups') }],
      ok: t('webAdd'),
      submit: function (v, ctl) {
        if (v.name.trim() === '') { ctl.close(false); return; }
        call('contacts.groupSave', { name: v.name }, function (err) { if (err) { ctl.setError(err.message); } else { ctl.close(true); } });
      }
    }, function (ok) { if (ok) { load(); } });
  }

  function deleteGroup(group) {
    confirmModal({ title: t('contactsDeleteGroupTitle'), body: t('contactsDeleteGroupBody', { name: group.name }), ok: t('delete'), danger: true },
      function (yes) { if (yes) { call('contacts.groupDelete', { id: group.id }, then()); } });
  }

  function tracked(id, details) {
    details.open = open.has(id);
    details.addEventListener('toggle', function () { if (details.open) { open.add(id); } else { open.delete(id); } });
    return details;
  }

  function groupBlock(group) {
    const c = state.contacts;
    const members = c.members[group.id] || [];
    const targets = new Set(c.targets.filter(function (x) { return x.groupId === group.id; }).map(function (x) { return x.repositoryId; }));
    return tracked('g' + group.id, h('details', { class: 'block', 'data-group': group.id },
      h('summary', {}, h('strong', { text: group.name }), ' ', h('span', { class: 'dim', text: t('webMemberCount', { count: members.length }) })),
      h('div', { class: 'row-actions' },
        h('button', { type: 'button', onClick: function () { addMember(group); } }, t('webAddMember')),
        h('button', { type: 'button', class: 'danger', onClick: function () { deleteGroup(group); } }, t('delete'))),
      c.repositories.map(function (repo) {
        const input = h('input', { type: 'checkbox' });
        input.checked = targets.has(repo.id);
        input.addEventListener('change', function () {
          call('contacts.groupRule', { groupId: group.id, repositoryId: repo.id, enable: input.checked }, then());
        });
        return h('label', { class: 'switch' }, input, h('span', { class: 'text' },
          h('span', { class: 'label', text: repo.name }), h('span', { class: 'hint', text: repo.kind })));
      }),
      members.map(function (m) {
        return h('div', { class: 'member' }, h('span', { text: m.displayName + (m.primaryEmail ? ' ' + m.primaryEmail : '') }),
          h('button', {
            type: 'button', title: t('webRemoveMember'), 'aria-label': t('webRemoveMember'),
            onClick: function () { call('contacts.groupRemoveMember', { contactId: m.id, groupId: group.id }, then()); }
          }, '×'));
      })));
  }

  function contactBlock(contact) {
    const slot = state.contacts.links[contact.id];
    const gated = !contact.externallyShareOk && contact.importOrigin === 'learned_from_mail';
    const details = tracked('c' + contact.id, h('details', { class: 'block', 'data-contact': contact.id },
      h('summary', {}, h('strong', { text: contact.displayName }), ' ',
        h('span', { class: 'dim', text: [contact.primaryEmail, contact.importOrigin].filter(Boolean).join(' · ') })),
      gated ? h('div', { class: 'gate' }, h('span', { text: t('settingsContactsAllowExternalSync') }),
        h('button', { type: 'button', onClick: function () { call('contacts.allowSharing', { contactId: contact.id }, then()); } },
          t('settingsContactsAllowExternalSync'))) : null,
      !slot || !slot.done ? h('p', { class: 'hint', text: t('webLoadingTargets') }) : slot.rows.map(function (link) {
        const input = h('input', { type: 'checkbox' });
        input.checked = link.linked;
        input.disabled = gated;
        input.addEventListener('change', function () {
          call('contacts.setMembership', { contactId: contact.id, repositoryId: link.repositoryId, include: input.checked },
            function (err) {
              if (err) { toast(t('operationFailed', { error: err.message })); return; }
              state.contacts.links[contact.id] = { rows: [], done: false };
              call('contacts.repoLinks', { contactId: contact.id }, function () {});
              load();
            });
        });
        return h('label', { class: 'switch' }, input, h('span', { class: 'text' },
          h('span', { class: 'label', text: link.name }), h('span', { class: 'hint', text: link.kind })));
      })));
    // Sync targets are fetched when a contact is first opened.
    details.addEventListener('toggle', function () {
      if (details.open && !state.contacts.links[contact.id]) {
        state.contacts.links[contact.id] = { rows: [], done: false };
        call('contacts.repoLinks', { contactId: contact.id }, function () {});
      }
    });
    return details;
  }

  function importVcard() {
    pickPath({ mode: 'file', title: t('settingsContactsImportVcard') }, function (path) {
      if (path) { call('contacts.importVcardFile', { path: path }, then(function (r) { return t('contactsImportDone', { count: r.imported }); })); }
    });
  }

  function exportVcard() {
    pickPath({ mode: 'save', title: t('settingsContactsExportVcard'), fileName: 'contacts.vcf' }, function (path) {
      if (path) { call('contacts.exportVcardFile', { path: path }, function (err, r) {
        toast(err ? t('operationFailed', { error: err.message }) : t('savedToPath', { path: r.path }));
      }); }
    });
  }

  function render() {
    const c = state.contacts;
    if (!c.loaded) { setChildren(root, [h('p', { class: 'hint', text: t('webLoadingTargets') })]); return; }
    const q = filter.trim().toLowerCase();
    const matching = c.contacts.filter(function (x) {
      return !q || (x.displayName + ' ' + (x.primaryEmail || '')).toLowerCase().indexOf(q) >= 0;
    });
    const filterInput = h('input', { type: 'text', value: filter, placeholder: t('webFilterContacts'), spellcheck: 'false' });
    filterInput.addEventListener('input', function () { filter = filterInput.value; shown = PAGE; render(); filterInput.focus(); });
    setChildren(root, [
      h('p', { class: 'hint', text: t('settingsContactsLearnedNote') }),

      h('h3', { text: t('settingsContactsRepositories') }),
      c.repositories.map(function (repo) {
        return h('div', { class: 'item', 'data-repository': repo.id },
          h('div', { class: 'item-main static' },
            h('strong', { text: repo.name }),
            h('span', { class: 'dim', text: [repo.kind, repo.enabled ? '' : t('webOff'), repo.syncError].filter(Boolean).join(' · ') })),
          h('div', { class: 'row-actions' },
            h('button', { type: 'button', onClick: function () { repositoryDialog(repo); } }, t('settingsContactsEditRepository')),
            repo.kind === 'carddav' ? h('button', { type: 'button', onClick: function () { sync(repo, 'pull'); } }, t('settingsContactsCarddavPull')) : null,
            repo.kind === 'carddav' ? h('button', { type: 'button', onClick: function () { sync(repo, 'push'); } }, t('settingsContactsCarddavPush')) : null,
            h('button', { type: 'button', class: 'danger', onClick: function () { deleteRepository(repo); } }, t('settingsContactsDeleteRepository'))));
      }),
      h('button', { type: 'button', onClick: function () { repositoryDialog(null); } }, '+ ' + t('settingsContactsAddCarddavRepo')),

      h('h3', { text: t('settingsContactsGroups') }),
      h('p', { class: 'hint', text: t('settingsContactsGroupShareHint') }),
      c.groups.map(groupBlock),
      h('div', { class: 'row-actions' },
        h('button', { type: 'button', onClick: newGroup }, '+ ' + t('settingsContactsNewGroup')),
        h('button', { type: 'button', onClick: function () {
          call('contacts.applyRules', undefined, then(function (r) { return t('webRulesApplied', { count: r.materialized }); }));
        } }, t('settingsContactsApplyRules'))),

      h('h3', { text: t('settingsContactsLocalContacts') }),
      filterInput,
      matching.length === 0 ? h('p', { class: 'hint', text: t('webNoContacts') }) : matching.slice(0, shown).map(contactBlock),
      matching.length > shown ? h('button', { type: 'button', onClick: function () { shown += PAGE; render(); } }, t('webShowMore')) : null,

      h('div', { class: 'row-actions bottom' },
        h('button', { type: 'button', onClick: importVcard }, t('settingsContactsImportVcard')),
        h('button', { type: 'button', onClick: exportVcard }, t('settingsContactsExportVcard')))
    ]);
  }

  return {
    root: root,
    refresh: function () {
      if (!started) { started = true; load(); }
      // Typing in the filter re-renders the page itself, so only changes from
      // elsewhere (the address book, a contact's targets) come through here.
      if (!root.contains(document.activeElement) || document.activeElement.tagName !== 'INPUT') { render(); }
    }
  };
}
