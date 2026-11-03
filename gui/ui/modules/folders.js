// The folder pane for the selected account.

import { state } from './store.js';
import { openFolder } from './messages.js';
import { h } from './dom.js';
import { t } from './i18n.js';

// What a folder is called in the list: its display name, else the last part of its path.
export function folderLabel(acc, path) {
  const d = acc.delimiter;
  const parts = d ? path.split(d) : [path];
  return acc.names[path] || parts[parts.length - 1];
}

function rowsFor(acc) {
  const d = acc.delimiter;
  const paths = acc.folders.slice().sort(function (a, b) {
    const ai = a.toUpperCase() === 'INBOX';
    const bi = b.toUpperCase() === 'INBOX';
    if (ai !== bi) { return ai ? -1 : 1; }
    return a.localeCompare(b);
  });
  return paths.map(function (path) {
    const parts = d ? path.split(d) : [path];
    return {
      path: path,
      depth: parts.length - 1,
      label: folderLabel(acc, path),
      unread: acc.unread[path] || 0
    };
  });
}

export function foldersContent() {
  const acc = state.accounts[state.selectedAccount];
  if (!acc) { return [h('p', { class: 'empty', text: t('noAccountsYet') })]; }
  const out = [h('h2', { class: 'pane-title', text: acc.label || acc.id })];
  if (acc.folderError) { out.push(h('p', { class: 'notice', text: acc.folderError })); }
  const rows = rowsFor(acc);
  if (rows.length === 0) { out.push(h('p', { class: 'empty', text: t('webNoFolders') })); }
  for (const r of rows) {
    out.push(h('button', {
      class: 'folder-row',
      depth: r.depth,
      'data-folder': r.path,
      'aria-current': state.selectedFolder === r.path ? 'true' : null,
      onClick: function () { openFolder(acc.id, r.path); }
    },
    h('span', { class: 'name', text: r.label }),
    r.unread > 0 ? h('span', { class: 'badge', text: String(r.unread) }) : null));
  }
  return out;
}
