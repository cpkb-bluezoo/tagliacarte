/*
 * message_view.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 */

import 'dart:async' show unawaited;
import 'dart:convert';

import 'package:flutter/foundation.dart'
    show defaultTargetPlatform, kIsWeb, TargetPlatform;
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'package:webview_flutter_android/webview_flutter_android.dart';
import 'package:webview_flutter_wkwebview/webview_flutter_wkwebview.dart';

import '../l10n/app_localizations.dart';
import '../providers/mail_sync.dart';
import '../providers/view_prefs.dart';
import '../rust/frb_api.dart';
import '../util/mail_link_hover.dart';
import '../util/mailbox_format.dart';
import '../util/message_dates.dart';
import 'message_attachments.dart';
import 'selectable_header_line.dart';

bool get _mailBodyWebViewSupported =>
    !kIsWeb &&
    (defaultTargetPlatform == TargetPlatform.android ||
        defaultTargetPlatform == TargetPlatform.iOS ||
        defaultTargetPlatform == TargetPlatform.macOS);

String _colorToCssHexRgb(Color c) {
  int x(double comp) => (comp * 255.0).round().clamp(0, 255);
  final int r = x(c.r);
  final int g = x(c.g);
  final int b = x(c.b);
  return '${r.toRadixString(16).padLeft(2, '0')}'
      '${g.toRadixString(16).padLeft(2, '0')}'
      '${b.toRadixString(16).padLeft(2, '0')}';
}

String _mailBodyUrlQuery(BuildContext context, {required bool allowRemote}) {
  final ThemeData theme = Theme.of(context);
  final ColorScheme cs = theme.colorScheme;
  final int fs = (theme.textTheme.bodyMedium?.fontSize ?? 14).round();
  final String fg = _colorToCssHexRgb(cs.onSurface);
  final String bg = _colorToCssHexRgb(cs.surface);
  final String link = _colorToCssHexRgb(cs.primary);
  final String ar = allowRemote ? 'true' : 'false';
  return 'fg=$fg&bg=$bg&link=$link&fs=$fs&allowRemote=$ar';
}

/// JavaScript channel name (must match injected script).
const String _kMailLinkHoverChannel = 'TagliacarteLinkHover';

/// Injects link mouseenter/mouseleave → [postMessage] for status bar + spoofing hint.
///
/// **Security:** Requires [JavaScriptMode.unrestricted], so scripts embedded in the
/// message HTML may run. This matches common HTML mail viewers; sanitize server-side
/// if stricter isolation is required.
String _mailLinkHoverInjectionJs() => '''
(function(){
  var ch = window.$_kMailLinkHoverChannel;
  if (!ch || !ch.postMessage) return;
  function send(href, text) {
    ch.postMessage(JSON.stringify({href:href||'',text:text||''}));
  }
  function clear() { send('', ''); }
  function attach(a) {
    if (a.__tagliHover) return;
    a.__tagliHover = true;
    a.addEventListener('mouseenter', function() {
      send(a.getAttribute('href') || '', (a.innerText || a.textContent || '').trim());
    });
    a.addEventListener('mouseleave', clear);
  }
  function scan(root) {
    var nodes = root.querySelectorAll('a[href]');
    for (var i = 0; i < nodes.length; i++) attach(nodes[i]);
  }
  scan(document);
  try {
    var mo = new MutationObserver(function(records) {
      records.forEach(function(r) {
        r.addedNodes.forEach(function(n) {
          if (n.nodeType !== 1) return;
          if ((n.tagName||'').toUpperCase() === 'A' && n.hasAttribute('href')) attach(n);
          if (n.querySelectorAll) scan(n);
        });
      });
    });
    mo.observe(document.documentElement, { childList: true, subtree: true });
  } catch (e) {}
})();
''';

