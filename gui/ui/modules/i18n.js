// Strings come from the ARB files the other front ends use, served at
// /locales/<lang>.arb. The user's language is loaded over English, so a key
// missing from a translation falls back to English rather than showing its
// name. Messages use {name} placeholders only.

const SUPPORTED = ['de', 'el', 'en', 'es', 'fr', 'it', 'ja', 'pt', 'ru', 'zh'];

let strings = {};
let fallback = {};

export function pickLanguage(tags) {
  for (const tag of tags) {
    const base = String(tag).toLowerCase().split('-')[0];
    if (SUPPORTED.includes(base)) { return base; }
  }
  return 'en';
}

function fetchArb(lang, cb) {
  fetch('/locales/' + lang + '.arb').then(function (r) {
    if (!r.ok) { throw new Error('HTTP ' + r.status); }
    return r.json();
  }).then(function (data) { cb(null, data); }, function (e) { cb(e, null); });
}

// Loads English, then the user's language over it, then calls cb().
export function loadLocale(cb) {
  const tags = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || 'en'];
  const lang = pickLanguage(tags);
  document.documentElement.lang = lang;
  fetchArb('en', function (err, en) {
    fallback = en || {};
    strings = fallback;
    if (lang === 'en') { cb(); return; }
    fetchArb(lang, function (err2, data) {
      if (data) { strings = data; }
      cb();
    });
  });
}

export function t(key, params) {
  let s = strings[key];
  if (typeof s !== 'string') { s = fallback[key]; }
  if (typeof s !== 'string') { return key; }
  if (params) {
    s = s.replace(/\{(\w+)\}/g, function (m, name) { return name in params ? String(params[name]) : m; });
  }
  return s;
}
