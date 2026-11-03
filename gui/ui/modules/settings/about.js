import { h } from '../dom.js';
import { t } from '../i18n.js';
import { call } from '../wire.js';
import { makePage } from './kit.js';

export function build() {
  const page = makePage();
  const version = h('span', { class: 'dim' });
  call('app.info', undefined, function (err, info) {
    if (!err && info) { version.textContent = ' ' + info.version; }
  });
  const row = function (titleKey, text) {
    return h('div', { class: 'about-row' }, h('strong', { text: t(titleKey) }), h('span', { text: text }));
  };
  page.add(
    h('div', { class: 'about-head' },
      h('img', { src: '/icons/app-icon.svg', width: '36', height: '36', alt: '' }),
      h('div', {}, h('div', {}, h('strong', { text: t('appTitle') }), version), h('div', { class: 'dim', text: t('aboutSubtitle') }))),
    row('supportedBackends', t('supportedBackendsList')),
    row('licenseTitle', t('licenseGpl')),
    row('copyrightTitle', t('copyrightLine')));
  return page;
}
