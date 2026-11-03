// Work that finishes later is started with a request id and ends with an
// event carrying that id (OAuth sign-in, SMTP verification, Nostr sync).
// waitFor registers the callback; events.js calls resolve() for those events.

let seq = 0;
const waiting = {};

export function newRequestId(prefix) { return prefix + (++seq); }

// cb receives the error message, or null when the work succeeded.
export function waitFor(requestId, cb) { waiting[requestId] = cb; }

export function cancel(requestId) { delete waiting[requestId]; delete waitingForEvent[requestId]; }

export function resolve(ev) {
  const cb = waiting[ev.requestId];
  if (!cb) { return; }
  delete waiting[ev.requestId];
  cb(ev.error || null);
}

// For work whose end event carries results, not just success or failure: cb
// receives the whole event.
const waitingForEvent = {};

export function waitForEvent(requestId, cb) { waitingForEvent[requestId] = cb; }

export function resolveEvent(ev) {
  const cb = waitingForEvent[ev.requestId];
  if (!cb) { return false; }
  delete waitingForEvent[ev.requestId];
  cb(ev);
  return true;
}
