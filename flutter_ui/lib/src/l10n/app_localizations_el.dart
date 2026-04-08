// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Modern Greek (`el`).
class AppLocalizationsEl extends AppLocalizations {
  AppLocalizationsEl([String locale = 'el']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Ρυθμίσεις';

  @override
  String get compose => 'Σύνθεση';

  @override
  String get send => 'Αποστολή';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'Ακύρωση';

  @override
  String get save => 'Αποθήκευση';

  @override
  String get remove => 'Αφαίρεση';

  @override
  String get delete => 'Διαγραφή';

  @override
  String get discard => 'Απόρριψη';

  @override
  String get back => 'Πίσω';

  @override
  String get create => 'Δημιουργία';

  @override
  String get rename => 'Μετονομασία';

  @override
  String get folderLabel => 'Φάκελος';

  @override
  String get messageTitle => 'Μήνυμα';

  @override
  String get selectFolder => 'Επιλέξτε φάκελο';

  @override
  String get selectMessage => 'Επιλέξτε μήνυμα';

  @override
  String get selectMessageToRead => 'Επιλέξτε μήνυμα για ανάγνωση.';

  @override
  String get noMessages => 'Κανένα μήνυμα';

  @override
  String get attachments => 'Συνημμένα';

  @override
  String get saveAttachment => 'Αποθήκευση συνημμένου';

  @override
  String savedToPath(String path) {
    return 'Αποθηκεύτηκε στο $path';
  }

  @override
  String saveFailed(String error) {
    return 'Αποτυχία αποθήκευσης: $error';
  }

  @override
  String get cannotDownloadAttachment =>
      'Δεν είναι δυνατή η λήψη αυτού του συνημμένου';

  @override
  String get emptyAttachmentData => 'Κενά δεδομένα συνημμένου';

  @override
  String downloadFailed(String error) {
    return 'Αποτυχία λήψης: $error';
  }

  @override
  String get saveVerb => 'Αποθήκευση';

  @override
  String get loadImages => 'Φόρτωση εικόνων';

  @override
  String get remoteImagesBlocked =>
      'Οι απομακρυσμένες εικόνες αποκλείονται για την προστασία της ιδιωτικότητας.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'Δεν ήταν δυνατό το άνοιγμα του σώματος HTML: $error';
  }

  @override
  String webViewError(String error) {
    return 'Σφάλμα WebView: $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'Το ορατό κείμενο του συνδέσμου εμφανίζει διαφορετική διεύθυνση από τον πραγματικό προορισμό.';

  @override
  String get headerFrom => 'Από:';

  @override
  String get headerTo => 'Προς:';

  @override
  String get headerCc => 'Κοιν:';

  @override
  String get headerDate => 'Ημερομηνία:';

  @override
  String get folderInbox => 'Εισερχόμενα';

  @override
  String get messageActionReply => 'Απάντηση';

  @override
  String get messageActionReplyAll => 'Απάντηση σε όλους';

  @override
  String get messageActionForward => 'Προώθηση';

  @override
  String get messageActionDelete => 'Διαγραφή';

  @override
  String get messageActionJunk => 'Ανεπιθύμητα';

  @override
  String get messageActionMove => 'Μετακίνηση';

  @override
  String get messageActionCopy => 'Αντιγραφή';

  @override
  String get messageMenuTooltip => 'Ενέργειες μηνύματος';

  @override
  String get settingsViewMinimalHeaders => 'Ελάχιστες κεφαλίδες μηνύματος';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'Όταν είναι ενεργό, κρύβεται μόνο το Κοιν· Από, Προς και ημερομηνία εμφανίζονται όταν υπάρχουν.';

  @override
  String get settingsTooltip => 'Ρυθμίσεις';

  @override
  String get accountsAndFoldersTooltip => 'Λογαριασμοί και φάκελοι';

  @override
  String get cancelSelectionTooltip => 'Ακύρωση επιλογής';

  @override
  String multiSelectCount(int count) {
    return '$count επιλεγμένα';
  }

  @override
  String get composeTooltip => 'Σύνθεση';

  @override
  String get composeNeedTransportTooltip =>
      'Προσθέστε εξερχόμενη μεταφορά στις Ρυθμίσεις';

  @override
  String get mailToolbarMoreTooltip => 'Περισσότερα';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count επιλεγμένα';
  }

