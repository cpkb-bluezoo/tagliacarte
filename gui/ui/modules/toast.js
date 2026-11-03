// A short message that fades away; the page equivalent of a snackbar.

import { h } from './dom.js';

let box = null;

export function toast(message) {
  if (!box) {
    box = h('div', { class: 'toasts', role: 'status', 'aria-live': 'polite' });
    document.body.append(box);
  }
  const el = h('div', { class: 'toast', text: message });
  box.append(el);
  setTimeout(function () { el.remove(); }, 5000);
}
