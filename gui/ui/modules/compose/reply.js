// What replying to or forwarding a message means, as pure functions of plain
// data: the subject, who it goes to, the threading headers, and the quoted
// original in plain text and in rich text. Nothing here touches the page, so
// the self-test can check all of it.
//
// A source message is { from: [{name,email}], to, cc, subject, dateMs,
// messageId, references, bodyPlain, bodyHtml } as the back end reports it.

import { formatReplyDate, formatReplyTime, expandReplyHeader } from '../replyformat.js';

export const QUOTED_CLASS = 'quoted-message-reply';
export const MAX_QUOTED_LINE = 80;

// ---- subjects ---------------------------------------------------------------

export function replySubject(subject) {
  const t = String(subject || '').trim();
  if (!t) { return ''; }
  return /^(re:\s*)+/i.test(t) ? t : 'Re: ' + t;
}

export function forwardSubject(subject) {
  const t = String(subject || '').trim();
  if (!t) { return ''; }
  return /^(fwd:\s*)+/i.test(t) ? t : 'Fwd: ' + t;
}

// ---- addresses --------------------------------------------------------------

// A name that needs quoting to survive a header: anything with a comma, a
// bracket, a quote or a semicolon.
function displayName(name) {
  const n = String(name || '').trim();
  if (!n) { return ''; }
  return /[,;<>"()\[\]:@\\]/.test(n) ? '"' + n.replace(/\\/g, '\\\\').replace(/"/g, '\\"') + '"' : n;
}

export function formatAddress(a) {
  const n = displayName(a.name);
  return n ? n + ' <' + a.email + '>' : a.email;
}

// Splits an address list on commas that are outside quotes and angle brackets,
// so `"Doe, John" <j@x.org>, b@x.org` is two addresses, not three.
export function splitAddressList(raw) {
  const out = [];
  let cur = '';
  let quoted = false;
  let angle = 0;
  const s = String(raw || '');
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (quoted) {
      cur += c;
      if (c === '\\' && i + 1 < s.length) { cur += s[++i]; } else if (c === '"') { quoted = false; }
    } else if (c === '"') {
      quoted = true;
      cur += c;
    } else if (c === '<') {
      angle++;
      cur += c;
    } else if (c === '>') {
      angle = Math.max(0, angle - 1);
      cur += c;
    } else if (c === ',' && angle === 0) {
      if (cur.trim()) { out.push(cur.trim()); }
      cur = '';
    } else {
      cur += c;
    }
  }
  if (cur.trim()) { out.push(cur.trim()); }
  return out;
}

// The address in `Name <addr>` or a bare address; '' if there is none.
export function extractEmail(s) {
  const text = String(s || '');
  const angle = /<([^<>]*)>/.exec(text);
  if (angle && angle[1].trim()) { return angle[1].trim(); }
  const bare = /\b[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}\b/.exec(text);
  return bare ? bare[0] : '';
}

// Who a reply goes to. A reply goes to the sender; a reply to all also goes to
// everyone else who was addressed, except the user's own addresses (`self`,
// lower-case) and the sender, each address once. Forwarding goes to nobody.
export function replyRecipients(kind, source, self) {
  if (kind === 'forward') { return { to: [], cc: [] }; }
  const to = (source.from || []).map(formatAddress);
  if (kind !== 'replyAll') { return { to, cc: [] }; }
  const skip = new Set(self);
  for (const a of source.from || []) { skip.add(String(a.email).toLowerCase()); }
  const cc = [];
  for (const a of [].concat(source.to || [], source.cc || [])) {
    const key = String(a.email).toLowerCase();
    if (!key || skip.has(key)) { continue; }
    skip.add(key);
    cc.push(formatAddress(a));
  }
  return { to, cc };
}

// ---- threading --------------------------------------------------------------

export function normaliseMessageId(raw) {
  const t = String(raw || '').trim();
  if (!t) { return null; }
  if (t.startsWith('<') && t.endsWith('>')) { return t; }
  const inner = t.replace(/^[<\s]+/, '').replace(/[>\s]+$/, '');
  return inner ? '<' + inner + '>' : null;
}

// In-Reply-To is the original's id; References is its References with the id added.
export function threading(kind, source) {
  if (kind === 'forward') { return { inReplyTo: null, references: null }; }
  const id = normaliseMessageId(source.messageId);
  if (!id) { return { inReplyTo: null, references: null }; }
  const prev = String(source.references || '').trim();
  return { inReplyTo: id, references: prev ? prev + ' ' + id : id };
}

// ---- wrapping and quoting ------------------------------------------------------

const segmenter = typeof Intl !== 'undefined' && Intl.Segmenter ? new Intl.Segmenter(undefined, { granularity: 'grapheme' }) : null;

function graphemes(text) {
  return segmenter ? Array.from(segmenter.segment(text), function (s) { return s.segment; }) : Array.from(text);
}

