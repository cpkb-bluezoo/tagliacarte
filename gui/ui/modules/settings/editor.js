// A form that edits a copy (the draft) and saves it with an explicit button,
// as opposed to the preference controls in kit.js that save on every change.
// The editor knows whether the draft differs from what was last saved, so
// leaving with unsaved edits can ask first.

import { h } from '../dom.js';
import { t } from '../i18n.js';

export function makeEditor(initial) {
  const draft = JSON.parse(JSON.stringify(initial));
  let clean = JSON.stringify(draft);
  const root = h('div', { class: 'settings-page editor' });
  const rows = [];
  const listeners = [];

  function refresh() {
    for (const r of rows) { if (r.visible) { r.el.hidden = !r.visible(draft); } }
    for (const fn of listeners) { fn(draft); }
  }

  function add(el, visible) {
    rows.push({ el: el, visible: visible });
    root.append(el);
    if (visible) { el.hidden = !visible(draft); }
    return el;
  }

  function label(o) {
    return h('span', { class: 'text' },
      h('span', { class: 'label', text: o.label }),
      o.hint ? h('span', { class: 'hint', text: o.hint }) : null);
  }

  function input(key, o, type) {
    const el = h(type === 'select' ? 'select' : 'input', {
      type: type === 'select' ? null : type,
      placeholder: o.placeholder || null,
      'data-field': key,
      autocomplete: 'off',
      spellcheck: 'false'
    });
    if (type === 'select') {
      for (const opt of o.options) { el.append(h('option', { value: opt.value, text: opt.label })); }
    }
    el.value = draft[key] == null ? '' : draft[key];
    el.addEventListener('input', function () { draft[key] = el.value; refresh(); });
    el.addEventListener('change', function () { draft[key] = el.value; refresh(); });
    return el;
  }

  const editor = {
    draft: draft,
    root: root,
    refresh: refresh,
    onChange: function (fn) { listeners.push(fn); },
    isDirty: function () { return JSON.stringify(draft) !== clean; },
    markClean: function () { clean = JSON.stringify(draft); refresh(); },
    heading: function (text, visible) { return add(h('h3', { class: 'editor-heading', text: text }), visible); },
    note: function (text, visible) { return add(h('p', { class: 'hint', text: text }), visible); },
    node: function (el, visible) { return add(el, visible); },
    readonly: function (o) {
      return add(h('div', { class: 'field' }, h('span', { class: 'label', text: o.label }),
        h('div', { class: 'readonly', text: o.value }),
        o.hint ? h('span', { class: 'hint', text: o.hint }) : null), o.visible);
    },
    text: function (key, o) {
      return add(h('div', { class: 'field' },
        h('label', { class: 'stack' }, label(o), input(key, o, 'text'))), o.visible);
    },
    number: function (key, o) {
      return add(h('div', { class: 'field' },
        h('label', { class: 'stack' }, label(o), input(key, o, 'text'))), o.visible);
    },
    // A text field with a Browse button; o.browse(cb) asks for a path and calls cb(path|null).
    path: function (key, o) {
      const el = input(key, o, 'text');
      const browse = h('button', {
        type: 'button',
        onClick: function () {
          o.browse(function (p) {
            if (p) { el.value = p; draft[key] = p; refresh(); }
          });
        }
      }, t('webBrowse'));
      return add(h('div', { class: 'field' },
        h('label', { class: 'stack' }, label(o), h('span', { class: 'row' }, el, browse))), o.visible);
    },
    select: function (key, o) {
      return add(h('div', { class: 'field' },
        h('label', { class: 'stack' }, label(o), input(key, o, 'select'))), o.visible);
    }
  };
  return editor;
}

// The Save / Back bar at the top of an editor page.
export function editorBar(o) {
  const save = h('button', { type: 'button', class: 'primary', onClick: o.onSave }, t('save'));
  const back = h('button', { type: 'button', onClick: o.onBack }, t('back'));
  const bar = h('div', { class: 'editor-bar' }, back, h('h3', { class: 'editor-title', text: o.title }), save);
  return { el: bar, save: save, setBusy: function (b) { save.disabled = b; } };
}
