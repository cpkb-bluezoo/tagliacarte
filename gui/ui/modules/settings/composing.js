import { h } from '../dom.js';
import { t } from '../i18n.js';
import { makePage, section, switchField, selectField, textField } from './kit.js';
import {
  DATE_PATTERNS, TIME_PATTERNS, DATE_PATTERN_LABELS, TIME_PATTERN_LABELS,
  formatReplyDate, formatReplyTime, expandReplyHeader
} from '../replyformat.js';
import { state } from '../store.js';

const SAMPLE_SENDER = 'alice@example.com';

function presetOptions(patterns, labels) {
  return patterns.map(function (p) { return { value: p, label: t(labels[p]) }; });
}

export function build() {
  const page = makePage();
  const preview = h('p', { class: 'hint preview' });
  let template = '';

  function showPreview() {
    const cfg = state.config || {};
    const now = new Date();
    const locale = document.documentElement.lang || undefined;
    const date = formatReplyDate(now, cfg.replyDateFormat || '', locale);
    const time = formatReplyTime(now, cfg.replyTimeFormat || '', locale);
    preview.textContent = t('replyHeaderPreviewLabel') + ': ' + expandReplyHeader(template, date, time, SAMPLE_SENDER);
  }

  const position = selectField(page, {
    key: 'replyPlainPosition',
    label: 'replyPlainPositionLabel',
    hint: 'replyPlainPositionSubtitle',
    options: [
      { value: 'before_quote', label: t('replyPlainPositionBefore') },
      { value: 'after_quote', label: t('replyPlainPositionAfter') }
    ]
  });
  const prefix = textField(page, {
    key: 'replyLinePrefix',
    label: 'replyLinePrefixLabel',
    hint: 'replyLinePrefixSubtitle'
  });

  page.add(
    section('composingReplySection'),
    switchField(page, { key: 'quoteOriginal', label: 'quoteOriginalOnReply', hint: 'quoteOriginalOnReplySubtitle' }),
    position,
    textField(page, {
      key: 'replyHeaderTemplate',
      label: 'replyHeaderTemplateLabel',
      hint: 'replyHeaderTemplateHelp',
      multiline: true,
      onInput: function (v) { template = v; showPreview(); }
    }),
    preview,
    selectField(page, {
      key: 'replyDateFormat',
      label: 'replyDateFormatLabel',
      options: presetOptions(DATE_PATTERNS, DATE_PATTERN_LABELS)
    }),
    selectField(page, {
      key: 'replyTimeFormat',
      label: 'replyTimeFormatLabel',
      options: presetOptions(TIME_PATTERNS, TIME_PATTERN_LABELS)
    }),
    prefix,
    selectField(page, {
      key: 'replyQuoteMode',
      label: 'replyQuoteModeLabel',
      hint: 'replyQuoteModeHtmlSmtpSubtitle',
      options: [
        { value: 'plain', label: t('replyQuoteModePlain') },
        { value: 'html_smtp', label: t('replyQuoteModeHtmlSmtp') }
      ]
    }),
    switchField(page, { key: 'composeUseRichText', label: 'settingsComposeRichText', hint: 'settingsComposeRichTextSubtitle' }),
    switchField(page, { key: 'matrixChatUseRichText', label: 'settingsMatrixChatRichText', hint: 'settingsMatrixChatRichTextSubtitle' }));

  // Quoting off hides the position choice and disables the prefix.
  page.bind(function (cfg) {
    position.hidden = !cfg.quoteOriginal;
    prefix.querySelector('input').disabled = !cfg.quoteOriginal;
    showPreview();
  });
  return page;
}
