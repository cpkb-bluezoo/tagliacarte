import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_de.dart';
import 'app_localizations_el.dart';
import 'app_localizations_en.dart';
import 'app_localizations_es.dart';
import 'app_localizations_fr.dart';
import 'app_localizations_it.dart';
import 'app_localizations_ja.dart';
import 'app_localizations_pt.dart';
import 'app_localizations_ru.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('de'),
    Locale('el'),
    Locale('en'),
    Locale('es'),
    Locale('fr'),
    Locale('it'),
    Locale('ja'),
    Locale('pt'),
    Locale('ru'),
    Locale('zh'),
  ];

  /// No description provided for @appTitle.
  ///
  /// In en, this message translates to:
  /// **'Tagliacarte'**
  String get appTitle;

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @compose.
  ///
  /// In en, this message translates to:
  /// **'Compose'**
  String get compose;

  /// No description provided for @send.
  ///
  /// In en, this message translates to:
  /// **'Send'**
  String get send;

  /// No description provided for @dialogOk.
  ///
  /// In en, this message translates to:
  /// **'OK'**
  String get dialogOk;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @remove.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get remove;

  /// No description provided for @delete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get delete;

  /// No description provided for @discard.
  ///
  /// In en, this message translates to:
  /// **'Discard'**
  String get discard;

  /// No description provided for @back.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get back;

  /// No description provided for @create.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get create;

  /// No description provided for @rename.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get rename;

  /// No description provided for @folderLabel.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get folderLabel;

  /// No description provided for @messageTitle.
  ///
  /// In en, this message translates to:
  /// **'Message'**
  String get messageTitle;

  /// No description provided for @selectFolder.
  ///
  /// In en, this message translates to:
  /// **'Select a folder'**
  String get selectFolder;

  /// No description provided for @selectMessage.
  ///
  /// In en, this message translates to:
  /// **'Select a message'**
  String get selectMessage;

  /// No description provided for @selectMessageToRead.
  ///
  /// In en, this message translates to:
  /// **'Select a message to read.'**
  String get selectMessageToRead;

  /// No description provided for @noMessages.
  ///
  /// In en, this message translates to:
  /// **'No messages'**
  String get noMessages;

  /// No description provided for @attachments.
  ///
  /// In en, this message translates to:
  /// **'Attachments'**
  String get attachments;

  /// No description provided for @saveAttachment.
  ///
  /// In en, this message translates to:
  /// **'Save attachment'**
  String get saveAttachment;

  /// No description provided for @savedToPath.
  ///
  /// In en, this message translates to:
  /// **'Saved {path}'**
  String savedToPath(String path);

  /// No description provided for @saveFailed.
  ///
  /// In en, this message translates to:
  /// **'Save failed: {error}'**
  String saveFailed(String error);

  /// No description provided for @cannotDownloadAttachment.
  ///
  /// In en, this message translates to:
  /// **'Cannot download this attachment'**
  String get cannotDownloadAttachment;

  /// No description provided for @emptyAttachmentData.
  ///
  /// In en, this message translates to:
  /// **'Empty attachment data'**
  String get emptyAttachmentData;

  /// No description provided for @downloadFailed.
  ///
  /// In en, this message translates to:
  /// **'Download failed: {error}'**
  String downloadFailed(String error);

  /// No description provided for @saveVerb.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get saveVerb;

  /// No description provided for @loadImages.
  ///
  /// In en, this message translates to:
  /// **'Load images'**
  String get loadImages;

  /// No description provided for @remoteImagesBlocked.
  ///
  /// In en, this message translates to:
  /// **'Remote images blocked for privacy.'**
  String get remoteImagesBlocked;

  /// No description provided for @couldNotOpenHtmlBody.
  ///
  /// In en, this message translates to:
  /// **'Could not open HTML body: {error}'**
  String couldNotOpenHtmlBody(String error);

  /// No description provided for @webViewError.
  ///
  /// In en, this message translates to:
  /// **'WebView error: {error}'**
  String webViewError(String error);

  /// Shown above the real URL when the anchor text looks like a URL but href differs (phishing).
  ///
  /// In en, this message translates to:
  /// **'The visible link text shows a different address than where this link goes.'**
  String get linkHoverMisleadingCaption;

  /// No description provided for @headerFrom.
  ///
  /// In en, this message translates to:
  /// **'From:'**
  String get headerFrom;

  /// No description provided for @headerTo.
  ///
  /// In en, this message translates to:
  /// **'To:'**
  String get headerTo;

  /// No description provided for @headerCc.
  ///
  /// In en, this message translates to:
  /// **'Cc:'**
  String get headerCc;

  /// No description provided for @headerDate.
  ///
  /// In en, this message translates to:
  /// **'Date:'**
  String get headerDate;

  /// Localized label for the canonical INBOX folder (IMAP/maildir synthesised name).
  ///
  /// In en, this message translates to:
  /// **'Inbox'**
  String get folderInbox;

  /// No description provided for @messageActionReply.
  ///
  /// In en, this message translates to:
  /// **'Reply'**
  String get messageActionReply;

  /// No description provided for @messageActionReplyAll.
  ///
  /// In en, this message translates to:
  /// **'Reply all'**
  String get messageActionReplyAll;

  /// No description provided for @messageActionForward.
  ///
  /// In en, this message translates to:
  /// **'Forward'**
  String get messageActionForward;

  /// No description provided for @messageActionDelete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get messageActionDelete;

  /// No description provided for @messageActionJunk.
  ///
  /// In en, this message translates to:
  /// **'Junk'**
  String get messageActionJunk;

  /// No description provided for @messageActionMove.
  ///
  /// In en, this message translates to:
  /// **'Move'**
  String get messageActionMove;

  /// No description provided for @messageActionCopy.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get messageActionCopy;

  /// No description provided for @messageMenuTooltip.
  ///
  /// In en, this message translates to:
  /// **'Message actions'**
  String get messageMenuTooltip;

  /// No description provided for @settingsViewMinimalHeaders.
  ///
  /// In en, this message translates to:
  /// **'Minimal message headers'**
  String get settingsViewMinimalHeaders;

  /// No description provided for @settingsViewMinimalHeadersSubtitle.
  ///
  /// In en, this message translates to:
  /// **'When on, hide Cc only; From, To, and Date still show when available.'**
  String get settingsViewMinimalHeadersSubtitle;

  /// No description provided for @settingsTooltip.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settingsTooltip;

  /// No description provided for @accountsAndFoldersTooltip.
  ///
  /// In en, this message translates to:
  /// **'Accounts and folders'**
  String get accountsAndFoldersTooltip;

  /// No description provided for @cancelSelectionTooltip.
  ///
  /// In en, this message translates to:
  /// **'Cancel selection'**
  String get cancelSelectionTooltip;

  /// No description provided for @multiSelectCount.
  ///
  /// In en, this message translates to:
  /// **'{count} selected'**
  String multiSelectCount(int count);

  /// No description provided for @composeTooltip.
  ///
  /// In en, this message translates to:
  /// **'Compose'**
  String get composeTooltip;

  /// No description provided for @composeNeedTransportTooltip.
  ///
  /// In en, this message translates to:
  /// **'Add an outgoing transport in Settings'**
  String get composeNeedTransportTooltip;

  /// No description provided for @mailToolbarMoreTooltip.
  ///
  /// In en, this message translates to:
  /// **'More'**
  String get mailToolbarMoreTooltip;

  /// No description provided for @mailToolbarSelectedCount.
  ///
  /// In en, this message translates to:
  /// **'{count} selected'**
  String mailToolbarSelectedCount(int count);

  /// No description provided for @settingsTabAccounts.
  ///
  /// In en, this message translates to:
  /// **'Accounts'**
  String get settingsTabAccounts;

  /// No description provided for @settingsTabOutgoing.
  ///
  /// In en, this message translates to:
  /// **'Outgoing'**
  String get settingsTabOutgoing;

  /// No description provided for @settingsTabSecurity.
  ///
  /// In en, this message translates to:
  /// **'Security'**
  String get settingsTabSecurity;

  /// No description provided for @settingsTabViewing.
  ///
  /// In en, this message translates to:
  /// **'Viewing'**
  String get settingsTabViewing;

  /// No description provided for @settingsTabComposing.
  ///
  /// In en, this message translates to:
  /// **'Composing'**
  String get settingsTabComposing;

  /// No description provided for @settingsTabAbout.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get settingsTabAbout;

  /// No description provided for @settingsLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not load settings from disk.'**
  String get settingsLoadFailed;

  /// No description provided for @settingsLoadRetry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get settingsLoadRetry;

  /// No description provided for @useSystemKeychain.
  ///
  /// In en, this message translates to:
  /// **'Use system keychain'**
  String get useSystemKeychain;

  /// No description provided for @storeCredentialsInKeychain.
  ///
  /// In en, this message translates to:
  /// **'Store credentials in platform keychain'**
  String get storeCredentialsInKeychain;

  /// No description provided for @oauthSection.
  ///
  /// In en, this message translates to:
  /// **'OAuth'**
  String get oauthSection;

  /// No description provided for @authenticateGoogle.
  ///
  /// In en, this message translates to:
  /// **'Authenticate Google'**
  String get authenticateGoogle;

  /// No description provided for @authenticateMicrosoft.
  ///
  /// In en, this message translates to:
  /// **'Authenticate Microsoft'**
  String get authenticateMicrosoft;

  /// No description provided for @reloadOAuthToken.
  ///
  /// In en, this message translates to:
  /// **'Reload OAuth Token'**
  String get reloadOAuthToken;

  /// No description provided for @matrixE2eeSection.
  ///
  /// In en, this message translates to:
  /// **'Matrix E2EE'**
  String get matrixE2eeSection;

  /// No description provided for @initCrypto.
  ///
  /// In en, this message translates to:
  /// **'Init Crypto'**
  String get initCrypto;

  /// No description provided for @setupBackup.
  ///
  /// In en, this message translates to:
  /// **'Setup Backup'**
  String get setupBackup;

  /// No description provided for @restoreBackup.
  ///
  /// In en, this message translates to:
  /// **'Restore Backup'**
  String get restoreBackup;

  /// No description provided for @showDeviceFingerprint.
  ///
  /// In en, this message translates to:
  /// **'Show Device Fingerprint'**
  String get showDeviceFingerprint;

  /// No description provided for @messageDetailInlineDesktopTitle.
  ///
  /// In en, this message translates to:
  /// **'Message detail below list (desktop)'**
  String get messageDetailInlineDesktopTitle;

  /// No description provided for @messageDetailInlineDesktopSubtitle.
  ///
  /// In en, this message translates to:
  /// **'When off, opening a message uses a separate full-screen view.'**
  String get messageDetailInlineDesktopSubtitle;

  /// No description provided for @loadRemoteImages.
  ///
  /// In en, this message translates to:
  /// **'Load remote images'**
  String get loadRemoteImages;

  /// No description provided for @loadRemoteImagesSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Allow external images in HTML email'**
  String get loadRemoteImagesSubtitle;

  /// No description provided for @threadedView.
  ///
  /// In en, this message translates to:
  /// **'Threaded view'**
  String get threadedView;

  /// No description provided for @threadedViewSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Group email messages by thread'**
  String get threadedViewSubtitle;

  /// No description provided for @deletionAndTrashSection.
  ///
  /// In en, this message translates to:
  /// **'Deletion & trash'**
  String get deletionAndTrashSection;

  /// No description provided for @deletionAppliesGlobally.
  ///
  /// In en, this message translates to:
  /// **'Applies to mail-style accounts globally.'**
  String get deletionAppliesGlobally;

  /// No description provided for @deleteModeLabel.
  ///
  /// In en, this message translates to:
  /// **'Delete mode'**
  String get deleteModeLabel;

  /// No description provided for @trashFolderNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Trash folder name'**
  String get trashFolderNameLabel;

  /// No description provided for @junkFolderNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Junk folder name'**
  String get junkFolderNameLabel;

  /// No description provided for @exchangeTrashFolderHelper.
  ///
  /// In en, this message translates to:
  /// **'Leave empty to use “Deleted Items” (English mailbox). Use the exact folder name shown in Outlook if yours differs.'**
  String get exchangeTrashFolderHelper;

  /// No description provided for @exchangeJunkFolderHelper.
  ///
  /// In en, this message translates to:
  /// **'Leave empty to use “Junk Email” (English mailbox). Use the exact folder name shown in Outlook if yours differs.'**
  String get exchangeJunkFolderHelper;

  /// No description provided for @deleteModeDeleteImmediately.
  ///
  /// In en, this message translates to:
  /// **'Delete immediately'**
  String get deleteModeDeleteImmediately;

  /// No description provided for @deleteModeMoveToTrash.
  ///
  /// In en, this message translates to:
  /// **'Move to Trash'**
  String get deleteModeMoveToTrash;

  /// No description provided for @deleteModeMarkDeleted.
  ///
  /// In en, this message translates to:
  /// **'Mark Deleted'**
  String get deleteModeMarkDeleted;

  /// No description provided for @quoteOriginalOnReply.
  ///
  /// In en, this message translates to:
  /// **'Quote original message on reply'**
  String get quoteOriginalOnReply;

  /// No description provided for @quoteOriginalOnReplySubtitle.
  ///
  /// In en, this message translates to:
  /// **'Adds the original under the reply header in new replies. Rich compose wraps it in a marked quote block; plain compose prefixes each line of the original. The text/plain part of the message still includes the original when this is on.'**
  String get quoteOriginalOnReplySubtitle;

  /// No description provided for @composingReplySection.
  ///
  /// In en, this message translates to:
  /// **'Reply quoting'**
  String get composingReplySection;

  /// No description provided for @replyHeaderTemplateLabel.
  ///
  /// In en, this message translates to:
  /// **'Reply header line'**
  String get replyHeaderTemplateLabel;

  /// No description provided for @replyHeaderTemplateHelp.
  ///
  /// In en, this message translates to:
  /// **'Shown above the quoted original. Include the three words date, time, and sender, each with a dollar sign immediately in front (see preview). They are replaced with the message’s date, time, and From when you reply.'**
  String get replyHeaderTemplateHelp;

  /// No description provided for @replyHeaderPreviewLabel.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get replyHeaderPreviewLabel;

  /// No description provided for @replyDateFormatLabel.
  ///
  /// In en, this message translates to:
  /// **'Reply date (in header)'**
  String get replyDateFormatLabel;

  /// No description provided for @replyTimeFormatLabel.
  ///
  /// In en, this message translates to:
  /// **'Reply time (in header)'**
  String get replyTimeFormatLabel;

  /// No description provided for @replyDatePresetLocale.
  ///
  /// In en, this message translates to:
  /// **'Same as system (long date)'**
  String get replyDatePresetLocale;

  /// No description provided for @replyDatePresetIso.
  ///
  /// In en, this message translates to:
  /// **'ISO: 2026-04-08'**
  String get replyDatePresetIso;

  /// No description provided for @replyDatePresetUs.
  ///
  /// In en, this message translates to:
  /// **'US: 04/08/2026'**
  String get replyDatePresetUs;

  /// No description provided for @replyDatePresetEu.
  ///
  /// In en, this message translates to:
  /// **'Day/month/year: 08/04/2026'**
  String get replyDatePresetEu;

  /// No description provided for @replyDatePresetMedium.
  ///
  /// In en, this message translates to:
  /// **'Medium: Apr 8, 2026'**
  String get replyDatePresetMedium;

  /// No description provided for @replyDatePresetWeekday.
  ///
  /// In en, this message translates to:
  /// **'With weekday: Wed, Apr 8, 2026'**
  String get replyDatePresetWeekday;

  /// No description provided for @replyDatePresetCustom.
  ///
  /// In en, this message translates to:
  /// **'Custom ({pattern})'**
  String replyDatePresetCustom(String pattern);

  /// No description provided for @replyTimePresetLocale.
  ///
  /// In en, this message translates to:
  /// **'Same as system'**
  String get replyTimePresetLocale;

  /// No description provided for @replyTimePreset12h.
  ///
  /// In en, this message translates to:
  /// **'12-hour (e.g. 1:30 PM)'**
  String get replyTimePreset12h;

  /// No description provided for @replyTimePreset24h.
  ///
  /// In en, this message translates to:
  /// **'24-hour (15:30)'**
  String get replyTimePreset24h;

  /// No description provided for @replyTimePreset24hSeconds.
  ///
  /// In en, this message translates to:
  /// **'24-hour with seconds'**
  String get replyTimePreset24hSeconds;

  /// No description provided for @replyTimePresetCustom.
  ///
  /// In en, this message translates to:
  /// **'Custom ({pattern})'**
  String replyTimePresetCustom(String pattern);

  /// No description provided for @replyLinePrefixLabel.
  ///
  /// In en, this message translates to:
  /// **'Quoted line prefix'**
  String get replyLinePrefixLabel;

  /// No description provided for @replyLinePrefixSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Prepended to each line of the original in plain-text quotes (classic “> ” quoting). Only used when quoting the original is enabled.'**
  String get replyLinePrefixSubtitle;

  /// No description provided for @replyPlainPositionLabel.
  ///
  /// In en, this message translates to:
  /// **'Ordering of quoted text'**
  String get replyPlainPositionLabel;

  /// No description provided for @replyPlainPositionBefore.
  ///
  /// In en, this message translates to:
  /// **'Reply before quoted text'**
  String get replyPlainPositionBefore;

  /// No description provided for @replyPlainPositionAfter.
  ///
  /// In en, this message translates to:
  /// **'Reply after quoted text'**
  String get replyPlainPositionAfter;

  /// No description provided for @replyPlainPositionSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Plain or rich compose: two blank lines and caret before the reply header, or two blank lines and caret after the quoted block. The text/plain part when sending follows the same layout.'**
  String get replyPlainPositionSubtitle;

  /// No description provided for @replyQuoteModeLabel.
  ///
  /// In en, this message translates to:
  /// **'SMTP HTML parts'**
  String get replyQuoteModeLabel;

  /// No description provided for @replyQuoteModePlain.
  ///
  /// In en, this message translates to:
  /// **'Original only in plain-text quote'**
  String get replyQuoteModePlain;

  /// No description provided for @replyQuoteModeHtmlSmtp.
  ///
  /// In en, this message translates to:
  /// **'Also include original as separate HTML (SMTP)'**
  String get replyQuoteModeHtmlSmtp;

  /// No description provided for @replyQuoteModeHtmlSmtpSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Adds a second HTML part preserving the source message’s formatting for HTML-capable clients. Plain-text-only clients still see the quoted plain body. NNTP posting always uses plain quoting.'**
  String get replyQuoteModeHtmlSmtpSubtitle;

  /// No description provided for @settingsComposeRichText.
  ///
  /// In en, this message translates to:
  /// **'Rich text in email compose'**
  String get settingsComposeRichText;

  /// No description provided for @settingsComposeRichTextSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Formatted editor for new mail and replies. Usenet (NNTP) posting stays plain text.'**
  String get settingsComposeRichTextSubtitle;

  /// No description provided for @settingsMatrixChatRichText.
  ///
  /// In en, this message translates to:
  /// **'Rich text in Matrix chats'**
  String get settingsMatrixChatRichText;

  /// No description provided for @settingsMatrixChatRichTextSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Send formatted messages in Matrix rooms (includes a plain-text fallback).'**
  String get settingsMatrixChatRichTextSubtitle;

  /// No description provided for @testSend.
  ///
  /// In en, this message translates to:
  /// **'Test Send'**
  String get testSend;

  /// No description provided for @openSignatureEditor.
  ///
  /// In en, this message translates to:
  /// **'Open Signature Editor'**
  String get openSignatureEditor;

  /// No description provided for @aboutSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Cross platform email and messaging'**
  String get aboutSubtitle;

  /// No description provided for @supportedBackends.
  ///
  /// In en, this message translates to:
  /// **'Supported backends'**
  String get supportedBackends;

  /// No description provided for @supportedBackendsList.
  ///
  /// In en, this message translates to:
  /// **'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph'**
  String get supportedBackendsList;

  /// No description provided for @licenseGpl.
  ///
  /// In en, this message translates to:
  /// **'GPLv3'**
  String get licenseGpl;

  /// No description provided for @copyrightLine.
  ///
  /// In en, this message translates to:
  /// **'Copyright (C) 2026 Chris Burdess'**
  String get copyrightLine;

  /// No description provided for @stubInvoked.
  ///
  /// In en, this message translates to:
  /// **'{operation} invoked'**
  String stubInvoked(String operation);

  /// No description provided for @accountTypeDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Account type'**
  String get accountTypeDialogTitle;

  /// No description provided for @removeAccountTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove account?'**
  String get removeAccountTitle;

  /// No description provided for @removeAccountBody.
  ///
  /// In en, this message translates to:
  /// **'Remove “{label}” from this device’s saved configuration?'**
  String removeAccountBody(String label);

  /// No description provided for @removedAccount.
  ///
  /// In en, this message translates to:
  /// **'Removed {label}'**
  String removedAccount(String label);

  /// No description provided for @accountsListTitle.
  ///
  /// In en, this message translates to:
  /// **'Accounts'**
  String get accountsListTitle;

  /// No description provided for @accountsListSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Tap an account to edit, or add a new one below.'**
  String get accountsListSubtitle;

  /// No description provided for @deleteTooltip.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get deleteTooltip;

  /// No description provided for @addAccount.
  ///
  /// In en, this message translates to:
  /// **'Add account'**
  String get addAccount;

  /// No description provided for @noAccountsYet.
  ///
  /// In en, this message translates to:
  /// **'No accounts yet. Tap “Add account” to create one.'**
  String get noAccountsYet;

  /// No description provided for @discardChangesTitle.
  ///
  /// In en, this message translates to:
  /// **'Discard changes?'**
  String get discardChangesTitle;

  /// No description provided for @discardChangesBody.
  ///
  /// In en, this message translates to:
  /// **'Your edits will be lost. This matches leaving without saving.'**
  String get discardChangesBody;

  /// No description provided for @keepEditing.
  ///
  /// In en, this message translates to:
  /// **'Keep editing'**
  String get keepEditing;

  /// No description provided for @pickNotSupportedWeb.
  ///
  /// In en, this message translates to:
  /// **'Picking files or folders is not supported in the web build'**
  String get pickNotSupportedWeb;

  /// No description provided for @chooseMaildirFolderTitle.
  ///
  /// In en, this message translates to:
  /// **'Choose Maildir root folder'**
  String get chooseMaildirFolderTitle;

  /// No description provided for @chooseMboxFileTitle.
  ///
  /// In en, this message translates to:
  /// **'Choose mbox file'**
  String get chooseMboxFileTitle;

  /// No description provided for @validationAccountNameRequired.
  ///
  /// In en, this message translates to:
  /// **'Account name is required'**
  String get validationAccountNameRequired;

  /// No description provided for @validationLocalPathRequired.
  ///
  /// In en, this message translates to:
  /// **'Local mailbox path is required'**
  String get validationLocalPathRequired;

  /// No description provided for @validationUsernameRequired.
  ///
  /// In en, this message translates to:
  /// **'Username / email is required for this account type'**
  String get validationUsernameRequired;

  /// No description provided for @validationEmailAddressRequired.
  ///
  /// In en, this message translates to:
  /// **'Email address is required'**
  String get validationEmailAddressRequired;

  /// No description provided for @validationMatrixUserIdRequired.
  ///
  /// In en, this message translates to:
  /// **'Matrix user ID is required'**
  String get validationMatrixUserIdRequired;

  /// No description provided for @accountEmailAddressLabel.
  ///
  /// In en, this message translates to:
  /// **'Email address'**
  String get accountEmailAddressLabel;

  /// No description provided for @accountMatrixUserIdLabel.
  ///
  /// In en, this message translates to:
  /// **'Matrix ID (MXID)'**
  String get accountMatrixUserIdLabel;

  /// No description provided for @accountMatrixMxidHelper.
  ///
  /// In en, this message translates to:
  /// **'Example: @you:matrix.org — homeserver URL is derived from the domain after the colon.'**
  String get accountMatrixMxidHelper;

  /// No description provided for @validationMatrixMxidInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter a Matrix ID like @user:server'**
  String get validationMatrixMxidInvalid;

  /// No description provided for @accountNntpDefaultFromLabel.
  ///
  /// In en, this message translates to:
  /// **'Default From (Usenet)'**
  String get accountNntpDefaultFromLabel;

  /// No description provided for @accountNntpDefaultFromHelper.
  ///
  /// In en, this message translates to:
  /// **'Shown when composing posts; this NNTP account posts through its own server connection.'**
  String get accountNntpDefaultFromHelper;

  /// No description provided for @accountEmailOptionalLabel.
  ///
  /// In en, this message translates to:
  /// **'Email address (optional)'**
  String get accountEmailOptionalLabel;

  /// No description provided for @accountTcpLoginHelper.
  ///
  /// In en, this message translates to:
  /// **'Sign-in identity for this server (usually your email address).'**
  String get accountTcpLoginHelper;

  /// No description provided for @validationHostRequired.
  ///
  /// In en, this message translates to:
  /// **'Server host is required'**
  String get validationHostRequired;

  /// No description provided for @validationPortRequired.
  ///
  /// In en, this message translates to:
  /// **'Valid port number is required'**
  String get validationPortRequired;

  /// No description provided for @accountSaved.
  ///
  /// In en, this message translates to:
  /// **'Account saved'**
  String get accountSaved;

  /// No description provided for @createTransportFirst.
  ///
  /// In en, this message translates to:
  /// **'Create a transport on the Outgoing tab first'**
  String get createTransportFirst;

  /// No description provided for @addTransportDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Add transport'**
  String get addTransportDialogTitle;

  /// No description provided for @accountTypeLabel.
  ///
  /// In en, this message translates to:
  /// **'Account type'**
  String get accountTypeLabel;

  /// No description provided for @accountTypeHelper.
  ///
  /// In en, this message translates to:
  /// **'Chosen when you add the account; it cannot be changed here.'**
  String get accountTypeHelper;

  /// No description provided for @accountNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Account name'**
  String get accountNameLabel;

  /// No description provided for @usernameEmailOptional.
  ///
  /// In en, this message translates to:
  /// **'Username / email (optional)'**
  String get usernameEmailOptional;

  /// No description provided for @usernameEmailRequired.
  ///
  /// In en, this message translates to:
  /// **'Username / email'**
  String get usernameEmailRequired;

  /// No description provided for @avatarUrlLabel.
  ///
  /// In en, this message translates to:
  /// **'Avatar URL or file path (optional)'**
  String get avatarUrlLabel;

  /// No description provided for @avatarUrlHelper.
  ///
  /// In en, this message translates to:
  /// **'Optional image URL or local file path for the account strip'**
  String get avatarUrlHelper;

  /// No description provided for @localMailboxSection.
  ///
  /// In en, this message translates to:
  /// **'Local mailbox'**
  String get localMailboxSection;

  /// No description provided for @pathMboxFile.
  ///
  /// In en, this message translates to:
  /// **'Path to mbox file'**
  String get pathMboxFile;

  /// No description provided for @pathMaildirRoot.
  ///
  /// In en, this message translates to:
  /// **'Path to Maildir root'**
  String get pathMaildirRoot;

  /// No description provided for @helperMboxPath.
  ///
  /// In en, this message translates to:
  /// **'Use the file button to browse, or type an absolute path'**
  String get helperMboxPath;

  /// No description provided for @helperMaildirPath.
  ///
  /// In en, this message translates to:
  /// **'Use the folder button to browse, or type an absolute path'**
  String get helperMaildirPath;

  /// No description provided for @chooseMboxTooltip.
  ///
  /// In en, this message translates to:
  /// **'Choose mbox file'**
  String get chooseMboxTooltip;

  /// No description provided for @chooseMaildirTooltip.
  ///
  /// In en, this message translates to:
  /// **'Choose Maildir folder'**
  String get chooseMaildirTooltip;

  /// No description provided for @imapServerSection.
  ///
  /// In en, this message translates to:
  /// **'IMAP server'**
  String get imapServerSection;

  /// No description provided for @pop3ServerSection.
  ///
  /// In en, this message translates to:
  /// **'POP3 server'**
  String get pop3ServerSection;

  /// No description provided for @nntpServerSection.
  ///
  /// In en, this message translates to:
  /// **'NNTP server'**
  String get nntpServerSection;

  /// No description provided for @hostLabel.
  ///
  /// In en, this message translates to:
  /// **'Host'**
  String get hostLabel;

  /// No description provided for @serverHostLabel.
  ///
  /// In en, this message translates to:
  /// **'Server host'**
  String get serverHostLabel;

  /// No description provided for @portLabel.
  ///
  /// In en, this message translates to:
  /// **'Port'**
  String get portLabel;

  /// No description provided for @portHelperImap.
  ///
  /// In en, this message translates to:
  /// **'Usually 993 (IMAPS) or 143 (STARTTLS)'**
  String get portHelperImap;

  /// No description provided for @portHelperPop3.
  ///
  /// In en, this message translates to:
  /// **'Usually 995 (POP3S, implicit TLS)'**
  String get portHelperPop3;

  /// No description provided for @portHelperNntp.
  ///
  /// In en, this message translates to:
  /// **'Usually 563 (NNTPS, implicit TLS)'**
  String get portHelperNntp;

  /// No description provided for @securityLabel.
  ///
  /// In en, this message translates to:
  /// **'Security'**
  String get securityLabel;

  /// No description provided for @mailSecurityImplicitTlsImap.
  ///
  /// In en, this message translates to:
  /// **'IMAPS (implicit TLS)'**
  String get mailSecurityImplicitTlsImap;

  /// No description provided for @mailSecurityImplicitTlsSmtp.
  ///
  /// In en, this message translates to:
  /// **'SMTPS (implicit TLS)'**
  String get mailSecurityImplicitTlsSmtp;

  /// No description provided for @mailSecurityImplicitTlsPop3.
  ///
  /// In en, this message translates to:
  /// **'POP3S (implicit TLS)'**
  String get mailSecurityImplicitTlsPop3;

  /// No description provided for @mailSecurityImplicitTlsNntp.
  ///
  /// In en, this message translates to:
  /// **'NNTPS (implicit TLS)'**
  String get mailSecurityImplicitTlsNntp;

  /// No description provided for @mailSecurityStarttls.
  ///
  /// In en, this message translates to:
  /// **'STARTTLS'**
  String get mailSecurityStarttls;

  /// No description provided for @mailSecurityNoEncryption.
  ///
  /// In en, this message translates to:
  /// **'No encryption'**
  String get mailSecurityNoEncryption;

  /// No description provided for @outgoingTransportsSection.
  ///
  /// In en, this message translates to:
  /// **'Outgoing transports'**
  String get outgoingTransportsSection;

  /// No description provided for @noTransportsHintLinked.
  ///
  /// In en, this message translates to:
  /// **'No transports selected — compose and reply stay disabled until you pick at least one. Use the Outgoing tab and select it here.'**
  String get noTransportsHintLinked;

  /// No description provided for @transportsOrderHint.
  ///
  /// In en, this message translates to:
  /// **'First in the list is the default for send. Use Outgoing to create transports.'**
  String get transportsOrderHint;

  /// No description provided for @unknownTransport.
  ///
  /// In en, this message translates to:
  /// **'Unknown transport'**
  String get unknownTransport;

  /// No description provided for @moveUpTooltip.
  ///
  /// In en, this message translates to:
  /// **'Move up'**
  String get moveUpTooltip;

  /// No description provided for @moveDownTooltip.
  ///
  /// In en, this message translates to:
  /// **'Move down'**
  String get moveDownTooltip;

  /// No description provided for @removeFromAccountTooltip.
  ///
  /// In en, this message translates to:
  /// **'Remove from account'**
  String get removeFromAccountTooltip;

  /// No description provided for @addTransportToAccount.
  ///
  /// In en, this message translates to:
  /// **'Add transport to account'**
  String get addTransportToAccount;

  /// No description provided for @matrixSection.
  ///
  /// In en, this message translates to:
  /// **'Matrix'**
  String get matrixSection;

  /// No description provided for @homeserverLabel.
  ///
  /// In en, this message translates to:
  /// **'Homeserver'**
  String get homeserverLabel;

  /// No description provided for @nostrSection.
  ///
  /// In en, this message translates to:
  /// **'Nostr'**
  String get nostrSection;

  /// No description provided for @relayUrlsLabel.
  ///
  /// In en, this message translates to:
  /// **'Relay URLs'**
  String get relayUrlsLabel;

  /// No description provided for @relayUrlsHelper.
  ///
  /// In en, this message translates to:
  /// **'Each row is one relay WebSocket URL. Press Enter when you finish editing a URL.'**
  String get relayUrlsHelper;

  /// No description provided for @relayAddFieldHint.
  ///
  /// In en, this message translates to:
  /// **'New relay URL'**
  String get relayAddFieldHint;

  /// No description provided for @relayAddTooltip.
  ///
  /// In en, this message translates to:
  /// **'Add relay'**
  String get relayAddTooltip;

  /// No description provided for @relayRemoveTooltip.
  ///
  /// In en, this message translates to:
  /// **'Remove relay'**
  String get relayRemoveTooltip;

  /// No description provided for @nostrNewIdentityTooltip.
  ///
  /// In en, this message translates to:
  /// **'Create new Nostr identity'**
  String get nostrNewIdentityTooltip;

  /// No description provided for @nostrRelayUrlsRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter at least one relay URL.'**
  String get nostrRelayUrlsRequired;

  /// No description provided for @storeUriLabel.
  ///
  /// In en, this message translates to:
  /// **'Connection: {uri}'**
  String storeUriLabel(String uri);

  /// No description provided for @transportUriLabel.
  ///
  /// In en, this message translates to:
  /// **'Legacy outbound URI: {uri}'**
  String transportUriLabel(String uri);

  /// No description provided for @accountDetailTitleNew.
  ///
  /// In en, this message translates to:
  /// **'New {type}'**
  String accountDetailTitleNew(String type);

  /// No description provided for @accountDetailTitleEdit.
  ///
  /// In en, this message translates to:
  /// **'Edit {label}'**
  String accountDetailTitleEdit(String label);

  /// No description provided for @foldersLoadError.
  ///
  /// In en, this message translates to:
  /// **'Folders: {error}'**
  String foldersLoadError(String error);

  /// No description provided for @sortMessagesTooltip.
  ///
  /// In en, this message translates to:
  /// **'Sort messages'**
  String get sortMessagesTooltip;

  /// No description provided for @sort.
  ///
  /// In en, this message translates to:
  /// **'Sort'**
  String get sort;

  /// No description provided for @sortFromAz.
  ///
  /// In en, this message translates to:
  /// **'From A–Z'**
  String get sortFromAz;

  /// No description provided for @sortFromZa.
  ///
  /// In en, this message translates to:
  /// **'From Z–A'**
  String get sortFromZa;

  /// No description provided for @sortSubjectAz.
  ///
  /// In en, this message translates to:
  /// **'Subject A–Z'**
  String get sortSubjectAz;

  /// No description provided for @sortSubjectZa.
  ///
  /// In en, this message translates to:
  /// **'Subject Z–A'**
  String get sortSubjectZa;

  /// No description provided for @sortDateOldest.
  ///
  /// In en, this message translates to:
  /// **'Date oldest first'**
  String get sortDateOldest;

  /// No description provided for @sortDateNewest.
  ///
  /// In en, this message translates to:
  /// **'Date newest first'**
  String get sortDateNewest;

  /// No description provided for @removeTransportTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove transport?'**
  String get removeTransportTitle;

  /// No description provided for @removeTransportBody.
  ///
  /// In en, this message translates to:
  /// **'“{name}” will be removed from all accounts’ outgoing lists.'**
  String removeTransportBody(String name);

  /// No description provided for @removedTransport.
  ///
  /// In en, this message translates to:
  /// **'Removed {name}'**
  String removedTransport(String name);

  /// No description provided for @outgoingListTitle.
  ///
  /// In en, this message translates to:
  /// **'Outgoing'**
  String get outgoingListTitle;

  /// No description provided for @outgoingListSubtitle.
  ///
  /// In en, this message translates to:
  /// **'SMTP and other send transports. Link them to mail accounts on the Accounts tab.'**
  String get outgoingListSubtitle;

  /// No description provided for @addTransport.
  ///
  /// In en, this message translates to:
  /// **'Add transport'**
  String get addTransport;

  /// No description provided for @noTransportsYet.
  ///
  /// In en, this message translates to:
  /// **'No outgoing transports yet. Tap “Add transport” to create one.'**
  String get noTransportsYet;

  /// No description provided for @transportDisplayHostRequired.
  ///
  /// In en, this message translates to:
  /// **'Display name and host are required.'**
  String get transportDisplayHostRequired;

  /// No description provided for @transportSaved.
  ///
  /// In en, this message translates to:
  /// **'Transport saved'**
  String get transportSaved;

  /// No description provided for @transportSavedAndVerified.
  ///
  /// In en, this message translates to:
  /// **'Transport saved and SMTP verified'**
  String get transportSavedAndVerified;

  /// No description provided for @transportSavedVerifyPending.
  ///
  /// In en, this message translates to:
  /// **'Transport saved, but the server could not be reached or authenticated. Check host, security, and credentials, then press Save again.'**
  String get transportSavedVerifyPending;

  /// No description provided for @transportTypeDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Outgoing transport type'**
  String get transportTypeDialogTitle;

  /// No description provided for @transportTypeFixedHelper.
  ///
  /// In en, this message translates to:
  /// **'Chosen when you added this transport; it cannot be changed here.'**
  String get transportTypeFixedHelper;

  /// No description provided for @transportDisplayNameRequired.
  ///
  /// In en, this message translates to:
  /// **'Display name is required.'**
  String get transportDisplayNameRequired;

  /// No description provided for @transportKindLabel.
  ///
  /// In en, this message translates to:
  /// **'Outgoing type'**
  String get transportKindLabel;

  /// No description provided for @transportKindSmtp.
  ///
  /// In en, this message translates to:
  /// **'SMTP'**
  String get transportKindSmtp;

  /// No description provided for @transportKindGmail.
  ///
  /// In en, this message translates to:
  /// **'Gmail (Google)'**
  String get transportKindGmail;

  /// No description provided for @gmailTransportPresetHelper.
  ///
  /// In en, this message translates to:
  /// **'Uses smtp.gmail.com with OAuth (XOAUTH2). Save the transport, then sign in with the same Google account you use for Gmail IMAP.'**
  String get gmailTransportPresetHelper;

  /// No description provided for @newTransport.
  ///
  /// In en, this message translates to:
  /// **'New transport'**
  String get newTransport;

  /// No description provided for @editTransport.
  ///
  /// In en, this message translates to:
  /// **'Edit transport'**
  String get editTransport;

  /// No description provided for @displayNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Display name'**
  String get displayNameLabel;

  /// No description provided for @smtpHostLabel.
  ///
  /// In en, this message translates to:
  /// **'SMTP host'**
  String get smtpHostLabel;

  /// No description provided for @smtpPortHelper.
  ///
  /// In en, this message translates to:
  /// **'Usually 587 (STARTTLS) or 465 (SMTPS)'**
  String get smtpPortHelper;

  /// No description provided for @imapSignInTitle.
  ///
  /// In en, this message translates to:
  /// **'IMAP sign-in'**
  String get imapSignInTitle;

  /// No description provided for @matrixSignInTitle.
  ///
  /// In en, this message translates to:
  /// **'Matrix sign-in'**
  String get matrixSignInTitle;

  /// No description provided for @gmailSignInTitle.
  ///
  /// In en, this message translates to:
  /// **'Sign in with Google'**
  String get gmailSignInTitle;

  /// No description provided for @gmailSignInBody.
  ///
  /// In en, this message translates to:
  /// **'Your browser will open to authorize Gmail (IMAP). Requires TAGLIACARTE_GOOGLE_CLIENT_ID (and usually TAGLIACARTE_GOOGLE_CLIENT_SECRET) to be set for the app.'**
  String get gmailSignInBody;

  /// No description provided for @gmailSignInBrowserButton.
  ///
  /// In en, this message translates to:
  /// **'Continue in browser'**
  String get gmailSignInBrowserButton;

  /// No description provided for @smtpSignInTitle.
  ///
  /// In en, this message translates to:
  /// **'SMTP sign-in'**
  String get smtpSignInTitle;

  /// No description provided for @smtpSignInSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Enter the username and password for “{transportName}” ({host}).'**
  String smtpSignInSubtitle(String transportName, String host);

  /// No description provided for @composeSendCancelledNoSmtpCredentials.
  ///
  /// In en, this message translates to:
  /// **'Message not sent: SMTP credentials were not saved.'**
  String get composeSendCancelledNoSmtpCredentials;

  /// No description provided for @enterUsernameAndPassword.
  ///
  /// In en, this message translates to:
  /// **'Enter username and password.'**
  String get enterUsernameAndPassword;

  /// No description provided for @usernameLabel.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get usernameLabel;

  /// No description provided for @passwordLabel.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get passwordLabel;

  /// No description provided for @showPasswordTooltip.
  ///
  /// In en, this message translates to:
  /// **'Show password'**
  String get showPasswordTooltip;

  /// No description provided for @hidePasswordTooltip.
  ///
  /// In en, this message translates to:
  /// **'Hide password'**
  String get hidePasswordTooltip;

  /// No description provided for @fieldFrom.
  ///
  /// In en, this message translates to:
  /// **'From'**
  String get fieldFrom;

  /// No description provided for @composeOutgoingTransport.
  ///
  /// In en, this message translates to:
  /// **'Outgoing transport'**
  String get composeOutgoingTransport;

  /// No description provided for @composeSendSucceeded.
  ///
  /// In en, this message translates to:
  /// **'Message sent'**
  String get composeSendSucceeded;

  /// No description provided for @composeMissingFrom.
  ///
  /// In en, this message translates to:
  /// **'Enter a from address.'**
  String get composeMissingFrom;

  /// No description provided for @composeMissingTo.
  ///
  /// In en, this message translates to:
  /// **'Enter at least one recipient.'**
  String get composeMissingTo;

  /// No description provided for @composeMissingNewsgroups.
  ///
  /// In en, this message translates to:
  /// **'Enter at least one newsgroup name.'**
  String get composeMissingNewsgroups;

  /// No description provided for @composeNntpPostingBlurb.
  ///
  /// In en, this message translates to:
  /// **'Posts are sent through this account’s NNTP server (no separate transport).'**
  String get composeNntpPostingBlurb;

  /// No description provided for @fieldNewsgroups.
  ///
  /// In en, this message translates to:
  /// **'Newsgroups'**
  String get fieldNewsgroups;

  /// No description provided for @fieldTo.
  ///
  /// In en, this message translates to:
  /// **'To'**
  String get fieldTo;

  /// No description provided for @fieldCc.
  ///
  /// In en, this message translates to:
  /// **'Cc'**
  String get fieldCc;

  /// No description provided for @fieldBcc.
  ///
  /// In en, this message translates to:
  /// **'Bcc'**
  String get fieldBcc;

  /// No description provided for @fieldSubject.
  ///
  /// In en, this message translates to:
  /// **'Subject'**
  String get fieldSubject;

  /// No description provided for @fieldBody.
  ///
  /// In en, this message translates to:
  /// **'Body'**
  String get fieldBody;

  /// No description provided for @attach.
  ///
  /// In en, this message translates to:
  /// **'Attach'**
  String get attach;

  /// No description provided for @composeRemoveAttachment.
  ///
  /// In en, this message translates to:
  /// **'Remove attachment'**
  String get composeRemoveAttachment;

  /// No description provided for @defaultFromLabel.
  ///
  /// In en, this message translates to:
  /// **'Default From address'**
  String get defaultFromLabel;

  /// No description provided for @defaultFromHelper.
  ///
  /// In en, this message translates to:
  /// **'e.g. Your Name <you@example.com> or you@example.com'**
  String get defaultFromHelper;

  /// No description provided for @dsnLabel.
  ///
  /// In en, this message translates to:
  /// **'Delivery notifications'**
  String get dsnLabel;

  /// No description provided for @dsnUseTransportDefault.
  ///
  /// In en, this message translates to:
  /// **'Use transport default'**
  String get dsnUseTransportDefault;

  /// No description provided for @dsnNever.
  ///
  /// In en, this message translates to:
  /// **'Never'**
  String get dsnNever;

  /// No description provided for @dsnFailure.
  ///
  /// In en, this message translates to:
  /// **'On failure'**
  String get dsnFailure;

  /// No description provided for @dsnSuccess.
  ///
  /// In en, this message translates to:
  /// **'On success'**
  String get dsnSuccess;

  /// No description provided for @dsnDelay.
  ///
  /// In en, this message translates to:
  /// **'On delay'**
  String get dsnDelay;

  /// No description provided for @dsnFailureAndSuccess.
  ///
  /// In en, this message translates to:
  /// **'On failure and success'**
  String get dsnFailureAndSuccess;

  /// No description provided for @dsnNotifyLabel.
  ///
  /// In en, this message translates to:
  /// **'DSN notify'**
  String get dsnNotifyLabel;

  /// No description provided for @folderNewSubfolder.
  ///
  /// In en, this message translates to:
  /// **'New subfolder'**
  String get folderNewSubfolder;

  /// No description provided for @folderRename.
  ///
  /// In en, this message translates to:
  /// **'Rename…'**
  String get folderRename;

  /// No description provided for @folderDelete.
  ///
  /// In en, this message translates to:
  /// **'Delete…'**
  String get folderDelete;

  /// No description provided for @folderNewTooltip.
  ///
  /// In en, this message translates to:
  /// **'New folder'**
  String get folderNewTooltip;

  /// No description provided for @folderNewDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'New folder'**
  String get folderNewDialogTitle;

  /// No description provided for @folderNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Folder name'**
  String get folderNameLabel;

  /// No description provided for @folderNewTopLevelHelper.
  ///
  /// In en, this message translates to:
  /// **'Creates a mailbox at the top level'**
  String get folderNewTopLevelHelper;

  /// No description provided for @subfolderDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Subfolder of {parent}'**
  String subfolderDialogTitle(String parent);

  /// No description provided for @subfolderNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Subfolder name'**
  String get subfolderNameLabel;

  /// No description provided for @subfolderPathHelper.
  ///
  /// In en, this message translates to:
  /// **'Path: {path}'**
  String subfolderPathHelper(String path);

  /// No description provided for @folderCreated.
  ///
  /// In en, this message translates to:
  /// **'Created folder “{name}”'**
  String folderCreated(String name);

  /// No description provided for @renameFolderTitle.
  ///
  /// In en, this message translates to:
  /// **'Rename folder'**
  String get renameFolderTitle;

  /// No description provided for @newFolderPathLabel.
  ///
  /// In en, this message translates to:
  /// **'New folder path'**
  String get newFolderPathLabel;

  /// No description provided for @folderRenamed.
  ///
  /// In en, this message translates to:
  /// **'Folder renamed'**
  String get folderRenamed;

  /// No description provided for @deleteFolderTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete folder?'**
  String get deleteFolderTitle;

  /// No description provided for @deleteFolderBody.
  ///
  /// In en, this message translates to:
  /// **'Remove “{name}” and its messages from the server (if supported)? This cannot be undone.'**
  String deleteFolderBody(String name);

  /// No description provided for @folderDeleted.
  ///
  /// In en, this message translates to:
  /// **'Folder deleted'**
  String get folderDeleted;

  /// No description provided for @licenseTitle.
  ///
  /// In en, this message translates to:
  /// **'License'**
  String get licenseTitle;

  /// No description provided for @copyrightTitle.
  ///
  /// In en, this message translates to:
  /// **'Copyright'**
  String get copyrightTitle;

  /// No description provided for @chatHintTypeMessage.
  ///
  /// In en, this message translates to:
  /// **'Type a message'**
  String get chatHintTypeMessage;

  /// No description provided for @chatAttachmentsNotSentInChat.
  ///
  /// In en, this message translates to:
  /// **'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.'**
  String get chatAttachmentsNotSentInChat;

  /// No description provided for @operationFailed.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong: {error}'**
  String operationFailed(String error);

  /// No description provided for @expandFolder.
  ///
  /// In en, this message translates to:
  /// **'Expand'**
  String get expandFolder;

  /// No description provided for @collapseFolder.
  ///
  /// In en, this message translates to:
  /// **'Collapse'**
  String get collapseFolder;

  /// No description provided for @noTextBody.
  ///
  /// In en, this message translates to:
  /// **'(No text body)'**
  String get noTextBody;

  /// No description provided for @messageActionFeedback.
  ///
  /// In en, this message translates to:
  /// **'{label} · {messageId}'**
  String messageActionFeedback(String label, String messageId);

  /// No description provided for @folderMoveHere.
  ///
  /// In en, this message translates to:
  /// **'Move here'**
  String get folderMoveHere;

  /// No description provided for @folderCopyHere.
  ///
  /// In en, this message translates to:
  /// **'Copy here'**
  String get folderCopyHere;

  /// No description provided for @folderExpunge.
  ///
  /// In en, this message translates to:
  /// **'Expunge deleted messages'**
  String get folderExpunge;

  /// No description provided for @folderExpungeDone.
  ///
  /// In en, this message translates to:
  /// **'Expunge completed'**
  String get folderExpungeDone;

  /// No description provided for @pendingMoveTagged.
  ///
  /// In en, this message translates to:
  /// **'Pick a folder, then choose Move here ({count} messages)'**
  String pendingMoveTagged(int count);

  /// No description provided for @pendingCopyTagged.
  ///
  /// In en, this message translates to:
  /// **'Pick a folder, then choose Copy here ({count} messages)'**
  String pendingCopyTagged(int count);

  /// No description provided for @transferResultOk.
  ///
  /// In en, this message translates to:
  /// **'Done: {count} message(s).'**
  String transferResultOk(int count);

  /// No description provided for @transferResultMixed.
  ///
  /// In en, this message translates to:
  /// **'{ok} succeeded, {failed} failed.'**
  String transferResultMixed(int ok, int failed);

  /// No description provided for @transferFailed.
  ///
  /// In en, this message translates to:
  /// **'Transfer failed: {error}'**
  String transferFailed(String error);

  /// No description provided for @deleteMessagesFailed.
  ///
  /// In en, this message translates to:
  /// **'Delete failed: {error}'**
  String deleteMessagesFailed(String error);

  /// No description provided for @settingsNotifyNewMessages.
  ///
  /// In en, this message translates to:
  /// **'New-message notifications'**
  String get settingsNotifyNewMessages;

  /// No description provided for @settingsNotifyNewMessagesSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Snackbar while the app is open; a system notification when it is in the background (IMAP).'**
  String get settingsNotifyNewMessagesSubtitle;

  /// No description provided for @newMailNotificationTitle.
  ///
  /// In en, this message translates to:
  /// **'New mail'**
  String get newMailNotificationTitle;

  /// No description provided for @newMailNotificationBody.
  ///
  /// In en, this message translates to:
  /// **'{count} new message(s) in {folder}'**
  String newMailNotificationBody(int count, String folder);

  /// No description provided for @accountImapMinIdleSecondsLabel.
  ///
  /// In en, this message translates to:
  /// **'Min. quiet seconds before IDLE'**
  String get accountImapMinIdleSecondsLabel;

  /// No description provided for @accountImapMinIdleSecondsHelper.
  ///
  /// In en, this message translates to:
  /// **'Leave empty for default (120). Minimum 15. Applies after the connection is idle.'**
  String get accountImapMinIdleSecondsHelper;

  /// No description provided for @validationImapMinIdleSeconds.
  ///
  /// In en, this message translates to:
  /// **'Enter a whole number from 15 to 864000, or leave empty for the default.'**
  String get validationImapMinIdleSeconds;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) => <String>[
    'de',
    'el',
    'en',
    'es',
    'fr',
    'it',
    'ja',
    'pt',
    'ru',
    'zh',
  ].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'de':
      return AppLocalizationsDe();
    case 'el':
      return AppLocalizationsEl();
    case 'en':
      return AppLocalizationsEn();
    case 'es':
      return AppLocalizationsEs();
    case 'fr':
      return AppLocalizationsFr();
    case 'it':
      return AppLocalizationsIt();
    case 'ja':
      return AppLocalizationsJa();
    case 'pt':
      return AppLocalizationsPt();
    case 'ru':
      return AppLocalizationsRu();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
