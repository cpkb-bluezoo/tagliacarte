// A recipient field: comma-separated addresses in one text box, with the best
// completion for what is being typed shown as grey text after it. Tab takes it.

import { h } from '../dom.js';
import { call } from '../wire.js';
import { splitAddressList } from './reply.js';

const DEBOUNCE_MS = 150;

// The part being typed: what follows the last comma.
function activeSegment(value) {
  const i = value.lastIndexOf(',');
  return (i < 0 ? value : value.slice(i + 1)).trim();
}

// opts: { label, name, onChange() }
export function recipientField(opts) {
  const input = h('input', { type: 'text', 'data-field': opts.name, autocomplete: 'off', spellcheck: 'false', 'aria-label': opts.label });
  const typed = h('span', { class: 'typed' });
  const hint = h('span', { class: 'hint-suffix' });
  const ghost = h('div', { class: 'ghost', 'aria-hidden': 'true' }, typed, hint);
  let timer = null;
  let suggestion = null;       // { full, ghost } for the segment as it was typed
  let askedFor = '';

  function clear() {
    suggestion = null;
    typed.textContent = '';
    hint.textContent = '';
  }

  function show() {
    if (!suggestion) { clear(); return; }
    typed.textContent = input.value;
    hint.textContent = suggestion.ghost;
  }

  function fetchCompletion() {
    const seg = activeSegment(input.value);
    askedFor = seg;
    if (seg === '') { clear(); return; }
    call('contacts.complete', { prefix: seg }, function (err, r) {
      // A slow answer for text that has since changed is stale.
      if (activeSegment(input.value) !== askedFor) { return; }
      if (err || !r || !r.fullAddress) { clear(); return; }
      const sameAsTyped = r.fullAddress.toLowerCase() === seg.toLowerCase();
      if (!r.ghostSuffix && sameAsTyped) { clear(); return; }
      suggestion = { full: r.fullAddress, ghost: r.ghostSuffix || '' };
      show();
    });
  }

  input.addEventListener('input', function () {
    clear();
    clearTimeout(timer);
    timer = setTimeout(fetchCompletion, DEBOUNCE_MS);
    opts.onChange();
  });
  input.addEventListener('keydown', function (e) {
    if (e.key !== 'Tab' || e.shiftKey || !suggestion) { return; }
    e.preventDefault();
    const i = input.value.lastIndexOf(',');
    input.value = (i < 0 ? '' : input.value.slice(0, i + 1).trimEnd() + ' ') + suggestion.full;
    clear();
    opts.onChange();
  });
  input.addEventListener('blur', clear);

  return {
    el: h('label', { class: 'recipient' }, h('span', { class: 'label', text: opts.label }),
      h('span', { class: 'ghost-wrap' }, ghost, input)),
    input: input,
    // The addresses, split on commas that are not inside a name or brackets.
    list: function () { return splitAddressList(input.value); },
    set: function (addresses) { input.value = addresses.join(', '); },
    value: function () { return input.value; }
  };
}
