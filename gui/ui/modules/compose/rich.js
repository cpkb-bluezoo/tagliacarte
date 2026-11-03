// The rich-text body: a contenteditable region with a small toolbar.
//
// The region is ordinary DOM that this page owns, so a quoted original keeps
// the block that marks it (see reply.js) from seed to send, which an editor
// that models its own document cannot promise. Formatting uses the browser's
// editing commands; everything that leaves the editor, by paste or by send,
// goes through sanitiseRich.

import { h } from '../dom.js';
import { t } from '../i18n.js';
import { sanitiseRich, htmlToPlain, plainFromRich, containsQuotedBlock } from './reply.js';

const BUTTONS = [
  ['webFmtUndo', 'undo', '↶'],
  ['webFmtRedo', 'redo', '↷'],
  ['webFmtBold', 'bold', 'B'],
  ['webFmtItalic', 'italic', 'I'],
  ['webFmtCode', 'code', '</>'],
  ['webFmtClear', 'removeFormat', 'Tx'],
  ['webFmtCodeBlock', 'pre', '{ }'],
  ['webFmtQuote', 'blockquote', '“'],
  ['webFmtDivider', 'insertHorizontalRule', '—']
];

function escapeHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// opts: { onChange(), onFiles(fileList) } for edits, and for images pasted in.
export function makeRichEditor(opts) {
  const area = h('div', {
    class: 'rich-area',
    contenteditable: 'true',
    role: 'textbox',
    'aria-multiline': 'true',
    'aria-label': t('fieldBody'),
    spellcheck: 'true'
  });

  function run(cmd) {
    area.focus();
    if (cmd === 'code') {
      const sel = window.getSelection();
      const text = sel ? sel.toString() : '';
      document.execCommand('insertHTML', false, '<code>' + escapeHtml(text || '​') + '</code>');
    } else if (cmd === 'pre' || cmd === 'blockquote') {
      document.execCommand('formatBlock', false, cmd);
    } else {
      document.execCommand(cmd, false, null);
    }
    opts.onChange();
  }

  const bar = h('div', { class: 'rich-bar', role: 'toolbar' }, BUTTONS.map(function (b) {
    return h('button', {
      type: 'button',
      title: t(b[0]),
      'aria-label': t(b[0]),
      'data-cmd': b[1],
      // Keep the selection while a toolbar button is pressed.
      onMousedown: function (e) { e.preventDefault(); },
      onClick: function () { run(b[1]); }
    }, b[2]);
  }));

  area.addEventListener('input', function () { opts.onChange(); });
  area.addEventListener('paste', function (e) {
    const cd = e.clipboardData;
    if (!cd) { return; }
    if (cd.files && cd.files.length) {
      e.preventDefault();
      opts.onFiles(cd.files);
      return;
    }
    const html = cd.getData('text/html');
    e.preventDefault();
    if (html) {
      document.execCommand('insertHTML', false, sanitiseRich(html));
    } else {
      document.execCommand('insertText', false, cd.getData('text/plain'));
    }
    opts.onChange();
  });

  return {
    el: h('div', { class: 'rich' }, bar, area),
    area: area,
    focus: function () { area.focus(); },
    html: function () { return sanitiseRich(area.innerHTML); },
    setHtml: function (html, caretAtEnd) {
      area.innerHTML = sanitiseRich(html);
      area.focus();
      const range = document.createRange();
      range.selectNodeContents(area);
      range.collapse(!caretAtEnd);
      const sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
    },
    isEmpty: function () { return htmlToPlain(area.innerHTML) === ''; },
    // The text/plain form: the quote with its header and a prefix on each line when
    // there is a quoted original and the setting asks for one, plain text otherwise.
    plain: function (cfg) {
      const html = sanitiseRich(area.innerHTML);
      return cfg.quoteOriginal && containsQuotedBlock(html) ? plainFromRich(html, cfg) : htmlToPlain(html);
    }
  };
}