const WS = /^[ \t\n\r\f\v]$/;
const trimLeft = function (s) { return s.replace(/^[\s ]+/, ''); };
const trimRight = function (s) { return s.replace(/[\s ]+$/, ''); };

// One line of the original as quoted lines: the prefix before each, none longer
// than `max` characters where a break is possible. Lengths count what a person
// sees as one character. A line that is only blank gives the bare prefix.
export function prefixedWrappedLines(line, prefix, max) {
  const limit = max || MAX_QUOTED_LINE;
  const pLen = graphemes(prefix).length;
  if (limit <= pLen) { return [prefix + line]; }
  const maxContent = limit - pLen;
  let rest = trimLeft(String(line));
  if (rest === '') { return [prefix]; }
  const out = [];
  for (;;) {
    const gs = graphemes(rest);
    if (gs.length <= maxContent) { out.push(prefix + rest); break; }
    let lastWs = -1;
    for (let i = maxContent - 1; i >= 0; i--) { if (WS.test(gs[i])) { lastWs = i; break; } }
    if (lastWs > 0) {
      out.push(prefix + trimRight(gs.slice(0, lastWs).join('')));
      rest = trimLeft(gs.slice(lastWs + 1).join(''));
    } else {
      // No place to break in the window: break at the next one, or not at all.
      let next = -1;
      for (let i = maxContent; i < gs.length; i++) { if (WS.test(gs[i])) { next = i; break; } }
      if (next < 0) { out.push(prefix + rest); break; }
      out.push(prefix + trimRight(gs.slice(0, next).join('')));
      rest = trimLeft(gs.slice(next + 1).join(''));
    }
    if (rest === '') { break; }
  }
  return out;
}

export function quotedLines(plain, prefix) {
  const out = [];
  for (const line of String(plain).split('\n')) { out.push(...prefixedWrappedLines(line, prefix, MAX_QUOTED_LINE)); }
  return out;
}

// The header line above a quote: "On <date> at <time>, <sender> wrote:".
export function quoteHeader(source, cfg, locale) {
  const when = source.dateMs != null ? new Date(source.dateMs) : new Date();
  const date = formatReplyDate(when, cfg.replyDateFormat || '', locale);
  const time = formatReplyTime(when, cfg.replyTimeFormat || '', locale);
  const sender = (source.from || []).length ? source.from.map(formatAddress).join(', ') : '(unknown)';
  return expandReplyHeader(cfg.replyHeaderTemplate || 'On $date at $time, $sender wrote:', date, time, sender);
}

export function aboveQuote(cfg) {
  return String(cfg.replyPlainPosition || '').trim().toLowerCase() !== 'after_quote';
}

// ---- HTML --------------------------------------------------------------------

const BLOCKS = new Set(['P', 'DIV', 'BLOCKQUOTE', 'PRE', 'UL', 'OL', 'LI', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'HR', 'TABLE', 'TR']);

// The text of an HTML fragment, with a line break where the layout would have
// one. Whitespace in the source collapses as it does on screen (a newline in
// the markup is a space, not a line), except inside <pre>.
export function htmlToPlain(html) {
  const doc = new DOMParser().parseFromString('<body>' + String(html || ''), 'text/html');
  let out = '';
  (function walk(node) {
    for (const n of node.childNodes) {
      if (n.nodeType === Node.TEXT_NODE) {
        let text = n.textContent;
        if (!(n.parentElement && n.parentElement.closest('pre'))) {
          text = text.replace(/[ \t\r\n\f]+/g, ' ');
          if (out === '' || out.endsWith('\n')) { text = text.replace(/^ /, ''); }
        }
        out += text;
      } else if (n.nodeType === Node.ELEMENT_NODE) {
        const tag = n.tagName;
        if (tag === 'SCRIPT' || tag === 'STYLE') { continue; }
        if (tag === 'BR') { out = out.replace(/ +$/, '') + '\n'; continue; }
        const block = BLOCKS.has(tag);
        if (block && out !== '' && !out.endsWith('\n')) { out += '\n'; }
        walk(n);
        if (block && out !== '' && !out.endsWith('\n')) { out += '\n'; }
      }
    }
  })(doc.body);
  return out.replace(/\r\n?/g, '\n').replace(/[ \t]+\n/g, '\n').replace(/\n{3,}/g, '\n\n').trim();
}

export function plainBodyForQuote(source) {
  const p = String(source.bodyPlain || '').trim();
  return p || htmlToPlain(source.bodyHtml);
}

const ALLOWED = new Set(['P', 'BR', 'DIV', 'SPAN', 'STRONG', 'B', 'EM', 'I', 'U', 'S', 'STRIKE', 'DEL', 'CODE', 'PRE',
  'BLOCKQUOTE', 'A', 'UL', 'OL', 'LI', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'HR']);
const DROPPED = new Set(['SCRIPT', 'STYLE', 'IFRAME', 'OBJECT', 'EMBED']);

