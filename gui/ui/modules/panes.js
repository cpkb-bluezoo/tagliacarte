// The widths of the folder and message panes, which the user can change by dragging
// the handle on the border of each (or with the arrow keys when it has the focus; a
// double click puts the pane back). The rail is a fixed width and has no handle, and
// the reader takes what is left. The widths are this viewer's own and are kept in
// local storage; a window too small for them shows them reduced, without forgetting
// them.

import { h } from './dom.js';
import { t } from './i18n.js';
import { getLocal, setLocal } from './localprefs.js';

const DEFAULTS = { folders: 260, messages: 380 };
const MIN = { folders: 140, messages: 240 };
const READER_MIN = 320;
const STEP = 16;
const KEY = 'paneWidths';
const PANES = ['folders', 'messages'];

const handles = {};
let shell = null;
let rail = null;

function usable(v, fallback) {
  return typeof v === 'number' && isFinite(v) && v > 0 ? v : fallback;
}

const saved = getLocal(KEY, DEFAULTS) || DEFAULTS;
const wanted = { folders: usable(saved.folders, DEFAULTS.folders), messages: usable(saved.messages, DEFAULTS.messages) };

function clamp(v, lo, hi) {
  return Math.min(Math.max(v, lo), Math.max(lo, hi));
}

// What is shown: the wanted widths, limited by what the pane may be and by the window.
function shown() {
  const room = shell.clientWidth - rail.getBoundingClientRect().width;
  const folders = clamp(wanted.folders, MIN.folders, room - MIN.messages - READER_MIN);
  const messages = clamp(wanted.messages, MIN.messages, room - folders - READER_MIN);
  return { folders: Math.round(folders), messages: Math.round(messages) };
}

function layout() {
  const w = shown();
  shell.style.setProperty('--folders-w', w.folders + 'px');
  shell.style.setProperty('--messages-w', w.messages + 'px');
  PANES.forEach(function (pane) {
    handles[pane].setAttribute('aria-valuenow', String(w[pane]));
  });
}

// What the user sees becomes what is wanted, and is kept.
function settle() {
  const w = shown();
  wanted.folders = w.folders;
  wanted.messages = w.messages;
  setLocal(KEY, w);
  layout();
}

function handle(pane) {
  let startX = 0;
  let start = 0;
  let dragging = false;
  const el = h('div', {
    class: 'pane-handle pane-handle-' + pane,
    role: 'separator',
    'aria-orientation': 'vertical',
    'aria-valuemin': String(MIN[pane]),
    tabindex: '0',
    'data-pane': pane
  });
  el.addEventListener('pointerdown', function (e) {
    if (e.button !== 0) { return; }
    dragging = true;
    startX = e.clientX;
    start = shown()[pane];
    shell.classList.add('resizing');
    el.classList.add('active');
    try { el.setPointerCapture(e.pointerId); } catch (err) { /* the events still reach it while the pointer is over it */ }
    e.preventDefault();
  });
  el.addEventListener('pointermove', function (e) {
    if (!dragging) { return; }
    wanted[pane] = start + (e.clientX - startX);
    layout();
  });
  const end = function () {
    if (!dragging) { return; }
    dragging = false;
    shell.classList.remove('resizing');
    el.classList.remove('active');
    settle();
  };
  el.addEventListener('pointerup', end);
  el.addEventListener('pointercancel', end);
  el.addEventListener('keydown', function (e) {
    const step = e.shiftKey ? STEP * 4 : STEP;
    if (e.key === 'ArrowLeft') { wanted[pane] = shown()[pane] - step; } else if (e.key === 'ArrowRight') { wanted[pane] = shown()[pane] + step; } else if (e.key === 'Home') { wanted[pane] = DEFAULTS[pane]; } else { return; }
    e.preventDefault();
    settle();
  });
  el.addEventListener('dblclick', function () {
    wanted[pane] = DEFAULTS[pane];
    settle();
  });
  handles[pane] = el;
  return el;
}

// Puts the handles in the shell. `railElement` is the fixed-width pane at its left.
export function installPanes(shellElement, railElement) {
  shell = shellElement;
  rail = railElement;
  shell.append(handle('folders'), handle('messages'));
  layout();
  window.addEventListener('resize', layout);
}

// The words on the handles come with the language, which may arrive or change.
export function updatePanes() {
  if (!shell) { return; }
  handles.folders.setAttribute('aria-label', t('webResizeFolders'));
  handles.folders.title = t('webResizeFolders');
  handles.messages.setAttribute('aria-label', t('webResizeMessages'));
  handles.messages.title = t('webResizeMessages');
}
