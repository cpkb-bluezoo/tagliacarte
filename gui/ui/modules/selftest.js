// `tagliacarte-gui --selftest`: drives the real UI against the real session
// and reports to the terminal. Needs a scratch config whose first account is a
// Maildir with an INBOX, and a vCard file called test.vcf beside it (see
// gui/README.md). It changes settings, accounts, transports and contacts, and
// puts them back, so never point it at real data.

import { state, changed } from './store.js';
import { apply, ensureAccount } from './events.js';
import { composeSendKind, closeCompose, filesDropped } from './compose/view.js';
import { call, upload } from './wire.js';
import { bodyUrl, messageActions } from './reader.js';
import { windowTitle, currentTitle } from './title.js';
import { hueOf, initials, accountAvatar } from './avatar.js';
import { allowRestore, beginRestore, abandonRestore, restoreStep } from './restore.js';
import * as CD from './calendar/dates.js';
import { flushLocation } from './location.js';
import { LABELS as MENU_LABELS, sendMenuLabels, enabledMenuItems } from './menu.js';
import { runMessageAction } from './reader.js';
import { t, pickLanguage } from './i18n.js';
import * as S from './settings/accountschema.js';
import * as R from './compose/reply.js';
import {
  smtpErrorWantsCredentials, smtpAdvertisedAuth, wantsGoogleOAuth, imapStyleCredentialKind,
  isMissingImapCredentials, isMissingNostrCredentials
} from './credprompt.js';

function report(line) {
  call('app.report', { line: line }, function () {});
}

// Calls cb(true) once pred() holds, or cb(false, why) after ms. The first check
// is a tick later, so that anything the caller's last click queued has run.
function waitFor(what, pred, ms, cb) {
  const t0 = Date.now();
  function check() {
    let ok = false;
    try { ok = pred(); } catch (e) { ok = false; }
    if (ok) { cb(true, ''); }
    else if (Date.now() - t0 > ms) { cb(false, 'timed out waiting for ' + what); }
    else { setTimeout(check, 50); }
  }
  setTimeout(check, 0);
}

// Runs steps one after another; each is function(next) and calls next() to
// go on or next(false, why) to fail the whole sequence.
function sequence(steps, done) {
  let i = 0;
  (function go(ok, why) {
    if (ok === false) { done(false, why); return; }
    if (i === steps.length) { done(true, ''); return; }
    sequence.at = i + 1 + '/' + steps.length;
    steps[i++](go);
  })();
}

