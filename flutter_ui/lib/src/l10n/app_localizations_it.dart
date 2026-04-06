// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Italian (`it`).
class AppLocalizationsIt extends AppLocalizations {
  AppLocalizationsIt([String locale = 'it']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Impostazioni';

  @override
  String get compose => 'Componi';

  @override
  String get send => 'Invia';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'Annulla';

  @override
  String get save => 'Salva';

  @override
  String get remove => 'Rimuovi';

  @override
  String get delete => 'Elimina';

  @override
  String get discard => 'Scarta';

  @override
  String get back => 'Indietro';

  @override
  String get create => 'Crea';

  @override
  String get rename => 'Rinomina';

  @override
  String get folderLabel => 'Cartella';

  @override
  String get messageTitle => 'Messaggio';

  @override
  String get selectFolder => 'Seleziona una cartella';

  @override
  String get selectMessage => 'Seleziona un messaggio';

  @override
  String get selectMessageToRead => 'Seleziona un messaggio da leggere.';

  @override
  String get noMessages => 'Nessun messaggio';

  @override
  String get attachments => 'Allegati';

  @override
  String get saveAttachment => 'Salva allegato';

  @override
  String savedToPath(String path) {
    return 'Salvato in $path';
  }

  @override
  String saveFailed(String error) {
    return 'Salvataggio non riuscito: $error';
  }

  @override
  String get cannotDownloadAttachment =>
      'Impossibile scaricare questo allegato';

  @override
  String get emptyAttachmentData => 'Dati allegato vuoti';

  @override
  String downloadFailed(String error) {
    return 'Download non riuscito: $error';
  }

  @override
  String get saveVerb => 'Salva';

  @override
  String get loadImages => 'Carica immagini';

  @override
  String get remoteImagesBlocked => 'Immagini remote bloccate per la privacy.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'Impossibile aprire il corpo HTML: $error';
  }

  @override
  String webViewError(String error) {
    return 'Errore WebView: $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'Il testo del collegamento mostra un indirizzo diverso dalla destinazione effettiva.';

  @override
  String get headerFrom => 'Da:';

  @override
  String get headerTo => 'A:';

  @override
  String get headerCc => 'Cc:';

  @override
  String get headerDate => 'Data:';

  @override
  String get folderInbox => 'Posta in arrivo';

  @override
  String get messageActionReply => 'Rispondi';

  @override
  String get messageActionReplyAll => 'Rispondi a tutti';

  @override
  String get messageActionForward => 'Inoltra';

  @override
  String get messageActionDelete => 'Elimina';

  @override
  String get messageActionJunk => 'Posta indesiderata';

  @override
  String get messageActionMove => 'Sposta';

  @override
  String get messageActionCopy => 'Copia';

  @override
  String get messageMenuTooltip => 'Azioni sul messaggio';

  @override
  String get settingsViewMinimalHeaders => 'Intestazioni messaggio ridotte';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'Se attivo, viene nascosto solo Cc; Da, A e data restano visibili se disponibili.';

  @override
  String get settingsTooltip => 'Impostazioni';

  @override
  String get accountsAndFoldersTooltip => 'Account e cartelle';

  @override
  String get cancelSelectionTooltip => 'Annulla selezione';

  @override
  String multiSelectCount(int count) {
    return '$count selezionati';
  }

  @override
  String get composeTooltip => 'Componi';

  @override
  String get composeNeedTransportTooltip =>
      'Aggiungi un trasporto in uscita nelle Impostazioni';

  @override
  String get mailToolbarMoreTooltip => 'Altro';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count selezionati';
  }

  @override
  String get settingsTabAccounts => 'Account';

  @override
  String get settingsTabOutgoing => 'In uscita';

  @override
  String get settingsTabSecurity => 'Sicurezza';

  @override
  String get settingsTabViewing => 'Visualizzazione';

  @override
  String get settingsTabComposing => 'Composizione';

  @override
  String get settingsTabAbout => 'Informazioni';

  @override
  String get useSystemKeychain => 'Usa portachiavi di sistema';

  @override
  String get storeCredentialsInKeychain =>
      'Salva le credenziali nel portachiavi della piattaforma';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Autentica con Google';

  @override
  String get authenticateMicrosoft => 'Autentica con Microsoft';

  @override
  String get reloadOAuthToken => 'Ricarica token OAuth';

