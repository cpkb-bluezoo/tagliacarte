// Form controls for the settings pages.
//
// A page is built once and kept, because a form that is rebuilt while the
// user types loses its focus and its half-typed text. Each control registers
// how to show a value; when the configuration changes the page calls
// refresh(), and a control that has focus is left alone.
//
// Preference controls save one setting as soon as it changes
// (settings.set). If the back end refuses, the message is shown under the
// control and the control goes back to the saved value.

import { state } from '../store.js';
import { call } from '../wire.js';
import { h, setChildren } from '../dom.js';
import { t } from '../i18n.js';
import { pickPath } from '../dialogs.js';
import { getLocal, setLocal } from '../localprefs.js';

export function makePage() {
  const bindings = [];
  const root = h('div', { class: 'settings-page' });
  return {
    root: root,
    add: function (...nodes) { root.append(...nodes.flat().filter(Boolean)); },
    bind: function (fn) { bindings.push(fn); },
    refresh: function () {
      const cfg = state.config;
      if (!cfg) { return; }
      for (const fn of bindings) { fn(cfg); }
    }
  };
}

export function section(titleKey, noteKey) {
  return h('div', { class: 'section' },
    h('h3', { text: t(titleKey) }),
    noteKey ? h('p', { class: 'hint', text: t(noteKey) }) : null);
}

function label(labelKey, hintKey) {
  return h('span', { class: 'text' },
    h('span', { class: 'label', text: t(labelKey) }),
    hintKey ? h('span', { class: 'hint', text: t(hintKey) }) : null);
}

// Saves one preference. `onError` receives the message when it is refused.
function save(page, key, value, errorEl) {
  errorEl.textContent = '';
  call('settings.set', { key: key, value: value }, function (err) {
    if (err) {
      errorEl.textContent = err.message;
      page.refresh();
    }
  });
}

export function switchField(page, o) {
  const input = h('input', { type: 'checkbox', 'data-setting': o.key });
  const error = h('p', { class: 'error' });
  input.addEventListener('change', function () { save(page, o.key, input.checked, error); });
  page.bind(function (cfg) {
    if (document.activeElement !== input) { input.checked = !!cfg[o.key]; }
  });
  return h('div', { class: 'field' }, h('label', { class: 'switch' }, input, label(o.label, o.hint)), error);
}

export function textField(page, o) {
  const input = o.multiline
    ? h('textarea', { rows: '2', 'data-setting': o.key })
    : h('input', { type: o.password ? 'password' : 'text', 'data-setting': o.key, autocomplete: 'off' });
  const error = h('p', { class: 'error' });
  input.addEventListener('change', function () { save(page, o.key, input.value, error); });
  if (o.onInput) { input.addEventListener('input', function () { o.onInput(input.value); }); }
  page.bind(function (cfg) {
    if (o.password) {
      // A secret is never sent back; only whether one is set.
      input.placeholder = cfg[o.key + 'Set'] || cfg.mailCryptoPgpHasPassphrase ? '••••••' : '';
      return;
    }
    if (document.activeElement !== input) {
      input.value = cfg[o.key] == null ? '' : cfg[o.key];
      if (o.onInput) { o.onInput(input.value); }
    }
  });
  return h('div', { class: 'field' },
    h('label', { class: 'stack' }, label(o.label, o.hint), input), error);
}

// A text field with a Browse button that opens a native dialog.
export function pathField(page, o) {
  const input = h('input', { type: 'text', 'data-setting': o.key, autocomplete: 'off' });
  const error = h('p', { class: 'error' });
  input.addEventListener('change', function () { save(page, o.key, input.value, error); });
  const browse = h('button', {
    type: 'button',
    onClick: function () {
      pickPath({ folder: !!o.folder, title: t(o.label) }, function (path) {
        if (path) {
          input.value = path;
          save(page, o.key, path, error);
        }
      });
    }
  }, t('webBrowse'));
  page.bind(function (cfg) {
    if (document.activeElement !== input) { input.value = cfg[o.key] == null ? '' : cfg[o.key]; }
  });
  return h('div', { class: 'field' },
    h('label', { class: 'stack' }, label(o.label, o.hint), h('span', { class: 'row' }, input, browse)), error);
}

// options: [{ value, label }] where label is already display text.
export function selectField(page, o) {
  const select = h('select', { 'data-setting': o.key });
  const error = h('p', { class: 'error' });
  const fill = function (extra) {
    const opts = o.options.slice();
    if (extra !== undefined && !opts.some(function (x) { return x.value === extra; })) {
      opts.push({ value: extra, label: extra });
    }
    setChildren(select, opts.map(function (x) { return h('option', { value: x.value, text: x.label }); }));
  };
  fill();
  select.addEventListener('change', function () { save(page, o.key, select.value, error); });
  page.bind(function (cfg) {
    if (document.activeElement === select) { return; }
    const v = cfg[o.key] == null ? '' : cfg[o.key];
    fill(v);
    select.value = v;
  });
  return h('div', { class: 'field' },
    h('label', { class: 'stack' }, label(o.label, o.hint), select), error);
}

// Two or three exclusive choices shown side by side.
export function segmentedField(page, o) {
  const error = h('p', { class: 'error' });
  const buttons = o.options.map(function (opt) {
    return h('button', {
      type: 'button',
      'data-value': opt.value,
      onClick: function () { save(page, o.key, opt.value, error); }
    }, opt.label);
  });
  page.bind(function (cfg) {
    const cur = String(cfg[o.key] || '').toLowerCase();
    const norm = o.normalise ? o.normalise(cur) : cur;
    for (const b of buttons) { b.setAttribute('aria-pressed', b.dataset.value === norm ? 'true' : 'false'); }
  });
  return h('div', { class: 'field' },
    o.label ? h('span', { class: 'label', text: t(o.label) }) : null,
    h('span', { class: 'segmented' }, buttons), error);
}

// A switch for a preference that belongs to this viewer only (see localprefs.js).
export function localSwitchField(o) {
  const input = h('input', { type: 'checkbox', 'data-local': o.key });
  input.checked = !!getLocal(o.key, o.fallback);
  input.addEventListener('change', function () {
    setLocal(o.key, input.checked);
    if (o.onChange) { o.onChange(input.checked); }
  });
  return h('div', { class: 'field' }, h('label', { class: 'switch' }, input, label(o.label, o.hint)));
}
