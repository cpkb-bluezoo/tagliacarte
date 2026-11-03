// h('div', { class: 'x', onClick: fn }, child, ...) builds elements. Strings
// are always appended as text nodes, never parsed as HTML.
export function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (v == null || v === false) { continue; }
      if (k.startsWith('on') && typeof v === 'function') {
        el.addEventListener(k.slice(2).toLowerCase(), v);
      } else if (k === 'class') {
        el.className = v;
      } else if (k === 'text') {
        el.textContent = v;
      } else if (k === 'depth') {
        el.style.setProperty('--depth', String(v));
      } else if (v === true) {
        el.setAttribute(k, '');
      } else {
        el.setAttribute(k, v);
      }
    }
  }
  appendAll(el, children);
  return el;
}

function appendAll(el, children) {
  for (const c of children.flat()) {
    if (c == null || c === false) { continue; }
    el.append(c.nodeType ? c : String(c));
  }
}

// Replaces el's children. Use this rather than el.replaceChildren(...), which
// turns a null or false argument into the text "null" or "false".
export function setChildren(el, nodes) {
  el.replaceChildren();
  appendAll(el, nodes);
}
