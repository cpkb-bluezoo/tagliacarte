// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for German (`de`).
class AppLocalizationsDe extends AppLocalizations {
  AppLocalizationsDe([String locale = 'de']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Einstellungen';

  @override
  String get compose => 'Verfassen';

  @override
  String get send => 'Senden';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'Abbrechen';

  @override
  String get save => 'Speichern';

  @override
  String get remove => 'Entfernen';

  @override
  String get delete => 'Löschen';

  @override
  String get discard => 'Verwerfen';

  @override
  String get back => 'Zurück';

  @override
  String get create => 'Anlegen';

  @override
  String get rename => 'Umbenennen';

  @override
  String get folderLabel => 'Ordner';

  @override
  String get messageTitle => 'Nachricht';

  @override
  String get selectFolder => 'Ordner auswählen';

  @override
  String get selectMessage => 'Nachricht auswählen';

  @override
  String get selectMessageToRead => 'Wählen Sie eine Nachricht zum Lesen.';

  @override
  String get noMessages => 'Keine Nachrichten';

  @override
  String get attachments => 'Anhänge';

  @override
  String get saveAttachment => 'Anhang speichern';

  @override
  String savedToPath(String path) {
    return 'Gespeichert unter $path';
  }

  @override
  String saveFailed(String error) {
    return 'Speichern fehlgeschlagen: $error';
  }

  @override
  String get cannotDownloadAttachment =>
      'Dieser Anhang kann nicht heruntergeladen werden';

  @override
  String get emptyAttachmentData => 'Leere Anhangsdaten';

  @override
  String downloadFailed(String error) {
    return 'Download fehlgeschlagen: $error';
  }

  @override
  String get saveVerb => 'Speichern';

  @override
  String get loadImages => 'Bilder laden';

  @override
  String get remoteImagesBlocked =>
      'Externe Bilder sind aus Datenschutzgründen blockiert.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'HTML-Inhalt konnte nicht geöffnet werden: $error';
  }

  @override
  String webViewError(String error) {
    return 'WebView-Fehler: $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'Der sichtbare Linktext zeigt eine andere Adresse als das Ziel dieses Links.';

  @override
  String get headerFrom => 'Von:';

  @override
  String get headerTo => 'An:';

  @override
  String get headerCc => 'Cc:';

  @override
  String get headerDate => 'Datum:';

  @override
  String get folderInbox => 'Posteingang';

  @override
  String get messageActionReply => 'Antworten';

  @override
  String get messageActionReplyAll => 'Allen antworten';

  @override
  String get messageActionForward => 'Weiterleiten';

  @override
  String get messageActionDelete => 'Löschen';

  @override
  String get messageActionJunk => 'Spam';

  @override
  String get messageActionMove => 'Verschieben';

  @override
  String get messageActionCopy => 'Kopieren';

  @override
  String get messageMenuTooltip => 'Nachrichtenaktionen';

  @override
  String get settingsViewMinimalHeaders => 'Kurze Nachrichtenköpfe';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'Wenn aktiv, wird nur Cc ausgeblendet; Von, An und Datum bleiben sichtbar, sofern vorhanden.';

  @override
  String get settingsTooltip => 'Einstellungen';

  @override
  String get accountsAndFoldersTooltip => 'Konten und Ordner';

  @override
  String get cancelSelectionTooltip => 'Auswahl aufheben';

  @override
  String multiSelectCount(int count) {
    return '$count ausgewählt';
  }

  @override
  String get composeTooltip => 'Verfassen';

  @override
  String get composeNeedTransportTooltip =>
      'Legen Sie unter Einstellungen einen ausgehenden Transport an';

  @override
  String get mailToolbarMoreTooltip => 'Mehr';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count ausgewählt';
  }

  @override
  String get settingsTabAccounts => 'Konten';

  @override
  String get settingsTabOutgoing => 'Ausgang';

  @override
  String get settingsTabSecurity => 'Sicherheit';

  @override
  String get settingsTabViewing => 'Ansicht';

  @override
  String get settingsTabComposing => 'Verfassen';

  @override
  String get settingsTabAbout => 'Info';

  @override
  String get settingsLoadFailed => 'Could not load settings from disk.';

  @override
  String get settingsLoadRetry => 'Retry';

  @override
  String get useSystemKeychain => 'System-Schlüsselbund verwenden';

  @override
  String get storeCredentialsInKeychain =>
      'Zugangsdaten im Plattform-Schlüsselbund speichern';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Mit Google anmelden';

  @override
  String get authenticateMicrosoft => 'Mit Microsoft anmelden';

  @override
  String get reloadOAuthToken => 'OAuth-Token neu laden';

