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
  String get messageSignatureVerifiedTooltip =>
      'Firma crittografica verificata per questo contatto';

  @override
  String get messageSignatureInvalidTooltip =>
      'Verifica della firma non riuscita';

  @override
  String get messageSignatureUnknownTooltip =>
      'Impossibile verificare la firma (chiave assente o sconosciuta)';

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
  String get settingsTabContacts => 'Contacts';

  @override
  String get settingsTabAbout => 'Informazioni';

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
      'Impossibile caricare le impostazioni dal disco.';

  @override
  String get settingsLoadRetry => 'Riprova';

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
  String get junkFolderNameLabel => 'Nome cartella posta indesiderata';

  @override
  String get exchangeTrashFolderHelper =>
      'Lasciare vuoto per usare «Elementi eliminati» (casella in inglese). Usare il nome esatto mostrato in Outlook se diverso.';

  @override
  String get exchangeJunkFolderHelper =>
      'Lasciare vuoto per usare «Posta indesiderata» (casella in inglese). Usare il nome esatto mostrato in Outlook se diverso.';

  @override
  String get deleteModeDeleteImmediately => 'Elimina subito';

  @override
  String get deleteModeMoveToTrash => 'Sposta nel cestino';

  @override
  String get deleteModeMarkDeleted => 'Segna come eliminato';

  @override
  String get quoteOriginalOnReply =>
      'Cita il messaggio originale nella risposta';

  @override
  String get quoteOriginalOnReplySubtitle =>
      'Inserisce l’originale sotto l’intestazione di risposta nelle nuove risposte. La composizione formattata lo racchiude in un blocco citazione; il testo semplice antepone un prefisso a ogni riga. La parte text/plain include ancora l’originale se l’opzione è attiva.';

  @override
  String get composingReplySection => 'Citazioni nelle risposte';

  @override
  String get replyHeaderTemplateLabel => 'Riga intestazione risposta';

  @override
  String get replyHeaderTemplateHelp =>
      'Mostrato sopra l’originale citato. Includere le parole date, time e sender, ciascuna con il dollaro davanti (vedere anteprima). Alla risposta sono sostituite con data, ora e mittente.';

  @override
  String get replyHeaderPreviewLabel => 'Anteprima';

  @override
  String get replyDateFormatLabel => 'Data (nell’intestazione)';

  @override
  String get replyTimeFormatLabel => 'Ora (nell’intestazione)';

  @override
  String get replyDatePresetLocale => 'Come il sistema (data lunga)';

  @override
  String get replyDatePresetIso => 'ISO: 2026-04-08';

  @override
  String get replyDatePresetUs => 'USA: 04/08/2026';

  @override
  String get replyDatePresetEu => 'Giorno/mese/anno: 08/04/2026';

  @override
  String get replyDatePresetMedium => 'Media: 8 apr 2026';

  @override
  String get replyDatePresetWeekday =>
      'Con giorno della settimana: mer 8 apr 2026';

  @override
  String replyDatePresetCustom(String pattern) {
    return 'Personalizzato ($pattern)';
  }

  @override
  String get replyTimePresetLocale => 'Come il sistema';

  @override
  String get replyTimePreset12h => '12 ore (es. 13:30)';

  @override
  String get replyTimePreset24h => '24 ore (15:30)';

  @override
  String get replyTimePreset24hSeconds => '24 ore con secondi';

  @override
  String replyTimePresetCustom(String pattern) {
    return 'Personalizzato ($pattern)';
  }

  @override
  String get replyLinePrefixLabel => 'Prefisso riga citata';

  @override
  String get replyLinePrefixSubtitle =>
      'Anteposto a ogni riga dell’originale nelle citazioni in solo testo (classico «> »). Solo se la citazione è attiva.';

  @override
  String get replyPlainPositionLabel => 'Ordine risposta e citazione';

  @override
  String get replyPlainPositionBefore => 'Risposta prima del testo citato';

  @override
  String get replyPlainPositionAfter => 'Risposta dopo il testo citato';

  @override
  String get replyPlainPositionSubtitle =>
      'Testo semplice o formattato: due righe vuote e cursore prima dell’intestazione di risposta, o due righe vuote e cursore dopo il blocco citato. La parte text/plain all’invio segue lo stesso layout.';

  @override
  String get replyQuoteModeLabel => 'Parti HTML SMTP';

  @override
  String get replyQuoteModePlain =>
      'Solo originale in citazione testo semplice';

  @override
  String get replyQuoteModeHtmlSmtp =>
      'Includi anche l’originale come HTML separato (SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'Aggiunge una seconda parte HTML che conserva la formattazione del messaggio sorgente. I client solo testo vedono ancora il corpo citato in chiaro. Gli invii NNTP usano sempre citazioni in solo testo.';

  @override
  String get settingsComposeRichText =>
      'Testo formattato nella composizione e-mail';

  @override
  String get settingsComposeRichTextSubtitle =>
      'Editor formattato per nuove e-mail e risposte. Usenet (NNTP) resta solo testo.';

  @override
  String get settingsMatrixChatRichText => 'Testo formattato nelle chat Matrix';

  @override
  String get settingsMatrixChatRichTextSubtitle =>
      'Invia messaggi formattati nelle stanze Matrix (con fallback in solo testo).';

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
  String get matrixSignInTitle => 'Accesso Matrix';

  @override
  String get gmailSignInTitle => 'Accedi con Google';

  @override
  String get gmailSignInBody =>
      'Si aprirà il browser per accedere con Google e autorizzare l’accesso a Gmail (IMAP).';

  @override
  String get gmailSignInBrowserButton => 'Continua nel browser';

  @override
  String get smtpSignInTitle => 'Accesso SMTP';

  @override
  String smtpSignInSubtitle(String transportName, String host) {
    return 'Inserire nome utente e password per «$transportName» ($host).';
  }

  @override
  String get composeSendCancelledNoSmtpCredentials =>
      'Messaggio non inviato: le credenziali SMTP non sono state salvate.';

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
  String get attach => 'Allega';

  @override
  String get composeRemoveAttachment => 'Rimuovi allegato';

  @override
  String get defaultFromLabel => 'Indirizzo Da predefinito';

  @override
  String get defaultFromHelper =>
      'es. Il tuo nome <you@example.com> o you@example.com';

  @override
  String get dsnLabel => 'Notifiche di consegna';

  @override
  String get dsnUseTransportDefault => 'Predefinito del trasporto';

  @override
  String get dsnNever => 'Mai';

  @override
  String get dsnFailure => 'In caso di errore';

  @override
  String get dsnSuccess => 'In caso di successo';

  @override
  String get dsnDelay => 'In caso di ritardo';

  @override
  String get dsnFailureAndSuccess => 'In caso di errore e di successo';

  @override
  String get dsnNotifyLabel => 'Notifica DSN';

  @override
  String get composeCryptoLabel => 'Firma / crittografia';

  @override
  String get composeCryptoTitle => 'Firma e crittografia in uscita';

  @override
  String get composeCryptoNone => 'Nessuna crittografia';

  @override
  String get composeCryptoSign => 'Firma';

  @override
  String get composeCryptoEncrypt => 'Crittografa';

  @override
  String get composeCryptoSignEncrypt => 'Firma e crittografa';

  @override
  String get settingsMailCryptoSection => 'Firma messaggi (in uscita)';

  @override
  String get settingsMailCryptoStackSubtitle =>
      'Stack crittografico per firma e crittografia in uscita (composizione).';

  @override
  String get settingsMailCryptoStackOpenpgp => 'OpenPGP';

  @override
  String get settingsMailCryptoStackSmime => 'S/MIME';

  @override
  String get settingsMailCryptoPgpSecretKeyPath =>
      'Directory home GnuPG (opzionale)';

  @override
  String get settingsMailCryptoPgpPassphrase =>
      'ID chiave o impronta OpenPGP per la firma';

  @override
  String get settingsMailCryptoSmimeCert =>
      'Certificato di firma S/MIME (percorso PEM)';

  @override
  String get settingsMailCryptoSmimeKey =>
      'Chiave privata di firma S/MIME (percorso PEM)';

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
      'La chat non può ancora inviare allegati. Rimuoverli per inviare il messaggio o usare la composizione e-mail per i file.';

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
  String get matrixE2eeUndecryptableTitle =>
      'Questo messaggio non può ancora essere decifrato';

  @override
  String get matrixE2eeUndecryptableHelp =>
      'Questa chat è protetta dalla crittografia end-to-end di Matrix. Tagliacarte non ha la chiave della stanza per questo messaggio.\n\nCosa puoi fare:\n• In Element (o in un altro client Matrix): Impostazioni → Sicurezza → Backup sicuro — sblocca con la chiave o la passphrase di recupero. Se Tagliacarte offre il ripristino del backup delle chiavi, usa la stessa chiave lì.\n• Su un altro dispositivo dove hai già letto questa chat (es. Element su telefono o desktop): accedi, verifica questa sessione Tagliacarte se richiesto, tieni il dispositivo online e apri questo messaggio diretto così le chiavi possono essere inoltrate.\n• Dopo che i dispositivi si fidano a vicenda, chiedi al contatto un nuovo messaggio — serve solo per i messaggi nuovi; quelli vecchi servono ancora chiavi dal backup o da un altro dispositivo.\n\nSenza backup sicuro e senza un altro client connesso, la cronologia cifrata più vecchia può restare illeggibile — è voluto in Matrix.';

  @override
  String get matrixE2eeUndecryptableListPreview =>
      'Decifratura non ancora possibile — apri il messaggio per i passaggi';

  @override
  String get matrixE2eeUndecryptableChatSnippet =>
      'Decifratura non ancora possibile';

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
  String get folderTabSubscribed => 'Iscritti';

  @override
  String get folderTabAvailable => 'Disponibili';

  @override
  String get matrixFolderTabRooms => 'Stanze';

  @override
  String get matrixFolderTabDirectMessages => 'Messaggi diretti';

  @override
  String get folderActionSubscribe => 'Iscriviti';

  @override
  String get folderActionUnsubscribe => 'Disiscriviti';

  @override
  String get folderActionJoinRoom => 'Entra nella stanza';

  @override
  String get folderActionLeaveRoom => 'Lascia la stanza';

  @override
  String get nntpWildmatHint => 'Modello (es. comp.os.linux.*)';

  @override
  String get nntpWildmatQuery => 'Elenca';

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
  String deleteMessagesFailed(String error) {
    return 'Eliminazione non riuscita: $error';
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