  @override
  String get settingsTabAccounts => 'Λογαριασμοί';

  @override
  String get settingsTabOutgoing => 'Εξερχόμενα';

  @override
  String get settingsTabSecurity => 'Ασφάλεια';

  @override
  String get settingsTabViewing => 'Προβολή';

  @override
  String get settingsTabComposing => 'Σύνθεση';

  @override
  String get settingsTabAbout => 'Σχετικά';

  @override
  String get settingsLoadFailed => 'Could not load settings from disk.';

  @override
  String get settingsLoadRetry => 'Retry';

  @override
  String get useSystemKeychain => 'Χρήση κλειδοθήκης συστήματος';

  @override
  String get storeCredentialsInKeychain =>
      'Αποθήκευση διαπιστευτηρίων στην κλειδοθήκη της πλατφόρμας';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Ταυτοποίηση Google';

  @override
  String get authenticateMicrosoft => 'Ταυτοποίηση Microsoft';

  @override
  String get reloadOAuthToken => 'Επαναφόρτωση διακριτικού OAuth';

  @override
  String get matrixE2eeSection => 'Κρυπτογράφηση άκρο-προς-άκρο Matrix';

  @override
  String get initCrypto => 'Αρχικοποίηση κρυπτογράφησης';

  @override
  String get setupBackup => 'Ρύθμιση αντιγράφου ασφαλείας';

  @override
  String get restoreBackup => 'Επαναφορά αντιγράφου ασφαλείας';

  @override
  String get showDeviceFingerprint => 'Εμφάνιση αποτυπώματος συσκευής';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Λεπτομέρειες μηνύματος κάτω από τη λίστα (επιτραπέζιο)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'Όταν είναι ανενεργό, το άνοιγμα μηνύματος χρησιμοποιεί ξεχωριστή προβολή πλήρους οθόνης.';

  @override
  String get loadRemoteImages => 'Φόρτωση απομακρυσμένων εικόνων';

  @override
  String get loadRemoteImagesSubtitle =>
      'Να επιτρέπονται εξωτερικές εικόνες σε HTML email';

  @override
  String get threadedView => 'Προβολή νημάτων';

  @override
  String get threadedViewSubtitle => 'Ομαδοποίηση μηνυμάτων κατά συνομιλία';

  @override
  String get deletionAndTrashSection => 'Διαγραφή και κάδος';

  @override
  String get deletionAppliesGlobally =>
      'Ισχύει για όλους τους λογαριασμούς αλληλογραφίας.';

  @override
  String get deleteModeLabel => 'Τρόπος διαγραφής';

  @override
  String get trashFolderNameLabel => 'Όνομα φακέλου κάδου';

  @override
  String get junkFolderNameLabel => 'Όνομα φακέλου ανεπιθύμητων';

  @override
  String get exchangeTrashFolderHelper =>
      'Leave empty to use “Deleted Items” (English mailbox). Use the exact folder name shown in Outlook if yours differs.';

  @override
  String get exchangeJunkFolderHelper =>
      'Leave empty to use “Junk Email” (English mailbox). Use the exact folder name shown in Outlook if yours differs.';

  @override
  String get deleteModeDeleteImmediately => 'Άμεση διαγραφή';

  @override
  String get deleteModeMoveToTrash => 'Μετακίνηση στον κάδο';

  @override
  String get deleteModeMarkDeleted => 'Σήμανση ως διαγραμμένο';

  @override
  String get quoteOriginalOnReply => 'Quote original message on reply';

  @override
  String get quoteOriginalOnReplySubtitle =>
      'Adds the original under the reply header in new replies. Rich compose wraps it in a marked quote block; plain compose prefixes each line of the original. The text/plain part of the message still includes the original when this is on.';

  @override
  String get composingReplySection => 'Reply quoting';

  @override
  String get replyHeaderTemplateLabel => 'Reply header line';

  @override
  String get replyHeaderTemplateHelp =>
      'Shown above the quoted original. Include the three words date, time, and sender, each with a dollar sign immediately in front (see preview). They are replaced with the message’s date, time, and From when you reply.';

  @override
  String get replyHeaderPreviewLabel => 'Preview';

  @override
  String get replyDateFormatLabel => 'Reply date (in header)';

  @override
  String get replyTimeFormatLabel => 'Reply time (in header)';