  @override
  String get matrixE2eeSection => 'Crittografia end-to-end Matrix';

  @override
  String get initCrypto => 'Inizializza crittografia';

  @override
  String get setupBackup => 'Configura backup';

  @override
  String get restoreBackup => 'Ripristina backup';

  @override
  String get showDeviceFingerprint => 'Mostra impronta dispositivo';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Dettaglio messaggio sotto l’elenco (desktop)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'Se disattivato, l’apertura di un messaggio usa una vista a schermo intero separata.';

  @override
  String get loadRemoteImages => 'Carica immagini remote';

  @override
  String get loadRemoteImagesSubtitle =>
      'Consenti immagini esterne nelle e-mail HTML';

  @override
  String get threadedView => 'Vista per conversazione';

  @override
  String get threadedViewSubtitle => 'Raggruppa i messaggi per thread';

  @override
  String get deletionAndTrashSection => 'Eliminazione e cestino';

  @override
  String get deletionAppliesGlobally =>
      'Si applica a tutti gli account di posta.';

  @override
  String get deleteModeLabel => 'Modalità di eliminazione';

  @override
  String get trashFolderNameLabel => 'Nome cartella cestino';

  @override
  String get deleteModeMoveToTrash => 'Sposta nel cestino';

  @override
  String get deleteModeMarkDeleted => 'Segna come eliminato';

  @override
  String get quoteOriginalOnReply =>
      'Cita il messaggio originale nella risposta';

  @override
  String get composingReplySection => 'Citazione nella risposta';

  @override
  String get replyHeaderTemplateLabel => 'Riga di intestazione della risposta';

  @override
  String get replyHeaderTemplateHint =>
      'Segnaposto: \\u0024date, \\u0024time, \\u0024sender';

  @override
  String get replyDateFormatLabel => 'Formato data della risposta (ICU)';

  @override
  String get replyDateFormatHint =>
      'Lasciare vuoto per il formato data lungo della lingua';

  @override
  String get replyTimeFormatLabel => 'Formato ora della risposta (ICU)';

  @override
  String get replyTimeFormatHint =>
      'Lasciare vuoto per il formato ora della lingua (senza frazioni di secondo)';

  @override
  String get replyLinePrefixLabel => 'Prefisso righe citate';

  @override
  String get replyQuoteModeLabel => 'Messaggio originale nell’email in uscita';

  @override
  String get replyQuoteModePlain =>
      'Testo semplice con prefisso (tutti i trasporti)';

  @override
  String get replyQuoteModeHtmlSmtp => 'Aggiungi HTML originale (solo SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'La modalità HTML aggiunge una seconda parte MIME con il testo formattato originale; i client solo testo vedono comunque il corpo citato in chiaro. NNTP usa sempre testo semplice.';

  @override
  String get testSend => 'Invio di prova';

  @override
  String get openSignatureEditor => 'Apri editor firma';

  @override
  String get aboutSubtitle => 'E-mail e messaggistica multipiattaforma';

