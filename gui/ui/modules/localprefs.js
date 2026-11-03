// Preferences that belong to this viewer only and have no meaning to the back
// end (how the window is laid out, not what the account does). Storage can be
// unavailable, so every access is guarded and the defaults always work.

const PREFIX = 'tagliacarte.';

export function getLocal(key, fallback) {
  try {
    const v = window.localStorage.getItem(PREFIX + key);
    return v === null ? fallback : JSON.parse(v);
  } catch (e) {
    return fallback;
  }
}

export function setLocal(key, value) {
  try {
    window.localStorage.setItem(PREFIX + key, JSON.stringify(value));
  } catch (e) {
    // Not persisted; the setting still applies for this session.
  }
}