  @override
  String get matrixE2eeSection => 'Matrix Ende-zu-Ende-Verschlüsselung';

  @override
  String get initCrypto => 'Verschlüsselung einrichten';

  @override
  String get setupBackup => 'Sicherung einrichten';

  @override
  String get restoreBackup => 'Sicherung wiederherstellen';

  @override
  String get showDeviceFingerprint => 'Geräte-Fingerprint anzeigen';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Nachricht unter der Liste (Desktop)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'Wenn aus, öffnet sich die Nachricht in einer eigenen Vollbildansicht.';

  @override
  String get loadRemoteImages => 'Externe Bilder laden';

  @override
  String get loadRemoteImagesSubtitle =>
      'Externe Bilder in HTML-Mails zulassen';

  @override
  String get threadedView => 'Konversationsansicht';

  @override
  String get threadedViewSubtitle => 'E-Mails nach Unterhaltung gruppieren';

  @override
  String get deletionAndTrashSection => 'Löschen und Papierkorb';

  @override
  String get deletionAppliesGlobally => 'Gilt für alle Mail-Konten.';

  @override
  String get deleteModeLabel => 'Löschmodus';

  @override
  String get trashFolderNameLabel => 'Name des Papierkorb-Ordners';

  @override
  String get junkFolderNameLabel => 'Name des Spam-Ordners';

  @override
  String get exchangeTrashFolderHelper =>
      'Leave empty to use “Deleted Items” (English mailbox). Use the exact folder name shown in Outlook if yours differs.';

  @override
  String get exchangeJunkFolderHelper =>
      'Leave empty to use “Junk Email” (English mailbox). Use the exact folder name shown in Outlook if yours differs.';

  @override
  String get deleteModeDeleteImmediately => 'Sofort löschen';

  @override
  String get deleteModeMoveToTrash => 'In Papierkorb verschieben';

  @override
  String get deleteModeMarkDeleted => 'Als gelöscht markieren';

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
  String get settingsComposeRichText =>
      'Formatierter Text beim E-Mail-Schreiben';

  @override
  String get settingsComposeRichTextSubtitle =>
      'Formatierter Editor für neue E-Mails und Antworten. Usenet (NNTP) bleibt unformatiert.';

  @override
  String get settingsMatrixChatRichText => 'Formatierter Text in Matrix-Chats';

  @override
  String get settingsMatrixChatRichTextSubtitle =>
      'Formatierte Nachrichten in Matrix-Räumen senden (mit Nur-Text-Fallback).';

  @override
  String get testSend => 'Testsendung';

  @override
  String get openSignatureEditor => 'Signatur-Editor öffnen';

  @override
  String get aboutSubtitle => 'E-Mail und Messaging plattformübergreifend';

