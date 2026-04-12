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
  String get messageSignatureVerifiedTooltip =>
      'Η υπογραφή επαληθεύτηκε για αυτή την επαφή';

  @override
  String get messageSignatureInvalidTooltip => 'Η επαλήθευση υπογραφής απέτυχε';

  @override
  String get messageSignatureUnknownTooltip =>
      'Δεν ήταν δυνατή η επαλήθευση υπογραφής (λείπει ή είναι άγνωστο το κλειδί)';

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
  String get settingsTabContacts => 'Contacts';

  @override
  String get settingsTabAbout => 'Σχετικά';

  @override
  String get settingsContactsRepositories => 'Address book sync';

  @override
  String get settingsContactsAddPlatformRepo => 'Add platform address book';

  @override
  String get settingsContactsAddCarddavRepo => 'Add CardDAV account';

  @override
  String get settingsContactsImportVcard => 'Import vCard file';

  @override
  String get settingsContactsExportVcard => 'Export contacts (vCard)';

  @override
  String get settingsContactsMergePlatform => 'Import from system contacts';

  @override
  String get settingsContactsGroups => 'Groups';

  @override
  String get settingsContactsNewGroup => 'New group';

  @override
  String get settingsContactsRepoName => 'Name';

  @override
  String get settingsContactsRepoUrl => 'Server URL';

  @override
  String get settingsContactsLearnedNote =>
      'Contacts learned from mail stay local until you allow external sync.';

  @override
  String get settingsContactsEditRepository => 'Edit repository';

  @override
  String get settingsContactsDeleteRepository => 'Delete';

  @override
  String get settingsContactsCollectionPath => 'Collection path';

  @override
  String get settingsContactsDefaultNewContact => 'Default for new contacts';

  @override
  String get settingsContactsCarddavPull => 'Pull from server';

  @override
  String get settingsContactsCarddavPush => 'Push local changes';

  @override
  String get settingsContactsUsername => 'Username';

  @override
  String get settingsContactsPassword => 'Password';

  @override
  String get settingsContactsApplyRules => 'Apply group rules';

  @override
  String get settingsContactsLocalContacts => 'Local contacts';

  @override
  String get settingsContactsAllowExternalSync =>
      'Allow syncing to external accounts';

  @override
  String get settingsContactsGroupShareHint =>
      'When enabled, members are linked to the repository when you apply rules (learned contacts must allow sync first).';

  @override
  String get settingsContactsLinkPlatformMerge =>
      'Link imported contacts to the platform repository';

  @override
  String get settingsLoadFailed =>
      'Δεν ήταν δυνατή η φόρτωση των ρυθμίσεων από τον δίσκο.';

  @override
  String get settingsLoadRetry => 'Επανάληψη';

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
      'Αφήστε κενό για «Deleted Items» (γραμματοκιβώτιο στα αγγλικά). Χρησιμοποιήστε το ακριβές όνομα φακέλου που εμφανίζεται στο Outlook αν διαφέρει.';

  @override
  String get exchangeJunkFolderHelper =>
      'Αφήστε κενό για «Junk Email» (γραμματοκιβώτιο στα αγγλικά). Χρησιμοποιήστε το ακριβές όνομα φακέλου που εμφανίζεται στο Outlook αν διαφέρει.';

  @override
  String get deleteModeDeleteImmediately => 'Άμεση διαγραφή';

  @override
  String get deleteModeMoveToTrash => 'Μετακίνηση στον κάδο';

  @override
  String get deleteModeMarkDeleted => 'Σήμανση ως διαγραμμένο';

  @override
  String get quoteOriginalOnReply =>
      'Παράθεση του αρχικού μηνύματος στην απάντηση';

  @override
  String get quoteOriginalOnReplySubtitle =>
      'Προσθέτει το πρωτότυπο κάτω από την κεφαλίδα απάντησης. Η μορφοποιημένη σύνθεση το τυλίγει σε μπλοκ παράθεσης· η απλή σύνθεση προτάσσει κάθε γραμμή. Το τμήμα text/plain συνεχίζει να περιλαμβάνει το πρωτότυπο όταν είναι ενεργό.';

  @override
  String get composingReplySection => 'Παραπομπές σε απαντήσεις';

  @override
  String get replyHeaderTemplateLabel => 'Γραμμή κεφαλίδας απάντησης';

  @override
  String get replyHeaderTemplateHelp =>
      'Εμφανίζεται πάνω από το παρατιθέμενο πρωτότυπο. Συμπεριλάβετε τις λέξεις date, time και sender με το σύμβολο \$ μπροστά (δείτε προεπισκόπηση). Κατά την απάντηση αντικαθίστανται από ημερομηνία, ώρα και αποστολέα.';

  @override
  String get replyHeaderPreviewLabel => 'Προεπισκόπηση';

  @override
  String get replyDateFormatLabel => 'Ημερομηνία (στην κεφαλίδα)';

  @override
  String get replyTimeFormatLabel => 'Ώρα (στην κεφαλίδα)';

  @override
  String get replyDatePresetLocale => 'Όπως το σύστημα (μακρά ημερομηνία)';

  @override
  String get replyDatePresetIso => 'ISO: 2026-04-08';

  @override
  String get replyDatePresetUs => 'ΗΠΑ: 04/08/2026';

  @override
  String get replyDatePresetEu => 'Ημέρα/μήνας/έτος: 08/04/2026';

  @override
  String get replyDatePresetMedium => 'Μέτριο: 8 Απρ 2026';

  @override
  String get replyDatePresetWeekday => 'Με ημέρα εβδομάδας: Τετ 8 Απρ 2026';

  @override
  String replyDatePresetCustom(String pattern) {
    return 'Προσαρμοσμένο ($pattern)';
  }

  @override
  String get replyTimePresetLocale => 'Όπως το σύστημα';

  @override
  String get replyTimePreset12h => '12ώρο (π.χ. 1:30 μ.μ.)';

  @override
  String get replyTimePreset24h => '24ώρο (15:30)';

  @override
  String get replyTimePreset24hSeconds => '24ώρο με δευτερόλεπτα';

  @override
  String replyTimePresetCustom(String pattern) {
    return 'Προσαρμοσμένο ($pattern)';
  }

  @override
  String get replyLinePrefixLabel => 'Πρόθεμα γραμμής παράθεσης';

  @override
  String get replyLinePrefixSubtitle =>
      'Προστίθεται στην αρχή κάθε γραμμής του πρωτοτύπου σε απλό κείμενο (κλασικό «> »). Μόνο όταν είναι ενεργή η παράθεση.';

  @override
  String get replyPlainPositionLabel => 'Σειρά απάντησης και παράθεσης';

  @override
  String get replyPlainPositionBefore =>
      'Απάντηση πριν το παρατιθέμενο κείμενο';

  @override
  String get replyPlainPositionAfter => 'Απάντηση μετά το παρατιθέμενο κείμενο';

  @override
  String get replyPlainPositionSubtitle =>
      'Απλό ή πλούσιο κείμενο: δύο κενές γραμμές και δρομέας πριν από την κεφαλίδα απάντησης, ή μετά το μπλοκ παράθεσης. Το text/plain κατά την αποστολή ακολουθεί τη διάταξη.';

  @override
  String get replyQuoteModeLabel => 'Μέρη HTML SMTP';

  @override
  String get replyQuoteModePlain => 'Μόνο πρωτότυπο σε παράθεση απλού κειμένου';

  @override
  String get replyQuoteModeHtmlSmtp =>
      'Συμπερίληψη πρωτοτύπου και ως ξεχωριστό HTML (SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'Προσθέτει δεύτερο τμήμα HTML διατηρώντας τη μορφοποίηση. Οι πελάτες μόνο κειμένου βλέπουν ακόμη το απλό σώμα. Οι αναρτήσεις NNTP χρησιμοποιούν πάντα απλή παράθεση.';

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
      'Θα ανοίξει ο περιηγητής για σύνδεση μέσω Google και εξουσιοδότηση πρόσβασης στο Gmail (IMAP).';

  @override
  String get gmailSignInBrowserButton => 'Συνέχεια στον περιηγητή';

  @override
  String get smtpSignInTitle => 'Σύνδεση SMTP';

  @override
  String smtpSignInSubtitle(String transportName, String host) {
    return 'Εισαγάγετε όνομα χρήστη και κωδικό για το «$transportName» ($host).';
  }

  @override
  String get composeSendCancelledNoSmtpCredentials =>
      'Το μήνυμα δεν στάλθηκε: τα διαπιστευτήρια SMTP δεν αποθηκεύτηκαν.';

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
  String get attach => 'Επισύναψη';

  @override
  String get composeRemoveAttachment => 'Αφαίρεση συνημμένου';

  @override
  String get defaultFromLabel => 'Προεπιλεγμένη διεύθυνση Από';

  @override
  String get defaultFromHelper =>
      'π.χ. Το όνομά σας <you@example.com> ή you@example.com';

  @override
  String get dsnLabel => 'Ειδοποιήσεις παράδοσης';

  @override
  String get dsnUseTransportDefault => 'Προεπιλογή μεταφορέα';

  @override
  String get dsnNever => 'Ποτέ';

  @override
  String get dsnFailure => 'Σε αποτυχία';

  @override
  String get dsnSuccess => 'Σε επιτυχία';

  @override
  String get dsnDelay => 'Σε καθυστέρηση';

  @override
  String get dsnFailureAndSuccess => 'Σε αποτυχία και επιτυχία';

  @override
  String get dsnNotifyLabel => 'Ειδοποίηση DSN';

  @override
  String get composeCryptoLabel => 'Υπογραφή / κρυπτογράφηση';

  @override
  String get composeCryptoTitle => 'Εξερχόμενη υπογραφή και κρυπτογράφηση';

  @override
  String get composeCryptoNone => 'Χωρίς κρυπτογράφηση';

  @override
  String get composeCryptoSign => 'Υπογραφή';

  @override
  String get composeCryptoEncrypt => 'Κρυπτογράφηση';

  @override
  String get composeCryptoSignEncrypt => 'Υπογραφή και κρυπτογράφηση';

  @override
  String get settingsMailCryptoSection => 'Υπογραφή αλληλογραφίας (εξερχόμενα)';

  @override
  String get settingsMailCryptoStackSubtitle =>
      'Στοίβα κρυπτογράφησης για εξερχόμενη υπογραφή και κρυπτογράφηση (σύνθεση).';

  @override
  String get settingsMailCryptoStackOpenpgp => 'OpenPGP';

  @override
  String get settingsMailCryptoStackSmime => 'S/MIME';

  @override
  String get settingsMailCryptoPgpSecretKeyPath =>
      'Αρχικός κατάλογος GnuPG (προαιρετικό)';

  @override
  String get settingsMailCryptoPgpPassphrase =>
      'Κλειδί υπογραφής OpenPGP (ID ή δακτυλικό αποτύπωμα)';

  @override
  String get settingsMailCryptoSmimeCert =>
      'Πιστοποιητικό υπογραφής S/MIME (διαδρομή PEM)';

  @override
  String get settingsMailCryptoSmimeKey =>
      'Ιδιωτικό κλειδί υπογραφής S/MIME (διαδρομή PEM)';

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
      'Η συνομιλία δεν μπορεί ακόμη να στείλει συνημμένα αρχεία. Αφαιρέστε τα για αποστολή ή χρησιμοποιήστε τη σύνθεση αλληλογραφίας για αρχεία.';

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
  String get matrixE2eeUndecryptableTitle =>
      'Αυτό το μήνυμα δεν μπορεί να αποκρυπτογραφηθεί ακόμη';

  @override
  String get matrixE2eeUndecryptableHelp =>
      'Αυτή η συνομιλία προστατεύεται με κρυπτογράφηση άκρο-προς-άκρο Matrix. Το Tagliacarte δεν διαθέτει το κλειδί δωματίου για αυτό το μήνυμα.\n\nΤι μπορείτε να κάνετε:\n• Στο Element (ή άλλο πελάτη Matrix): Ρυθμίσεις → Ασφάλεια → Ασφαλής δημιουργία αντιγράφων — ξεκλειδώστε με το κλειδί ή τη φράση ανάκτησης. Αν το Tagliacarte προσφέρει επαναφορά αντιγράφου κλειδιών, χρησιμοποιήστε το ίδιο κλειδί εκεί.\n• Σε άλλη συσκευή όπου έχετε ήδη διαβάσει αυτή τη συνομιλία (π.χ. Element σε τηλέφωνο ή υπολογιστή): συνδεθείτε, επαληθεύστε αυτή τη συνεδρία Tagliacarte αν ζητηθεί, κρατήστε τη συσκευή συνδεδεμένη και ανοίξτε αυτό το άμεσο μήνυμα ώστε να προωθηθούν τα κλειδιά.\n• Αφού οι συσκευές εμπιστευτούν η μία την άλλη, ζητήστε από την επαφή νέο μήνυμα — βοηθά μόνο τα νέα μηνύματα· τα παλιά χρειάζονται ακόμη κλειδιά από αντίγραφο ή άλλη συσκευή.\n\nΧωρίς ασφαλές αντίγραφο και χωρίς άλλον συνδεδεμένο πελάτη, το παλιό κρυπτογραφημένο ιστορικό μπορεί να μείνει αδιάβαστο — έτσι ορίζεται στο Matrix.';

  @override
  String get matrixE2eeUndecryptableListPreview =>
      'Δεν αποκρυπτογραφείται ακόμη — ανοίξτε το μήνυμα για βήματα';

  @override
  String get matrixE2eeUndecryptableChatSnippet =>
      'Δεν αποκρυπτογραφείται ακόμη';

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
  String get folderTabSubscribed => 'Σε συνδρομή';

  @override
  String get folderTabAvailable => 'Διαθέσιμα';

  @override
  String get matrixFolderTabRooms => 'Δωμάτια';

  @override
  String get matrixFolderTabDirectMessages => 'Άμεσα μηνύματα';

  @override
  String get folderActionSubscribe => 'Συνδρομή';

  @override
  String get folderActionUnsubscribe => 'Κατάργηση συνδρομής';

  @override
  String get folderActionJoinRoom => 'Συμμετοχή σε δωμάτιο';

  @override
  String get folderActionLeaveRoom => 'Αποχώρηση από δωμάτιο';

  @override
  String get nntpWildmatHint => 'Μοτίβο (π.χ. comp.os.linux.*)';

  @override
  String get nntpWildmatQuery => 'Λίστα';

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

  @override
  String get contactsCalendarComingSoonTooltip => 'Ημερολόγιο (σύντομα)';

  @override
  String get contactsOpenTooltip => 'Επαφές';

  @override
  String get contactsScreenTitle => 'Επαφές';

  @override
  String get contactsAllGroups => 'Όλες οι επαφές';

  @override
  String get contactsGroupsDrawerTooltip => 'Βιβλία διευθύνσεων και ομάδες';

  @override
  String get contactsToolbarImport => 'Εισαγωγή';

  @override
  String get contactsToolbarImportTooltip => 'Εισαγωγή αρχείου vCard';

  @override
  String get contactsToolbarSync => 'Συγχρονισμός';

  @override
  String get contactsToolbarSyncTooltip =>
      'Συγχρονισμός με απομακρυσμένο βιβλίο διευθύνσεων';

  @override
  String get contactsToolbarDelete => 'Διαγραφή';

  @override
  String get contactsToolbarDeleteTooltip =>
      'Διαγραφή επαφών, ομάδας ή βιβλίου διευθύνσεων';

  @override
  String get contactsSelect => 'Επιλογή';

  @override
  String get contactsSelectDone => 'Τέλος';

  @override
  String get contactsEmptyDetail => 'Επιλέξτε επαφή';

  @override
  String get contactsEmptyList => 'Καμία επαφή';

  @override
  String contactsLoadError(String error) {
    return 'Δεν ήταν δυνατή η φόρτωση επαφών: $error';
  }

  @override
  String get contactsDeleteSelectedTitle => 'Διαγραφή επαφών;';

  @override
  String contactsDeleteSelectedBody(int count) {
    return 'Διαγραφή $count επαφών; Δεν είναι δυνατή η αναίρεση.';
  }

  @override
  String get contactsDeleteGroupTitle => 'Διαγραφή ομάδας;';

  @override
  String contactsDeleteGroupBody(String name) {
    return 'Αφαίρεση «$name»; Οι επαφές παραμένουν στη βάση· αφαιρείται μόνο η συμμετοχή στην ομάδα.';
  }

  @override
  String get contactsDeleteRepositoryTitle => 'Διαγραφή βιβλίου διευθύνσεων;';

  @override
  String contactsDeleteRepositoryBody(String name) {
    return 'Αφαίρεση «$name» από αυτή τη συσκευή; Οι επαφές μπορεί να παραμείνουν τοπικά.';
  }

  @override
  String get contactsSyncNeedGroup =>
      'Επιλέξτε ομάδα συνδεδεμένη με βιβλίο διευθύνσεων για συγχρονισμό.';

  @override
  String contactsImportDone(int count) {
    return 'Εισήχθησαν $count επαφές.';
  }

  @override
  String get contactsRepositoriesTitle => 'Βιβλία διευθύνσεων';

  @override
  String get contactsNewContact => 'Νέα επαφή';

  @override
  String get contactsDetailTitle => 'Επαφή';

  @override
  String get contactsSave => 'Αποθήκευση';

  @override
  String get contactsRepositoryLinks => 'Βιβλία διευθύνσεων';

  @override
  String get contactsGroupMembership => 'Ομάδες';

  @override
  String get contactsSyncPickRepository => 'Επιλογή βιβλίου διευθύνσεων';

  @override
  String get contactsDeleteMenuContacts => 'Διαγραφή επιλεγμένων επαφών';

  @override
  String get contactsDeleteMenuGroup => 'Διαγραφή αυτής της ομάδας…';

  @override
  String get contactsDeleteMenuRepository => 'Διαγραφή βιβλίου διευθύνσεων…';

  @override
  String contactsSyncMenuPull(String name) {
    return 'Λήψη $name';
  }

  @override
  String contactsSyncMenuPush(String name) {
    return 'Αποστολή $name';
  }

  @override
  String get contactsMergePlatform => 'Εισαγωγή από επαφές συστήματος';

  @override
  String get contactsToolbarNewGroup => 'Νέα ομάδα';

  @override
  String get contactsToolbarNewGroupTooltip => 'Δημιουργία νέας ομάδας';

  @override
  String get contactsDisplayName => 'Εμφανιζόμενο όνομα';

  @override
  String get contactsNotes => 'Σημειώσεις';

  @override
  String get contactsAddEmail => 'Προσθήκη email';

  @override
  String get contactsAllowExternalShare =>
      'Να επιτρέπεται κοινή χρήση με συγχρονισμένα βιβλία διευθύνσεων';

  @override
  String get contactsSaved => 'Η επαφή αποθηκεύτηκε';

  @override
  String get contactsEmailLabel => 'Ετικέτα';
}
