import { makePage, switchField, localSwitchField } from './kit.js';
import { changed } from '../store.js';

// "Message detail below list" is not offered: this layout always shows the
// message beside the list.
export function build() {
  const page = makePage();
  page.add(
    localSwitchField({
      key: 'headersMinimal',
      fallback: true,
      label: 'settingsViewMinimalHeaders',
      hint: 'settingsViewMinimalHeadersSubtitle',
      onChange: changed
    }),
    switchField(page, { key: 'loadRemoteImages', label: 'loadRemoteImages', hint: 'loadRemoteImagesSubtitle' }),
    switchField(page, { key: 'threadedView', label: 'threadedView', hint: 'threadedViewSubtitle' }),
    switchField(page, { key: 'notifyNewMessages', label: 'settingsNotifyNewMessages', hint: 'settingsNotifyNewMessagesSubtitle' }));
  return page;
}