  @override
  String get supportedBackends => 'Unterstützte Backends';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation (Demo)';
  }

  @override
  String get accountTypeDialogTitle => 'Kontotyp';

  @override
  String get removeAccountTitle => 'Konto entfernen?';

  @override
  String removeAccountBody(String label) {
    return '„$label“ aus der auf diesem Gerät gespeicherten Konfiguration entfernen?';
  }

  @override
  String removedAccount(String label) {
    return '„$label“ entfernt';
  }

  @override
  String get accountsListTitle => 'Konten';

  @override
  String get accountsListSubtitle =>
      'Tippen Sie auf ein Konto zum Bearbeiten oder fügen Sie unten eines hinzu.';

  @override
  String get deleteTooltip => 'Löschen';

  @override
  String get addAccount => 'Konto hinzufügen';

  @override
  String get noAccountsYet =>
      'Noch keine Konten. Tippen Sie auf „Konto hinzufügen“.';

  @override
  String get discardChangesTitle => 'Änderungen verwerfen?';

  @override
  String get discardChangesBody =>
      'Ihre Änderungen gehen verloren – wie beim Verlassen ohne Speichern.';

  @override
  String get keepEditing => 'Weiter bearbeiten';

  @override
  String get pickNotSupportedWeb =>
      'Datei- oder Ordnerauswahl wird in der Web-Version nicht unterstützt';

  @override
  String get chooseMaildirFolderTitle => 'Maildir-Stammordner wählen';

  @override
  String get chooseMboxFileTitle => 'mbox-Datei wählen';

  @override
  String get validationAccountNameRequired => 'Kontoname ist erforderlich';

  @override
  String get validationLocalPathRequired =>
      'Pfad zum lokalen Postfach ist erforderlich';

  @override
  String get validationUsernameRequired =>
      'Benutzername oder E-Mail ist für diesen Kontotyp erforderlich';

  @override
  String get validationEmailAddressRequired =>
      'E-Mail-Adresse ist erforderlich';

  @override
  String get validationMatrixUserIdRequired =>
      'Matrix-Benutzer-ID ist erforderlich';

  @override
  String get accountEmailAddressLabel => 'E-Mail-Adresse';

  @override
  String get accountMatrixUserIdLabel => 'Matrix-ID (MXID)';

  @override
  String get accountMatrixMxidHelper =>
      'Beispiel: @you:matrix.org — die Homeserver-URL wird aus der Domain nach dem Doppelpunkt abgeleitet.';

  @override
  String get validationMatrixMxidInvalid =>
      'Geben Sie eine Matrix-ID wie @user:server ein';

  @override
  String get accountNntpDefaultFromLabel => 'Standard-Absender (Usenet)';

  @override
  String get accountNntpDefaultFromHelper =>
      'Beim Verfassen von Beiträgen; dieses NNTP-Konto postet über seine eigene Serververbindung.';

  @override
  String get accountEmailOptionalLabel => 'E-Mail-Adresse (optional)';

  @override
  String get accountTcpLoginHelper =>
      'Anmeldekennung für diesen Server (meist Ihre E-Mail-Adresse).';

  @override
  String get validationHostRequired => 'Server-Hostname ist erforderlich';

  @override
  String get validationPortRequired => 'Gültige Portnummer erforderlich';

  @override
  String get accountSaved => 'Konto gespeichert';

  @override
  String get createTransportFirst =>
      'Legen Sie zuerst unter „Ausgang“ einen Transport an';

  @override
  String get addTransportDialogTitle => 'Transport hinzufügen';

  @override
  String get accountTypeLabel => 'Kontotyp';

  @override
  String get accountTypeHelper => 'Beim Anlegen gewählt; hier nicht änderbar.';

  @override
  String get accountNameLabel => 'Kontoname';

  @override
  String get usernameEmailOptional => 'Benutzername / E-Mail (optional)';

  @override
  String get usernameEmailRequired => 'Benutzername / E-Mail';

  @override
  String get avatarUrlLabel => 'Avatar-URL oder Dateipfad (optional)';

  @override
  String get avatarUrlHelper =>
      'Optionales Bild oder lokaler Pfad für die Kontoleiste';

  @override
  String get localMailboxSection => 'Lokales Postfach';

  @override
  String get pathMboxFile => 'Pfad zur mbox-Datei';

  @override
  String get pathMaildirRoot => 'Pfad zum Maildir-Stamm';

  @override
  String get helperMboxPath =>
      'Dateischaltfläche zum Durchsuchen oder absoluten Pfad eingeben';

  @override
  String get helperMaildirPath =>
      'Ordnerschaltfläche zum Durchsuchen oder absoluten Pfad eingeben';

  @override
  String get chooseMboxTooltip => 'mbox-Datei wählen';

  @override
  String get chooseMaildirTooltip => 'Maildir-Ordner wählen';

  @override
  String get imapServerSection => 'IMAP-Server';

  @override
  String get pop3ServerSection => 'POP3-Server';

  @override
  String get nntpServerSection => 'NNTP-Server';

  @override
  String get hostLabel => 'Host';

  @override
  String get serverHostLabel => 'Server-Host';

  @override
  String get portLabel => 'Port';

  @override
  String get portHelperImap => 'Üblich: 993 (IMAPS) oder 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'Üblich: 995 (POP3S, implizites TLS)';

  @override
  String get portHelperNntp => 'Üblich: 563 (NNTPS, implizites TLS)';

  @override
  String get securityLabel => 'Sicherheit';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (implizites TLS)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (implizites TLS)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (implizites TLS)';

  @override
  String get mailSecurityImplicitTlsNntp => 'NNTPS (implizites TLS)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'Keine Verschlüsselung';

  @override
  String get outgoingTransportsSection => 'Ausgehende Transporte';

  @override
  String get noTransportsHintLinked =>
      'Kein Transport gewählt – Verfassen und Antworten bleiben deaktiviert, bis mindestens einer ausgewählt ist. Legen Sie Transporte unter „Ausgang“ an und wählen Sie sie hier.';

  @override
  String get transportsOrderHint =>
      'Der erste in der Liste ist der Standard zum Senden. Transporte werden unter „Ausgang“ angelegt.';

  @override
  String get unknownTransport => 'Unbekannter Transport';

  @override
  String get moveUpTooltip => 'Nach oben';

  @override
  String get moveDownTooltip => 'Nach unten';

  @override
  String get removeFromAccountTooltip => 'Vom Konto entfernen';

  @override
  String get addTransportToAccount => 'Transport zum Konto hinzufügen';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Homeserver';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'Relay-URLs';

  @override
  String get relayUrlsHelper =>
      'Jede Zeile ist eine Relay-WebSocket-URL. Drücken Sie Enter, wenn Sie die Bearbeitung einer URL beenden.';

  @override
  String get relayAddFieldHint => 'Neue Relay-URL';

  @override
  String get relayAddTooltip => 'Relay hinzufügen';

  @override
  String get relayRemoveTooltip => 'Relay entfernen';

  @override
  String get nostrNewIdentityTooltip => 'Neue Nostr-Identität erstellen';

  @override
  String get nostrRelayUrlsRequired => 'Mindestens eine Relay-URL eingeben.';

  @override
  String storeUriLabel(String uri) {
    return 'Verbindung: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'Veraltete Ausgangs-URI: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'Neues $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return '$label bearbeiten';
  }

  @override
  String foldersLoadError(String error) {
    return 'Ordner: $error';
  }

  @override
  String get sortMessagesTooltip => 'Nachrichten sortieren';

  @override
  String get sort => 'Sortieren';

  @override
  String get sortFromAz => 'Von A–Z';

  @override
  String get sortFromZa => 'Von Z–A';

  @override
  String get sortSubjectAz => 'Betreff A–Z';

  @override
  String get sortSubjectZa => 'Betreff Z–A';

  @override
  String get sortDateOldest => 'Datum: älteste zuerst';

  @override
  String get sortDateNewest => 'Datum: neueste zuerst';

  @override
  String get removeTransportTitle => 'Transport entfernen?';

  @override
  String removeTransportBody(String name) {
    return '„$name“ wird aus allen ausgehenden Listen der Konten entfernt.';
  }

  @override
  String removedTransport(String name) {
    return '„$name“ entfernt';
  }

  @override
  String get outgoingListTitle => 'Ausgang';

  @override
  String get outgoingListSubtitle =>
      'SMTP und andere Sende-Transporte. Verknüpfen Sie sie unter „Konten“ mit Mail-Konten.';

  @override
  String get addTransport => 'Transport hinzufügen';

  @override
  String get noTransportsYet =>
      'Noch keine ausgehenden Transporte. Tippen Sie auf „Transport hinzufügen“.';

  @override
  String get transportDisplayHostRequired =>
      'Anzeigename und Host sind erforderlich.';

  @override
  String get transportSaved => 'Transport gespeichert';

  @override
  String get transportSavedAndVerified =>
      'Transport gespeichert und SMTP geprüft';

  @override
  String get transportSavedVerifyPending =>
      'Transport gespeichert, aber der Server war nicht erreichbar oder die Anmeldung ist fehlgeschlagen. Prüfen Sie Host, Sicherheit und Zugangsdaten und speichern Sie erneut.';

  @override
  String get transportTypeDialogTitle => 'Ausgangstransport-Typ';

  @override
  String get transportTypeFixedHelper =>
      'Beim Anlegen gewählt; hier nicht änderbar.';

  @override
  String get transportDisplayNameRequired => 'Anzeigename ist erforderlich.';

  @override
  String get transportKindLabel => 'Ausgangstyp';

  @override
  String get transportKindSmtp => 'SMTP';

  @override
  String get transportKindGmail => 'Gmail (Google)';

  @override
  String get gmailTransportPresetHelper =>
      'Nutzt smtp.gmail.com mit OAuth (XOAUTH2). Transport speichern, dann mit demselben Google-Konto anmelden wie für Gmail-IMAP.';

  @override
  String get newTransport => 'Neuer Transport';

  @override
  String get editTransport => 'Transport bearbeiten';

  @override
  String get displayNameLabel => 'Anzeigename';

  @override
  String get smtpHostLabel => 'SMTP-Host';

  @override
  String get smtpPortHelper => 'Üblich: 587 (STARTTLS) oder 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'IMAP-Anmeldung';

  @override
  String get matrixSignInTitle => 'Matrix-Anmeldung';

  @override
  String get gmailSignInTitle => 'Mit Google anmelden';

  @override
  String get gmailSignInBody =>
      'Der Browser öffnet sich zur Gmail-Autorisierung (IMAP). Die App benötigt TAGLIACARTE_GOOGLE_CLIENT_ID (und üblicherweise TAGLIACARTE_GOOGLE_CLIENT_SECRET).';

  @override
  String get gmailSignInBrowserButton => 'Im Browser fortfahren';

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
  String get enterUsernameAndPassword => 'Benutzername und Passwort eingeben.';

  @override
  String get usernameLabel => 'Benutzername';

  @override
  String get passwordLabel => 'Passwort';

  @override
  String get showPasswordTooltip => 'Passwort anzeigen';

  @override
  String get hidePasswordTooltip => 'Passwort ausblenden';

  @override
  String get fieldFrom => 'Von';

  @override
  String get composeOutgoingTransport => 'Ausgangsserver';

  @override
  String get composeSendSucceeded => 'Nachricht gesendet';

  @override
  String get composeMissingFrom => 'Geben Sie eine Absenderadresse ein.';

  @override
  String get composeMissingTo => 'Geben Sie mindestens einen Empfänger ein.';

  @override
  String get composeMissingNewsgroups =>
      'Geben Sie mindestens einen Newsgruppennamen ein.';

  @override
  String get composeNntpPostingBlurb =>
      'Beiträge gehen über den NNTP-Server dieses Kontos (kein separater Transport).';

  @override
  String get fieldNewsgroups => 'Newsgruppen';

  @override
  String get fieldTo => 'An';

  @override
  String get fieldCc => 'Cc';

  @override
  String get fieldBcc => 'Bcc';

  @override
  String get fieldSubject => 'Betreff';

  @override
  String get fieldBody => 'Text';

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
  String get folderNewSubfolder => 'Neuer Unterordner';

  @override
  String get folderRename => 'Umbenennen…';

  @override
  String get folderDelete => 'Löschen…';

  @override
  String get folderNewTooltip => 'Neuer Ordner';

  @override
  String get folderNewDialogTitle => 'Neuer Ordner';

  @override
  String get folderNameLabel => 'Ordnername';

  @override
  String get folderNewTopLevelHelper =>
      'Legt einen Postfach-Ordner auf oberster Ebene an';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Unterordner von $parent';
  }

  @override
  String get subfolderNameLabel => 'Name des Unterordners';

  @override
  String subfolderPathHelper(String path) {
    return 'Pfad: $path';
  }

  @override
  String folderCreated(String name) {
    return 'Ordner „$name“ angelegt';
  }

  @override
  String get renameFolderTitle => 'Ordner umbenennen';

  @override
  String get newFolderPathLabel => 'Neuer Ordnerpfad';

  @override
  String get folderRenamed => 'Ordner umbenannt';

  @override
  String get deleteFolderTitle => 'Ordner löschen?';

  @override
  String deleteFolderBody(String name) {
    return '„$name“ und seine Nachrichten auf dem Server entfernen (falls unterstützt)? Dies kann nicht rückgängig gemacht werden.';
  }

  @override
  String get folderDeleted => 'Ordner gelöscht';

  @override
  String get licenseTitle => 'Lizenz';

  @override
  String get copyrightTitle => 'Urheberrecht';

  @override
  String get chatHintTypeMessage => 'Nachricht eingeben';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.';

  @override
  String operationFailed(String error) {
    return 'Etwas ist schiefgegangen: $error';
  }

  @override
  String get expandFolder => 'Aufklappen';

  @override
  String get collapseFolder => 'Einklappen';

  @override
  String get noTextBody => '(Kein Text)';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Hierher verschieben';

  @override
  String get folderCopyHere => 'Hierher kopieren';

  @override
  String get folderExpunge => 'Gelöschte Nachrichten endgültig entfernen';

  @override
  String get folderExpungeDone => 'Expunge abgeschlossen';

  @override
  String pendingMoveTagged(int count) {
    return 'Ordner wählen, dann „Hierher verschieben“ ($count Nachrichten)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Ordner wählen, dann „Hierher kopieren“ ($count Nachrichten)';
  }

  @override
  String transferResultOk(int count) {
    return 'Fertig: $count Nachricht(en).';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok OK, $failed fehlgeschlagen.';
  }

  @override
  String transferFailed(String error) {
    return 'Transfer fehlgeschlagen: $error';
  }

  @override
  String deleteMessagesFailed(String error) {
    return 'Löschen fehlgeschlagen: $error';
  }

  @override
  String get settingsNotifyNewMessages =>
      'Benachrichtigungen für neue Nachrichten';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Snackbar bei geöffneter App; Systembenachrichtigung im Hintergrund (IMAP).';

  @override
  String get newMailNotificationTitle => 'Neue E-Mail';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count neue Nachricht(en) in $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel => 'Min. Ruhesekunden vor IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Leer lassen für Standard (120). Minimum 15. Gilt bei inaktiver Verbindung.';

  @override
  String get validationImapMinIdleSeconds =>
      'Ganze Zahl von 15 bis 864000 oder leer lassen.';
}