  @override
  String get replyDatePresetLocale => 'Same as system (long date)';

  @override
  String get replyDatePresetIso => 'ISO: 2026-04-08';

  @override
  String get replyDatePresetUs => 'US: 04/08/2026';

  @override
  String get replyDatePresetEu => 'Day/month/year: 08/04/2026';

  @override
  String get replyDatePresetMedium => 'Medium: Apr 8, 2026';

  @override
  String get replyDatePresetWeekday => 'With weekday: Wed, Apr 8, 2026';

  @override
  String replyDatePresetCustom(String pattern) {
    return 'Custom ($pattern)';
  }

  @override
  String get replyTimePresetLocale => 'Same as system';

  @override
  String get replyTimePreset12h => '12-hour (e.g. 1:30 PM)';

  @override
  String get replyTimePreset24h => '24-hour (15:30)';

  @override
  String get replyTimePreset24hSeconds => '24-hour with seconds';

  @override
  String replyTimePresetCustom(String pattern) {
    return 'Custom ($pattern)';
  }

  @override
  String get replyLinePrefixLabel => 'Quoted line prefix';

  @override
  String get replyLinePrefixSubtitle =>
      'Prepended to each line of the original in plain-text quotes (classic “> ” quoting). Only used when quoting the original is enabled.';

  @override
  String get replyPlainPositionLabel => 'Ordering of quoted text';

  @override
  String get replyPlainPositionBefore => 'Reply before quoted text';

  @override
  String get replyPlainPositionAfter => 'Reply after quoted text';

  @override
  String get replyPlainPositionSubtitle =>
      'Plain or rich compose: two blank lines and caret before the reply header, or two blank lines and caret after the quoted block. The text/plain part when sending follows the same layout.';

  @override
  String get replyQuoteModeLabel => 'SMTP HTML parts';

  @override
  String get replyQuoteModePlain => 'Original only in plain-text quote';

  @override
  String get replyQuoteModeHtmlSmtp =>
      'Also include original as separate HTML (SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'Adds a second HTML part preserving the source message’s formatting for HTML-capable clients. Plain-text-only clients still see the quoted plain body. NNTP posting always uses plain quoting.';

  @override
  String get settingsComposeRichText => 'Πλούσιο κείμενο κατά τη σύνθεση email';

  @override
  String get settingsComposeRichTextSubtitle =>
      'Μορφοποιημένος επεξεργαστής για νέα email και απαντήσεις. Το Usenet (NNTP) παραμένει απλό κείμενο.';

  @override
  String get settingsMatrixChatRichText =>
      'Πλούσιο κείμενο σε συνομιλίες Matrix';

  @override
  String get settingsMatrixChatRichTextSubtitle =>
      'Αποστολή μορφοποιημένων μηνυμάτων σε δωμάτια Matrix (με εναλλακτικό απλό κείμενο).';

  @override
  String get testSend => 'Δοκιμαστική αποστολή';

  @override
  String get openSignatureEditor => 'Άνοιγμα επεξεργαστή υπογραφής';

  @override
  String get aboutSubtitle =>
      'Email και ανταλλαγή μηνυμάτων πολλαπλών πλατφορμών';

