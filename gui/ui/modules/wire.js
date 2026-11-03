// The page's only link to the back end: JSON calls and a long-poll for
// events. fetch has no callback form, so it is wrapped here once; everything
// else in the UI sees callbacks.

const token = window.__TOKEN__;

function post(path, body, cb) {
  fetch(path, {
    method: 'POST',
    body: body,
    cache: 'no-store',
    headers: { 'X-Token': token }
  }).then(function (r) {
    if (!r.ok) { throw new Error('HTTP ' + r.status); }
    return r.text();
  }).then(function (text) { cb(null, text); }, function (e) { cb(e, null); });
}

// call('config.load', undefined, (err, result) => ...). The reply's `ok` and
// `error` are folded into err; result is the reply's `result` (may be undefined).
export function call(name, args, cb) {
  const body = args === undefined ? '' : JSON.stringify(args);
  post('/api/' + name, body, function (err, text) {
    if (err) { cb(err); return; }
    let reply;
    try { reply = JSON.parse(text); } catch (e) { cb(e); return; }
    if (!reply.ok) { cb(new Error(reply.error || 'call failed')); return; }
    cb(null, reply.result);
  });
}

const listeners = [];

// Every event object is passed to fn, in arrival order.
export function onEvent(fn) { listeners.push(fn); }

function poll() {
  post('/poll', '', function (err, text) {
    if (err) { setTimeout(poll, 1000); return; }
    let events = [];
    try { events = JSON.parse(text); } catch (e) { console.error('bad event batch', e); }
    for (const ev of events) {
      for (const fn of listeners) { fn(ev); }
    }
    poll();
  });
}

export function start() { poll(); }

// Sends a file's bytes as a raw request body (no encoding into JSON) and calls
// cb(err, info) with {id, name, mimeType, size}. Used for pasted images; files
// chosen in a dialog or dropped are named by path and never pass through here.
//
// WKWebView hands a custom-scheme handler a Blob body as a stream the handler
// never sees, so a Blob is read into a typed array first. That is fine for what
// this is for; anything large goes by path.
export function upload(name, mimeType, body, cb) {
  const send = function (bytes) {
    return fetch('/upload', {
      method: 'POST',
      body: bytes,
      cache: 'no-store',
      headers: {
        'X-Token': token,
        'X-Filename': encodeURIComponent(name),
        'Content-Type': mimeType || 'application/octet-stream'
      }
    });
  };
  const read = body instanceof Blob ? body.arrayBuffer().then(function (b) { return new Uint8Array(b); }) : Promise.resolve(body);
  read.then(send).then(function (r) {
    return r.json().then(function (j) { return { ok: r.ok, json: j }; });
  }).then(function (res) {
    if (res.ok) { cb(null, res.json); } else { cb(new Error(res.json.error || 'upload failed')); }
  }, function (e) { cb(e); });
}
