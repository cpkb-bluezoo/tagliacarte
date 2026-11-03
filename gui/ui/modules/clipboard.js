// Putting text on the system clipboard. The page only ever writes to it, from a
// click; it never reads it (a read makes the system ask the user, and nothing
// here needs one). A refusal is said, not swallowed.

import { t } from './i18n.js';
import { toast } from './toast.js';

// `message` is what the toast says when it worked.
export function copyText(text, message) {
  let done;
  try {
    done = navigator.clipboard.writeText(text);
  } catch (e) {
    done = Promise.reject(e);
  }
  done.then(function () { toast(message); }, function (e) {
    toast(t('webCopyFailed', { error: (e && e.message) || String(e) }));
  });
}