  @override
  String get supportedBackends => 'Υποστηριζόμενα backends';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation (επίδειξη)';
  }

  @override
  String get accountTypeDialogTitle => 'Τύπος λογαριασμού';

  @override
  String get removeAccountTitle => 'Αφαίρεση λογαριασμού;';

  @override
  String removeAccountBody(String label) {
    return 'Να αφαιρεθεί το «$label» από τη ρύθμιση αυτής της συσκευής;';
  }

  @override
  String removedAccount(String label) {
    return 'Αφαιρέθηκε το «$label»';
  }

  @override
  String get accountsListTitle => 'Λογαριασμοί';

  @override
  String get accountsListSubtitle =>
      'Πατήστε λογαριασμό για επεξεργασία ή προσθέστε νέο παρακάτω.';

  @override
  String get deleteTooltip => 'Διαγραφή';

  @override
  String get addAccount => 'Προσθήκη λογαριασμού';

  @override
  String get noAccountsYet =>
      'Δεν υπάρχουν λογαριασμοί ακόμα. Πατήστε «Προσθήκη λογαριασμού».';

  @override
  String get discardChangesTitle => 'Απόρριψη αλλαγών;';

  @override
  String get discardChangesBody =>
      'Οι αλλαγές θα χαθούν, όπως έξοδος χωρίς αποθήκευση.';

  @override
  String get keepEditing => 'Συνέχεια επεξεργασίας';

  @override
  String get pickNotSupportedWeb =>
      'Η επιλογή αρχείων ή φακέλων δεν υποστηρίζεται στην έκδοση web';

  @override
  String get chooseMaildirFolderTitle => 'Επιλογή ριζικού φακέλου Maildir';

  @override
  String get chooseMboxFileTitle => 'Επιλογή αρχείου mbox';

  @override
  String get validationAccountNameRequired => 'Απαιτείται όνομα λογαριασμού';

  @override
  String get validationLocalPathRequired =>
      'Απαιτείται διαδρομή τοπικής θυρίδας';

  @override
  String get validationUsernameRequired =>
      'Απαιτείται όνομα χρήστη / email για αυτόν τον τύπο λογαριασμού';

  @override
  String get validationEmailAddressRequired => 'Απαιτείται διεύθυνση email';

  @override
  String get validationMatrixUserIdRequired =>
      'Απαιτείται αναγνωριστικό χρήστη Matrix';

  @override
  String get accountEmailAddressLabel => 'Διεύθυνση email';

  @override
  String get accountMatrixUserIdLabel => 'Αναγνωριστικό Matrix (MXID)';

  @override
  String get accountMatrixMxidHelper =>
      'Παράδειγμα: @you:matrix.org — το URL του homeserver προκύπτει από το domain μετά την άνω τελεία.';

  @override
  String get validationMatrixMxidInvalid =>
      'Εισαγάγετε Matrix ID όπως @user:server';

  @override
  String get accountNntpDefaultFromLabel =>
      'Προεπιλεγμένος αποστολέας (Usenet)';

  @override
  String get accountNntpDefaultFromHelper =>
      'Εμφανίζεται κατά τη σύνθεση· αυτός ο λογαριασμός NNTP δημοσιεύει μέσω της δικής του σύνδεσης διακομιστή.';

  @override
  String get accountEmailOptionalLabel => 'Διεύθυνση email (προαιρετικό)';

  @override
  String get accountTcpLoginHelper =>
      'Ταυτότητα σύνδεσης για αυτόν τον διακομιστή (συνήθως το email σας).';

  @override
  String get validationHostRequired => 'Απαιτείται διακομιστής';

  @override
  String get validationPortRequired => 'Απαιτείται έγκυρη θύρα';

  @override
  String get accountSaved => 'Ο λογαριασμός αποθηκεύτηκε';

  @override
  String get createTransportFirst =>
      'Δημιουργήστε πρώτα μεταφορά στην καρτέλα Εξερχόμενα';

  @override
  String get addTransportDialogTitle => 'Προσθήκη μεταφοράς';

  @override
  String get accountTypeLabel => 'Τύπος λογαριασμού';

  @override
  String get accountTypeHelper =>
      'Επιλέγεται κατά την προσθήκη· δεν αλλάζει εδώ.';

  @override
  String get accountNameLabel => 'Όνομα λογαριασμού';

  @override
  String get usernameEmailOptional => 'Όνομα χρήστη / email (προαιρετικό)';

  @override
  String get usernameEmailRequired => 'Όνομα χρήστη / email';

  @override
  String get avatarUrlLabel => 'URL avatar ή διαδρομή αρχείου (προαιρετικό)';

  @override
  String get avatarUrlHelper =>
      'Προαιρετική εικόνα ή τοπική διαδρομή για τη γραμμή λογαριασμών';

  @override
  String get localMailboxSection => 'Τοπική θυρίδα';

  @override
  String get pathMboxFile => 'Διαδρομή προς αρχείο mbox';

  @override
  String get pathMaildirRoot => 'Διαδρομή ρίζας Maildir';

  @override
  String get helperMboxPath =>
      'Χρησιμοποιήστε το κουμπί αρχείου ή πληκτρολογήστε απόλυτη διαδρομή';

  @override
  String get helperMaildirPath =>
      'Χρησιμοποιήστε το κουμπί φακέλου ή πληκτρολογήστε απόλυτη διαδρομή';

  @override
  String get chooseMboxTooltip => 'Επιλογή αρχείου mbox';

  @override
  String get chooseMaildirTooltip => 'Επιλογή φακέλου Maildir';

  @override
  String get imapServerSection => 'Διακομιστής IMAP';

  @override
  String get pop3ServerSection => 'Διακομιστής POP3';

  @override
  String get nntpServerSection => 'Διακομιστής NNTP';

  @override
  String get hostLabel => 'Κόμβος';

  @override
  String get serverHostLabel => 'Κόμβος διακομιστή';

  @override
  String get portLabel => 'Θύρα';

  @override
  String get portHelperImap => 'Συνήθως 993 (IMAPS) ή 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'Συνήθως 995 (POP3S, σιωπηρό TLS)';

  @override
  String get portHelperNntp => 'Συνήθως 563 (NNTPS, σιωπηρό TLS)';

  @override
  String get securityLabel => 'Ασφάλεια';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (σιωπηρό TLS)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (σιωπηρό TLS)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (σιωπηρό TLS)';

  @override
  String get mailSecurityImplicitTlsNntp => 'NNTPS (σιωπηρό TLS)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'Χωρίς κρυπτογράφηση';

  @override
  String get outgoingTransportsSection => 'Εξερχόμενες μεταφορές';

  @override
  String get noTransportsHintLinked =>
      'Δεν έχει επιλεγεί μεταφορά — η σύνθεση και οι απαντήσεις παραμένουν ανενεργές μέχρι να επιλέξετε τουλάχιστον μία. Χρησιμοποιήστε την καρτέλα Εξερχόμενα.';

  @override
  String get transportsOrderHint =>
      'Το πρώτο στη λίστα είναι προεπιλογή αποστολής. Δημιουργήστε μεταφορές στα Εξερχόμενα.';

  @override
  String get unknownTransport => 'Άγνωστη μεταφορά';

  @override
  String get moveUpTooltip => 'Πάνω';

  @override
  String get moveDownTooltip => 'Κάτω';

  @override
  String get removeFromAccountTooltip => 'Αφαίρεση από λογαριασμό';

  @override
  String get addTransportToAccount => 'Προσθήκη μεταφοράς στον λογαριασμό';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Homeserver (Matrix)';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'URL αναμεταδοτών';

  @override
  String get relayUrlsHelper =>
      'Κάθε γραμμή είναι ένα WebSocket URL αναμεταδότη. Πατήστε Enter όταν τελειώσετε την επεξεργασία ενός URL.';

  @override
  String get relayAddFieldHint => 'Νέο URL αναμεταδότη';

  @override
  String get relayAddTooltip => 'Προσθήκη αναμεταδότη';

  @override
  String get relayRemoveTooltip => 'Αφαίρεση αναμεταδότη';

  @override
  String get nostrNewIdentityTooltip => 'Δημιουργία νέας ταυτότητας Nostr';

  @override
  String get nostrRelayUrlsRequired =>
      'Εισαγάγετε τουλάχιστον ένα URL αναμεταδότη.';

  @override
  String storeUriLabel(String uri) {
    return 'Σύνδεση: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'Κληρονομούμενο URI εξερχόμενης μεταφοράς: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'Νέο $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return 'Επεξεργασία $label';
  }

  @override
  String foldersLoadError(String error) {
    return 'Φάκελοι: $error';
  }

  @override
  String get sortMessagesTooltip => 'Ταξινόμηση μηνυμάτων';

  @override
  String get sort => 'Ταξινόμηση';

  @override
  String get sortFromAz => 'Από Α–Ω';

  @override
  String get sortFromZa => 'Από Ω–Α';

  @override
  String get sortSubjectAz => 'Θέμα Α–Ω';

  @override
  String get sortSubjectZa => 'Θέμα Ω–Α';

  @override
  String get sortDateOldest => 'Ημερομηνία: παλαιότερα πρώτα';

  @override
  String get sortDateNewest => 'Ημερομηνία: νεότερα πρώτα';

  @override
  String get removeTransportTitle => 'Αφαίρεση μεταφοράς;';

  @override
  String removeTransportBody(String name) {
    return 'Το «$name» θα αφαιρεθεί από όλες τις λίστες εξερχομένων.';
  }

  @override
  String removedTransport(String name) {
    return 'Αφαιρέθηκε το «$name»';
  }

  @override
  String get outgoingListTitle => 'Εξερχόμενα';

  @override
  String get outgoingListSubtitle =>
      'SMTP και άλλες μεταφορές αποστολής. Συνδέστε τα με λογαριασμούς στην καρτέλα Λογαριασμοί.';

  @override
  String get addTransport => 'Προσθήκη μεταφοράς';

  @override
  String get noTransportsYet =>
      'Δεν υπάρχουν εξερχόμενες μεταφορές. Πατήστε «Προσθήκη μεταφοράς».';

  @override
  String get transportDisplayHostRequired =>
      'Απαιτούνται εμφανιζόμενο όνομα και κόμβος.';

  @override
  String get transportSaved => 'Η μεταφορά αποθηκεύτηκε';

  @override
  String get transportSavedAndVerified =>
      'Η μεταφορά αποθηκεύτηκε και επαληθεύτηκε το SMTP';

  @override
  String get transportSavedVerifyPending =>
      'Η μεταφορά αποθηκεύτηκε, αλλά ο διακομιστής δεν ήταν προσβάσιμος ή η ταυτοποίηση απέτυχε. Ελέγξτε κεντρικό υπολογιστή, ασφάλεια και διαπιστευτήρια και πατήστε Αποθήκευση ξανά.';

  @override
  String get transportTypeDialogTitle => 'Τύπος εξερχόμενης μεταφοράς';

  @override
  String get transportTypeFixedHelper =>
      'Επιλέγεται κατά την προσθήκη· δεν αλλάζει εδώ.';

  @override
  String get transportDisplayNameRequired => 'Απαιτείται εμφανιζόμενο όνομα.';

  @override
  String get transportKindLabel => 'Τύπος εξερχόμενης αποστολής';

  @override
  String get transportKindSmtp => 'SMTP';

  @override
  String get transportKindGmail => 'Gmail (Google)';

  @override
  String get gmailTransportPresetHelper =>
      'Χρησιμοποιεί smtp.gmail.com με OAuth (XOAUTH2). Αποθηκεύστε τη μεταφορά και συνδεθείτε με τον ίδιο λογαριασμό Google όπως στο Gmail IMAP.';

  @override
  String get newTransport => 'Νέα μεταφορά';

  @override
  String get editTransport => 'Επεξεργασία μεταφοράς';

  @override
  String get displayNameLabel => 'Εμφανιζόμενο όνομα';

  @override
  String get smtpHostLabel => 'Κόμβος SMTP';

  @override
  String get smtpPortHelper => 'Συνήθως 587 (STARTTLS) ή 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'Σύνδεση IMAP';

  @override
  String get matrixSignInTitle => 'Σύνδεση Matrix';

  @override
  String get gmailSignInTitle => 'Σύνδεση με Google';

  @override
  String get gmailSignInBody =>
      'Θα ανοίξει ο περιηγητής για εξουσιοδότηση Gmail (IMAP). Απαιτούνται TAGLIACARTE_GOOGLE_CLIENT_ID (και συνήθως TAGLIACARTE_GOOGLE_CLIENT_SECRET).';

  @override
  String get gmailSignInBrowserButton => 'Συνέχεια στον περιηγητή';

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
  String get enterUsernameAndPassword => 'Εισάγετε όνομα χρήστη και κωδικό.';

  @override
  String get usernameLabel => 'Όνομα χρήστη';

  @override
  String get passwordLabel => 'Κωδικός';

  @override
  String get showPasswordTooltip => 'Εμφάνιση κωδικού';

  @override
  String get hidePasswordTooltip => 'Απόκρυψη κωδικού';

  @override
  String get fieldFrom => 'Από';

  @override
  String get composeOutgoingTransport => 'Εξερχόμενη μεταφορά';

  @override
  String get composeSendSucceeded => 'Το μήνυμα στάλθηκε';

  @override
  String get composeMissingFrom => 'Εισαγάγετε διεύθυνση αποστολέα.';

  @override
  String get composeMissingTo => 'Εισαγάγετε τουλάχιστον έναν παραλήπτη.';

  @override
  String get composeMissingNewsgroups =>
      'Εισαγάγετε τουλάχιστον ένα όνομα ομάδας συζητήσεων.';

  @override
  String get composeNntpPostingBlurb =>
      'Η δημοσίευση γίνεται μέσω του διακομιστή NNTP αυτού του λογαριασμού (χωρίς ξεχωριστό μέσο αποστολής).';

  @override
  String get fieldNewsgroups => 'Ομάδες συζητήσεων';

  @override
  String get fieldTo => 'Προς';

  @override
  String get fieldCc => 'Κοιν';

  @override
  String get fieldBcc => 'Κρυφή κοιν';

  @override
  String get fieldSubject => 'Θέμα';

  @override
  String get fieldBody => 'Σώμα';

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
  String get folderNewSubfolder => 'Νέος υποφάκελος';

  @override
  String get folderRename => 'Μετονομασία…';

  @override
  String get folderDelete => 'Διαγραφή…';

  @override
  String get folderNewTooltip => 'Νέος φάκελος';

  @override
  String get folderNewDialogTitle => 'Νέος φάκελος';

  @override
  String get folderNameLabel => 'Όνομα φακέλου';

  @override
  String get folderNewTopLevelHelper => 'Δημιουργεί θυρίδα στο ανώτερο επίπεδο';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Υποφάκελος του $parent';
  }

  @override
  String get subfolderNameLabel => 'Όνομα υποφακέλου';

  @override
  String subfolderPathHelper(String path) {
    return 'Διαδρομή: $path';
  }

  @override
  String folderCreated(String name) {
    return 'Δημιουργήθηκε ο φάκελος «$name»';
  }

  @override
  String get renameFolderTitle => 'Μετονομασία φακέλου';

  @override
  String get newFolderPathLabel => 'Νέα διαδρομή φακέλου';

  @override
  String get folderRenamed => 'Ο φάκελος μετονομάστηκε';

  @override
  String get deleteFolderTitle => 'Διαγραφή φακέλου;';

  @override
  String deleteFolderBody(String name) {
    return 'Να αφαιρεθεί το «$name» και τα μηνύματά του από τον διακομιστή (αν υποστηρίζεται); Δεν αναιρείται.';
  }

  @override
  String get folderDeleted => 'Ο φάκελος διαγράφηκε';

  @override
  String get licenseTitle => 'Άδεια';

  @override
  String get copyrightTitle => 'Πνευματικά δικαιώματα';

  @override
  String get chatHintTypeMessage => 'Πληκτρολογήστε μήνυμα';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.';

  @override
  String operationFailed(String error) {
    return 'Κάτι πήγε στραβά: $error';
  }

  @override
  String get expandFolder => 'Ανάπτυξη';

  @override
  String get collapseFolder => 'Σύμπτυξη';

  @override
  String get noTextBody => '(Χωρίς κείμενο σώματος)';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Μετακίνηση εδώ';

  @override
  String get folderCopyHere => 'Αντιγραφή εδώ';

  @override
  String get folderExpunge => 'Οριστική διαγραφή σημασμένων μηνυμάτων';

  @override
  String get folderExpungeDone => 'Η εκκαθάριση ολοκληρώθηκε';

  @override
  String pendingMoveTagged(int count) {
    return 'Επιλέξτε φάκελο, μετά «Μετακίνηση εδώ» ($count μηνύματα)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Επιλέξτε φάκελο, μετά «Αντιγραφή εδώ» ($count μηνύματα)';
  }

  @override
  String transferResultOk(int count) {
    return 'Έγινε: $count μηνύματα.';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok επιτυχία, $failed αποτυχίες.';
  }

  @override
  String transferFailed(String error) {
    return 'Η μεταφορά απέτυχε: $error';
  }

  @override
  String deleteMessagesFailed(String error) {
    return 'Η διαγραφή απέτυχε: $error';
  }

  @override
  String get settingsNotifyNewMessages => 'Ειδοποιήσεις νέων μηνυμάτων';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Snackbar όταν η εφαρμογή είναι ανοιχτή· ειδοποίηση συστήματος στο παρασκήνιο (IMAP).';

  @override
  String get newMailNotificationTitle => 'Νέα αλληλογραφία';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count νέο/α μήνυμα/τα στο $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel =>
      'Ελάχ. δευτερόλεπτα αδράνειας πριν το IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Κενό = προεπιλογή (120). Ελάχιστο 15. Μετά από αδράνεια σύνδεσης.';

  @override
  String get validationImapMinIdleSeconds => 'Ακέραιος 15–864000 ή κενό.';
}
