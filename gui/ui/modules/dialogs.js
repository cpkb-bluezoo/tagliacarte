// Native file and folder dialogs. The back end shows the dialog on the UI
// thread and answers with a pathPicked event; the callback gets the chosen
// path, or null if the dialog was cancelled.

import { call } from './wire.js';

let seq = 0;
const pending = {};

// opts: { mode: 'file' | 'folder' | 'save', title, fileName } (or the older { folder: true }).
export function pickPath(opts, cb) {
  const requestId = 'p' + (++seq);
  pending[requestId] = cb;
  call('dialog.pickPath', {
    requestId: requestId,
    mode: opts.mode || (opts.folder ? 'folder' : 'file'),
    title: opts.title || '',
    fileName: opts.fileName || ''
  }, function (err) {
    if (err) {
      delete pending[requestId];
      cb(null);
    }
  });
}

export function onPathPicked(ev) {
  const cb = pending[ev.requestId];
  if (!cb) { return; }
  delete pending[ev.requestId];
  cb(ev.path || null);
}
