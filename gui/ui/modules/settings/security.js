import { makePage, section, switchField, segmentedField, textField, pathField } from './kit.js';

export function build() {
  const page = makePage();
  page.add(
    switchField(page, { key: 'useKeychain', label: 'useSystemKeychain', hint: 'storeCredentialsInKeychain' }),
    section('settingsMailCryptoSection', 'settingsMailCryptoStackSubtitle'),
    segmentedField(page, {
      key: 'mailCryptoStack',
      options: [
        { value: 'openpgp', label: 'OpenPGP' },
        { value: 'smime', label: 'S/MIME' }
      ],
      normalise: function (v) { return v === 'smime' || v === 's/mime' ? 'smime' : 'openpgp'; }
    }),
    pathField(page, { key: 'mailCryptoPgpSecretKeyPath', label: 'settingsMailCryptoPgpSecretKeyPath' }),
    textField(page, {
      key: 'mailCryptoPgpPassphrase',
      label: 'settingsMailCryptoPgpPassphrase',
      password: true,
      hasFlag: 'mailCryptoPgpHasPassphrase'
    }),
    pathField(page, { key: 'mailCryptoSmimeCertPath', label: 'settingsMailCryptoSmimeCert' }),
    pathField(page, { key: 'mailCryptoSmimeKeyPath', label: 'settingsMailCryptoSmimeKey' }));
  return page;
}