Future<WebViewController> _createMailHtmlWebViewController({
  required String url,
  required void Function(String? truncatedUrl, bool misleading) onLinkHover,
}) async {
  late final PlatformWebViewControllerCreationParams params;
  if (WebViewPlatform.instance is WebKitWebViewPlatform) {
    params = WebKitWebViewControllerCreationParams(
      allowsInlineMediaPlayback: true,
    );
  } else {
    params = const PlatformWebViewControllerCreationParams();
  }

  final WebViewController c =
      WebViewController.fromPlatformCreationParams(params);

  if (c.platform is AndroidWebViewController) {
    AndroidWebViewController.enableDebugging(false);
  }

  await c.setJavaScriptMode(JavaScriptMode.unrestricted);

  await c.addJavaScriptChannel(
    _kMailLinkHoverChannel,
    onMessageReceived: (JavaScriptMessage message) {
      try {
        final Object? decoded = jsonDecode(message.message);
        if (decoded is! Map<String, dynamic>) {
          return;
        }
        final String href = decoded['href'] as String? ?? '';
        if (href.isEmpty) {
          onLinkHover(null, false);
          return;
        }
        final String text = decoded['text'] as String? ?? '';
        onLinkHover(
          truncateUrlForStatusBar(href),
          linkTextMisrepresentsHttpHref(href, text),
        );
      } catch (_) {
        onLinkHover(null, false);
      }
    },
  );

  await c.setNavigationDelegate(
    NavigationDelegate(
      onPageStarted: (_) {
        onLinkHover(null, false);
      },
      onNavigationRequest: (NavigationRequest request) {
        final Uri uri = Uri.parse(request.url);
        if (uri.scheme == 'https' && uri.host == '127.0.0.1') {
          return NavigationDecision.navigate;
        }
        if (uri.scheme == 'http' || uri.scheme == 'https') {
          unawaited(
            launchUrl(uri, mode: LaunchMode.externalApplication),
          );
          return NavigationDecision.prevent;
        }
        if (uri.scheme == 'mailto') {
          unawaited(
            launchUrl(uri, mode: LaunchMode.externalApplication),
          );
          return NavigationDecision.prevent;
        }
        return NavigationDecision.prevent;
      },
      onPageFinished: (_) async {
        try {
          await c.runJavaScript(_mailLinkHoverInjectionJs());
        } catch (_) {}
      },
      onSslAuthError: (SslAuthError error) async {
        await error.proceed();
      },
    ),
  );

  await c.loadRequest(Uri.parse(url));
  return c;
}

class MessageView extends ConsumerStatefulWidget {
  const MessageView({
    super.key,
    required this.subject,
    required this.subjectInAppBar,
    required this.fromRaw,
    required this.toRaw,
    this.ccRaw,
    this.dateMs,
    required this.bodyHtml,
    required this.bodyPlain,
    this.attachments = const [],
    this.attachmentFetchParams,
    this.mailBodyStoreKey,
  });

  final String subject;
  final bool subjectInAppBar;
  final String fromRaw;
  final String toRaw;
  final String? ccRaw;

  /// UTC epoch ms from the envelope; formatted with locale (long style) in the detail header.
  final int? dateMs;
  final String? bodyHtml;
  final String? bodyPlain;
  final List<MailAttachmentDetail> attachments;
  final MailMessageDetailParams? attachmentFetchParams;

  /// When non-null with HTML body, load message HTML from the Rust loopback server in a WebView.
  final String? mailBodyStoreKey;

  @override
  ConsumerState<MessageView> createState() => _MessageViewState();
}

class _MessageViewState extends ConsumerState<MessageView> {
  bool _allowRemoteImages = false;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool minimalHeaders = ref.watch(messageHeadersMinimalProvider);
    final Locale locale = Localizations.localeOf(context);
    final String fromShown = formatMailboxLine(widget.fromRaw, short: false);
    final String toShown = formatMailboxLine(widget.toRaw, short: false);
    final String? cc = widget.ccRaw?.trim();
    final String? dateShown = _formatDetailDate(context, locale, widget.dateMs);
    final bool showCc = !minimalHeaders && cc != null && cc.isNotEmpty;
    final bool showDate = dateShown != null && dateShown.isNotEmpty;

    final ThemeData theme = Theme.of(context);
    final double bodyFontSize = theme.textTheme.bodyMedium?.fontSize ?? 14;

    final bool htmlNonEmpty =
        widget.bodyHtml != null && widget.bodyHtml!.trim().isNotEmpty;
    final bool useWebForHtml = htmlNonEmpty &&
        _mailBodyWebViewSupported &&
        widget.mailBodyStoreKey != null &&
        widget.attachmentFetchParams != null;