const tests = [
  ['app.info answers', function (done) {
    call('app.info', undefined, function (err, r) {
      done(!err && typeof r.version === 'string', err ? err.message : 'version ' + (r && r.version));
    });
  }],
  ['unknown command is an error reply, not a crash', function (done) {
    call('no.such.command', undefined, function (err) {
      done(!!err && /unknown command/.test(err.message), err ? err.message : 'no error');
    });
  }],
  ['command channel refuses a request without the token', function (done) {
    fetch('/api/app.info', { method: 'POST' }).then(
      function (r) { done(r.status === 403, 'HTTP ' + r.status); },
      function (e) { done(false, String(e)); });
  }],
  ['strings come from the ARB files, with language picking and a fallback to the key', function (done) {
    fetch('/locales/de.arb').then(function (r) {
      return r.json().then(function (de) {
        const ok = r.status === 200 && typeof de.settings === 'string' &&
          t('settings') !== 'settings' && t('no.such.key') === 'no.such.key' &&
          t('savedToPath', { path: '/x' }).indexOf('/x') >= 0 &&
          pickLanguage(['de-AT']) === 'de' && pickLanguage(['xx']) === 'en';
        done(ok, 'lang ' + document.documentElement.lang + ', settings="' + t('settings') + '", de="' + de.settings + '"');
      });
    }, function (e) { done(false, String(e)); });
  }],
  ['assets validate: ETag, and 304 for a matching If-None-Match', function (done) {
    fetch('/style.css').then(function (r) {
      const etag = r.headers.get('ETag');
      if (!etag) { done(false, 'no ETag'); return; }
      fetch('/style.css', { headers: { 'If-None-Match': etag } }).then(
        function (r2) { done(r2.status === 304, etag + ' -> ' + r2.status); },
        function (e) { done(false, String(e)); });
    }, function (e) { done(false, String(e)); });
  }],
  ['remote fetch is blocked by the CSP', function (done) {
    fetch('https://example.com/', { mode: 'no-cors' }).then(
      function () { done(false, 'request succeeded'); },
      function () { done(true, ''); });
  }],
  ['config has an account and the rail shows it', function (done) {
    waitFor('account button', function () {
      return state.config && document.querySelector('.rail .account');
    }, 5000, function (ok, why) {
      const b = document.querySelector('.rail .account');
      done(ok, ok ? 'accounts: ' + state.accountOrder.join(',') + ', first: ' + b.title : why);
    });
  }],
  ['session connects and the folder list arrives', function (done) {
    waitFor('folders', function () {
      const a = state.accounts[state.selectedAccount];
      return a && a.folders.length > 0 && document.querySelector('.folder-row');
    }, 15000, function (ok, why) {
      const a = state.accounts[state.selectedAccount];
      done(ok, ok ? a.connection + ', ' + a.folders.length + ' folders: ' + a.folders.join(', ') : why);
    });
  }],
  ['opening INBOX lists its messages', function (done) {
    const row = document.querySelector('.folder-row[data-folder="INBOX"]');
    if (!row) { done(false, 'no INBOX row'); return; }
    row.click();
    waitFor('message list', function () {
      return state.list && state.list.ready;
    }, 15000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const rows = document.querySelectorAll('.message-row').length;
      const loaded = state.list.slots.filter(Boolean).length;
      done(!state.list.error && rows === loaded,
        'total ' + state.list.total + ', loaded ' + loaded + ', shown ' + rows +
        (state.list.error ? ', error ' + state.list.error : ''));
    });
  }],
  ['opening a message shows its envelope and keeps the body in a sandboxed frame', function (done) {
    const row = document.querySelector('.message-row');
    if (!row) { done(false, 'no message rows'); return; }
    row.click();
    waitFor('message opened', function () {
      return state.message && state.message.opened && state.message.headers;
    }, 15000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const m = state.message;
      const frame = document.querySelector('iframe.msg-body');
      const subject = document.querySelector('.msg-head .subject').textContent;
      const good = frame && frame.getAttribute('sandbox') === 'allow-popups' &&
        frame.getAttribute('src') === bodyUrl(m) && subject && m.headers.from.length > 0 && !m.error;
      done(!!good, 'subject "' + subject + '", from ' + m.headers.from.map(function (a) { return a.email; }).join(',') +
        ', to ' + m.headers.to.length + ', attachments ' + m.attachments.length +
        (m.error ? ', error ' + m.error : ''));
    });
  }],
  ['the header area holds only elements, no stray text', function (done) {
    const strays = Array.from(document.querySelector('.msg-head').childNodes)
      .filter(function (n) { return n.nodeType !== 1; })
      .map(function (n) { return JSON.stringify(n.textContent); });
    done(strays.length === 0, strays.length ? 'stray text nodes: ' + strays.join(', ') : 'elements only');
  }],
  ['an empty envelope never blanks the From and Date the list already showed', function (done) {
    const m = state.message;
    const real = m.headers;
    const rowText = function (labelKey) {
      const row = Array.from(document.querySelectorAll('.msg-head .hdr')).find(function (r) {
        return r.querySelector('.label').textContent === t(labelKey);
      });
      return row ? row.querySelector('.value').textContent : null;
    };
    apply({ type: 'messageHeaders', requestId: m.requestId, from: [], to: [], cc: [] });
    setTimeout(function () {
      const from = rowText('headerFrom');
      const date = rowText('headerDate');
      const expectDate = m.summary.dateMs != null;
      m.headers = real;
      done(!!from && from === m.summary.from && (!expectDate || !!date),
        'from "' + from + '", date ' + (date ? 'shown' : 'missing'));
    }, 100);
  }],
  ['the body is served as HTML with a locked-down CSP', function (done) {
    fetch(bodyUrl(state.message)).then(function (r) {
      return r.text().then(function (text) {
        const csp = r.headers.get('Content-Security-Policy') || '';
        const ok = r.status === 200 && /^text\/html/.test(r.headers.get('Content-Type') || '') &&
          /default-src 'none'/.test(csp) && /sandbox/.test(csp) && text.indexOf('<meta charset') >= 0 &&
          text.indexOf('<base target="_blank">') >= 0 &&
          // Not kept: loading the body is what tells the page how many remote images it holds.
          r.headers.get('Cache-Control') === 'no-store';
        done(ok, r.status + ', ' + text.length + ' bytes');
      });
    }, function (e) { done(false, String(e)); });
  }],
  ['an unknown inline part is a 404', function (done) {
    fetch(bodyUrl(state.message).replace(/body$/, 'cid/nope')).then(
      function (r) { done(r.status === 404, 'HTTP ' + r.status); },
      function (e) { done(false, String(e)); });
  }],
  ['reading a message marks it read', function (done) {
    waitFor('read flag', function () {
      const m = state.message;
      const row = state.list.slots.find(function (r) { return r && r.id === m.id; });
      return row && row.isRead;
    }, 8000, function (ok, why) { done(ok, why); });
  }],
  ['an attachment is saved to the chosen file by the back end, not fetched by the page', function (done) {
    let dir = '';
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms, function (ok, why) { next(ok, why); }); };
    };
    const toastText = function () {
      return Array.from(document.querySelectorAll('.toast')).map(function (el) { return el.textContent; }).join('|');
    };
    sequence([
      function (next) {
        call('app.info', undefined, function (err, info) {
          if (err) { next(false, err.message); return; }
          dir = info.configPath.replace(/[\\/][^\\/]*$/, '');
          next();
        });
      },
      function (next) {
        const row = Array.from(document.querySelectorAll('.message-row')).find(function (r) { return /A file for you/.test(r.textContent); });
        if (!row) { next(false, 'no row for the message with an attachment'); return; }
        row.click();
        next();
      },
      wait('attachment listed', function () {
        return state.message && state.message.opened && state.message.attachments.length === 1 &&
          document.querySelector('button.attachment');
      }, 15000),
      function (next) {
        const b = document.querySelector('button.attachment');
        if (!/sample\.bin/.test(b.textContent) || !/3(\.0)? ?KB|3000/i.test(b.textContent)) { next(false, 'button reads "' + b.textContent + '"'); return; }
        b.click();
        next();
      },
      wait('saved toast', function () { return /sample\.bin/.test(toastText()) && !/fail/i.test(toastText()); }, 10000),
      // Its size on disk, by attaching the saved file and taking it off again.
      function (next) {
        call('compose.attachPath', { path: dir + '/saved-sample.bin' }, function (err, info) {
          if (err) { next(false, err.message); return; }
          call('compose.removeAttachment', { id: info.id }, function () {});
          next(info.size === 3000, 'saved file is ' + info.size + ' bytes');
        });
      },
      function (next) {
        call('compose.attachPath', { path: dir + '/saved-sample.bin.part' }, function (err) {
          next(!!err, err ? 'no temporary file is left behind' : 'a .part file was left behind');
        });
      }
    ], function (ok, why) { done(ok, ok ? 'saved sample.bin, 3000 bytes, no temporary file left' : why); });
  }],
  ['remote images stay blocked until asked for, then are served by the back end', function (done) {
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms, function (ok, why) { next(ok, why); }); };
    };
    const open = function (subject) {
      return function (next) {
        const row = Array.from(document.querySelectorAll('.message-row')).find(function (r) { return r.textContent.indexOf(subject) >= 0; });
        if (!row) { next(false, 'no row for "' + subject + '"'); return; }
        row.click();
        next();
      };
    };
    const opened = function () { return state.message && state.message.opened && state.message.headers; };
    let first = '';
    let proxied = [];
    sequence([
      function (next) {
        // The scratch config starts with the setting at its default, which is off.
        next(!state.config.loadRemoteImages, 'loadRemoteImages is ' + state.config.loadRemoteImages);
      },
      open('Remote pictures'),
      // The count comes with the body, which the frame loads after the message opens.
      wait('the message opened and its body counted', function () { return opened() && state.message.remoteCount > 0; }, 15000),
      function (next) {
        const m = state.message;
        const bar = document.querySelector('.imagesbar');
        const frameUrl = document.querySelector('iframe.msg-body').getAttribute('src');
        const ok = bar && !bar.hidden && bar.textContent.indexOf(t('remoteImagesBlocked')) >= 0 &&
          bar.querySelector('button.load-images') && frameUrl.indexOf('images=1') < 0 && !m.images;
        next(!!ok, ok ? '' : 'banner ' + (bar ? 'hidden=' + bar.hidden : 'missing') + ', frame ' + frameUrl);
      },
      function (next) {
        fetch(bodyUrl(state.message)).then(function (r) {
          return r.text().then(function (text) {
            first = text;
            const csp = r.headers.get('Content-Security-Policy') || '';
            const ok = text.indexOf('https://images.invalid/logo.png') >= 0 && text.indexOf('/remote/') < 0 &&
              /img-src app:\/\/localhost;/.test(csp.replace('http://app.localhost', 'app://localhost'));
            next(ok, ok ? '' : 'blocked body is wrong; csp ' + csp);
          });
        });
      },
      function (next) { document.querySelector('.imagesbar button.load-images').click(); next(); },
      wait('images requested', function () {
        return document.querySelector('iframe.msg-body').getAttribute('src').indexOf('images=1') >= 0 &&
          document.querySelector('.imagesbar').hidden;
      }, 5000),
      function (next) {
        fetch(bodyUrl(state.message)).then(function (r) {
          return r.text().then(function (text) {
            proxied = text.match(/\/remote\/[0-9a-f]+\/[A-Za-z0-9_-]+/g) || [];
            const ok = r.status === 200 && proxied.length === 2 && text.indexOf('images.invalid') < 0 &&
              text.indexOf('127.0.0.1:9') < 0 && text.indexOf('href="https://example.com/read"') >= 0;
            next(ok, ok ? '' : 'allowed body: ' + proxied.length + ' proxied, status ' + r.status);
          });
        });
      },
      // The proxy refuses an address inside the user's own network, whatever the message says.
      function (next) {
        fetch(proxied[1]).then(function (r) {
          return r.text().then(function (text) {
            next(r.status === 403 && /public internet/.test(text), 'loopback tracker: HTTP ' + r.status + ' ' + text);
          });
        });
      },
      // And a proxy URL a message makes up has no key.
      function (next) {
        const forged = proxied[1].replace(/\/remote\/[0-9a-f]+\//, '/remote/' + '0'.repeat(64) + '/');
        fetch(forged).then(function (r) { next(r.status === 403, 'forged key: HTTP ' + r.status); });
      },
      function (next) {
        fetch('/remote/' + 'x'.repeat(10)).then(function (r) { next(r.status === 404, 'no address: HTTP ' + r.status); });
      },
      // Allowing images for one message does not allow them for the next.
      open('A file for you'),
      wait('the other message opened', function () { return opened() && state.message.headers.subject === 'A file for you'; }, 15000),
      function (next) {
        const m = state.message;
        next(!m.images && bodyUrl(m).indexOf('images=1') < 0 && document.querySelector('.imagesbar').hidden,
          'images=' + m.images + ', ' + bodyUrl(m));
      },
      // The setting allows them for every message, without a button.
      function (next) { call('settings.set', { key: 'loadRemoteImages', value: true }, function (err) { next(!err, err && err.message); }); },
      wait('setting reached the page', function () { return state.config.loadRemoteImages === true; }, 5000),
      open('Remote pictures'),
      wait('the remote message again', function () { return opened() && state.message.headers.subject === 'Remote pictures' && state.message.html; }, 15000),
      function (next) {
        const m = state.message;
        const ok = !m.images && bodyUrl(m).indexOf('images=1') >= 0 && document.querySelector('.imagesbar').hidden;
        next(ok, ok ? '' : 'with the setting on: images=' + m.images + ', ' + bodyUrl(m));
      },
      function (next) { call('settings.set', { key: 'loadRemoteImages', value: false }, function (err) { next(!err, err && err.message); }); },
      wait('setting back off', function () { return state.config.loadRemoteImages === false; }, 5000)
    ], function (ok, why) { done(ok, ok ? 'blocked with a banner, proxied on request, key and address checked, setting honoured' : why); });
  }],
  ['the window cannot be sent to a web page; the link is offered instead', function (done) {
    location.href = 'https://example.com/window';
    waitFor('link event', function () { return state.message && state.message.link; }, 4000, function (ok, why) {
      const link = state.message.link;
      state.message.link = null;
      done(ok && link === 'https://example.com/window' && location.hostname.indexOf('localhost') >= 0,
        ok ? link : why);
    });
  }],
  ['only web and mail links can be opened', function (done) {
    call('app.openUrl', { url: 'file:///etc/passwd' }, function (err) {
      done(!!err && /only http/.test(err.message), err ? err.message : 'was accepted');
    });
  }],
  ['the settings button opens the tabs, and an account returns to mail', function (done) {
    const btn = document.querySelector('.rail .settings-btn');
    if (!btn) { done(false, 'no settings button'); return; }
    btn.click();
    waitFor('settings view', function () {
      return document.getElementById('app').classList.contains('view-settings') &&
        document.querySelector('.settings .tabs button') && document.querySelector('.settings .tabpanel .settings-page');
    }, 3000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const tabs = document.querySelectorAll('.settings .tabs button').length;
      done(tabs >= 5, tabs + ' tabs');
    });
  }],
  ['a switch saves one setting and it survives a reload from disk', function (done) {
    document.querySelector('.tabs [data-tab="viewing"]').click();
    waitFor('viewing page', function () { return document.querySelector('[data-setting="threadedView"]'); }, 3000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const sw = document.querySelector('[data-setting="threadedView"]');
      const before = state.config.threadedView;
      sw.click();
      waitFor('saved', function () { return state.config.threadedView === !before; }, 5000, function (ok2, why2) {
        if (!ok2) { done(false, why2); return; }
        // Read it back from config.xml, not from memory.
        state.config.threadedView = before;
        call('config.load', undefined, function () {});
        waitFor('reloaded', function () { return state.config.threadedView === !before; }, 5000, function (ok3, why3) {
          document.querySelector('[data-setting="threadedView"]').click();
          waitFor('restored', function () { return state.config.threadedView === before; }, 5000, function (ok4, why4) {
            done(ok3 && ok4, ok3 ? 'toggled and read back from disk' : why3 || why4);
          });
        });
      });
    });
  }],
  ['a text setting saves on change and the reply preview follows it', function (done) {
    document.querySelector('.tabs [data-tab="composing"]').click();
    waitFor('composing page', function () { return document.querySelector('[data-setting="replyHeaderTemplate"]'); }, 3000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const input = document.querySelector('[data-setting="replyHeaderTemplate"]');
      const original = state.config.replyHeaderTemplate;
      input.value = '$sender said at $time:';
      input.dispatchEvent(new Event('input'));
      input.dispatchEvent(new Event('change'));
      waitFor('saved', function () { return state.config.replyHeaderTemplate === '$sender said at $time:'; }, 5000, function (ok2, why2) {
        const preview = document.querySelector('.preview').textContent;
        const good = ok2 && /alice@example\.com said at .+:/.test(preview) && preview.indexOf('$') < 0;
        input.value = original;
        input.dispatchEvent(new Event('change'));
        waitFor('restored', function () { return state.config.replyHeaderTemplate === original; }, 5000, function () {
          done(good, ok2 ? preview : why2);
        });
      });
    });
  }],
  ['the passphrase is accepted but never sent back', function (done) {
    document.querySelector('.tabs [data-tab="security"]').click();
    waitFor('security page', function () { return document.querySelector('[data-setting="mailCryptoPgpPassphrase"]'); }, 3000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const input = document.querySelector('[data-setting="mailCryptoPgpPassphrase"]');
      input.value = 'hunter2-secret';
      input.dispatchEvent(new Event('change'));
      waitFor('flag', function () { return state.config.mailCryptoPgpHasPassphrase === true; }, 5000, function (ok2, why2) {
        const leaked = JSON.stringify(state.config).indexOf('hunter2-secret') >= 0;
        input.value = '';
        input.dispatchEvent(new Event('change'));
        waitFor('cleared', function () { return state.config.mailCryptoPgpHasPassphrase === false; }, 5000, function () {
          done(ok2 && !leaked, ok2 ? 'flag set, secret not in the page state' : why2);
        });
      });
    });
  }],
  ['unknown settings and invalid values are refused, and nothing changes', function (done) {
    call('settings.set', { key: 'accounts', value: 'x' }, function (e1) {
      call('settings.set', { key: 'mailCryptoStack', value: 'bogus' }, function (e2) {
        done(!!e1 && /unknown setting/.test(e1.message) && !!e2 && /mailCryptoStack/.test(e2.message) &&
          state.config.mailCryptoStack !== 'bogus', (e1 && e1.message) + ' | ' + (e2 && e2.message));
      });
    });
  }],
  ['server errors are classified as the Flutter UI did', function (done) {
    const checks = [
      smtpErrorWantsCredentials('credential required for SMTP transport t1; server EHLO AUTH: PLAIN XOAUTH2') === true,
      smtpErrorWantsCredentials('auth failed: 535 bad') === true,
      smtpErrorWantsCredentials('connect: Connection refused (os error 61)') === false,
      smtpErrorWantsCredentials('failed to lookup address information: nodename nor servname provided') === false,
      smtpErrorWantsCredentials('something else entirely') === false,
      smtpAdvertisedAuth('x server EHLO AUTH: PLAIN, xoauth2').join() === 'PLAIN,XOAUTH2',
      wantsGoogleOAuth({ oauthProvider: 'google' }, 'server EHLO AUTH: LOGIN XOAUTH2') === true,
      wantsGoogleOAuth({ oauthProvider: '' }, 'server EHLO AUTH: LOGIN XOAUTH2') === false,
      wantsGoogleOAuth({ oauthProvider: 'google' }, 'server EHLO AUTH: LOGIN') === false,
      imapStyleCredentialKind('imap', 'x TAGLIACARTE_IMAP_CAPS:IMAP4REV1,AUTH=XOAUTH2,AUTH=PLAIN') === 'google',
      imapStyleCredentialKind('imap', 'x TAGLIACARTE_IMAP_CAPS:IMAP4REV1,AUTH=PLAIN,AUTH=XOAUTH2') === 'password',
      imapStyleCredentialKind('imap', 'x TAGLIACARTE_IMAP_CAPS:AUTH=GSSAPI,AUTH=LOGIN') === 'password',
      imapStyleCredentialKind('gmail', '') === 'google',
      imapStyleCredentialKind('imap', 'no caps here', { oauthProvider: 'google' }) === 'google',
      imapStyleCredentialKind('imap', 'x TAGLIACARTE_IMAP_CAPS:AUTH=XOAUTH2', { oauthProvider: 'microsoft' }) === 'password',
      imapStyleCredentialKind('imap', 'no caps here', { oauthProvider: '' }) === 'password',
      imapStyleCredentialKind('matrix', '') === 'password',
      isMissingImapCredentials('no saved password for IMAP account s1') === true,
      isMissingImapCredentials('timeout') === false,
      isMissingNostrCredentials('no secret key') === true
    ];
    const bad = checks.map(function (ok, i) { return ok ? null : i; }).filter(function (x) { return x !== null; });
    done(bad.length === 0, bad.length ? 'failed checks: ' + bad.join(',') : checks.length + ' checks');
  }],
  ['every account type is written the way the previous interface wrote it', function (done) {
    const checks = {};
    const eq = function (a, b) { return JSON.stringify(a) === JSON.stringify(b); };
    const make = function (type, existing, edit) {
      const d = S.initialDraft(type, existing);
      if (edit) { edit(d); }
      return S.toAccount(type, d, existing, existing ? existing.id : 's9');
    };

    const imapOld = { id: 's1', label: 'Work', backendType: 'imaps', lastFolder: 'Archive',
      attrs: { host: 'imap.x', port: '143', security: 'starttls', username: 'old', email: 'o@x', keepme: 'y',
        imapDeleteMode: 'Mark Deleted', imapTrashFolderName: 'Trash' },
      lists: { transportIds: ['t1'], other: ['x'] } };
    const imap = make('imap', imapOld);
    checks.imapKeepsAndDrops = imap.attrs.keepme === 'y' && !('username' in imap.attrs) && !('email' in imap.attrs) &&
      !('imapTrashFolderName' in imap.attrs) && imap.attrs.imapDeleteMode === 'Mark Deleted' &&
      imap.attrs.port === '143' && imap.backendType === 'imap' && eq(imap.lists.transportIds, ['t1']) &&
      eq(imap.lists.other, ['x']) && imap.lastFolder === 'Archive';
    const imap2 = make('imap', imapOld, function (d) { d.deleteMode = 'Move to Trash'; d.trashFolder = ' Bin '; d.idleMin = ''; });
    checks.imapTrash = imap2.attrs.imapTrashFolderName === 'Bin' && !('imapIdleMinIdleSeconds' in imap2.attrs);

    const gm = make('gmail', null, function (d) { d.email = ' me@gmail.com '; d.sentLabel = ''; });
    checks.gmail = gm.attrs.email === 'me@gmail.com' && gm.attrs.gmailSentLabelId === 'SENT' && gm.attrs.gmailTrashLabelId === 'TRASH' &&
      gm.attrs.oauthProvider === 'google' && !('host' in gm.attrs) && !('transportIds' in gm.lists);

    const ex = make('exchange', { id: 's2', label: 'O', backendType: 'graph', attrs: { email: 'x', username: 'y' }, lists: { transportIds: ['t'] } });
    checks.exchange = ex.attrs.oauthProvider === 'microsoft' && !('email' in ex.attrs) && !('transportIds' in ex.lists) && ex.backendType === 'exchange';

    const md = make('maildir', null, function (d) { d.label = 'L'; d.path = 'home/me/Mail'; d.deleteMode = 'Delete immediately'; d.trashFolder = 'T'; });
    checks.maildir = md.attrs.path === '/home/me/Mail' && !('maildirTrashFolderName' in md.attrs) &&
      md.attrs.maildirDeleteMode === 'Delete immediately' && eq(md.lists.transportIds, []);
    checks.paths = S.localPath('//a//b') === '/a//b' && S.localPath('') === '/' && S.localPath('C:\\Mail') === 'C:\\Mail';

    const nostr = make('nostr', null, function (d) { d.relays = [' wss://a ', '', 'wss://b']; d.npub = 'npub1x'; d.nip05 = ''; });
    checks.nostr = eq(nostr.lists.relayUrls, ['wss://a', 'wss://b']) && nostr.attrs.npub === 'npub1x' && !('nip05' in nostr.attrs);
    checks.nostrDefaults = eq(S.initialDraft('nostr', null).relays, ['wss://relay.damus.io', 'wss://nos.lol']);

    const mx = make('matrix', null, function (d) { d.mxid = '@me:matrix.org'; });
    checks.matrix = mx.attrs.username === '@me:matrix.org' && mx.attrs.homeserver === 'https://matrix.org' && mx.attrs.host === 'https://matrix.org';
    checks.mxidParse = S.parseMxid('me:matrix.org') === null && S.parseMxid('@a:http://h').homeserverUrl === 'http://h' &&
      S.mxidFor({ attrs: { username: 'me', homeserver: 'https://matrix.org' } }) === '@me:matrix.org';

    const v = function (type, edit) { const d = S.initialDraft(type, null); d.label = 'x'; edit(d); return (S.validate(type, d) || {}).key || null; };
    checks.validation =
      v('imap', function (d) { d.label = ' '; }) === 'validationAccountNameRequired' &&
      v('imap', function (d) { d.host = ''; }) === 'validationHostRequired' &&
      v('imap', function (d) { d.host = 'h'; d.port = 'abc'; }) === 'validationPortRequired' &&
      v('imap', function (d) { d.host = 'h'; d.port = '70000'; }) === 'validationPortRequired' &&
      v('imap', function (d) { d.host = 'h'; d.idleMin = '10'; }) === 'validationImapMinIdleSeconds' &&
      v('imap', function (d) { d.host = 'h'; d.idleMin = '15'; }) === null &&
      v('imap', function (d) { d.host = 'h'; d.idleMin = '864001'; }) === 'validationImapMinIdleSeconds' &&
      v('maildir', function (d) {}) === 'validationLocalPathRequired' &&
      v('matrix', function (d) {}) === 'validationMatrixUserIdRequired' &&
      v('matrix', function (d) { d.mxid = 'nope'; }) === 'validationMatrixMxidInvalid' &&
      v('nostr', function (d) { d.relays = [' ']; }) === 'nostrRelayUrlsRequired' &&
      v('gmail', function (d) {}) === 'validationEmailAddressRequired';
    checks.types = S.normaliseType('IMAPS') === 'imap' && S.typeLabel('graph') === 'Exchange' && S.requiresTransport('Maildir') &&
      !S.requiresTransport('gmail');
    const bad = Object.keys(checks).filter(function (k) { return !checks[k]; });
    done(bad.length === 0, bad.length ? 'failed: ' + bad.join(', ') : Object.keys(checks).length + ' groups');
  }],
  ['an account can be added, edited and removed from the Accounts tab', function (done) {
    state.view = 'settings';
    document.querySelector('.tabs [data-tab="accounts"]').click();
    waitFor('accounts tab', function () { return document.querySelector('.settings-page .item-main.add'); }, 3000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const first = state.accountOrder[0];
      const sharedPath = state.accounts[first].attrs.path;
      const rows = document.querySelectorAll('.settings-page .item[data-account]').length;
      document.querySelector('.item-main.add').click();
      waitFor('type choices', function () { return document.querySelector('dialog.modal .choice[data-type="maildir"]'); }, 3000, function (ok2, why2) {
        if (!ok2) { done(false, why2); return; }
        document.querySelector('dialog.modal .choice[data-type="maildir"]').click();
        waitFor('editor', function () { return document.querySelector('[data-field="path"]'); }, 3000, function (ok3, why3) {
          if (!ok3) { done(false, why3); return; }
          const set = function (f, v) { const el = document.querySelector('[data-field="' + f + '"]'); el.value = v; el.dispatchEvent(new Event('input')); };
          document.querySelector('.editor-bar .primary').click();           // empty: refused with a toast
          set('label', 'Selftest Mail');
          set('path', sharedPath);
          document.querySelector('.editor-bar .primary').click();
          waitFor('saved', function () {
            return state.accountOrder.some(function (id) { return state.accounts[id].label === 'Selftest Mail'; });
          }, 8000, function (ok4, why4) {
            if (!ok4) { done(false, why4); return; }
            const id = state.accountOrder.find(function (x) { return state.accounts[x].label === 'Selftest Mail'; });
            const acc = state.accounts[id];
            const stored = acc.backendType === 'maildir' && acc.attrs.path === sharedPath && acc.attrs.maildirDeleteMode === 'Move to Trash' &&
              (acc.lists.transportIds || []).length === 0;
            const atEnd = state.accountOrder[state.accountOrder.length - 1] === id;
            waitFor('list', function () { return document.querySelector('[data-account="' + id + '"]'); }, 3000, function (ok5, why5) {
              if (!ok5) { done(false, why5); return; }
              document.querySelector('[data-account="' + id + '"] .item-main').click();       // edit
              waitFor('edit form', function () { return document.querySelector('[data-field="label"]'); }, 3000, function (ok6, why6) {
                if (!ok6) { done(false, why6); return; }
                const el = document.querySelector('[data-field="label"]');
                const beforeOrder = state.accountOrder.join();
                el.value = 'Selftest Renamed'; el.dispatchEvent(new Event('input'));
                document.querySelector('.editor-bar .primary').click();
                waitFor('renamed', function () { return state.accounts[id] && state.accounts[id].label === 'Selftest Renamed'; }, 8000, function (ok7, why7) {
                  if (!ok7) { done(false, why7); return; }
                  const inPlace = state.accountOrder.join() === beforeOrder;
                  waitFor('list again', function () { return document.querySelector('[data-account="' + id + '"] .danger'); }, 3000, function () {
                    document.querySelector('[data-account="' + id + '"] .danger').click();
                    waitFor('confirm', function () { return document.querySelector('dialog.modal[open] .primary'); }, 3000, function () {
                      document.querySelector('dialog.modal .primary').click();
                      waitFor('removed', function () { return !state.accounts[id]; }, 8000, function (ok8, why8) {
                        const stayed = state.view === 'settings';
                        done(stored && atEnd && inPlace && ok8 && stayed && state.accountOrder.length === rows,
                          ok8 ? 'added at the end (' + atEnd + '), stored ' + stored + (stored ? '' : ' ' + JSON.stringify(acc)) + ', renamed in place ' + inPlace + ', removed, still in settings ' + stayed : why8);
                      });
                    });
                  });
                });
              });
            });
          });
        });
      });
    });
  }],
  ['a missing login is asked for once, only for the account in view, and says why', function (done) {
    const id = 'selftest-fake';
    const saved = { account: state.selectedAccount, view: state.view };
    const fake = ensureAccount(id);
    fake.label = 'Fake Gmail';
    fake.backendType = 'imap';
    fake.attrs = { oauthProvider: 'google' };
    state.accountOrder.push(id);
    const open = function () { return document.querySelector('dialog.modal[open]'); };
    const restore = function () {
      const d = open(); if (d) { d.querySelector('.buttons button').click(); }
      state.accountOrder = state.accountOrder.filter(function (x) { return x !== id; });
      delete state.accounts[id];
      state.selectedAccount = saved.account; state.view = saved.view;
    };
    const err = 'credential required for imap.example.org';

    // Not in view: remembered, no dialog.
    apply({ type: 'accountConnectionChanged', accountId: id, storeKind: 'email', connectionState: 'error', message: err });
    setTimeout(function () {
      const quiet = !open() && fake.lastError === err;
      // In view: the dialog opens, is Google's (the account says so) and gives the reason.
      state.selectedAccount = id; state.view = 'mail';
      apply({ type: 'accountConnectionChanged', accountId: id, storeKind: 'email', connectionState: 'error', message: err });
      setTimeout(function () {
        const dlg = open();
        const title = dlg && dlg.querySelector('h2') && dlg.querySelector('h2').textContent;
        const reason = dlg && dlg.querySelector('.reason') && dlg.querySelector('.reason').textContent;
        const google = title === t('gmailSignInTitle') && !dlg.querySelector('input[type="password"]');
        const says = !!reason && reason.indexOf(err) >= 0;
        dlg.querySelector('.buttons button').click();           // Cancel
        // Declined: a repeat of the same error does not ask again.
        apply({ type: 'accountConnectionChanged', accountId: id, storeKind: 'email', connectionState: 'error', message: err });
        setTimeout(function () {
          const once = !open();
          restore();
          done(quiet && google && says && once, 'quiet ' + quiet + ', google ' + google + ', reason ' + says + ', once ' + once + (title ? ' (' + title + ')' : ''));
        }, 100);
      }, 150);
    }, 100);
  }],
  ['reply and forward: subjects, recipients, threading, quoting and the sanitiser', function (done) {
    const checks = {};
    const src = {
      from: [{ name: 'Ann Example', email: 'ann@example.org' }],
      to: [{ name: 'Me', email: 'me@example.org' }, { email: 'bob@example.org' }, { name: 'Doe, John', email: 'john@example.org' }],
      cc: [{ email: 'ANN@example.org' }, { email: 'bob@example.org' }, { email: 'carol@example.org' }],
      subject: 'Plans', dateMs: Date.UTC(2026, 10, 3, 11, 30), messageId: 'abc@example.org', references: '<r1@x>',
      bodyPlain: 'first line\nsecond line', bodyHtml: null
    };
    const cfg = { replyHeaderTemplate: 'On $date at $time, $sender wrote:', replyDateFormat: 'y-MM-dd', replyTimeFormat: 'HH:mm',
      replyLinePrefix: '> ', replyPlainPosition: 'before_quote', quoteOriginal: true };

    checks.subjects = R.replySubject('Plans') === 'Re: Plans' && R.replySubject('re: Re: Plans') === 're: Re: Plans' &&
      R.replySubject('  ') === '' && R.forwardSubject('Plans') === 'Fwd: Plans' && R.forwardSubject('FWD: x') === 'FWD: x';
    checks.splitting = JSON.stringify(R.splitAddressList('"Doe, John" <j@x.org>, b@x.org ,, "a\\"b, c" <d@x.org>')) ===
      JSON.stringify(['"Doe, John" <j@x.org>', 'b@x.org', '"a\\"b, c" <d@x.org>']);
    checks.format = R.formatAddress({ name: 'Doe, John', email: 'j@x.org' }) === '"Doe, John" <j@x.org>' &&
      R.formatAddress({ name: 'Ann', email: 'a@x.org' }) === 'Ann <a@x.org>' && R.formatAddress({ email: 'a@x.org' }) === 'a@x.org';
    checks.extract = R.extractEmail('Ann <ann@x.org>') === 'ann@x.org' && R.extractEmail('bare ann@x.org here') === 'ann@x.org' && R.extractEmail('none') === '';

    const self = ['me@example.org'];
    const reply = R.replyRecipients('reply', src, self);
    const all = R.replyRecipients('replyAll', src, self);
    const fwd = R.replyRecipients('forward', src, self);
    checks.recipients = reply.to.join() === 'Ann Example <ann@example.org>' && reply.cc.length === 0 &&
      all.cc.join() === 'bob@example.org,"Doe, John" <john@example.org>,carol@example.org' && fwd.to.length === 0 && fwd.cc.length === 0;

    checks.threading = R.threading('reply', src).inReplyTo === '<abc@example.org>' &&
      R.threading('reply', src).references === '<r1@x> <abc@example.org>' &&
      R.threading('forward', src).inReplyTo === null && R.threading('reply', { messageId: '' }).references === null &&
      R.normaliseMessageId(' <x@y> ') === '<x@y>' && R.normaliseMessageId(' x@y> ') === '<x@y>' && R.normaliseMessageId('x@y') === '<x@y>' && R.normaliseMessageId('<>') === '<>';

    const long = 'word '.repeat(40).trim();
    const wrapped = R.prefixedWrappedLines(long, '> ', 80);
    const unbroken = R.prefixedWrappedLines('x'.repeat(200), '> ', 80);
    checks.wrapping = wrapped.length > 2 && wrapped.every(function (l) { return l.length <= 80 && l.startsWith('> '); }) &&
      wrapped.join(' ').replace(/> /g, '').replace(/\s+/g, ' ') === long.replace(/\s+/g, ' ') &&
      unbroken.length === 1 && unbroken[0] === '> ' + 'x'.repeat(200) &&
      R.prefixedWrappedLines('', '> ', 80).join('|') === '> ' && R.prefixedWrappedLines('  ', '> ', 80).join('|') === '> ' &&
      R.prefixedWrappedLines('é'.repeat(100) + ' end', '> ', 80).length === 2;

    const above = R.quotedPlain(src, cfg, 'en');
    const below = R.quotedPlain(src, Object.assign({}, cfg, { replyPlainPosition: 'after_quote' }), 'en');
    checks.quoteLayout = above.startsWith('\n\nOn 2026-11-03 at 11:30, Ann Example <ann@example.org> wrote:\n> first line\n> second line\n\n') &&
      below.startsWith('\nOn 2026-11-03') && below.endsWith('> second line\n\n\n') &&
      R.quotedPlain(Object.assign({}, src, { bodyPlain: '' }), cfg, 'en').endsWith('wrote:\n\n');

    const dirty = '<p onclick="x()" style="color:red">hi <b>bold</b> <a href="javascript:alert(1)">bad</a> <a href="https://e.org/a" target="_blank">ok</a></p>' +
      '<script>alert(1)</script><img src="x" onerror="y()"><table><tr><td>cell</td></tr></table><div class="evil quoted-message-reply" id="i">d</div><div class="quoted-message-reply">q</div>';
    const clean = R.sanitiseRich(dirty);
    checks.sanitiser = clean.indexOf('onclick') < 0 && clean.indexOf('style') < 0 && clean.indexOf('script') < 0 &&
      clean.indexOf('alert') < 0 && clean.indexOf('onerror') < 0 && clean.indexOf('<img') < 0 && clean.indexOf('<table') < 0 &&
      clean.indexOf('javascript') < 0 && clean.indexOf('target') < 0 && clean.indexOf('href="https://e.org/a"') >= 0 &&
      clean.indexOf('<b>bold</b>') >= 0 && clean.indexOf('cell') >= 0 && clean.indexOf('id=') < 0 &&
      clean.split('class="quoted-message-reply"').length === 2;

    const seed = R.quotedRichSeed(Object.assign({}, src, { bodyHtml: '<p>html <i>body</i></p><script>x</script>' }), cfg, 'en');
    checks.richSeed = seed.startsWith('<p><br></p><p><br></p><div class="quoted-message-reply"><p>On 2026-11-03') &&
      seed.indexOf('<blockquote><p>html <i>body</i></p></blockquote>') >= 0 && seed.indexOf('script') < 0 &&
      R.quotedRichSeed(src, Object.assign({}, cfg, { replyPlainPosition: 'after_quote' }), 'en').endsWith('<p><br></p><p><br></p>');

    const rich = '<p>My reply</p>' + R.quotedRichSeed(src, cfg, 'en').replace('<p><br></p><p><br></p>', '');
    const plainAbove = R.plainFromRich(rich, cfg);
    const plainBelow = R.plainFromRich(rich, Object.assign({}, cfg, { replyPlainPosition: 'after_quote' }));
    checks.plainFromRich = plainAbove.startsWith('My reply\n\nOn 2026-11-03 at 11:30, Ann Example <ann@example.org> wrote:\n> first line\n> second line') &&
      plainBelow.startsWith('On 2026-11-03') && plainBelow.endsWith('> second line\n\nMy reply') &&
      R.plainFromRich('<p>a</p><p>b<br>c</p>', cfg) === 'a\nb\nc' && R.containsQuotedBlock(rich) && !R.containsQuotedBlock('<p>x</p>');

    const eml = R.syntheticEml(src);
    checks.eml = eml.startsWith('From: Ann Example <ann@example.org>\r\nTo: ') && eml.indexOf('\r\nDate: Tue, 03 Nov 2026 11:30:00 +0000\r\n') >= 0 &&
      eml.indexOf('\r\n\r\nfirst line') > 0 && eml.indexOf('Message-ID: <abc@example.org>') >= 0;

    const bad = Object.keys(checks).filter(function (k) { return !checks[k]; });
    done(bad.length === 0, bad.length ? 'failed: ' + bad.join(', ') : Object.keys(checks).length + ' groups');
  }],
  ['compose: reply, forward, attachments, completion, a failed send and discard', function (done) {
    const tid = 'tselftestcompose';
    let dir = '';
    const q = function (sel) { return document.querySelector(sel); };
    const step = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 10000, function (ok, why) { next(ok, why); }); };
    };
    const inCompose = function () { return q('#app').classList.contains('view-compose'); };
    const openMessageAndClick = function (action) {
      return function (next) {
        state.view = 'mail';
        q('.rail .account[data-account="s1"]').click();
        waitFor('folder', function () { return q('.folder-row[data-folder="INBOX"]'); }, 5000, function (ok, why) {
          if (!ok) { next(false, why); return; }
          q('.folder-row[data-folder="INBOX"]').click();
          waitFor('rows', function () { return q('.message-row'); }, 8000, function (ok2, why2) {
            if (!ok2) { next(false, why2); return; }
            q('.message-row').click();
            waitFor('actions', function () { return state.message && state.message.opened && q('.msg-actions [data-action="' + action + '"]'); }, 8000, function (ok3, why3) {
              if (!ok3) { next(false, why3); return; }
              q('.msg-actions [data-action="' + action + '"]').click();
              next();
            });
          });
        });
      };
    };
    const set = function (sel, value) { const el = q(sel); el.value = value; el.dispatchEvent(new Event('input')); };
    const cleanupContacts = function (next) {
      call('contacts.load', undefined, function () {
        setTimeout(function () {
          const mine = state.contacts.contacts.filter(function (c) { return /Selftest$/.test(c.displayName); });
          let left = mine.length;
          if (!left) { next(); return; }
          mine.forEach(function (c) { call('contacts.delete', { contactId: c.id }, function () { if (--left === 0) { next(); } }); });
        }, 300);
      });
    };

    sequence([
      function (next) {
        call('app.info', undefined, function (err, info) {
          if (err) { next(false, err.message); return; }
          dir = info.configPath.replace(/[\\/][^\\/]*$/, '');
          next();
        });
      },
      // A transport for the Maildir account, so that it can send (to nothing that listens).
      function (next) {
        call('transport.save', { id: tid, displayName: 'Compose SMTP', host: '127.0.0.1', port: 1, security: 'plain',
          defaultFrom: 'Me Selftest <me.selftest@example.org>' }, function (err) { next(!err, err && err.message); });
      },
      function (next) {
        const a = state.accounts.s1;
        call('account.save', { id: 's1', label: a.label, backendType: 'maildir', attrs: Object.assign({}, a.attrs),
          lists: Object.assign({}, a.lists, { transportIds: [tid] }) }, function (err) { next(!err, err && err.message); });
      },
      step('the account can send', function () { return composeSendKind(state.accounts.s1) === 'smtp'; }),

      // Reply: recipients, subject, the quote, From from the transport.
      openMessageAndClick('reply'),
      step('reply seeded', function () { return inCompose() && /^Re: /.test((q('[data-field="subject"]') || {}).value || '') && /wrote:/.test((q('[data-field="body"]') || {}).value || ''); }),
      function (next) {
        const sender = state.message.headers.from[0].email;
        const body = q('[data-field="body"]').value;
        next(q('[data-field="to"]').value.indexOf(sender) >= 0 && /\n> /.test(body) && q('[data-field="from"]').value === 'Me Selftest <me.selftest@example.org>' &&
          q('[data-field="transport"]').value === tid, 'to ' + q('[data-field="to"]').value + ', from ' + q('[data-field="from"]').value);
      },
      function (next) { q('.compose-bar [data-action="discard"]').click(); next(); },
      step('closed without asking (nothing was typed)', function () { return !inCompose() && !q('dialog.modal[open]'); }),

      // Forward: nobody addressed, Fwd: subject.
      openMessageAndClick('forward'),
      step('forward seeded', function () { return inCompose() && /^Fwd: /.test((q('[data-field="subject"]') || {}).value || ''); }),
      function (next) { next(q('[data-field="to"]').value === '' && q('[data-field="cc"]').value === '', 'to "' + q('[data-field="to"]').value + '"'); },

      // Attachments: a dropped path becomes a card; removing it takes it away.
      function (next) { filesDropped([dir + '/test.vcf']); next(); },
      step('card', function () { return q('.attachment-card .name') && q('.attachment-card .name').textContent === 'test.vcf'; }),
      function (next) {
        const info = state.contacts && true;
        upload('pasted.png', 'image/png', new Blob([new Uint8Array(4096)]), function (err, i) {
          next(!err && i.name === 'pasted.png' && i.mimeType === 'image/png' && i.size === 4096, err ? err.message : 'uploaded ' + i.size);
        });
      },
      function (next) { q('.attachment-card button').click(); next(); },
      step('card removed', function () { return !q('.attachment-card'); }),

      // Completion from the contacts: grey text, Tab takes it.
      function (next) { call('contacts.importVcardFile', { path: dir + '/test.vcf' }, function (err) { next(!err, err && err.message); }); },
      function (next) {
        set('[data-field="to"]', 'alice.s');
        waitFor('ghost', function () { return q('.recipient .hint-suffix') && q('.recipient .hint-suffix').textContent.length > 0; }, 6000, function (ok, why) {
          if (!ok) { next(false, why); return; }
          const input = q('[data-field="to"]');
          input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
          next(input.value === 'Alice Selftest <alice.selftest@example.org>', input.value);
        });
      },
      function (next) { set('[data-field="to"]', q('[data-field="to"]').value + ', "Doe, John" <john@example.org>'); next(); },

      // Sending to a port nobody listens on fails in front of the person, and the message stays.
      function (next) { q('.compose-bar [data-action="send"]').click(); next(); },
      step('the failure is shown', function () { const e = q('.compose-form .error'); return e && e.textContent.length > 0; }, 25000),
      function (next) {
        next(inCompose() && !q('.compose-bar [data-action="send"]').disabled && q('[data-field="subject"]').value !== '',
          q('.compose-form .error').textContent.slice(0, 80));
      },

      // Discarding a message that was changed asks first.
      function (next) { q('.compose-bar [data-action="discard"]').click(); next(); },
      step('asked', function () { return q('dialog.modal[open] .primary'); }),
      function (next) { q('dialog.modal .primary').click(); next(); },
      step('discarded', function () { return !inCompose() && !q('.compose-form'); }),

      // The rich editor keeps the quoted block, marked.
      function (next) { call('settings.set', { key: 'composeUseRichText', value: true }, function (err) { next(!err, err && err.message); }); },
      step('setting saved', function () { return state.config.composeUseRichText === true; }),
      openMessageAndClick('reply'),
      step('rich reply seeded', function () { const a = q('.rich-area'); return inCompose() && a && a.querySelector('div.quoted-message-reply blockquote'); }),
      function (next) {
        const area = q('.rich-area');
        next(!!q('.rich-bar [data-cmd="bold"]') && /wrote:/.test(area.querySelector('div.quoted-message-reply p').textContent),
          area.innerHTML.slice(0, 80));
      },
      function (next) { q('.compose-bar [data-action="discard"]').click(); next(); },
      step('closed', function () { return !inCompose(); }),
      function (next) { call('settings.set', { key: 'composeUseRichText', value: false }, function (err) { next(!err, err && err.message); }); },
      cleanupContacts
    ], function (ok, why) {
      // Whatever happened, leave nothing behind.
      closeCompose();
      call('settings.set', { key: 'composeUseRichText', value: false }, function () {
        call('transport.remove', { id: tid }, function () {
          cleanupContacts(function () { done(ok, ok ? 'reply, forward, attach, complete, failed send, discard, rich quote' : why); });
        });
      });
    });
  }],
  ['an outgoing transport can be added, is saved to disk, and can be removed', function (done) {
    state.view = 'settings';
    document.querySelector('.tabs [data-tab="transports"]').click();
    waitFor('outgoing tab', function () { return document.querySelector('.settings-page .item-main.add'); }, 3000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      document.querySelector('.item-main.add').click();
      waitFor('editor', function () { return document.querySelector('[data-field="host"]'); }, 3000, function (ok2, why2) {
        if (!ok2) { done(false, why2); return; }
        const set = function (field, value) {
          const el = document.querySelector('[data-field="' + field + '"]');
          el.value = value;
          el.dispatchEvent(new Event('input'));
        };
        set('displayName', 'Selftest SMTP');
        set('host', '127.0.0.1');
        set('port', '1');
        set('security', 'plain');
        set('defaultFrom', 'me@example.org');
        document.querySelector('.editor-bar .primary').click();
        waitFor('saved', function () {
          return Object.values(state.transports).some(function (x) { return x.displayName === 'Selftest SMTP'; });
        }, 8000, function (ok3, why3) {
          if (!ok3) { done(false, why3); return; }
          const tr = Object.values(state.transports).find(function (x) { return x.displayName === 'Selftest SMTP'; });
          const fields = tr.host === '127.0.0.1' && tr.port === 1 && tr.security === 'plain' &&
            tr.defaultFrom === 'me@example.org' && tr.transportType === 'smtp';
          // The server is not there, so the save is reported as unverified, not as an error dialog.
          waitFor('verify result', function () { return document.querySelector('.editor .error, .settings-page .error:not(:empty)'); }, 15000, function (ok4) {
            document.querySelector('.editor-bar button').click();
            waitFor('list', function () { return document.querySelector('[data-transport="' + tr.id + '"]'); }, 3000, function (ok5, why5) {
              if (!ok5) { done(false, why5); return; }
              document.querySelector('[data-transport="' + tr.id + '"] .danger').click();
              waitFor('confirm', function () { return document.querySelector('dialog.modal[open]'); }, 3000, function (ok6, why6) {
                if (!ok6) { done(false, why6); return; }
                document.querySelector('dialog.modal .primary').click();
                waitFor('removed', function () { return !state.transports[tr.id]; }, 8000, function (ok7, why7) {
                  done(fields && ok7, ok7 ? 'saved with ' + tr.host + ':' + tr.port + (ok4 ? ', unverified shown' : '') + ', then removed' : why7);
                });
              });
            });
          });
        });
      });
    });
  }],
  ['the compose button is second in the rail, always there, off while composing; the compose bar is Lucide icons', function (done) {
    const q = function (sel) { return document.querySelector(sel); };
    const wait = function (what, pred) {
      return function (next) { waitFor(what, pred, 8000, function (ok, why) { next(ok, why); }); };
    };
    const mask = function (el) {
      const i = el && el.querySelector('.icon');
      return i ? (getComputedStyle(i).maskImage || getComputedStyle(i).webkitMaskImage || '') : '';
    };
    const id = state.accountOrder[0];
    const had = (state.accounts[id].lists || {}).transportIds;
    const setSend = function (on) {
      if (on) { state.transports.lent = { id: 'lent', transportType: 'smtp', displayName: 'Lent', host: 'h', port: 25, security: 'plain' }; } else { delete state.transports.lent; }
      state.accounts[id].lists = Object.assign({}, state.accounts[id].lists, { transportIds: on ? ['lent'] : (had || []) });
      changed();
    };
    sequence([
      function (next) { state.view = 'mail'; state.selectedAccount = id; setSend(true); next(); },
      wait('the compose button', function () { return q('.rail .compose-btn'); }),
      function (next) {
        // After the stores, before the calendar and Settings at the foot; no longer a button in the folder pane.
        const btn = q('.rail .compose-btn');
        const accounts = Array.from(document.querySelectorAll('.rail .account[data-account]'));
        const last = accounts[accounts.length - 1].getBoundingClientRect();
        const b = btn.getBoundingClientRect();
        const cal = q('.rail .calendar-btn').getBoundingClientRect();
        const ok = b.top >= last.bottom - 1 && b.bottom <= cal.top + 1 && !btn.disabled && mask(btn).indexOf('/icons/square-pen.svg') >= 0 &&
          btn.title === t('composeTooltip') && !q('.compose-new') && !btn.textContent.trim();
        next(!!ok, 'button ' + Math.round(b.top) + '-' + Math.round(b.bottom) + ' after ' + Math.round(last.bottom) + ', before ' + Math.round(cal.top) + '; icon ' + mask(btn).slice(-24) + '; disabled ' + btn.disabled + '; folder-pane button ' + !!q('.compose-new'));
      },
      function (next) { q('.rail .compose-btn').click(); next(); },
      wait('the compose view', function () { return q('#app').classList.contains('view-compose') && q('.compose-bar [data-action="send"]'); }),
      function (next) {
        // Off while the compose view is the one showing.
        const btn = q('.rail .compose-btn');
        next(btn.disabled, 'disabled while composing: ' + btn.disabled);
      },
      function (next) {
        // The bar holds icons only, the Lucide ones for what each does.
        const want = { dsn: 'bell', crypto: 'shield', attach: 'paperclip', discard: 'trash-2', send: 'send' };
        const bad = [];
        Object.keys(want).forEach(function (action) {
          const b = q('.compose-bar [data-action="' + action + '"]');
          if (!b) { bad.push(action + ' missing'); return; }
          if (mask(b).indexOf('/icons/' + want[action] + '.svg') < 0) { bad.push(action + ' has ' + (mask(b).slice(-20) || 'no icon')); }
          if (b.textContent.trim() || !b.title || !b.getAttribute('aria-label')) { bad.push(action + ' has text or no name'); }
        });
        const loose = Array.from(document.querySelectorAll('.compose-bar button')).filter(function (b) { return b.textContent.trim() !== ''; }).length;
        next(bad.length === 0 && loose === 0, bad.join('; ') || (loose + ' buttons with text'));
      },
      function (next) {
        // Away from the message and back: the button returns to it.
        state.view = 'mail'; changed(); next();
      },
      wait('the button on again', function () { const b = q('.rail .compose-btn'); return b && !b.disabled; }),
      function (next) {
        const btn = q('.rail .compose-btn');
        const named = btn.title === t('webComposeInProgress');
        btn.click();
        next(named && state.view === 'compose', 'title ' + btn.title + ', view ' + state.view);
      },
      function (next) { q('.compose-bar [data-action="discard"]').click(); next(); },
      wait('back to mail', function () { return state.view === 'mail' && !q('dialog.modal[open]'); }),
      function (next) { setSend(false); next(); },
      wait('the button off with no way to send', function () { const b = q('.rail .compose-btn'); return b && b.disabled; }),
      function (next) {
        const b = q('.rail .compose-btn');
        next(b.title === t('composeNeedTransportTooltip'), 'title ' + b.title);
      }
    ], function (ok, why) { setSend(false); done(ok, ok ? 'compose is second in the rail, off while composing or unable to send; the compose bar is Lucide icons' : why); });
  }],
  ['the folder and message panes can be resized by their handles; the rail cannot, and the widths are kept and limited', function (done) {
    const q = function (sel) { return document.querySelector(sel); };
    const rect = function (sel) { return q(sel).getBoundingClientRect(); };
    const wait = function (what, pred) {
      return function (next) { waitFor(what, pred, 8000, function (ok, why) { next(ok, why); }); };
    };
    const pointer = function (el, type, x) {
      el.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: x, clientY: 100, pointerId: 7, button: 0, isPrimary: true }));
    };
    const drag = function (handle, dx) {
      const r = handle.getBoundingClientRect();
      const x = r.left + r.width / 2;
      pointer(handle, 'pointerdown', x);
      pointer(handle, 'pointermove', x + dx);
      pointer(handle, 'pointerup', x + dx);
    };
    const key = function (handle, k, shift) { handle.dispatchEvent(new KeyboardEvent('keydown', { key: k, shiftKey: !!shift, bubbles: true })); };
    const near = function (a, b, tol) { return Math.abs(a - b) <= (tol === undefined ? 1.5 : tol); };
    const stored = function () { try { return JSON.parse(window.localStorage.getItem('tagliacarte.paneWidths')); } catch (e) { return null; } };
    const forget = function () { try { window.localStorage.removeItem('tagliacarte.paneWidths'); } catch (e) { /* nothing kept */ } };
    let hs = [];
    let f0 = 0;
    let m0 = 0;
    let r0 = 0;
    sequence([
      function (next) { forget(); state.view = 'mail'; changed(); next(); },
      wait('the handles', function () { hs = Array.from(document.querySelectorAll('.pane-handle')); return hs.length === 2 && getComputedStyle(hs[0]).display !== 'none'; }),
      function (next) {
        // One on each border between the three panes, none by the rail.
        const f = rect('.folders');
        const m = rect('.messages');
        const rail = rect('.rail');
        const c = hs.map(function (h) { const r = h.getBoundingClientRect(); return r.left + r.width / 2; });
        f0 = f.width; m0 = m.width; r0 = rect('.reading').width;
        const ok = near(c[0], f.right, 4) && near(c[1], m.right, 4) && c.every(function (x) { return Math.abs(x - rail.right) > 20; }) &&
          hs.every(function (h) { return h.getAttribute('role') === 'separator' && h.getAttribute('aria-label') && h.tabIndex === 0; });
        next(!!ok, 'handles at ' + c.map(Math.round).join() + ' for borders ' + Math.round(f.right) + ',' + Math.round(m.right) + ', rail ' + Math.round(rail.right));
      },
      function (next) {
        drag(hs[0], 50);
        const f = rect('.folders');
        const m = rect('.messages');
        const r = rect('.reading');
        next(near(f.width, f0 + 50) && near(m.width, m0) && near(r.width, r0 - 50) && near(rect('.rail').width, 64),
          'folders ' + Math.round(f.width) + ' (was ' + Math.round(f0) + '), messages ' + Math.round(m.width) + ' (was ' + Math.round(m0) + '), reader ' + Math.round(r.width) + ' (was ' + Math.round(r0) + ')');
      },
      function (next) {
        drag(hs[1], -40);
        const m = rect('.messages');
        const c = hs[1].getBoundingClientRect();
        next(near(m.width, m0 - 40) && near(c.left + c.width / 2, m.right, 4), 'messages ' + Math.round(m.width) + ' (was ' + Math.round(m0) + ')');
      },
      function (next) {
        key(hs[0], 'ArrowLeft');
        const w = rect('.folders').width;
        const kept = stored();
        next(near(w, f0 + 50 - 16) && kept && near(kept.folders, w) && near(kept.messages, m0 - 40), 'folders ' + Math.round(w) + ', kept ' + JSON.stringify(kept));
      },
      function (next) {
        // Never smaller than a pane can be used, nor so large that the reader is squeezed out.
        drag(hs[0], -3000);
        const small = rect('.folders').width;
        drag(hs[1], 5000);
        const r = rect('.reading').width;
        next(small >= 120 && small <= 160 && r >= 300, 'smallest folder pane ' + Math.round(small) + ', reader at most squeezed to ' + Math.round(r));
      },
      function (next) {
        hs[0].dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
        hs[1].dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
        const f = rect('.folders').width;
        const m = rect('.messages').width;
        next(near(f, 260) && near(m, 380), 'after a double click: folders ' + Math.round(f) + ', messages ' + Math.round(m) + ' in a window ' + window.innerWidth + ' wide');
      },
      function (next) {
        // Only the mail panes have them.
        state.view = 'settings'; changed(); next();
      },
      wait('the handles gone', function () { return hs.every(function (h) { return getComputedStyle(h).display === 'none'; }); })
    ], function (ok, why) { forget(); state.view = 'mail'; changed(); done(ok, ok ? 'handles between the three mail panes, none by the rail; dragged, keyed, kept, limited, reset by a double click' : why); });
  }],
  ['copying: an address, the subject and a link from the reader go to the clipboard, and a refusal is reported', function (done) {
    const q = function (sel) { return document.querySelector(sel); };
    const wait = function (what, pred) {
      return function (next) { waitFor(what, pred, 8000, function (ok, why) { next(ok, why); }); };
    };
    const clip = navigator.clipboard;
    const had = Object.prototype.hasOwnProperty.call(clip, 'writeText') ? clip.writeText : null;
    const copied = [];
    let refuse = false;
    clip.writeText = function (text) {
      if (refuse) { return Promise.reject(new Error('NotAllowedError')); }
      copied.push(text);
      return Promise.resolve();
    };
    const toasts = function () { return Array.from(document.querySelectorAll('.toast')).map(function (e) { return e.textContent; }); };
    const sawToast = function (text) { return toasts().some(function (x) { return x === text; }); };
    const restore = function () { if (had) { clip.writeText = had; } else { delete clip.writeText; } };
    let address = null;
    sequence([
      function (next) { state.view = 'mail'; changed(); next(); },
      function (next) { q('.rail .account[data-account="s1"]').click(); next(); },
      wait('the folder', function () { return q('.folder-row[data-folder="INBOX"]'); }),
      function (next) { q('.folder-row[data-folder="INBOX"]').click(); next(); },
      wait('the rows', function () { return q('.message-row'); }),
      function (next) {
        const row = Array.from(document.querySelectorAll('.message-row')).find(function (r) { return r.textContent.indexOf('A file for you') >= 0; });
        if (!row) { next(false, 'no row'); return; }
        row.click();
        next();
      },
      wait('the headers', function () { return state.message && state.message.opened && state.message.headers && state.message.headers.from.length && q('.msg-head .addr'); }),
      function (next) {
        // An address copies as the address alone, not the name that goes with it.
        address = state.message.headers.from[0].email;
        q('.msg-head .addr').click();
        next();
      },
      wait('the address copied', function () { return copied.length === 1; }),
      function (next) {
        next(copied[0] === address && sawToast(t('webCopiedAddress', { address: address })), 'copied ' + JSON.stringify(copied) + ', toasts ' + JSON.stringify(toasts()));
      },
      function (next) {
        const subject = state.message.headers.subject;
        const b = q('.msg-head .copy-subject');
        if (!b || !b.title || !b.getAttribute('aria-label')) { next(false, 'no named copy button by the subject'); return; }
        b.click();
        waitFor('the subject copied', function () { return copied.length === 2; }, 3000, function (ok, why) {
          next(ok && copied[1] === subject && sawToast(t('webCopiedSubject')), ok ? 'copied ' + JSON.stringify(copied[1]) + ' for ' + JSON.stringify(subject) : why);
        });
      },
      function (next) {
        // The offer to open a link also offers to copy it, and stays for Open.
        state.message.link = 'https://example.org/a?b=c';
        changed();
        next();
      },
      wait('the link bar', function () { return q('.linkbar .copy-link'); }),
      function (next) {
        q('.linkbar .copy-link').click();
        waitFor('the link copied', function () { return copied.length === 3; }, 3000, function (ok, why) {
          next(ok && copied[2] === 'https://example.org/a?b=c' && state.message.link === 'https://example.org/a?b=c' && !!q('.linkbar .primary'), ok ? 'copied ' + copied[2] : why);
        });
      },
      function (next) {
        // A refusal by the system is said, not swallowed.
        refuse = true;
        q('.linkbar .copy-link').click();
        waitFor('the refusal', function () { return toasts().some(function (x) { return x.indexOf(t('webCopyFailed', { error: 'NotAllowedError' })) === 0; }); }, 3000, function (ok) {
          state.message.link = null;
          changed();
          next(ok, 'toasts ' + JSON.stringify(toasts()));
        });
      }
    ], function (ok, why) { restore(); done(ok, ok ? 'an address, the subject and a link copy, with a note; a refused copy says so' : why); });
  }],
  ['calendar sources: add a calendar by address with each kind of sign-in, edit it, see a failed sync named, and delete it', function (done) {
    const q = function (sel) { return document.querySelector(sel); };
    const dlg = function () { return q('dialog.modal[open]'); };
    const field = function (key) { return dlg().querySelector('[data-field="' + key + '"]'); };
    const shown = function (key) { const el = field(key); return !!el && !el.closest('label').hidden; };
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 8000, function (ok, why) { next(ok, why); }); };
    };
    const set = function (key, value) {
      const el = field(key);
      el.value = value;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
    };
    const item = function () { return q('.calendar-sources .item[data-source]'); };
    const toasts = function () { return Array.from(document.querySelectorAll('.toast')).map(function (e) { return e.textContent; }); };
    let id = null;
    sequence([
      // Anything left by an earlier half-run goes first.
      function (next) {
        call('calendar.sources', undefined, function () {
          setTimeout(function () {
            const left = (state.calendarSources && state.calendarSources.items || []).slice();
            if (!left.length) { next(); return; }
            let n = left.length;
            left.forEach(function (s) { call('calendar.sourceDelete', { id: s.id }, function () { if (--n === 0) { next(); } }); });
          }, 400);
        });
      },
      function (next) { state.view = 'settings'; state.settingsTab = 'calendars'; changed(); next(); },
      wait('the Calendars tab', function () { return q('.calendar-sources [data-action="add-source"]') && q('[data-tab="calendars"][aria-selected="true"]'); }),
      function (next) { q('.calendar-sources [data-action="add-source"]').click(); next(); },
      wait('the form', function () { return dlg() && field('name'); }),
      function (next) {
        // Only the fields that belong to the way of signing in are shown.
        set('authKind', 'none');
        const none = !shown('username') && !shown('password') && !shown('accountId');
        set('authKind', 'basic');
        const basic = shown('username') && shown('password') && !shown('accountId');
        set('authKind', 'account');
        const account = !shown('username') && !shown('password') && shown('accountId');
        next(none && basic && account, 'none ' + none + ', basic ' + basic + ', account ' + account);
      },
      function (next) {
        // An account has to be chosen, and the form says so instead of closing.
        set('name', 'Selftest cal');
        set('url', 'http://127.0.0.1:1/cal/');
        set('authKind', 'account');
        dlg().querySelector('.primary').click();
        waitFor('the refusal', function () { return dlg() && dlg().querySelector('.error') && dlg().querySelector('.error').textContent.trim() !== ''; }, 4000, function (ok, why) {
          next(ok, ok ? '' : why);
        });
      },
      function (next) {
        set('authKind', 'basic');
        set('username', 'alice');
        set('password', 'pw-selftest');
        dlg().querySelector('.primary').click();
        next();
      },
      wait('the source in the list', function () { return item() && !dlg(); }),
      function (next) {
        const s = state.calendarSources.items[0];
        id = s.id;
        const text = item().textContent;
        next(state.calendarSources.items.length === 1 && s.name === 'Selftest cal' && s.url === 'http://127.0.0.1:1/cal/' && s.authKind === 'basic' &&
          s.username === 'alice' && s.hasPassword === true && s.enabled === true && text.indexOf('Selftest cal') >= 0 && text.indexOf('alice') >= 0 && text.indexOf('pw-selftest') < 0,
          JSON.stringify(s) + ' / ' + text.slice(0, 80));
      },
      // Edit: the form is filled in, a blank password keeps the saved one.
      function (next) { item().querySelector('[data-action="edit-source"]').click(); next(); },
      wait('the edit form', function () { return dlg() && field('name') && field('name').value === 'Selftest cal'; }),
      function (next) {
        const ok = field('url').value === 'http://127.0.0.1:1/cal/' && field('authKind').value === 'basic' && field('username').value === 'alice' && field('password').value === '';
        set('name', 'Renamed cal');
        dlg().querySelector('.primary').click();
        next(ok, 'authKind ' + field('authKind').value + ', username ' + field('username').value);
      },
      wait('renamed', function () { return !dlg() && state.calendarSources.items.length === 1 && state.calendarSources.items[0].name === 'Renamed cal'; }),
      function (next) {
        const s = state.calendarSources.items[0];
        next(s.id === id && s.hasPassword === true, 'same source ' + (s.id === id) + ', password kept ' + s.hasPassword);
      },
      // A sync that cannot reach it names it.
      function (next) { q('.calendar-sources [data-action="sync-sources"]').click(); next(); },
      wait('the failure', function () { return toasts().some(function (x) { return x.indexOf('Renamed cal') >= 0; }); }, 15000),
      // Delete takes the calendar with it.
      function (next) { item().querySelector('[data-action="delete-source"]').click(); next(); },
      wait('the confirmation', function () { return dlg() && dlg().querySelector('.primary'); }),
      function (next) { dlg().querySelector('.primary').click(); next(); },
      wait('gone', function () { return !dlg() && !item() && state.calendarSources.items.length === 0; }),
      function (next) {
        call('calendar.load', undefined, function () {
          setTimeout(function () {
            const left = state.calendar.calendars.filter(function (c) { return c.id.indexOf('caldav:') === 0; });
            next(left.length === 0, 'calendars left: ' + left.map(function (c) { return c.id; }).join());
          }, 400);
        });
      }
    ], function (ok, why) {
      if (id) { call('calendar.sourceDelete', { id: id }, function () {}); }
      state.view = 'mail'; changed();
      done(ok, ok ? 'a calendar source is added, edited, synced (a failure is named) and deleted with its calendar' : why);
    });
  }],
  ['an account is a coloured disc in the rail, the same one Settings shows', function (done) {
    const first = state.accountOrder[0];
    const acc = state.accounts[first];
    const button = function () { return document.querySelector('.rail .account[data-account="' + first + '"]'); };
    const disc = button() && button().querySelector('.avatar');
    if (!disc) { done(false, 'no avatar in the rail button'); return; }
    const cs = getComputedStyle(button());
    const rail = { text: disc.textContent, hue: disc.style.getPropertyValue('--hue') };
    const ok = rail.text === initials(acc.label, (acc.attrs || {}).email) && rail.hue === String(hueOf(first)) &&
      cs.borderTopLeftRadius.indexOf('%') > 0 || parseFloat(cs.borderTopLeftRadius) >= 20;
    const plain = getComputedStyle(disc).backgroundColor !== getComputedStyle(button()).backgroundColor &&
      getComputedStyle(disc).backgroundColor !== 'rgba(0, 0, 0, 0)';
    if (!ok || !plain) { done(false, 'rail disc "' + rail.text + '" hue ' + rail.hue + ', radius ' + cs.borderTopLeftRadius + ', coloured ' + plain); return; }
    document.querySelector('.settings-btn').click();
    waitFor('the tabs', function () { return document.querySelector('.tabs [data-tab="accounts"]'); }, 5000, function (tabs, why0) {
     if (!tabs) { done(false, why0); return; }
     document.querySelector('.tabs [data-tab="accounts"]').click();
     waitFor('settings list', function () { return document.querySelector('.item-line .avatar'); }, 5000, function (found, why) {
      if (!found) { done(false, why); return; }
      const inList = document.querySelector('.item-line .avatar');
      const same = inList.textContent === rail.text && inList.style.getPropertyValue('--hue') === rail.hue;
      state.view = 'mail';
      changed();
      done(same, 'rail "' + rail.text + '" hue ' + rail.hue + '; Settings "' + inList.textContent + '" hue ' + inList.style.getPropertyValue('--hue'));
     });
    });
  }],
  ['a Matrix account asks for its picture before one is saved; other accounts wait to be given one', function (done) {
    const make = function (id, type, avatarUrl) {
      return accountAvatar({ id: id, label: 'Test ' + id, backendType: type, attrs: {}, avatarUrl: avatarUrl }, 'selftest-' + id, 40);
    };
    const matrix = make('mx1', 'matrix', '');
    const imap = make('im1', 'imap', '');
    const chosen = make('im2', 'imap', '/some/where.png');
    const src = function (el) { const i = el.querySelector('img.avatar-img'); return i ? i.getAttribute('src') : null; };
    const ok = /^\/avatar\/mx1\?v=/.test(src(matrix) || '') && src(imap) === null && /^\/avatar\/im2\?v=/.test(src(chosen) || '') &&
      matrix.textContent.length > 0;
    done(ok, 'matrix ' + src(matrix) + '; imap ' + src(imap) + '; chosen ' + src(chosen));
  }],
  ['an account\'s own image replaces the disc, is served by the back end, saved, and cleared', function (done) {
    let dir = '';
    const id = state.accountOrder[0];
    const existing = function () { return state.accounts[id]; };
    const rail = function () { return document.querySelector('.rail .account[data-account="' + id + '"]'); };
    const save = function (picture, next) {
      const d = S.initialDraft('maildir', existing());
      d.avatarUrl = picture;
      call('account.save', S.toAccount('maildir', d, existing(), id), function (err) { next(!err, err && err.message); });
    };
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 5000, function (ok, why) { next(ok, why); }); };
    };
    const pathBefore = (existing().attrs || {}).path;
    sequence([
      function (next) {
        call('app.info', undefined, function (err, info) {
          if (err) { next(false, err.message); return; }
          dir = info.configPath.replace(/[\\/][^\\/]*$/, '');
          next();
        });
      },
      function (next) { save(dir + '/avatar.png', next); },
      wait('the picture reached the page', function () { return existing().avatarUrl === dir + '/avatar.png'; }),
      wait('an image in the rail', function () { return rail() && rail().querySelector('img.avatar-img'); }),
      function (next) {
        const img = rail().querySelector('img.avatar-img');
        const src = img.getAttribute('src');
        if (!/^\/avatar\/[^?]+\?v=/.test(src)) { next(false, 'image source is ' + src); return; }
        fetch(src).then(function (r) {
          return r.arrayBuffer().then(function (b) {
            const sig = Array.from(new Uint8Array(b).slice(0, 4)).join(',');
            next(r.status === 200 && /^image\/png/.test(r.headers.get('Content-Type') || '') && sig === '137,80,78,71',
              'served ' + r.status + ' ' + r.headers.get('Content-Type') + ' ' + b.byteLength + ' bytes');
          });
        });
      },
      // It is in config.xml, not just in the page: read it back from disk.
      function (next) {
        call('config.load', undefined, function () {
          setTimeout(function () { next(existing().avatarUrl === dir + '/avatar.png' && (existing().attrs || {}).path === pathBefore, 'after reload: ' + existing().avatarUrl); }, 400);
        });
      },
      // A file that is not an image leaves the disc showing.
      function (next) { save(dir + '/test.vcf', next); },
      wait('the failed image is dropped', function () { return rail() && rail().querySelector('.avatar') && !rail().querySelector('img.avatar-img') && existing().avatarUrl === dir + '/test.vcf'; }, 8000),
      // And an empty field clears it.
      function (next) { save('', next); },
      wait('the picture cleared', function () { return existing().avatarUrl === '' && !rail().querySelector('img.avatar-img'); }),
      function (next) {
        call('config.load', undefined, function () {
          setTimeout(function () { next(existing().avatarUrl === '' && (existing().attrs || {}).path === pathBefore, 'after reload: "' + existing().avatarUrl + '"'); }, 400);
        });
      }
    ], function (ok, why) { done(ok, ok ? 'served as a PNG, kept across a reload from disk, dropped when it is not an image, cleared by an empty field' : why); });
  }],
  ['where the user was is saved as they move and returned to at start: folder, then message', function (done) {
    const id = state.accountOrder[0];
    const acc = function () { return state.accounts[id]; };
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 8000, function (ok, why) { next(ok, why); }); };
    };
    const openInbox = function (next) {
      const row = document.querySelector('.folder-row[data-folder="INBOX"]');
      if (!row) { next(false, 'no INBOX row'); return; }
      row.click();
      waitFor('the list', function () { return state.list && state.list.ready && document.querySelectorAll('.message-row').length > 1; }, 10000, function (ok, why) { next(ok, why); });
    };
    // As if the window had just opened: nothing selected, the configuration read from disk.
    const startAgain = function (next) {
      allowRestore(true);
      state.selectedAccount = null;
      state.selectedFolder = null;
      state.selectedMessage = null;
      state.list = null;
      state.message = null;
      state.view = 'mail';
      changed();
      call('config.load', undefined, function (err) { next(!err, err && err.message); });
    };
    // The arguments are worked out when the step runs, not when the list of steps is made.
    const saveAs = function (args) {
      return function (next) { call('location.save', typeof args === 'function' ? args() : args, function (err) { next(!err, err && err.message); }); };
    };
    let want = null;
    sequence([
      // Ordinary clicks, then a pause: they are in config.xml.
      function (next) { state.view = 'mail'; changed(); next(); },
      openInbox,
      function (next) {
        want = state.list.slots.filter(Boolean)[1];
        document.querySelector('.message-row[data-id="' + want.id.replace(/"/g, '\\"') + '"]').click();
        next();
      },
      wait('the message to open', function () { return state.message && state.message.opened; }),
      function (next) { setTimeout(function () { call('config.load', undefined, function () { setTimeout(next, 300); }); }, 600); },
      function (next) {
        next(acc().lastFolder === 'INBOX' && acc().lastMessageId === want.id,
          'saved after clicking: folder ' + acc().lastFolder + ', message ' + acc().lastMessageId + ' (wanted ' + want.id + ')');
      },
      // A start: the same folder, the same message, highlighted in the list.
      startAgain,
      wait('the folder to open again', function () { return state.selectedFolder === 'INBOX' && state.list && state.list.ready; }),
      wait('the message to open again', function () { return state.message && state.message.id === want.id && state.message.opened && state.message.headers; }),
      function (next) {
        const row = document.querySelector('.message-row[aria-current="true"]');
        next(!!row && row.getAttribute('data-id') === want.id && state.selectedAccount === id, 'returned to INBOX and "' + (state.message.headers.subject || '') + '"');
      },
      // The message was deleted meanwhile: the folder opens, nothing is said about it.
      saveAs({ accountId: id, folder: 'INBOX', messageId: 'maildir://INBOX/gone.1.local' }),
      startAgain,
      wait('the folder without the message', function () { return state.selectedFolder === 'INBOX' && state.list && state.list.ready && !state.message; }, 10000),
      function (next) { setTimeout(function () { next(!state.message && !document.querySelector('.reading .notice') && !state.error, 'a missing message is dropped quietly'); }, 700); },
      function (next) { setTimeout(function () { call('config.load', undefined, function () { setTimeout(next, 300); }); }, 500); },
      function (next) { next(acc().lastMessageId === null && acc().lastFolder === 'INBOX', 'the stale message is forgotten: ' + acc().lastMessageId); },
      // The folder was removed meanwhile: nothing opens.
      saveAs({ accountId: id, folder: 'Removed folder' }),
      startAgain,
      function (next) { setTimeout(function () { next(state.selectedAccount === id && state.selectedFolder === null && state.list === null && !state.error, 'a missing folder is skipped'); }, 1200); },
      // The user's own first move is theirs.
      saveAs(function () { return { accountId: id, folder: 'INBOX', messageId: want.id }; }),
      // (The plan is made and then cancelled before the page can act on it, as a click would.)
      function (next) {
        allowRestore(true);
        state.selectedAccount = id; state.selectedFolder = null; state.selectedMessage = null; state.list = null; state.message = null;
        beginRestore(id);
        abandonRestore();
        restoreStep();
        setTimeout(next, 500);
      },
      function (next) { next(state.selectedFolder === null && state.list === null && !state.message, 'cancelled: nothing was opened'); },
      // Back to how the other tests found things.
      function (next) { allowRestore(false); flushLocation(); openInbox(next); }
    ], function (ok, why) { done(ok, ok ? 'saved after a pause, returned to the folder and message, a deleted message and a removed folder skipped quietly' : why); });
  }],
  ['the calendar button at the foot of the rail opens the calendar, which replaces the mail panes', function (done) {
    const rail = document.querySelector('.rail').getBoundingClientRect();
    const btn = document.querySelector('.rail .calendar-btn');
    const settings = document.querySelector('.rail .settings-btn');
    if (!btn || !settings) { done(false, 'no calendar button'); return; }
    const b = btn.getBoundingClientRect();
    const sb = settings.getBoundingClientRect();
    // Calendar, then Settings, at the bottom of the rail, as in the Flutter interface.
    if (!(b.bottom <= sb.top + 1 && sb.bottom <= rail.bottom + 1 && rail.bottom - sb.bottom < 40 && b.top > rail.top + rail.height / 2)) {
      done(false, 'calendar ' + Math.round(b.top) + '-' + Math.round(b.bottom) + ', settings ' + Math.round(sb.top) + '-' + Math.round(sb.bottom) + ' of ' + Math.round(rail.bottom)); return;
    }
    const icon = getComputedStyle(btn.querySelector('.icon')).maskImage || '';
    if (icon.indexOf('/icons/calendar.svg') < 0 || !btn.title || btn.title !== t('calendarTitle')) { done(false, 'icon ' + icon + ', title ' + btn.title); return; }
    btn.click();
    waitFor('the calendar', function () { return state.view === 'calendar' && document.querySelector('.calendar .cal-title'); }, 8000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      const shell = document.getElementById('app');
      const hidden = ['.folders', '.messages', '.reading', '.settings'].every(function (sel) { return getComputedStyle(document.querySelector(sel)).display === 'none'; });
      const shown = getComputedStyle(document.querySelector('.calendar')).display !== 'none' && document.querySelector('.calendar').getBoundingClientRect().width > 300;
      waitFor('the window title', function () { return document.title === t('calendarTitle') + ' - Tagliacarte'; }, 3000, function (ok2) {
        // The rail is drawn again on a change, so the button is looked for again.
        const pressed = document.querySelector('.rail .calendar-btn').getAttribute('aria-pressed') === 'true';
        const marked = shell.classList.contains('view-calendar');
        // An account in the rail goes back to mail.
        document.querySelector('.rail .account[data-account]').click();
        const back = state.view === 'mail';
        const ok = hidden && shown && ok2 && pressed && marked && back;
        done(ok, ok ? 'Lucide calendar button above Settings; the calendar replaces the mail panes and an account returns to mail' : 'hidden ' + hidden + ', shown ' + shown + ', title ' + ok2 + ', pressed ' + pressed + ', marked ' + marked + ', back ' + back);
      });
    });
  }],
  ['calendar: make, see, change and delete events, one all-day, and hide a calendar', function (done) {
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 8000, function (ok, why) { next(ok, why); }); };
    };
    const dlg = function () { return document.querySelector('dialog.modal[open]'); };
    const field = function (name) { return dlg().querySelector('[data-field="' + name + '"]'); };
    const press = function (sel) { dlg().querySelector(sel).click(); };
    const cell = function (day) { return document.querySelector('.cal-cell[data-day="' + day + '"]'); };
    const chipsOn = function (day) { return cell(day) ? Array.from(cell(day).querySelectorAll('.cal-chip')).map(function (c) { return c.textContent; }) : null; };
    const buttonByText = function (text) { return Array.from(dlg().querySelectorAll('.buttons button')).find(function (b) { return b.textContent === text; }); };
    let created = null;
    sequence([
      function (next) {
        // June 2026, as a month.
        state.calendar.mode = 'month';
        state.calendar.anchor = new Date(2026, 5, 1);
        document.querySelector('.rail .calendar-btn').click();
        next();
      },
      wait('the month', function () { return document.querySelector('.cal-grid') && document.querySelector('.cal-title').textContent.indexOf('2026') >= 0; }),
      wait('the calendars', function () { return state.calendar.calendars.some(function (c) { return c.id === 'local'; }) && document.querySelector('.cal-toggle[data-calendar="local"]'); }),
      // Whatever an earlier run that stopped half way left behind is taken away first.
      function (next) {
        const left = state.calendar.events.filter(function (e) { return /^Selftest/.test(e.title); });
        let pending = left.length;
        if (!pending) { next(); return; }
        left.forEach(function (e) {
          call('calendar.delete', { eventId: e.id }, function () {
            if (--pending === 0) { document.querySelector('.cal-mode[data-mode="month"]').click(); setTimeout(next, 400); }
          });
        });
      },
      wait('a clean month', function () { return !state.calendar.events.some(function (e) { return /^Selftest/.test(e.title); }) && !state.calendar.loading; }),
      function (next) {
        const chip = document.querySelector('.cal-toggle[data-calendar="local"]');
        next(chip.textContent === t('webCalendarLocal') && chip.getAttribute('aria-pressed') === 'true', 'the Local calendar is always there: "' + chip.textContent + '"');
      },
      // A new timed event, from the Add button.
      function (next) { document.querySelector('.cal-add').click(); next(); },
      wait('the editor', function () { return dlg() && field('title'); }),
      function (next) {
        field('title').value = 'Selftest event';
        field('starts').value = '2026-06-10T09:00';
        field('ends').value = '2026-06-10T10:30';
        field('location').value = 'Room 9';
        field('invitees').value = 'a@example.org, b@example.org';
        press('.primary');
        next();
      },
      wait('the event on the 10th', function () { return (chipsOn('2026-06-10') || []).indexOf('Selftest event') >= 0; }),
      function (next) {
        created = state.calendar.events.find(function (e) { return e.title === 'Selftest event'; });
        const ok = created && created.location === 'Room 9' && created.attendees.length === 2 && !created.allDay && created.endMs - created.startMs === 90 * 60000 && /^local:tc-/.test(created.id);
        next(!!ok, 'created ' + JSON.stringify(created && { id: created.id, a: created.attendees, loc: created.location }));
      },
      // An empty title is refused in the editor, and nothing is made.
      function (next) { document.querySelector('.cal-add').click(); next(); },
      wait('the editor again', function () { return dlg() && field('title'); }),
      function (next) {
        press('.primary');
        const said = dlg().querySelector('.error').textContent;
        press('.buttons button');
        next(said === t('webCalendarNeedTitle'), 'refused: "' + said + '"');
      },
      wait('the editor closed', function () { return !dlg(); }),
      // Change it: its day opens a list, and the pen opens the editor with what it holds.
      function (next) { cell('2026-06-10').click(); next(); },
      wait('the day', function () { return dlg() && dlg().querySelector('.cal-event'); }),
      function (next) { press('.cal-edit'); next(); },
      wait('the editor with the event', function () { return dlg() && field('title') && field('title').value === 'Selftest event'; }),
      function (next) {
        const ok = field('location').value === 'Room 9' && field('invitees').value === 'a@example.org, b@example.org' && field('starts').value === '2026-06-10T09:00' && field('ends').value === '2026-06-10T10:30' && field('starts').type === 'datetime-local' && !field('allDay').checked;
        if (!ok) { next(false, 'editor held ' + field('starts').value + ' ' + field('ends').value + ' ' + field('location').value); return; }
        field('title').value = 'Selftest moved';
        field('starts').value = '2026-06-11T09:00';
        field('ends').value = '2026-06-11T10:30';
        press('.primary');
        next();
      },
      wait('the event moved to the 11th', function () { return (chipsOn('2026-06-11') || []).indexOf('Selftest moved') >= 0 && (chipsOn('2026-06-10') || []).indexOf('Selftest event') < 0; }),
      function (next) {
        const e = state.calendar.events.find(function (x) { return x.title === 'Selftest moved'; });
        next(e && e.id === created.id && e.sequence === created.sequence + 1, 'the same event, one step on: sequence ' + (e && e.sequence));
      },
      // An all-day event over two days, 12th and 13th, whatever zone this is.
      function (next) { document.querySelector('.cal-add').click(); next(); },
      wait('the editor for the holiday', function () { return dlg() && field('title'); }),
      function (next) {
        field('title').value = 'Selftest holiday';
        field('allDay').checked = true;
        field('allDay').dispatchEvent(new Event('change'));
        next(field('starts').type === 'date' && field('ends').type === 'date', 'the fields become dates: ' + field('starts').type);
      },
      function (next) {
        field('starts').value = '2026-06-12';
        field('ends').value = '2026-06-13';
        press('.primary');
        next();
      },
      wait('the holiday on the 12th and 13th', function () { return (chipsOn('2026-06-12') || []).indexOf('Selftest holiday') >= 0 && (chipsOn('2026-06-13') || []).indexOf('Selftest holiday') >= 0; }),
      function (next) {
        next((chipsOn('2026-06-11') || []).indexOf('Selftest holiday') < 0 && (chipsOn('2026-06-14') || []).indexOf('Selftest holiday') < 0, 'the 11th and 14th are free');
      },
      // Hiding the calendar hides its events, without asking the back end again.
      function (next) {
        const asked = state.calendar.request;
        document.querySelector('.cal-toggle[data-calendar="local"]').click();
        waitFor('hidden', function () { return (chipsOn('2026-06-11') || []).length === 0; }, 3000, function (ok, why) {
          const same = state.calendar.request === asked;
          document.querySelector('.cal-toggle[data-calendar="local"]').click();
          waitFor('shown', function () { return (chipsOn('2026-06-11') || []).indexOf('Selftest moved') >= 0; }, 3000, function (ok2, why2) {
            next(ok && ok2 && same, ok ? 'hidden and shown again, with no new request: ' + same : why);
          });
        });
      },
      // Delete both, through the interface.
      function (next) { cell('2026-06-11').click(); next(); },
      wait('the day list', function () { return dlg() && dlg().querySelector('.cal-event'); }),
      function (next) { press('.cal-delete'); next(); },
      wait('the confirmation', function () { return dlg() && dlg().querySelector('.buttons .danger'); }),
      function (next) { press('.buttons .danger'); next(); },
      wait('the event gone', function () { return (chipsOn('2026-06-11') || []).indexOf('Selftest moved') < 0 && !dlg(); }),
      function (next) { cell('2026-06-12').click(); next(); },
      wait('the holiday list', function () { return dlg() && dlg().querySelector('.cal-event'); }),
      function (next) { press('.cal-edit'); next(); },
      wait('the holiday editor', function () { return dlg() && field('title') && field('title').value === 'Selftest holiday' && field('allDay').checked; }),
      function (next) {
        next(field('starts').value === '2026-06-12' && field('ends').value === '2026-06-13' && field('starts').type === 'date', 'the all-day editor shows its first and last day: ' + field('starts').value + ' to ' + field('ends').value);
      },
      function (next) { press('.buttons .danger'); next(); },
      wait('the delete confirmation', function () { return dlg() && dlg().querySelector('.buttons .danger') && dlg().textContent.indexOf('Selftest holiday') >= 0; }),
      function (next) { press('.buttons .danger'); next(); },
      wait('the holiday gone', function () { return (chipsOn('2026-06-12') || []).indexOf('Selftest holiday') < 0 && !dlg(); }),
      function (next) { state.view = 'mail'; changed(); next(); }
    ], function (ok, why) { done(ok, ok ? 'made, seen, changed, an all-day one on its own dates, hidden and shown, deleted' : why); });
  }],
  ['calendar: the four scales, moving, today, and the viewer\'s choice is kept', function (done) {
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 6000, function (ok, why) { next(ok, why); }); };
    };
    const title = function () { return document.querySelector('.cal-title').textContent; };
    const mode = function (m) { return function (next) { document.querySelector('.cal-mode[data-mode="' + m + '"]').click(); next(); }; };
    let monthTitle = '';
    sequence([
      function (next) {
        state.calendar.mode = 'month';
        state.calendar.anchor = new Date(2026, 5, 15);
        document.querySelector('.rail .calendar-btn').click();
        next();
      },
      wait('June 2026', function () { return document.querySelector('.cal-grid') && /2026/.test(title()); }),
      function (next) {
        monthTitle = title();
        const heads = Array.from(document.querySelectorAll('.cal-head')).map(function (e) { return e.textContent; });
        next(document.querySelectorAll('.cal-cell').length === 42 && heads.length === 7 && document.querySelectorAll('.cal-cell.today').length <= 1, '42 days under 7 headings: ' + heads.join(','));
      },
      function (next) { document.querySelector('.cal-next').click(); next(); },
      wait('the next month', function () { return title() !== monthTitle && /2026/.test(title()); }),
      function (next) { document.querySelector('.cal-prev').click(); document.querySelector('.cal-prev').click(); next(); },
      wait('two months back', function () { return title() !== monthTitle; }),
      function (next) { document.querySelector('.cal-today').click(); next(); },
      wait('this month', function () { return document.querySelector('.cal-cell.today') || title() === new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' }).format(new Date()); }),
      mode('week'),
      wait('a week', function () { return document.querySelectorAll('.cal-weekday').length === 7; }),
      mode('day'),
      wait('a day', function () { return !document.querySelector('.cal-weekday') && !document.querySelector('.cal-grid') && document.querySelector('.cal-body'); }),
      function (next) { next(document.querySelector('.cal-mode[data-mode="day"]').getAttribute('aria-pressed') === 'true', 'the day is the pressed scale'); },
      mode('year'),
      wait('a year', function () { return document.querySelectorAll('.cal-year-card').length === 12; }),
      function (next) {
        // The choice is kept for next time.
        let kept = null;
        try { kept = JSON.parse(window.localStorage.getItem('tagliacarte.calendarMode')); } catch (e) { kept = 'unreadable'; }
        next(kept === 'year', 'kept: ' + kept);
      },
      // A month of the year opens as a month.
      function (next) { document.querySelector('.cal-year-card[data-month="3"]').click(); next(); },
      wait('March as a month', function () { return document.querySelector('.cal-grid') && state.calendar.anchor.getMonth() === 2; }),
      function (next) { state.calendar.mode = 'month'; state.view = 'mail'; changed(); next(); }
    ], function (ok, why) { done(ok, ok ? 'month grid, week, day, year; previous, next and today; the scale is kept' : why); });
  }],
  ['calendar: syncing with nothing to sync says so, and an invitation in mail is added to the calendar', function (done) {
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 8000, function (ok, why) { next(ok, why); }); };
    };
    const dlg = function () { return document.querySelector('dialog.modal[open]'); };
    const toasts = function () { return Array.from(document.querySelectorAll('.toast')).map(function (x) { return x.textContent; }).join('|'); };
    const eventId = 'inbox:invite-qr-1@example.org';
    sequence([
      function (next) {
        state.calendar.mode = 'month';
        state.calendar.anchor = new Date(2026, 5, 1);
        document.querySelector('.rail .calendar-btn').click();
        next();
      },
      wait('the calendar', function () { return document.querySelector('.cal-sync'); }),
      function (next) { document.querySelector('.cal-sync').click(); next(); },
      wait('the sync result', function () { return toasts().indexOf(t('calendarSyncComplete', { calendarCount: 0, eventCount: 0 })) >= 0 && !document.querySelector('.cal-sync.busy'); }, 12000),
      // The invitation message: a button beside the attachment.
      function (next) { call('calendar.delete', { eventId: eventId }, function () { next(); }); },
      function (next) { state.view = 'mail'; changed(); next(); },
      function (next) {
        const row = Array.from(document.querySelectorAll('.message-row')).find(function (r) { return r.textContent.indexOf('Meeting invitation') >= 0; });
        if (!row) { next(false, 'no invitation message'); return; }
        row.click();
        next();
      },
      wait('the invitation', function () { return state.message && state.message.opened && state.message.attachments.some(function (a) { return a.invite; }) && document.querySelector('.add-to-calendar'); }),
      function (next) {
        const plain = document.querySelectorAll('.attachment:not(.add-to-calendar)').length;
        const icon = getComputedStyle(document.querySelector('.add-to-calendar .icon')).maskImage || '';
        next(plain === 1 && icon.indexOf('/icons/calendar.svg') >= 0, 'a save button and a calendar button for the one attachment');
      },
      function (next) { document.querySelector('.add-to-calendar').click(); next(); },
      wait('the question', function () { return dlg() && dlg().textContent.indexOf(t('calendarInviteImportPrompt')) >= 0; }),
      function (next) { dlg().querySelector('.primary').click(); next(); },
      wait('the result', function () { return toasts().indexOf(t('calendarInviteImportDone', { count: 1 })) >= 0; }),
      // It is in the calendar, on 2 March 2026, in the calendar for invitations.
      function (next) {
        state.calendar.anchor = new Date(2026, 2, 1);
        document.querySelector('.rail .calendar-btn').click();
        next();
      },
      wait('March with the meeting', function () {
        const c = document.querySelector('.cal-cell[data-day="2026-03-02"]');
        return c && Array.from(c.querySelectorAll('.cal-chip')).some(function (x) { return x.textContent === 'Quarterly review'; });
      }),
      function (next) {
        const e = state.calendar.events.find(function (x) { return x.id === eventId; });
        const cal = state.calendar.calendars.find(function (c) { return c.id === 'inbox'; });
        next(!!e && !!cal && e.location === 'Room 4' && e.organizer && e.endMs - e.startMs === 3600000 && e.startMs === Date.UTC(2026, 2, 2, 14), 'in the invitations calendar, 14:00 UTC, in Room 4');
      },
      // Taken out again, so that the test can be run again.
      function (next) { call('calendar.delete', { eventId: eventId }, function () { state.view = 'mail'; changed(); next(); }); }
    ], function (ok, why) { done(ok, ok ? 'sync with nothing to sync reports zero; an invitation is added from the message and shows on its day' : why); });
  }],
  ['contacts: import, groups, repositories and a failing CardDAV pull', function (done) {
    let dir = '';
    const set = function (f, v) { const el = document.querySelector('dialog.modal [data-field="' + f + '"], dialog.modal input'); el.value = v; };
    const modalField = function (n) { return document.querySelectorAll('dialog.modal input')[n]; };
    const submit = function () { document.querySelector('dialog.modal .primary').click(); };
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 8000, function (ok, why) { next(ok, why); }); };
    };
    let groupName = 'Selftest group';
    // Removes what this test makes, so that it can be rerun and leaves nothing behind.
    const cleanup = function (next) {
      call('contacts.load', undefined, function () {
        setTimeout(function () {
          const groups = state.contacts.groups.filter(function (x) { return x.name === groupName; });
          const repos = state.contacts.repositories.filter(function (r) { return r.name === 'Selftest DAV'; });
          const mine = state.contacts.contacts.filter(function (c) { return /Selftest$/.test(c.displayName); });
          const jobs = [].concat(
            groups.map(function (g) { return ['contacts.groupDelete', { id: g.id }]; }),
            repos.map(function (r) { return ['contacts.repoDelete', { id: r.id }]; }),
            mine.map(function (c) { return ['contacts.delete', { contactId: c.id }]; }));
          let left = jobs.length;
          if (left === 0) { next(); return; }
          jobs.forEach(function (j) {
            call(j[0], j[1], function () { if (--left === 0) { call('contacts.load', undefined, function () { next(); }); } });
          });
        }, 300);
      });
    };
    sequence([
      function (next) {
        call('app.info', undefined, function (err, info) {
          if (err) { next(false, err.message); return; }
          dir = info.configPath.replace(/[\\/][^\\/]*$/, '');
          next();
        });
      },
      function (next) { state.view = 'settings'; document.querySelector('.tabs [data-tab="contacts"]').click(); next(); },
      wait('contacts loaded', function () { return state.contacts.loaded && document.querySelector('.contacts h3'); }),
      cleanup,
      wait('clean start', function () { return !state.contacts.contacts.some(function (c) { return /Selftest$/.test(c.displayName); }); }),
      function (next) {
        call('contacts.importVcardFile', { path: dir + '/test.vcf' }, function (err, r) {
          next(!err && r.imported === 2, err ? err.message : 'imported ' + (r && r.imported));
        });
      },
      function (next) { call('contacts.load', undefined, function () { next(); }); },
      wait('two contacts', function () {
        return state.contacts.contacts.filter(function (c) { return /Selftest$/.test(c.displayName); }).length === 2;
      }),
      // A group through the real dialog.
      function (next) {
        Array.from(document.querySelectorAll('.contacts .row-actions button')).find(function (b) { return /^\+/.test(b.textContent); }).click();
        waitFor('dialog', function () { return document.querySelector('dialog.modal[open] input'); }, 3000, function (ok, why) {
          if (!ok) { next(false, why); return; }
          modalField(0).value = groupName;
          submit();
          next();
        });
      },
      wait('group listed', function () { return state.contacts.groups.some(function (g) { return g.name === groupName; }); }),
      function (next) {
        const g = state.contacts.groups.find(function (x) { return x.name === groupName; });
        const alice = state.contacts.contacts.find(function (c) { return c.displayName === 'Alice Selftest'; });
        call('contacts.groupAddMember', { contactId: alice.id, groupId: g.id }, function (err) {
          if (err) { next(false, err.message); return; }
          call('contacts.load', undefined, function () { next(); });   // the UI reloads after each change
        });
      },
      wait('member shown', function () {
        const g = state.contacts.groups.find(function (x) { return x.name === groupName; });
        return (state.contacts.members[g.id] || []).length === 1 && document.querySelector('[data-group="' + g.id + '"]');
      }),
      // A repository through the real dialog, then a pull that cannot succeed.
      function (next) {
        Array.from(document.querySelectorAll('.contacts > button')).find(function (b) { return /^\+/.test(b.textContent); }).click();
        waitFor('repo dialog', function () { return document.querySelectorAll('dialog.modal[open] input').length >= 3; }, 3000, function (ok, why) {
          if (!ok) { next(false, why); return; }
          modalField(0).value = 'Selftest DAV';
          modalField(1).value = 'http://127.0.0.1:1';
          modalField(2).value = '/c/';
          submit();
          next();
        });
      },
      wait('repository listed', function () { return state.contacts.repositories.some(function (r) { return r.name === 'Selftest DAV'; }); }),
      wait('repository row', function () { return document.querySelector('[data-repository]'); }),
      function (next) {
        const row = Array.from(document.querySelectorAll('[data-repository]')).find(function (r) { return /Selftest DAV/.test(r.textContent); });
        Array.from(row.querySelectorAll('button')).find(function (b) { return b.textContent === t('settingsContactsCarddavPull'); }).click();
        waitFor('pull dialog', function () { return document.querySelectorAll('dialog.modal[open] input').length === 2; }, 3000, function (ok, why) {
          if (!ok) { next(false, why); return; }
          modalField(0).value = 'u';
          modalField(1).value = 'p';
          submit();
          next();
        });
      },
      wait('the failure is reported in the dialog', function () {
        const e = document.querySelector('dialog.modal[open] .error');
        return e && e.textContent.length > 0;
      }, 20000),
      function (next) { document.querySelector('dialog.modal .buttons button').click(); next(); },
      // Export, then clean everything up so a rerun starts clean.
      function (next) {
        call('contacts.exportVcardFile', { path: dir + '/exported.vcf' }, function (err, r) { next(!err && /exported\.vcf$/.test(r.path), err && err.message); });
      },
      cleanup,
      wait('cleaned up', function () {
        return !state.contacts.contacts.some(function (c) { return /Selftest$/.test(c.displayName); }) &&
          !state.contacts.groups.some(function (g) { return g.name === groupName; }) &&
          !state.contacts.repositories.some(function (r) { return r.name === 'Selftest DAV'; });
      })
    ], function (ok, why) { done(ok, ok ? 'imported 2, group + member, repository, pull failed cleanly, exported, cleaned up' : why); });
  }],
  ['about shows the version, and an account returns to mail', function (done) {
    document.querySelector('.tabs [data-tab="about"]').click();
    waitFor('about', function () { return document.querySelector('.about-head img') && /\d/.test(document.querySelector('.about-head').textContent); }, 3000, function (ok, why) {
      if (!ok) { done(false, why); return; }
      document.querySelector('.rail .account:not(.settings-btn)').click();
      waitFor('mail view', function () {
        return !document.getElementById('app').classList.contains('view-settings');
      }, 3000, function (ok2, why2) { done(ok2, ok2 ? 'ok' : why2); });
    });
  }],
  ['calendar dates: ranges, grids and where an event falls, in any time zone', function (done) {
    const D = function (y, m, d, hh, mm) { return new Date(y, m - 1, d, hh || 0, mm || 0); };
    const iso = function (d) { return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0'); };
    const bad = [];
    const check = function (what, got, want) { if (JSON.stringify(got) !== JSON.stringify(want)) { bad.push(what + ': ' + JSON.stringify(got) + ' != ' + JSON.stringify(want)); } };

    // 15 January 2026 is a Thursday. Weeks begin on Monday.
    const a = D(2026, 1, 15, 13, 30);
    check('week start', iso(CD.rangeStart('week', a)), '2026-01-12');
    check('week end', iso(CD.rangeEnd('week', a)), '2026-01-19');
    check('a Sunday belongs to the week before', iso(CD.rangeStart('week', D(2026, 1, 18))), '2026-01-12');
    check('month', [iso(CD.rangeStart('month', a)), iso(CD.rangeEnd('month', a))], ['2026-01-01', '2026-02-01']);
    check('year', [iso(CD.rangeStart('year', a)), iso(CD.rangeEnd('year', a))], ['2026-01-01', '2027-01-01']);
    check('day', [iso(CD.rangeStart('day', a)), iso(CD.rangeEnd('day', a))], ['2026-01-15', '2026-01-16']);
    check('month shift past the year', iso(CD.shiftAnchor('month', D(2026, 12, 20), 1)), '2027-01-01');
    check('month shift back over a short month', iso(CD.shiftAnchor('month', D(2026, 3, 31), -1)), '2026-02-01');
    check('week shift', iso(CD.shiftAnchor('week', a, -1)), '2026-01-08');
    check('year shift', iso(CD.shiftAnchor('year', a, 1)), '2027-01-01');
    const grid = CD.monthGrid(a);
    check('grid size and ends', [grid.length, iso(grid[0]), iso(grid[41])], [42, '2025-12-29', '2026-02-08']);
    check('a month that starts on Monday', iso(CD.monthGrid(D(2026, 6, 10))[0]), '2026-06-01');
    check('the days of a week', CD.weekDays(a).map(function (d) { return d.getDate(); }), [12, 13, 14, 15, 16, 17, 18]);

    // Across a change of clocks a day is still the next date, not 23 or 25 hours on.
    check('days across a clock change', iso(CD.addDays(D(2026, 3, 28), 2)), '2026-03-30');

    // A timed event, in the viewer's own day.
    const meeting = { allDay: false, title: 'm', startMs: D(2026, 1, 15, 23, 30).getTime(), endMs: D(2026, 1, 16, 0, 30).getTime() };
    check('overnight event on its first day', CD.occursOn(meeting, D(2026, 1, 15)), true);
    check('overnight event on its second day', CD.occursOn(meeting, D(2026, 1, 16)), true);
    check('overnight event not on the day before or after', [CD.occursOn(meeting, D(2026, 1, 14)), CD.occursOn(meeting, D(2026, 1, 17))], [false, false]);
    const ends = { allDay: false, title: 'e', startMs: D(2026, 1, 15, 22).getTime(), endMs: D(2026, 1, 16, 0).getTime() };
    check('an event ending at midnight is not on the next day', [CD.occursOn(ends, D(2026, 1, 15)), CD.occursOn(ends, D(2026, 1, 16))], [true, false]);
    const reminder = { allDay: false, title: 'r', startMs: D(2026, 1, 16, 0).getTime(), endMs: D(2026, 1, 16, 0).getTime() };
    check('a reminder at midnight is on that day only', [CD.occursOn(reminder, D(2026, 1, 15)), CD.occursOn(reminder, D(2026, 1, 16))], [false, true]);

    // An all-day event is its dates, whatever zone this is: 10th and 11th, not the 12th.
    const holiday = { allDay: true, title: 'h', startMs: Date.UTC(2026, 0, 10), endMs: Date.UTC(2026, 0, 12) };
    check('all-day days', [9, 10, 11, 12].map(function (d) { return CD.occursOn(holiday, D(2026, 1, d)); }), [false, true, true, false]);
    // And in summer, when many zones are an hour or more from UTC, which January may hide.
    const summer = { allDay: true, title: 'h2', startMs: Date.UTC(2026, 5, 10), endMs: Date.UTC(2026, 5, 12) };
    check('all-day days in June', [9, 10, 11, 12].map(function (d) { return CD.occursOn(summer, D(2026, 6, d)); }), [false, true, true, false]);
    const noEnd = { allDay: true, title: 'n', startMs: Date.UTC(2026, 0, 10), endMs: Date.UTC(2026, 0, 10) + 3600000 };
    check('an all-day event with an hour for an end is its one day', [CD.occursOn(noEnd, D(2026, 1, 10)), CD.occursOn(noEnd, D(2026, 1, 11))], [true, false]);
    const sorted = CD.eventsOn([meeting, holiday, { allDay: false, title: 'early', startMs: D(2026, 1, 11, 8).getTime(), endMs: D(2026, 1, 11, 9).getTime() }], D(2026, 1, 11)).map(function (e) { return e.title; });
    check('all-day first, then by start', sorted, ['h', 'early']);
    check('in the year view an event is in each month it touches', [CD.occursInMonth(holiday, D(2026, 1, 1)), CD.occursInMonth(holiday, D(2026, 2, 1)), CD.occursInMonth(holiday, D(2025, 12, 1))], [true, false, false]);
    const spans = { allDay: false, title: 's', startMs: D(2026, 1, 30).getTime(), endMs: D(2026, 2, 2).getTime() };
    check('across a month end', [CD.occursInMonth(spans, D(2026, 1, 1)), CD.occursInMonth(spans, D(2026, 2, 1)), CD.occursInMonth(spans, D(2026, 3, 1))], [true, true, false]);

    // What is fetched covers the view with a day to spare each side, and the whole month grid for a month.
    const [fs, fe] = CD.fetchRange('month', a);
    check('a month fetches its grid plus a day', [iso(new Date(fs)), iso(new Date(fe))], ['2025-12-28', '2026-02-10']);
    const [ws, we] = CD.fetchRange('week', a);
    check('a week fetches the week plus a day', [iso(new Date(ws)), iso(new Date(we))], ['2026-01-11', '2026-01-20']);

    // The editor's fields.
    const t0 = D(2026, 1, 15, 9, 5).getTime();
    check('local input round trip', CD.fromLocalInput(CD.toLocalInput(t0)), t0);
    check('local input text', CD.toLocalInput(t0), '2026-01-15T09:05');
    check('date input round trip', CD.fromDateInput(CD.toDateInput(Date.UTC(2026, 0, 10))), Date.UTC(2026, 0, 10));
    check('date input text', CD.toDateInput(Date.UTC(2026, 0, 10)), '2026-01-10');
    check('nonsense input', [CD.fromLocalInput(''), CD.fromLocalInput('tomorrow'), CD.fromDateInput('2026-1-1')], [null, null, null]);
    check('the editor shows the last day, the store the day after', [CD.toDateInput(CD.lastDayOf(Date.UTC(2026, 0, 10), Date.UTC(2026, 0, 12))), CD.dayAfter(Date.UTC(2026, 0, 11))], ['2026-01-11', Date.UTC(2026, 0, 12)]);
    check('a one-day all-day event shows its own day', CD.toDateInput(CD.lastDayOf(Date.UTC(2026, 0, 10), Date.UTC(2026, 0, 11))), '2026-01-10');
    check('a new event today is the next whole hour', new Date(CD.newEventStart(D(2026, 1, 15), D(2026, 1, 15, 13, 30))).getHours(), 14);
    check('a new event on another day is at nine', new Date(CD.newEventStart(D(2026, 1, 20), D(2026, 1, 15, 13, 30))).getHours(), 9);
    check('a calendar colour of its own', CD.calendarColour({ id: 'x', colorHex: '#336699' }, hueOf), '#336699');
    check('a colour with alpha loses it', CD.calendarColour({ id: 'x', colorHex: '33669980' }, hueOf), '#336699');
    check('a colour made from the id', CD.calendarColour({ id: 'work' }, hueOf), 'hsl(' + hueOf('work') + ' 55% 45%)');
    check('a bad colour falls back', CD.calendarColour({ id: 'work', colorHex: 'blue' }, hueOf), 'hsl(' + hueOf('work') + ' 55% 45%)');
    done(bad.length === 0, bad.length ? bad.join('; ') : 'ranges, grids, all-day dates and the editor fields');
  }],
  ['the reader: actions at the top right as round icons in front of the headers, headers at the top left, signature mark under the actions, attachments at the foot', function (done) {
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 10000, function (ok, why) { next(ok, why); }); };
    };
    const open = function (subject) {
      return function (next) {
        const row = Array.from(document.querySelectorAll('.message-row')).find(function (r) { return r.textContent.indexOf(subject) >= 0; });
        if (!row) { next(false, 'no row for "' + subject + '"'); return; }
        row.click();
        next();
      };
    };
    const box = function (sel) { const el = document.querySelector(sel); return el && !el.hidden ? el.getBoundingClientRect() : null; };
    const id = state.accountOrder[0];
    // The account can send for this test: a transport is lent to it, and taken back.
    const had = (state.accounts[id].lists || {}).transportIds;
    const lend = function (next) {
      state.transports.lent = { id: 'lent', transportType: 'smtp', displayName: 'Lent', host: 'h', port: 25, security: 'plain' };
      state.accounts[id].lists = Object.assign({}, state.accounts[id].lists, { transportIds: ['lent'] });
      changed();
      next();
    };
    const giveBack = function () {
      delete state.transports.lent;
      state.accounts[id].lists = Object.assign({}, state.accounts[id].lists, { transportIds: had || [] });
      changed();
    };
    sequence([
      function (next) { state.view = 'mail'; changed(); next(); },
      lend,
      open('A file for you'),
      wait('the message with its attachment', function () { return state.message && state.message.opened && state.message.attachments.length === 1 && document.querySelector('.msg-actions .round-btn'); }),
      function (next) {
        const pane = document.querySelector('.reading').getBoundingClientRect();
        const acts = box('.msg-actions');
        const buttons = Array.from(document.querySelectorAll('.msg-actions .round-btn'));
        const names = buttons.map(function (b) { return b.getAttribute('data-action'); });
        const icons = buttons.map(function (b) { return getComputedStyle(b.querySelector('.icon')).maskImage || getComputedStyle(b.querySelector('.icon')).webkitMaskImage || ''; });
        const round = buttons.every(function (b) { return parseFloat(getComputedStyle(b).borderTopLeftRadius) >= b.getBoundingClientRect().width / 2 - 1; });
        // At the top right of the pane, and in front of the headers' pane, not behind it.
        const first = buttons[0] && buttons[0].getBoundingClientRect();
        const hit = first && document.elementFromPoint(first.left + first.width / 2, first.top + first.height / 2);
        const front = !!hit && !!buttons[0] && buttons[0].contains(hit);
        const ok = acts && pane.right - acts.right < 30 && acts.top - pane.top < 12 && front &&
          names.join() === 'reply,replyAll,forward' && round &&
          ['reply.svg', 'reply-all.svg', 'forward.svg'].every(function (f, i) { return icons[i].indexOf('/icons/' + f) >= 0; }) &&
          buttons.every(function (b) { return !b.textContent.trim() && b.title && b.getAttribute('aria-label'); });
        next(!!ok, 'actions ' + (acts ? Math.round(pane.right - acts.right) + ' from the right, ' + Math.round(acts.top - pane.top) + ' from the top' : 'none') + '; in front ' + front + '; ' + names.join() + '; round ' + round + '; icons ' + icons.map(function (i) { return i.slice(-16); }).join(' '));
      },
      function (next) {
        // The headers start at the top left, not in the middle, and run the width of the pane;
        // the text stays clear of the action strip at the right.
        const pane = document.querySelector('.reading').getBoundingClientRect();
        const head = box('.msg-head');
        const subject = box('.msg-head .subject');
        const acts = box('.msg-actions');
        const ok = head && subject && acts && head.left - pane.left < 2 && head.top - pane.top < 2 && head.width > pane.width - 4 &&
          subject.left - pane.left < 40 && subject.top - pane.top < 20 && subject.right <= acts.left;
        next(!!ok, 'headers ' + (head ? Math.round(head.left - pane.left) + ',' + Math.round(head.top - pane.top) + ' +' + Math.round(head.width) + ' of ' + Math.round(pane.width) : 'none') + '; subject at ' + (subject ? Math.round(subject.left - pane.left) + ',' + Math.round(subject.top - pane.top) : 'none'));
      },
      function (next) {
        // The signature mark is under the actions, at the right of the pane.
        state.message.signature = 'valid';
        changed();
        next();
      },
      wait('the signature mark', function () { return document.querySelector('.msg-sign .icon'); }),
      function (next) {
        const pane = document.querySelector('.reading').getBoundingClientRect();
        const acts = box('.msg-actions');
        const sign = box('.msg-sign');
        const ok = sign && acts && pane.right - sign.right < 30 && sign.top >= acts.bottom - 2 && sign.top - acts.bottom < 20;
        state.message.signature = null;
        changed();
        next(!!ok, 'signature mark ' + (sign ? Math.round(pane.right - sign.right) + ' from the right, ' + Math.round(sign.top - acts.bottom) + ' under the actions' : 'missing'));
      },
      function (next) {
        // The attachments are the last thing in the pane, under the message body, which has the room.
        const pane = document.querySelector('.reading').getBoundingClientRect();
        const frame = box('iframe.msg-body');
        const files = box('.attachments');
        const ok = files && frame && Math.abs(files.bottom - pane.bottom) < 2 && files.top >= frame.bottom - 1 && frame.height > 100;
        next(!!ok, 'attachments ' + (files ? Math.round(pane.bottom - files.bottom) + ' above the foot, ' + Math.round(files.top - frame.bottom) + ' below the body' : 'missing'));
      },
      // A message without attachments has no strip.
      open('Another test'),
      wait('a message with no attachments', function () { return state.message && state.message.opened && state.message.headers && state.message.attachments.length === 0; }),
      function (next) { next(document.querySelector('.attachments').hidden, 'no attachment strip when there are none'); },
      // An editable draft has the pen, and news only the reply.
      function (next) {
        const m = state.message;
        const acc = state.accounts[id];
        const attrs = acc.attrs;
        const type = acc.backendType;
        acc.attrs = Object.assign({}, attrs, { imapDraftsFolderName: m.folder });
        const draft = messageActions(m).join();
        acc.attrs = attrs;
        acc.backendType = 'nntp';
        const news = messageActions(m).join();
        acc.backendType = type;
        next(draft === 'draft' && news === 'reply', 'draft: ' + draft + '; news: ' + news);
      }
    ], function (ok, why) { giveBack(); done(ok, ok ? 'round Lucide buttons top-right in front, headers top-left, signature mark under the buttons, attachments last and only when present' : why); });
  }],
  ['a signed message shows a green mark when it checks out and a red one when it does not, at the right', function (done) {
    const mark = function () { const el = document.querySelector('.msg-sign'); return el && !el.hidden ? el : null; };
    const m = state.message;
    if (!m) { done(false, 'no message open'); return; }
    const say = function (status) { apply({ type: 'messageSignature', requestId: m.requestId, status: status }); };
    const settle = function (fn) { changed(); setTimeout(fn, 60); };
    const pane = document.querySelector('.reading').getBoundingClientRect();
    const colourOf = function (el) { return getComputedStyle(el).color; };
    const results = [];
    if (mark()) { done(false, 'a mark with no verdict'); return; }
    say('valid');
    settle(function () {
      const el = mark();
      const r = el && el.getBoundingClientRect();
      const acts = document.querySelector('.msg-actions').getBoundingClientRect();
      const icon = el && getComputedStyle(el.querySelector('.icon')).maskImage;
      const green = el && /^rgb\((\d+), (\d+), (\d+)\)/.exec(colourOf(el));
      results.push('valid: ' + (el ? 'shown' : 'missing'));
      const placed = r && pane.right - r.right < 40 && pane.right - r.right > 0 && r.top >= acts.bottom - 2;
      const isGreen = green && +green[2] > +green[1] && +green[2] > +green[3];
      if (!el || !placed || !isGreen || String(icon).indexOf('badge-check.svg') < 0 || !el.title) {
        done(false, 'valid mark: placed ' + !!placed + ', green ' + !!isGreen + ', icon ' + icon + ', title "' + (el && el.title) + '"'); return;
      }
      say('invalid');
      settle(function () {
        const bad = mark();
        const red = bad && /^rgb\((\d+), (\d+), (\d+)\)/.exec(colourOf(bad));
        const icon2 = bad && getComputedStyle(bad.querySelector('.icon')).maskImage;
        if (!bad || !(red && +red[1] > +red[2] && +red[1] > +red[3]) || String(icon2).indexOf('badge-x.svg') < 0) {
          done(false, 'invalid mark: ' + (bad ? colourOf(bad) + ' ' + icon2 : 'missing')); return;
        }
        say('unknown');
        settle(function () {
          const odd = mark();
          const icon3 = odd && getComputedStyle(odd.querySelector('.icon')).maskImage;
          if (!odd || String(icon3).indexOf('circle-help.svg') < 0) { done(false, 'unknown mark: ' + icon3); return; }
          // Another message has no verdict until it is given one.
          const other = Array.from(document.querySelectorAll('.message-row')).find(function (row) { return row.getAttribute('data-id') !== m.id; });
          other.click();
          waitFor('another message', function () { return state.message && state.message.id !== m.id && state.message.opened; }, 10000, function (ok, why) {
            done(ok && !mark(), ok ? 'green badge-check, red badge-x, grey help; none for an unsigned message' : why);
          });
        });
      });
    });
  }],
  ['the offer to load images shows only when the message holds remote images, under the headers', function (done) {
    const wait = function (what, pred, ms) {
      return function (next) { waitFor(what, pred, ms || 10000, function (ok, why) { next(ok, why); }); };
    };
    const open = function (subject) {
      return function (next) {
        const row = Array.from(document.querySelectorAll('.message-row')).find(function (r) { return r.textContent.indexOf(subject) >= 0; });
        if (!row) { next(false, 'no row for "' + subject + '"'); return; }
        row.click();
        next();
      };
    };
    const bar = function () { const b = document.querySelector('.imagesbar'); return b && !b.hidden ? b : null; };
    sequence([
      // HTML with no remote picture in it: nothing to offer.
      open('A file for you'),
      wait('a message without remote images', function () { return state.message && state.message.opened && state.message.headers && state.message.headers.subject === 'A file for you'; }),
      function (next) { setTimeout(function () { next(!bar() && state.message.remoteCount === 0, 'no offer for a message with nothing to load'); }, 500); },
      // HTML that refers to two: offered, and under the headers, above the body.
      open('Remote pictures'),
      wait('a message with remote images', function () { return state.message && state.message.opened && state.message.remoteCount === 2 && bar(); }),
      function (next) {
        const head = document.querySelector('.msg-head').getBoundingClientRect();
        const b = bar().getBoundingClientRect();
        const frame = document.querySelector('iframe.msg-body').getBoundingClientRect();
        next(b.top >= head.bottom - 1 && b.bottom <= frame.top + 1, 'offer at ' + Math.round(b.top - head.bottom) + ' under the headers, ' + Math.round(frame.top - b.bottom) + ' above the body');
      },
      // Opened again, it is offered straight away, without waiting for the frame to ask.
      open('A file for you'),
      wait('the other message', function () { return state.message && state.message.opened && state.message.headers && state.message.headers.subject === 'A file for you'; }),
      open('Remote pictures'),
      function (next) { next(!!state.message && state.message.remoteCount === 2, 'known on reopening: ' + (state.message && state.message.remoteCount)); }
    ], function (ok, why) { done(ok, ok ? 'offered only with images to load, under the headers, and known again on reopening' : why); });
  }],
  ['the window title says what is open, and is the page\'s own document.title', function (done) {
    const p = { view: 'mail', account: 'Work', folder: 'Inbox', unread: 0, messageSubject: '', composeSubject: '', settings: 'Settings', compose: 'Compose' };
    const cases = [
      [p, 'Inbox - Work - Tagliacarte'],
      [Object.assign({}, p, { unread: 3 }), 'Inbox (3) - Work - Tagliacarte'],
      [Object.assign({}, p, { messageSubject: 'Lunch?' }), 'Lunch? - Inbox - Tagliacarte'],
      [Object.assign({}, p, { folder: '' }), 'Work - Tagliacarte'],
      [Object.assign({}, p, { account: '', folder: '' }), 'Tagliacarte'],
      [Object.assign({}, p, { view: 'settings', messageSubject: 'x' }), 'Settings - Tagliacarte'],
      [Object.assign({}, p, { view: 'compose' }), 'Compose - Tagliacarte'],
      [Object.assign({}, p, { view: 'compose', composeSubject: 'Re: Lunch?' }), 'Re: Lunch? - Tagliacarte']
    ];
    const wrong = cases.filter(function (c) { return windowTitle(c[0]) !== c[1]; })
      .map(function (c) { return JSON.stringify(windowTitle(c[0])) + ' != ' + JSON.stringify(c[1]); });
    if (wrong.length) { done(false, wrong.join('; ')); return; }
    // The live one: whatever is on screen, the document carries its title.
    waitFor('title', function () { return document.title === currentTitle(); }, 3000, function (ok, why) {
      if (!ok) { done(false, why + ': ' + JSON.stringify(document.title) + ' vs ' + JSON.stringify(currentTitle())); return; }
      const before = state.view;
      state.view = 'settings';
      changed();
      const want = t('settings') + ' - Tagliacarte';
      // ...and the native window has the same one, by way of the shell.
      waitFor('native title', function () { return document.title === want && state.nativeTitle === want; }, 5000, function (ok2, why2) {
        state.view = before;
        changed();
        done(ok2, ok2 ? cases.length + ' cases; the window is titled "' + state.nativeTitle + '"' : why2 + ': document ' + JSON.stringify(document.title) + ', window ' + JSON.stringify(state.nativeTitle));
      });
    });
  }],
  ['the native menu: every item is known to it, words come from the strings, actions arrive as events', function (done) {
    const missing = Object.keys(MENU_LABELS).filter(function (id) { return t(MENU_LABELS[id][0], MENU_LABELS[id][1]) === MENU_LABELS[id][0]; });
    if (missing.length) { done(false, 'no string for ' + missing.join(', ')); return; }
    const about = t(MENU_LABELS.about[0], MENU_LABELS.about[1]);
    if (about !== 'About Tagliacarte' && about.indexOf('Tagliacarte') < 0) { done(false, 'the app name was not filled in: ' + about); return; }
    // Rust checks the ids against its own table: one the page invents is an error.
    sendMenuLabels(function (err) {
      if (err) { done(false, 'labels refused: ' + err.message); return; }
      call('menu.labels', { ids: ['noSuchItem'], labels: ['x'] }, function (bad) {
        if (!bad) { done(false, 'an unknown menu id was accepted'); return; }
        const mine = enabledMenuItems();
        call('menu.enable', { ids: mine }, function (err2) {
          if (err2) { done(false, 'enable refused: ' + err2.message); return; }
          // A menu choice is an event like any other.
          state.view = 'mail';
          apply({ type: 'menuAction', id: 'settings' });
          const opened = state.view === 'settings';
          state.view = 'mail';
          const canReply = !!state.message && runMessageAction('reply') && state.view === 'compose';
          if (canReply) { closeCompose(); }
          const noSender = !state.accountOrder.some(function (a) { return composeSendKind(state.accounts[a]); });
          apply({ type: 'menuAction', id: 'newMessage' });
          const composing = state.view === 'compose';
          if (composing) { closeCompose(); }
          done(opened && composing === !noSender,
            'settings opened ' + opened + '; new message ' + (composing ? 'started' : 'refused: no account can send') + '; enabled ' + JSON.stringify(mine));
        });
      });
    });
  }],
  ['page cannot be navigated away', function (done) {
    location.href = 'https://example.com/';
    setTimeout(function () { done(location.hostname.indexOf('localhost') >= 0, location.href); }, 700);
  }]
];

let i = 0;
let failed = 0;
(function next() {
  if (i === tests.length) {
    report('FINAL ' + (failed === 0 ? 'ALL PASS' : failed + ' FAILED'));
    return;
  }
  const test = tests[i++];
  let finished = false;
  function finish(ok, detail) {
    if (finished) { return; }
    finished = true;
    clearTimeout(watchdog);
    if (!ok) { failed++; }
    report((ok ? 'PASS ' : 'FAIL ') + test[0] + (detail ? ' (' + detail + ')' : ''));
    next();
  }
  const watchdog = setTimeout(function () { finish(false, 'no result after 25 s' + (sequence.at ? ' (waiting in step ' + sequence.at + ')' : '')); }, 25000);
  // A test that throws before it answers is a failure now, not a wait for the watchdog.
  try { test[1](finish); } catch (e) { finish(false, 'threw ' + (e && e.stack ? e.stack.split('\n').slice(0, 2).join(' | ') : e)); }
})();