function sanitiseNode(node, out) {
  for (const n of Array.from(node.childNodes)) {
    if (n.nodeType === Node.TEXT_NODE) {
      out.append(out.ownerDocument.createTextNode(n.textContent));
    } else if (n.nodeType === Node.ELEMENT_NODE) {
      if (DROPPED.has(n.tagName)) { continue; }
      if (!ALLOWED.has(n.tagName)) {
        // Anything else is flattened to its text; images and tables do not survive.
        out.append(out.ownerDocument.createTextNode(n.textContent));
        continue;
      }
      const el = out.ownerDocument.createElement(n.tagName.toLowerCase());
      if (n.tagName === 'DIV' && n.getAttribute('class') === QUOTED_CLASS) { el.setAttribute('class', QUOTED_CLASS); }
      if (n.tagName === 'A') {
        const href = String(n.getAttribute('href') || '').trim();
        if (/^(https?:|mailto:)/i.test(href)) { el.setAttribute('href', href); }
      }
      sanitiseNode(n, el);
      out.append(el);
    }
  }
}

// HTML that is safe to send: a small set of formatting tags, links only to
// web and mail, no scripts, styles, event handlers or other attributes.
export function sanitiseRich(html) {
  const doc = new DOMParser().parseFromString('<body>' + String(html || ''), 'text/html');
  const out = doc.createElement('div');
  sanitiseNode(doc.body, out);
  return out.innerHTML;
}

function escapeHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

const GAP = '<p><br></p><p><br></p>';

// The quoted original as the starting HTML of the rich editor, in a block the
// editor and the plain-text export can find again.
export function quotedRichSeed(source, cfg, locale) {
  const header = '<p>' + escapeHtml(quoteHeader(source, cfg, locale)) + '</p>';
  const rawHtml = String(source.bodyHtml || '').trim();
  const inner = rawHtml ? sanitiseRich(rawHtml) : escapeHtml(plainBodyForQuote(source)).replace(/\n/g, '<br>\n');
  const quoted = '<div class="' + QUOTED_CLASS + '">' + header + '<blockquote>' + inner + '</blockquote></div>';
  return aboveQuote(cfg) ? GAP + quoted : quoted + GAP;
}

// The quoted original as text to start the plain editor with, laid out so the
// caret can go where the reply is written.
export function quotedPlain(source, cfg, locale) {
  const header = quoteHeader(source, cfg, locale);
  const plain = plainBodyForQuote(source).replace(/[\s ]+$/, '');
  const lines = plain === '' ? [] : quotedLines(plain, cfg.replyLinePrefix || '> ');
  const body = lines.length ? header + '\n' + lines.join('\n') : header;
  return aboveQuote(cfg) ? '\n\n' + body + '\n\n' : '\n' + body + '\n\n\n';
}

export function containsQuotedBlock(html) {
  return String(html || '').indexOf(QUOTED_CLASS) >= 0;
}

// The text/plain form of rich text that holds a quoted original: the user's
// own words, and the quote with its header and each line prefixed, in the order
// the person chose.
export function plainFromRich(html, cfg) {
  const doc = new DOMParser().parseFromString('<body>' + String(html || ''), 'text/html');
  const quoted = doc.body.querySelector('div.' + QUOTED_CLASS);
  if (!quoted) { return htmlToPlain(html); }
  const head = quoted.querySelector('p');
  const header = head ? head.textContent.trim() : '';
  const bq = quoted.querySelector('blockquote');
  const lines = bq ? quotedLines(htmlToPlain(bq.innerHTML), cfg.replyLinePrefix || '> ') : [];
  quoted.remove();
  const mine = htmlToPlain(doc.body.innerHTML);
  const quote = [header].concat(lines).filter(function (l, i) { return i > 0 || l; }).join('\n');
  const parts = aboveQuote(cfg) ? [mine, quote] : [quote, mine];
  return parts.filter(Boolean).join('\n\n');
}

// ---- the original, as an attachment ---------------------------------------------

// A minimal RFC 5322 rendering of the original, for a forward made without
// quoting it.
export function syntheticEml(source) {
  const addrs = function (list) { return (list || []).map(formatAddress).join(', '); };
  const lines = ['From: ' + addrs(source.from), 'To: ' + addrs(source.to)];
  if ((source.cc || []).length) { lines.push('Cc: ' + addrs(source.cc)); }
  lines.push('Subject: ' + String(source.subject || ''));
  if (source.dateMs != null) { lines.push('Date: ' + new Date(source.dateMs).toUTCString().replace('GMT', '+0000')); }
  if (source.messageId) { lines.push('Message-ID: ' + normaliseMessageId(source.messageId)); }
  lines.push('MIME-Version: 1.0', 'Content-Type: text/plain; charset=utf-8', '', plainBodyForQuote(source));
  return lines.join('\r\n') + '\r\n';
}
