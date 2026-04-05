// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Settings';

  @override
  String get compose => 'Compose';

  @override
  String get send => 'Send';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'Cancel';

  @override
  String get save => 'Save';

  @override
  String get remove => 'Remove';

  @override
  String get delete => 'Delete';

  @override
  String get discard => 'Discard';

  @override
  String get back => 'Back';

  @override
  String get create => 'Create';

  @override
  String get rename => 'Rename';

  @override
  String get folderLabel => 'Folder';

  @override
  String get messageTitle => 'Message';

  @override
  String get selectFolder => 'Select a folder';

  @override
  String get selectMessage => 'Select a message';

  @override
  String get selectMessageToRead => 'Select a message to read.';

  @override
  String get noMessages => 'No messages';

  @override
  String get attachments => 'Attachments';

  @override
  String get saveAttachment => 'Save attachment';

  @override
  String savedToPath(String path) {
    return 'Saved $path';
  }

  @override
  String saveFailed(String error) {
    return 'Save failed: $error';
  }

  @override
  String get cannotDownloadAttachment => 'Cannot download this attachment';

  @override
  String get emptyAttachmentData => 'Empty attachment data';

  @override
  String downloadFailed(String error) {
    return 'Download failed: $error';
  }

  @override
  String get saveVerb => 'Save';

  @override
  String get loadImages => 'Load images';

  @override
  String get remoteImagesBlocked => 'Remote images blocked for privacy.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'Could not open HTML body: $error';
  }

  @override
  String webViewError(String error) {
    return 'WebView error: $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'The visible link text shows a different address than where this link goes.';

  @override
  String get headerFrom => 'From:';

  @override
  String get headerTo => 'To:';

  @override
  String get headerCc => 'Cc:';

  @override
  String get headerDate => 'Date:';

  @override
  String get folderInbox => 'Inbox';

  @override
  String get messageActionReply => 'Reply';

  @override
  String get messageActionReplyAll => 'Reply all';

  @override
  String get messageActionForward => 'Forward';

  @override
  String get messageActionDelete => 'Delete';

  @override
  String get messageActionJunk => 'Junk';

  @override
  String get messageActionMove => 'Move';

  @override
  String get messageActionCopy => 'Copy';

  @override
  String get messageMenuTooltip => 'Message actions';

  @override
  String get settingsViewMinimalHeaders => 'Minimal message headers';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'When on, hide Cc only; From, To, and Date still show when available.';

  @override
  String get settingsTooltip => 'Settings';

  @override
  String get accountsAndFoldersTooltip => 'Accounts and folders';

  @override
  String get cancelSelectionTooltip => 'Cancel selection';

  @override
  String multiSelectCount(int count) {
    return '$count selected';
  }

  @override
  String get composeTooltip => 'Compose';

  @override
  String get composeNeedTransportTooltip =>
      'Add an outgoing transport in Settings';

  @override
  String get mailToolbarMoreTooltip => 'More';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count selected';
  }

  @override
  String get settingsTabAccounts => 'Accounts';

  @override
  String get settingsTabOutgoing => 'Outgoing';

  @override
  String get settingsTabSecurity => 'Security';

  @override
  String get settingsTabViewing => 'Viewing';

  @override
  String get settingsTabComposing => 'Composing';

  @override
  String get settingsTabAbout => 'About';

  @override
  String get useSystemKeychain => 'Use system keychain';

  @override
  String get storeCredentialsInKeychain =>
      'Store credentials in platform keychain';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Authenticate Google';

  @override
  String get authenticateMicrosoft => 'Authenticate Microsoft';

  @override
  String get reloadOAuthToken => 'Reload OAuth Token';

  @override
  String get matrixE2eeSection => 'Matrix E2EE';

  @override
  String get initCrypto => 'Init Crypto';

  @override
  String get setupBackup => 'Setup Backup';

  @override
  String get restoreBackup => 'Restore Backup';

  @override
  String get showDeviceFingerprint => 'Show Device Fingerprint';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Message detail below list (desktop)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'When off, opening a message uses a separate full-screen view.';

  @override
  String get loadRemoteImages => 'Load remote images';

  @override
  String get loadRemoteImagesSubtitle => 'Allow external images in HTML email';

  @override
  String get threadedView => 'Threaded view';

  @override
  String get threadedViewSubtitle => 'Group email messages by thread';

  @override
  String get deletionAndTrashSection => 'Deletion & trash';

  @override
  String get deletionAppliesGlobally =>
      'Applies to mail-style accounts globally.';

  @override
  String get deleteModeLabel => 'Delete mode';

  @override
  String get trashFolderNameLabel => 'Trash folder name';

  @override
  String get deleteModeMoveToTrash => 'Move to Trash';

  @override
  String get deleteModeMarkDeleted => 'Mark Deleted';

  @override
  String get quoteOriginalOnReply => 'Quote original message on reply';

  @override
  String get testSend => 'Test Send';

  @override
  String get openSignatureEditor => 'Open Signature Editor';

  @override
  String get aboutSubtitle => 'Cross platform email and messaging';

  @override
  String get supportedBackends => 'Supported backends';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation invoked';
  }

  @override
  String get accountTypeDialogTitle => 'Account type';

  @override
  String get removeAccountTitle => 'Remove account?';

  @override
  String removeAccountBody(String label) {
    return 'Remove “$label” from this device’s saved configuration?';
  }

  @override
  String removedAccount(String label) {
    return 'Removed $label';
  }

  @override
  String get accountsListTitle => 'Accounts';

  @override
  String get accountsListSubtitle =>
      'Tap an account to edit, or add a new one below.';

  @override
  String get deleteTooltip => 'Delete';

  @override
  String get addAccount => 'Add account';

  @override
  String get noAccountsYet =>
      'No accounts yet. Tap “Add account” to create one.';

  @override
  String get discardChangesTitle => 'Discard changes?';

  @override
  String get discardChangesBody =>
      'Your edits will be lost. This matches leaving without saving.';

  @override
  String get keepEditing => 'Keep editing';

  @override
  String get pickNotSupportedWeb =>
      'Picking files or folders is not supported in the web build';

  @override
  String get chooseMaildirFolderTitle => 'Choose Maildir root folder';

  @override
  String get chooseMboxFileTitle => 'Choose mbox file';

  @override
  String get validationAccountNameRequired => 'Account name is required';

  @override
  String get validationLocalPathRequired => 'Local mailbox path is required';

  @override
  String get validationUsernameRequired =>
      'Username / email is required for this account type';

  @override
  String get validationHostRequired => 'Server host is required';

  @override
  String get validationPortRequired => 'Valid port number is required';

  @override
  String get accountSaved => 'Account saved';

  @override
  String get createTransportFirst =>
      'Create a transport on the Outgoing tab first';

  @override
  String get addTransportDialogTitle => 'Add transport';

  @override
  String get accountTypeLabel => 'Account type';

  @override
  String get accountTypeHelper => 'Type is fixed when editing an account';

  @override
  String get accountNameLabel => 'Account name';

  @override
  String get usernameEmailOptional => 'Username / email (optional)';

  @override
  String get usernameEmailRequired => 'Username / email';

  @override
  String get avatarUrlLabel => 'Avatar URL or file path (optional)';

  @override
  String get avatarUrlHelper =>
      'Optional image URL or local file path for the account strip';

  @override
  String get localMailboxSection => 'Local mailbox';

  @override
  String get pathMboxFile => 'Path to mbox file';

  @override
  String get pathMaildirRoot => 'Path to Maildir root';

  @override
  String get helperMboxPath =>
      'Use the file button to browse, or type an absolute path';

  @override
  String get helperMaildirPath =>
      'Use the folder button to browse, or type an absolute path';

  @override
  String get chooseMboxTooltip => 'Choose mbox file';

  @override
  String get chooseMaildirTooltip => 'Choose Maildir folder';

  @override
  String get imapServerSection => 'IMAP server';

  @override
  String get pop3ServerSection => 'POP3 server';

  @override
  String get nntpServerSection => 'NNTP server';

  @override
  String get hostLabel => 'Host';

  @override
  String get serverHostLabel => 'Server host';

  @override
  String get portLabel => 'Port';

  @override
  String get portHelperImap => 'Usually 993 (IMAPS) or 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'Usually 995 (POP3S, implicit TLS)';

  @override
  String get portHelperNntp => 'Usually 563 (NNTPS, implicit TLS)';

  @override
  String get securityLabel => 'Security';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (implicit TLS)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (implicit TLS)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (implicit TLS)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'No encryption';

  @override
  String get outgoingTransportsSection => 'Outgoing transports';

  @override
  String get noTransportsHintLinked =>
      'No transports selected — compose and reply stay disabled until you pick at least one. Use the Outgoing tab and select it here.';

  @override
  String get transportsOrderHint =>
      'First in the list is the default for send. Use Outgoing to create transports.';

  @override
  String get unknownTransport => 'Unknown transport';

  @override
  String get moveUpTooltip => 'Move up';

  @override
  String get moveDownTooltip => 'Move down';

  @override
  String get removeFromAccountTooltip => 'Remove from account';

  @override
  String get addTransportToAccount => 'Add transport to account';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Homeserver';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'Relay URLs';

  @override
  String get relayUrlsHelper =>
      'Each row is one relay WebSocket URL. Press Enter when you finish editing a URL.';

  @override
  String get relayAddFieldHint => 'New relay URL';

  @override
  String get relayAddTooltip => 'Add relay';

  @override
  String get relayRemoveTooltip => 'Remove relay';

  @override
  String get nostrNewIdentityTooltip => 'Create new Nostr identity';

  @override
  String get nostrRelayUrlsRequired => 'Enter at least one relay URL.';

  @override
  String storeUriLabel(String uri) {
    return 'Store URI: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'Transport URI: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'New $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return 'Edit $label';
  }

  @override
  String foldersLoadError(String error) {
    return 'Folders: $error';
  }

  @override
  String get sortMessagesTooltip => 'Sort messages';

  @override
  String get sort => 'Sort';

  @override
  String get sortFromAz => 'From A–Z';

  @override
  String get sortFromZa => 'From Z–A';

  @override
  String get sortSubjectAz => 'Subject A–Z';

  @override
  String get sortSubjectZa => 'Subject Z–A';

  @override
  String get sortDateOldest => 'Date oldest first';

  @override
  String get sortDateNewest => 'Date newest first';

  @override
  String get removeTransportTitle => 'Remove transport?';

  @override
  String removeTransportBody(String name) {
    return '“$name” will be removed from all accounts’ outgoing lists.';
  }

  @override
  String removedTransport(String name) {
    return 'Removed $name';
  }

  @override
  String get outgoingListTitle => 'Outgoing';

  @override
  String get outgoingListSubtitle =>
      'SMTP and other send transports. Link them to mail accounts on the Accounts tab.';

  @override
  String get addTransport => 'Add transport';

  @override
  String get noTransportsYet =>
      'No outgoing transports yet. Tap “Add transport” to create one.';

  @override
  String get transportDisplayHostRequired =>
      'Display name and host are required.';

  @override
  String get transportSaved => 'Transport saved';

  @override
  String get newTransport => 'New transport';

  @override
  String get editTransport => 'Edit transport';

  @override
  String get displayNameLabel => 'Display name';

  @override
  String get smtpHostLabel => 'SMTP host';

  @override
  String get smtpPortHelper => 'Usually 587 (STARTTLS) or 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'IMAP sign-in';

  @override
  String get smtpSignInTitle => 'SMTP sign-in';

  @override
  String smtpSignInSubtitle(String transportName, String host) {
    return 'Enter the username and password for “$transportName” ($host).';
  }

  @override
  String get composeSendCancelledNoSmtpCredentials =>
      'Message not sent: SMTP credentials were not saved.';

  @override
  String get enterUsernameAndPassword => 'Enter username and password.';

  @override
  String get usernameLabel => 'Username';

  @override
  String get passwordLabel => 'Password';

  @override
  String get showPasswordTooltip => 'Show password';

  @override
  String get hidePasswordTooltip => 'Hide password';

  @override
  String get fieldFrom => 'From';

  @override
  String get composeOutgoingTransport => 'Outgoing transport';

  @override
  String get composeSendSucceeded => 'Message sent';

  @override
  String get composeMissingFrom => 'Enter a from address.';

  @override
  String get composeMissingTo => 'Enter at least one recipient.';

  @override
  String get fieldTo => 'To';

  @override
  String get fieldCc => 'Cc';

  @override
  String get fieldBcc => 'Bcc';

  @override
  String get fieldSubject => 'Subject';

  @override
  String get fieldBody => 'Body';

  @override
  String get attach => 'Attach';

  @override
  String get composeRemoveAttachment => 'Remove attachment';

  @override
  String get defaultFromLabel => 'Default From address';

  @override
  String get defaultFromHelper =>
      'e.g. Your Name <you@example.com> or you@example.com';

  @override
  String get dsnLabel => 'Delivery notifications';

  @override
  String get dsnUseTransportDefault => 'Use transport default';

  @override
  String get dsnNever => 'Never';

  @override
  String get dsnFailure => 'On failure';

  @override
  String get dsnSuccess => 'On success';

  @override
  String get dsnDelay => 'On delay';

  @override
  String get dsnFailureAndSuccess => 'On failure and success';

  @override
  String get dsnNotifyLabel => 'DSN notify';

  @override
  String get folderNewSubfolder => 'New subfolder';

  @override
  String get folderRename => 'Rename…';

  @override
  String get folderDelete => 'Delete…';

  @override
  String get folderNewTooltip => 'New folder';

  @override
  String get folderNewDialogTitle => 'New folder';

  @override
  String get folderNameLabel => 'Folder name';

  @override
  String get folderNewTopLevelHelper => 'Creates a mailbox at the top level';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Subfolder of $parent';
  }

  @override
  String get subfolderNameLabel => 'Subfolder name';

  @override
  String subfolderPathHelper(String path) {
    return 'Path: $path';
  }

  @override
  String folderCreated(String name) {
    return 'Created folder “$name”';
  }

  @override
  String get renameFolderTitle => 'Rename folder';

  @override
  String get newFolderPathLabel => 'New folder path';

  @override
  String get folderRenamed => 'Folder renamed';

  @override
  String get deleteFolderTitle => 'Delete folder?';

  @override
  String deleteFolderBody(String name) {
    return 'Remove “$name” and its messages from the server (if supported)? This cannot be undone.';
  }

  @override
  String get folderDeleted => 'Folder deleted';

  @override
  String get licenseTitle => 'License';

  @override
  String get copyrightTitle => 'Copyright';

  @override
  String get chatHintTypeMessage => 'Type a message';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.';

  @override
  String operationFailed(String error) {
    return 'Something went wrong: $error';
  }

  @override
  String get expandFolder => 'Expand';

  @override
  String get collapseFolder => 'Collapse';

  @override
  String get noTextBody => '(No text body)';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Move here';

  @override
  String get folderCopyHere => 'Copy here';

  @override
  String get folderExpunge => 'Expunge deleted messages';

  @override
  String get folderExpungeDone => 'Expunge completed';

  @override
  String pendingMoveTagged(int count) {
    return 'Pick a folder, then choose Move here ($count messages)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Pick a folder, then choose Copy here ($count messages)';
  }

  @override
  String transferResultOk(int count) {
    return 'Done: $count message(s).';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok succeeded, $failed failed.';
  }

  @override
  String transferFailed(String error) {
    return 'Transfer failed: $error';
  }

  @override
  String get settingsNotifyNewMessages => 'New-message notifications';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Snackbar while the app is open; a system notification when it is in the background (IMAP).';

  @override
  String get newMailNotificationTitle => 'New mail';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count new message(s) in $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel => 'Min. quiet seconds before IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Leave empty for default (120). Minimum 15. Applies after the connection is idle.';

  @override
  String get validationImapMinIdleSeconds =>
      'Enter a whole number from 15 to 864000, or leave empty for the default.';
}
