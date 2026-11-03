// The picture that stands for an account: a coloured disc with initials, or the
// account's own image when it has one. The same in the rail and in Settings.
//
// The image is the back end's to serve (`/avatar/<account>`): the page names the
// account, never the address, and a failed image leaves the disc showing. The
// `v` in the URL only changes when the saved picture does, so the ordinary HTTP
// cache keeps the image between renders without ever showing a stale one.

import { h } from './dom.js';

// The same colour for the same account every time.
export function hueOf(id) {
  let hue = 0;
  for (const ch of String(id)) { hue = (hue * 31 + ch.charCodeAt(0)) % 360; }
  return hue;
}

// The first user-perceived character: an emoji or an accented letter is one.
function firstGrapheme(s) {
  if (!s) { return ''; }
  if (typeof Intl !== 'undefined' && typeof Intl.Segmenter === 'function') {
    for (const part of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(s)) { return part.segment; }
  }
  return Array.from(s)[0] || '';
}

// One word gives its first letter; several give the first and the last word's.
// Without a name, the address does; without either, a question mark.
export function initials(label, email) {
  const words = String(label || '').trim().split(/\s+/).filter(Boolean);
  let out = '';
  if (words.length === 1) { out = firstGrapheme(words[0]); }
  else if (words.length > 1) { out = firstGrapheme(words[0]) + firstGrapheme(words[words.length - 1]); }
  else { out = firstGrapheme(String(email || '').trim()); }
  return out ? out.toLocaleUpperCase() : '?';
}

function digest(s) {
  let x = 5381;
  for (let i = 0; i < s.length; i++) { x = ((x * 33) ^ s.charCodeAt(i)) >>> 0; }
  return x.toString(36);
}

// A Matrix account's picture is in its profile, so it is asked for without having
// been chosen; for any other account there is one only once it is given.
function hasPicture(account) {
  return !!account.avatarUrl || String(account.backendType || '').toLowerCase() === 'matrix';
}

export function avatarSrc(account) {
  return '/avatar/' + encodeURIComponent(account.id) + '?v=' + digest(account.avatarUrl || '');
}

// Elements are kept, per place they are shown in, until what they show changes:
// the lists are redrawn on every change, and a new <img> each time would blink.
const kept = new Map();

// `place` is where it is shown ('rail', 'list'), `size` its width in pixels.
export function accountAvatar(account, place, size) {
  const email = (account.attrs || {}).email;
  const sig = [account.id, account.label, email, account.avatarUrl, size].join('\n');
  const key = place + '\n' + account.id;
  const hit = kept.get(key);
  if (hit && hit.sig === sig) { return hit.el; }

  const el = h('span', { class: 'avatar', 'aria-hidden': 'true' }, initials(account.label || account.id, email));
  el.style.setProperty('--hue', String(hueOf(account.id)));
  el.style.setProperty('--size', size + 'px');
  if (hasPicture(account)) {
    const img = h('img', { class: 'avatar-img', src: avatarSrc(account), alt: '', draggable: 'false' });
    img.addEventListener('error', function () { img.remove(); });
    el.append(img);
  }
  kept.set(key, { sig: sig, el: el });
  return el;
}
