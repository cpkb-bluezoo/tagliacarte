// Modal dialogs on the platform's <dialog> element: focus is trapped, Escape
// cancels, and nothing behind it can be clicked. Callback style throughout.

import { h } from './dom.js';
import { t } from './i18n.js';

// o: { title, subtitle, content, actions: [{ label, primary, danger, run(ctl) }] }
// ctl: { setError(msg), setBusy(bool), close(result) }. The dialog stays open
// until an action closes it, so an action can do work and report a failure.
// content is a node, or a function of ctl that returns one (for choices that
// close the dialog themselves).
export function openModal(o, onClose) {
  const error = h('p', { class: 'error', role: 'alert' });
  const buttons = h('div', { class: 'buttons' });
  const slot = h('div', { class: 'modal-slot' });
  const form = h('form', { class: 'modal-form', method: 'dialog' },
    h('h2', { text: o.title }),
    o.subtitle ? h('p', { class: 'dim', text: o.subtitle }) : null,
    o.note ? h('p', { class: 'hint reason', text: o.note }) : null,
    slot,
    error,
    buttons);
  const dlg = h('dialog', { class: 'modal' }, form);
  document.body.append(dlg);

  let closed = false;
  const pending = [];
  const ctl = {
    setError: function (msg) { error.textContent = msg || ''; },
    setBusy: function (busy) { for (const b of pending) { b.disabled = busy; } },
    close: function (result) {
      if (closed) { return; }
      closed = true;
      dlg.close();
      dlg.remove();
      if (onClose) { onClose(result); }
    }
  };

  const content = typeof o.content === 'function' ? o.content(ctl) : o.content;
  if (content) { slot.append(content); }

  for (const a of o.actions) {
    const b = h('button', {
      type: a.primary ? 'submit' : 'button',
      class: (a.primary ? 'primary ' : '') + (a.danger ? 'danger' : '')
    }, a.label);
    pending.push(b);
    b.addEventListener('click', function (e) {
      e.preventDefault();
      ctl.setError('');
      a.run(ctl);
    });
    buttons.append(b);
  }
  // Enter in a field runs the primary action; Escape runs the first (cancel) one.
  form.addEventListener('submit', function (e) {
    e.preventDefault();
    const primary = o.actions.find(function (a) { return a.primary; });
    if (primary) { ctl.setError(''); primary.run(ctl); }
  });
  dlg.addEventListener('cancel', function (e) {
    e.preventDefault();
    ctl.close(false);
  });
  dlg.showModal();
  const first = dlg.querySelector('input, textarea, select');
  if (first) { first.focus(); }
  return ctl;
}

export function confirmModal(o, cb) {
  openModal({
    title: o.title,
    content: o.body ? h('p', { text: o.body }) : null,
    actions: [
      { label: o.cancel || t('cancel'), run: function (c) { c.close(false); } },
      { label: o.ok, primary: true, danger: !!o.danger, run: function (c) { c.close(true); } }
    ]
  }, cb);
}

// Fields: [{ key, label, type, value, placeholder, options }]. type is "text"
// (default), "password", "checkbox" (value is a boolean) or "select" (options:
// [{ value, label }]). submit(values, ctl) must call ctl.close(true) on success
// or ctl.setError(msg) to stay open.
export function formModal(o, cb) {
  const inputs = {};
  const wrappers = {};
  const content = h('div', { class: 'modal-fields' }, o.fields.map(function (f) {
    let input;
    if (f.type === 'checkbox') {
      input = h('input', { type: 'checkbox', 'data-field': f.key });
      input.checked = !!f.value;
      inputs[f.key] = input;
      return (wrappers[f.key] = h('label', { class: 'switch' }, input, h('span', { class: 'label', text: f.label })));
    }
    if (f.type === 'select') {
      input = h('select', { 'data-field': f.key }, f.options.map(function (x) { return h('option', { value: x.value, text: x.label }); }));
      if (f.value != null) { input.value = f.value; }
    } else {
      input = h('input', {
        type: f.type || 'text',
        'data-field': f.key,
        value: f.value || '',
        placeholder: f.placeholder || '',
        autocomplete: 'off',
        spellcheck: 'false'
      });
    }
    inputs[f.key] = input;
    return (wrappers[f.key] = h('label', { class: 'stack' }, h('span', { class: 'label', text: f.label }), input));
  }));
  // A field with `showWhen: { key, is: [values] }` is shown only while the field named
  // by `key` has one of those values; a hidden field is still part of the form.
  const refreshShown = function () {
    o.fields.forEach(function (f) {
      if (!f.showWhen) { return; }
      const other = inputs[f.showWhen.key];
      wrappers[f.key].hidden = !(other && f.showWhen.is.indexOf(other.value) >= 0);
    });
  };
  o.fields.forEach(function (f) {
    if (f.showWhen && inputs[f.showWhen.key]) { inputs[f.showWhen.key].addEventListener('change', refreshShown); }
  });
  refreshShown();
  openModal({
    title: o.title,
    subtitle: o.subtitle,
    note: o.note,
    content: content,
    actions: [
      { label: t('cancel'), run: function (c) { c.close(false); } },
      {
        label: o.ok,
        primary: true,
        run: function (c) {
          const values = {};
          for (const k of Object.keys(inputs)) {
            values[k] = inputs[k].type === 'checkbox' ? inputs[k].checked : inputs[k].value;
          }
          c.setBusy(true);
          o.submit(values, {
            setError: function (m) { c.setBusy(false); c.setError(m); },
            close: c.close
          });
        }
      }
    ]
  }, cb);
}

// A dialog whose only job is to start something slow, such as a browser
// sign-in. work(ctl) reports success with ctl.close(true) or failure with
// ctl.setError(msg).
export function actionModal(o, cb) {
  openModal({
    title: o.title,
    subtitle: o.subtitle,
    note: o.note,
    content: o.body ? h('p', { text: o.body }) : null,
    actions: [
      { label: t('cancel'), run: function (c) { c.close(false); } },
      {
        label: o.ok,
        primary: true,
        run: function (c) {
          c.setBusy(true);
          o.work({
            setError: function (m) { c.setBusy(false); c.setError(m); },
            close: c.close
          });
        }
      }
    ]
  }, cb);
}