  @override
  String get supportedBackends => 'Backend supportati';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation (demo)';
  }

  @override
  String get accountTypeDialogTitle => 'Tipo di account';

  @override
  String get removeAccountTitle => 'Rimuovere l’account?';

  @override
  String removeAccountBody(String label) {
    return 'Rimuovere «$label» dalla configurazione salvata su questo dispositivo?';
  }

  @override
  String removedAccount(String label) {
    return 'Rimosso «$label»';
  }

  @override
  String get accountsListTitle => 'Account';

  @override
  String get accountsListSubtitle =>
      'Tocca un account per modificarlo o aggiungine uno sotto.';

  @override
  String get deleteTooltip => 'Elimina';

  @override
  String get addAccount => 'Aggiungi account';

  @override
  String get noAccountsYet => 'Nessun account. Tocca «Aggiungi account».';

  @override
  String get discardChangesTitle => 'Scartare le modifiche?';

  @override
  String get discardChangesBody =>
      'Le modifiche andranno perse, come uscendo senza salvare.';

  @override
  String get keepEditing => 'Continua a modificare';

  @override
  String get pickNotSupportedWeb =>
      'La scelta di file o cartelle non è supportata nella build web';

  @override
  String get chooseMaildirFolderTitle => 'Scegli cartella radice Maildir';

  @override
  String get chooseMboxFileTitle => 'Scegli file mbox';

  @override
  String get validationAccountNameRequired => 'Il nome account è obbligatorio';

  @override
  String get validationLocalPathRequired =>
      'Il percorso della casella locale è obbligatorio';

  @override
  String get validationUsernameRequired =>
      'Nome utente o e-mail obbligatorio per questo tipo di account';

  @override
  String get validationEmailAddressRequired =>
      'L’indirizzo e-mail è obbligatorio';

  @override
  String get validationMatrixUserIdRequired =>
      'L’ID utente Matrix è obbligatorio';

  @override
  String get accountEmailAddressLabel => 'Indirizzo e-mail';

  @override
  String get accountMatrixUserIdLabel => 'ID Matrix (MXID)';

  @override
  String get accountMatrixMxidHelper =>
      'Esempio: @you:matrix.org — l’URL dell’homeserver si ricava dal dominio dopo i due punti.';

  @override
  String get validationMatrixMxidInvalid =>
      'Inserisci un ID Matrix come @user:server';

  @override
  String get accountNntpDefaultFromLabel => 'Mittente predefinito (Usenet)';

  @override
  String get accountNntpDefaultFromHelper =>
      'Mostrato durante la composizione; questo account NNTP pubblica tramite la propria connessione al server.';

  @override
  String get accountEmailOptionalLabel => 'Indirizzo e-mail (facoltativo)';

  @override
  String get accountTcpLoginHelper =>
      'Identità di accesso per questo server (di solito l’e-mail).';

  @override
  String get validationHostRequired => 'L’host del server è obbligatorio';

  @override
  String get validationPortRequired => 'È richiesto un numero di porta valido';

  @override
  String get accountSaved => 'Account salvato';

  @override
  String get createTransportFirst =>
      'Crea prima un trasporto nella scheda In uscita';

  @override
  String get addTransportDialogTitle => 'Aggiungi trasporto';

  @override
  String get accountTypeLabel => 'Tipo di account';

  @override
  String get accountTypeHelper =>
      'Scelto quando aggiungi l’account; non modificabile qui.';

  @override
  String get accountNameLabel => 'Nome account';

  @override
  String get usernameEmailOptional => 'Nome utente / e-mail (facoltativo)';

  @override
  String get usernameEmailRequired => 'Nome utente / e-mail';

  @override
  String get avatarUrlLabel => 'URL avatar o percorso file (facoltativo)';

  @override
  String get avatarUrlHelper =>
      'Immagine o percorso locale facoltativo per la barra account';

  @override
  String get localMailboxSection => 'Casella locale';

  @override
  String get pathMboxFile => 'Percorso file mbox';

  @override
  String get pathMaildirRoot => 'Percorso radice Maildir';

  @override
  String get helperMboxPath =>
      'Usa il pulsante file per sfogliare, o digita un percorso assoluto';

  @override
  String get helperMaildirPath =>
      'Usa il pulsante cartella per sfogliare, o digita un percorso assoluto';

  @override
  String get chooseMboxTooltip => 'Scegli file mbox';

  @override
  String get chooseMaildirTooltip => 'Scegli cartella Maildir';

  @override
  String get imapServerSection => 'Server IMAP';

  @override
  String get pop3ServerSection => 'Server POP3';

  @override
  String get nntpServerSection => 'Server NNTP';

  @override
  String get hostLabel => 'Host';

  @override
  String get serverHostLabel => 'Host server';

  @override
  String get portLabel => 'Porta';

  @override
  String get portHelperImap => 'Di solito 993 (IMAPS) o 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'Di solito 995 (POP3S, TLS implicito)';

  @override
  String get portHelperNntp => 'Di solito 563 (NNTPS, TLS implicito)';

  @override
  String get securityLabel => 'Sicurezza';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (TLS implicito)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (TLS implicito)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (TLS implicito)';

  @override
  String get mailSecurityImplicitTlsNntp => 'NNTPS (TLS implicito)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'Nessuna crittografia';

  @override
  String get outgoingTransportsSection => 'Trasporti in uscita';

  @override
  String get noTransportsHintLinked =>
      'Nessun trasporto selezionato: componi e rispondi restano disattivati finché non ne scegli almeno uno. Usa la scheda In uscita e selezionalo qui.';

  @override
  String get transportsOrderHint =>
      'Il primo nell’elenco è il predefinito per l’invio. Crea i trasporti in In uscita.';

  @override
  String get unknownTransport => 'Trasporto sconosciuto';

  @override
  String get moveUpTooltip => 'Sposta su';

  @override
  String get moveDownTooltip => 'Sposta giù';

  @override
  String get removeFromAccountTooltip => 'Rimuovi dall’account';

  @override
  String get addTransportToAccount => 'Aggiungi trasporto all’account';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Homeserver';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'URL relay';

  @override
  String get relayUrlsHelper =>
      'Ogni riga è un URL WebSocket di un relay. Premi Invio quando hai finito di modificare un URL.';

  @override
  String get relayAddFieldHint => 'Nuovo URL relay';

  @override
  String get relayAddTooltip => 'Aggiungi relay';

  @override
  String get relayRemoveTooltip => 'Rimuovi relay';

  @override
  String get nostrNewIdentityTooltip => 'Crea nuova identità Nostr';

  @override
  String get nostrRelayUrlsRequired => 'Inserisci almeno un URL relay.';

  @override
  String storeUriLabel(String uri) {
    return 'Connessione: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'URI di trasporto legacy: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'Nuovo $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return 'Modifica $label';
  }

  @override
  String foldersLoadError(String error) {
    return 'Cartelle: $error';
  }

  @override
  String get sortMessagesTooltip => 'Ordina messaggi';

  @override
  String get sort => 'Ordina';

  @override
  String get sortFromAz => 'Da A a Z';

  @override
  String get sortFromZa => 'Da Z a A';

  @override
  String get sortSubjectAz => 'Oggetto A–Z';

  @override
  String get sortSubjectZa => 'Oggetto Z–A';

  @override
  String get sortDateOldest => 'Data: più vecchi prima';

  @override
  String get sortDateNewest => 'Data: più recenti prima';

  @override
  String get removeTransportTitle => 'Rimuovere il trasporto?';

  @override
  String removeTransportBody(String name) {
    return '«$name» verrà rimosso dagli elenchi in uscita di tutti gli account.';
  }

  @override
  String removedTransport(String name) {
    return 'Rimosso «$name»';
  }

  @override
  String get outgoingListTitle => 'In uscita';

  @override
  String get outgoingListSubtitle =>
      'SMTP e altri trasporti di invio. Collegali agli account nella scheda Account.';

  @override
  String get addTransport => 'Aggiungi trasporto';

  @override
  String get noTransportsYet =>
      'Nessun trasporto in uscita. Tocca «Aggiungi trasporto».';

  @override
  String get transportDisplayHostRequired =>
      'Nome visualizzato e host sono obbligatori.';

  @override
  String get transportSaved => 'Trasporto salvato';

  @override
  String get transportSavedAndVerified => 'Trasporto salvato e SMTP verificato';

  @override
  String get transportSavedVerifyPending =>
      'Trasporto salvato, ma il server non è raggiungibile o l’autenticazione non è riuscita. Controlla host, sicurezza e credenziali, poi salva di nuovo.';

  @override
  String get transportTypeDialogTitle => 'Tipo di trasporto in uscita';

  @override
  String get transportTypeFixedHelper =>
      'Scelto in aggiunta; non modificabile qui.';

  @override
  String get transportDisplayNameRequired =>
      'Il nome visualizzato è obbligatorio.';

  @override
  String get transportKindLabel => 'Tipo in uscita';

  @override
  String get transportKindSmtp => 'SMTP';

  @override
  String get transportKindGmail => 'Gmail (Google)';

  @override
  String get gmailTransportPresetHelper =>
      'Usa smtp.gmail.com con OAuth (XOAUTH2). Salva il trasporto, poi accedi con lo stesso account Google dell’IMAP Gmail.';

  @override
  String get newTransport => 'Nuovo trasporto';

  @override
  String get editTransport => 'Modifica trasporto';

  @override
  String get displayNameLabel => 'Nome visualizzato';

  @override
  String get smtpHostLabel => 'Host SMTP';

  @override
  String get smtpPortHelper => 'Di solito 587 (STARTTLS) o 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'Accesso IMAP';

  @override
  String get gmailSignInTitle => 'Accedi con Google';

  @override
  String get gmailSignInBody =>
      'Si aprirà il browser per autorizzare Gmail (IMAP). L’app richiede TAGLIACARTE_GOOGLE_CLIENT_ID (e di solito TAGLIACARTE_GOOGLE_CLIENT_SECRET).';

  @override
  String get gmailSignInBrowserButton => 'Continua nel browser';

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
  String get enterUsernameAndPassword => 'Inserisci nome utente e password.';

  @override
  String get usernameLabel => 'Nome utente';

  @override
  String get passwordLabel => 'Password';

  @override
  String get showPasswordTooltip => 'Mostra password';

  @override
  String get hidePasswordTooltip => 'Nascondi password';

  @override
  String get fieldFrom => 'Da';

  @override
  String get composeOutgoingTransport => 'Trasporto in uscita';

  @override
  String get composeSendSucceeded => 'Messaggio inviato';

  @override
  String get composeMissingFrom => 'Inserisci un mittente.';

  @override
  String get composeMissingTo => 'Inserisci almeno un destinatario.';

  @override
  String get composeMissingNewsgroups =>
      'Inserisci almeno un nome di newsgroup.';

  @override
  String get composeNntpPostingBlurb =>
      'I messaggi passano dal server NNTP di questo account (nessun transport separato).';

  @override
  String get fieldNewsgroups => 'Newsgroup';

  @override
  String get fieldTo => 'A';

  @override
  String get fieldCc => 'Cc';

  @override
  String get fieldBcc => 'Ccn';

  @override
  String get fieldSubject => 'Oggetto';

  @override
  String get fieldBody => 'Corpo';

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
  String get folderNewSubfolder => 'Nuova sottocartella';

  @override
  String get folderRename => 'Rinomina…';

  @override
  String get folderDelete => 'Elimina…';

  @override
  String get folderNewTooltip => 'Nuova cartella';

  @override
  String get folderNewDialogTitle => 'Nuova cartella';

  @override
  String get folderNameLabel => 'Nome cartella';

  @override
  String get folderNewTopLevelHelper => 'Crea una casella di primo livello';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Sottocartella di $parent';
  }

  @override
  String get subfolderNameLabel => 'Nome sottocartella';

  @override
  String subfolderPathHelper(String path) {
    return 'Percorso: $path';
  }

  @override
  String folderCreated(String name) {
    return 'Cartella «$name» creata';
  }

  @override
  String get renameFolderTitle => 'Rinomina cartella';

  @override
  String get newFolderPathLabel => 'Nuovo percorso cartella';

  @override
  String get folderRenamed => 'Cartella rinominata';

  @override
  String get deleteFolderTitle => 'Eliminare la cartella?';

  @override
  String deleteFolderBody(String name) {
    return 'Rimuovere «$name» e i suoi messaggi dal server (se supportato)? Operazione irreversibile.';
  }

  @override
  String get folderDeleted => 'Cartella eliminata';

  @override
  String get licenseTitle => 'Licenza';

  @override
  String get copyrightTitle => 'Copyright';

  @override
  String get chatHintTypeMessage => 'Scrivi un messaggio';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.';

  @override
  String operationFailed(String error) {
    return 'Si è verificato un errore: $error';
  }

  @override
  String get expandFolder => 'Espandi';

  @override
  String get collapseFolder => 'Comprimi';

  @override
  String get noTextBody => '(Nessun corpo di testo)';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Sposta qui';

  @override
  String get folderCopyHere => 'Copia qui';

  @override
  String get folderExpunge =>
      'Elimina definitivamente i messaggi contrassegnati';

  @override
  String get folderExpungeDone => 'Eliminazione completata';

  @override
  String pendingMoveTagged(int count) {
    return 'Scegli una cartella, poi Sposta qui ($count messaggi)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Scegli una cartella, poi Copia qui ($count messaggi)';
  }

  @override
  String transferResultOk(int count) {
    return 'Fatto: $count messaggio/i.';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok riusciti, $failed non riusciti.';
  }

  @override
  String transferFailed(String error) {
    return 'Trasferimento non riuscito: $error';
  }

  @override
  String get settingsNotifyNewMessages => 'Notifiche per nuovi messaggi';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Snack bar con l’app aperta; notifica di sistema in background (IMAP).';

  @override
  String get newMailNotificationTitle => 'Nuova posta';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count nuovo/i messaggio/i in $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel =>
      'Secondi di inattività prima di IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Vuoto = predefinito (120). Minimo 15. Dopo inattività della connessione.';

  @override
  String get validationImapMinIdleSeconds => 'Intero tra 15 e 864000, o vuoto.';
}