    final Widget bodyWidget;
    if (useWebForHtml) {
      final MailMessageDetailParams p = widget.attachmentFetchParams!;
      final String q = _mailBodyUrlQuery(context, allowRemote: _allowRemoteImages);
      bodyWidget = Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (!_allowRemoteImages)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: theme.colorScheme.surfaceContainerHighest,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  child: Row(
                    children: [
                      Expanded(
                        child: Text(
                          l10n.remoteImagesBlocked,
                          style: theme.textTheme.bodySmall,
                        ),
                      ),
                      TextButton(
                        onPressed: () =>
                            setState(() => _allowRemoteImages = true),
                        child: Text(l10n.loadImages),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          Expanded(
            child: FutureBuilder<String>(
              future: frbMailBodyMessageUrl(
                storeKey: widget.mailBodyStoreKey!,
                folderName: p.folderName,
                messageId: imapMessageIdForNativeApis(p.messageId),
                extraQuery: q,
              ),
              builder: (BuildContext context, AsyncSnapshot<String> snap) {
                if (snap.hasError) {
                  return SelectableText(
                    l10n.couldNotOpenHtmlBody('${snap.error}'),
                    style: theme.textTheme.bodyMedium,
                  );
                }
                if (!snap.hasData) {
                  return const Center(child: CircularProgressIndicator());
                }
                final String url = snap.data!;
                return _MailHtmlWebViewLoader(
                  key: ValueKey<String>(url),
                  url: url,
                  theme: theme,
                );
              },
            ),
          ),
        ],
      );
    } else if (htmlNonEmpty) {
      bodyWidget = SelectableText(
        widget.bodyHtml!,
        style: theme.textTheme.bodyMedium?.copyWith(fontSize: bodyFontSize),
      );
    } else {
      final String plain = widget.bodyPlain ?? '';
      bodyWidget = SelectableText(
        plain,
        style: theme.textTheme.bodyMedium,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (!widget.subjectInAppBar)
          Padding(
            padding: const EdgeInsets.all(12),
            child: SelectableText(
              widget.subject,
              style: Theme.of(context).textTheme.titleLarge,
            ),
          ),
        if (fromShown.isNotEmpty)
          SelectableHeaderLine(label: l10n.headerFrom, value: fromShown),
        if (toShown.isNotEmpty)
          SelectableHeaderLine(label: l10n.headerTo, value: toShown),
        if (showCc)
          SelectableHeaderLine(
            label: l10n.headerCc,
            value: formatMailboxLine(cc, short: false),
          ),
        if (showDate)
          SelectableHeaderLine(label: l10n.headerDate, value: dateShown),
        const Divider(),
        Expanded(
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Expanded(
                  child: useWebForHtml
                      ? bodyWidget
                      : SingleChildScrollView(
                          child: SelectionArea(child: bodyWidget),
                        ),
                ),
                MessageAttachmentsBlock(
                  attachments: widget.attachments,
                  fetchParams: widget.attachmentFetchParams,
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _MailHtmlWebViewLoader extends StatefulWidget {
  const _MailHtmlWebViewLoader({
    super.key,
    required this.url,
    required this.theme,
  });

  final String url;
  final ThemeData theme;

  @override
  State<_MailHtmlWebViewLoader> createState() => _MailHtmlWebViewLoaderState();
}

class _MailHtmlWebViewLoaderState extends State<_MailHtmlWebViewLoader> {
  String? _hoverUrlLine;
  bool _hoverMisleading = false;

  late final Future<WebViewController> _future = _createMailHtmlWebViewController(
    url: widget.url,
    onLinkHover: (String? truncated, bool misleading) {
      if (!mounted) {
        return;
      }
      setState(() {
        _hoverUrlLine = truncated;
        _hoverMisleading = misleading && truncated != null;
      });
    },
  );

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    final ThemeData theme = widget.theme;
    final ColorScheme scheme = theme.colorScheme;
    final TextStyle baseStyle = theme.textTheme.bodySmall ??
        TextStyle(fontSize: 12, color: scheme.onSurface);
    final TextStyle urlStyle = baseStyle.copyWith(
      fontWeight: _hoverMisleading ? FontWeight.w700 : FontWeight.w400,
    );

    return FutureBuilder<WebViewController>(
      future: _future,
      builder: (BuildContext context, AsyncSnapshot<WebViewController> snap) {
        if (snap.hasError) {
          return SelectableText(
            l10n.webViewError('${snap.error}'),
            style: theme.textTheme.bodyMedium,
          );
        }
        if (!snap.hasData) {
          return const Center(child: CircularProgressIndicator());
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(child: WebViewWidget(controller: snap.data!)),
            AnimatedSwitcher(
              duration: const Duration(milliseconds: 120),
              child: _hoverUrlLine == null
                  ? const SizedBox(height: 0)
                  : Material(
                      key: ValueKey<String>(_hoverUrlLine!),
                      color: scheme.surfaceContainerHighest,
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 10,
                          vertical: 6,
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (_hoverMisleading)
                              Padding(
                                padding: const EdgeInsets.only(bottom: 4),
                                child: Text(
                                  l10n.linkHoverMisleadingCaption,
                                  style: baseStyle.copyWith(
                                    fontWeight: FontWeight.w600,
                                    color: scheme.tertiary,
                                  ),
                                ),
                              ),
                            SelectableText(
                              _hoverUrlLine!,
                              style: urlStyle,
                              maxLines: 3,
                            ),
                          ],
                        ),
                      ),
                    ),
            ),
          ],
        );
      },
    );
  }
}

String? _formatDetailDate(
  BuildContext context,
  Locale locale,
  int? dateMs,
) {
  if (dateMs == null) {
    return null;
  }
  final DateTime local =
      DateTime.fromMillisecondsSinceEpoch(dateMs, isUtc: true).toLocal();
  try {
    return formatMessageDetailHeaderDate(local, locale);
  } catch (_) {
    return DateFormat.yMd(locale.toLanguageTag()).add_jm().format(local);
  }
}
