// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class AppLocalizationsFr extends AppLocalizations {
  AppLocalizationsFr([String locale = 'fr']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Réglages';

  @override
  String get compose => 'Rédiger';

  @override
  String get send => 'Envoyer';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'Annuler';

  @override
  String get save => 'Enregistrer';

  @override
  String get remove => 'Retirer';

  @override
  String get delete => 'Supprimer';

  @override
  String get discard => 'Abandonner';

  @override
  String get back => 'Retour';

  @override
  String get create => 'Créer';

  @override
  String get rename => 'Renommer';

  @override
  String get folderLabel => 'Dossier';

  @override
  String get messageTitle => 'Message';

  @override
  String get selectFolder => 'Sélectionnez un dossier';

  @override
  String get selectMessage => 'Sélectionnez un message';

  @override
  String get selectMessageToRead => 'Sélectionnez un message à lire.';

  @override
  String get noMessages => 'Aucun message';

  @override
  String get attachments => 'Pièces jointes';

  @override
  String get saveAttachment => 'Enregistrer la pièce jointe';

  @override
  String savedToPath(String path) {
    return 'Enregistré sous $path';
  }

  @override
  String saveFailed(String error) {
    return 'Échec de l’enregistrement : $error';
  }

  @override
  String get cannotDownloadAttachment =>
      'Impossible de télécharger cette pièce jointe';

  @override
  String get emptyAttachmentData => 'Données de pièce jointe vides';

  @override
  String downloadFailed(String error) {
    return 'Échec du téléchargement : $error';
  }

  @override
  String get saveVerb => 'Enregistrer';

  @override
  String get loadImages => 'Charger les images';

  @override
  String get remoteImagesBlocked =>
      'Images distantes bloquées pour protéger votre vie privée.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'Impossible d’ouvrir le corps HTML : $error';
  }

  @override
  String webViewError(String error) {
    return 'Erreur WebView : $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'Le texte visible du lien affiche une adresse différente de la destination réelle.';

  @override
  String get headerFrom => 'De :';

  @override
  String get headerTo => 'À :';

  @override
  String get headerCc => 'Cc :';

  @override
  String get headerDate => 'Date :';

  @override
  String get folderInbox => 'Boîte de réception';

  @override
  String get messageActionReply => 'Répondre';

  @override
  String get messageActionReplyAll => 'Répondre à tous';

  @override
  String get messageActionForward => 'Transférer';

  @override
  String get messageActionDelete => 'Supprimer';

  @override
  String get messageActionJunk => 'Indésirable';

  @override
  String get messageActionMove => 'Déplacer';

  @override
  String get messageActionCopy => 'Copier';

  @override
  String get messageMenuTooltip => 'Actions sur le message';

  @override
  String get settingsViewMinimalHeaders => 'En-têtes de message réduits';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'Si activé, seul le champ Cc est masqué ; De, À et la date restent affichés lorsqu’ils sont disponibles.';

  @override
  String get settingsTooltip => 'Réglages';

  @override
  String get accountsAndFoldersTooltip => 'Comptes et dossiers';

  @override
  String get cancelSelectionTooltip => 'Annuler la sélection';

  @override
  String multiSelectCount(int count) {
    return '$count sélectionné(s)';
  }

  @override
  String get composeTooltip => 'Rédiger';

  @override
  String get composeNeedTransportTooltip =>
      'Ajoutez un transport sortant dans les réglages';

  @override
  String get mailToolbarMoreTooltip => 'Plus';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count sélectionné(s)';
  }

  @override
  String get settingsTabAccounts => 'Comptes';

  @override
  String get settingsTabOutgoing => 'Envoi';

  @override
  String get settingsTabSecurity => 'Sécurité';

  @override
  String get settingsTabViewing => 'Affichage';

  @override
  String get settingsTabComposing => 'Rédaction';

  @override
  String get settingsTabAbout => 'À propos';

  @override
  String get settingsLoadFailed =>
      'Impossible de charger les paramètres depuis le disque.';

  @override
  String get settingsLoadRetry => 'Réessayer';

  @override
  String get useSystemKeychain => 'Utiliser le trousseau système';

  @override
  String get storeCredentialsInKeychain =>
      'Enregistrer les identifiants dans le trousseau du système';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'S’authentifier avec Google';

  @override
  String get authenticateMicrosoft => 'S’authentifier avec Microsoft';

  @override
  String get reloadOAuthToken => 'Recharger le jeton OAuth';

  @override
  String get matrixE2eeSection => 'Chiffrement de bout en bout Matrix';

  @override
  String get initCrypto => 'Initialiser le chiffrement';

  @override
  String get setupBackup => 'Configurer la sauvegarde';

  @override
  String get restoreBackup => 'Restaurer la sauvegarde';

  @override
  String get showDeviceFingerprint => 'Afficher l’empreinte de l’appareil';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Détail du message sous la liste (bureau)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'Si désactivé, l’ouverture d’un message utilise un écran dédié plein écran.';

  @override
  String get loadRemoteImages => 'Charger les images distantes';

  @override
  String get loadRemoteImagesSubtitle =>
      'Autoriser les images externes dans les e-mails HTML';

  @override
  String get threadedView => 'Vue par conversation';

  @override
  String get threadedViewSubtitle =>
      'Regrouper les messages par fil de discussion';

  @override
  String get deletionAndTrashSection => 'Suppression et corbeille';

  @override
  String get deletionAppliesGlobally =>
      'S’applique à tous les comptes de messagerie.';

  @override
  String get deleteModeLabel => 'Mode de suppression';

  @override
  String get trashFolderNameLabel => 'Nom du dossier corbeille';

  @override
  String get junkFolderNameLabel => 'Nom du dossier courrier indésirable';

  @override
  String get exchangeTrashFolderHelper =>
      'Laisser vide pour utiliser « Éléments supprimés » (boîte en anglais). Utilisez le nom exact affiché dans Outlook s’il diffère.';

  @override
  String get exchangeJunkFolderHelper =>
      'Laisser vide pour utiliser « Courrier indésirable » (boîte en anglais). Utilisez le nom exact affiché dans Outlook s’il diffère.';

  @override
  String get deleteModeDeleteImmediately => 'Supprimer immédiatement';

  @override
  String get deleteModeMoveToTrash => 'Déplacer vers la corbeille';

  @override
  String get deleteModeMarkDeleted => 'Marquer comme supprimé';

  @override
  String get quoteOriginalOnReply =>
      'Citer le message d’origine dans la réponse';

  @override
  String get quoteOriginalOnReplySubtitle =>
      'Insère l’original sous l’en-tête de réponse dans les nouvelles réponses. La composition enrichie l’encadre dans un bloc de citation ; le texte brut préfixe chaque ligne. La partie text/plain inclut toujours l’original si l’option est activée.';

  @override
  String get composingReplySection => 'Citations dans les réponses';

  @override
  String get replyHeaderTemplateLabel => 'Ligne d’en-tête de réponse';

  @override
  String get replyHeaderTemplateHelp =>
      'Affiché au-dessus de l’original cité. Incluez les mots date, time et sender, chacun précédé d’un dollar (voir l’aperçu). Ils sont remplacés par la date, l’heure et l’expéditeur lors de la réponse.';

  @override
  String get replyHeaderPreviewLabel => 'Aperçu';

  @override
  String get replyDateFormatLabel => 'Date (dans l’en-tête)';

  @override
  String get replyTimeFormatLabel => 'Heure (dans l’en-tête)';

  @override
  String get replyDatePresetLocale => 'Comme le système (date longue)';

  @override
  String get replyDatePresetIso => 'ISO : 2026-04-08';

  @override
  String get replyDatePresetUs => 'États-Unis : 04/08/2026';

  @override
  String get replyDatePresetEu => 'Jour/mois/année : 08/04/2026';

  @override
  String get replyDatePresetMedium => 'Moyen : 8 avr. 2026';

  @override
  String get replyDatePresetWeekday =>
      'Avec jour de la semaine : mer. 8 avr. 2026';

  @override
  String replyDatePresetCustom(String pattern) {
    return 'Personnalisé ($pattern)';
  }

  @override
  String get replyTimePresetLocale => 'Comme le système';

  @override
  String get replyTimePreset12h => '12 h (p. ex. 13:30)';

  @override
  String get replyTimePreset24h => '24 h (15:30)';

  @override
  String get replyTimePreset24hSeconds => '24 h avec secondes';

  @override
  String replyTimePresetCustom(String pattern) {
    return 'Personnalisé ($pattern)';
  }

  @override
  String get replyLinePrefixLabel => 'Préfixe des lignes citées';

  @override
  String get replyLinePrefixSubtitle =>
      'Ajouté avant chaque ligne de l’original dans les citations en texte brut (classique « > »). Uniquement si la citation est activée.';

  @override
  String get replyPlainPositionLabel => 'Ordre réponse et citation';

  @override
  String get replyPlainPositionBefore => 'Réponse avant le texte cité';

  @override
  String get replyPlainPositionAfter => 'Réponse après le texte cité';

  @override
  String get replyPlainPositionSubtitle =>
      'Texte brut ou enrichi : deux lignes vides et curseur avant l’en-tête de réponse, ou deux lignes vides et curseur après le bloc cité. La partie text/plain suit la même disposition à l’envoi.';

  @override
  String get replyQuoteModeLabel => 'Parties HTML SMTP';

  @override
  String get replyQuoteModePlain =>
      'Original uniquement en citation texte brut';

  @override
  String get replyQuoteModeHtmlSmtp =>
      'Inclure aussi l’original en HTML séparé (SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'Ajoute une seconde partie HTML conservant la mise en forme du message source. Les clients texte seul voient encore le corps cité en clair. Les envois NNTP utilisent toujours des citations en texte brut.';

  @override
  String get settingsComposeRichText =>
      'Texte riche dans la rédaction d\'e-mails';

  @override
  String get settingsComposeRichTextSubtitle =>
      'Éditeur avec mise en forme pour les nouveaux e-mails et les réponses. Usenet (NNTP) reste en texte brut.';

  @override
  String get settingsMatrixChatRichText =>
      'Texte riche dans les discussions Matrix';

  @override
  String get settingsMatrixChatRichTextSubtitle =>
      'Envoyer des messages mis en forme dans les salons Matrix (avec repli en texte brut).';

  @override
  String get testSend => 'Test d’envoi';

  @override
  String get openSignatureEditor => 'Ouvrir l’éditeur de signature';

  @override
  String get aboutSubtitle => 'Messagerie multiplateforme';

  @override
  String get supportedBackends => 'Moteurs pris en charge';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation (action de démonstration)';
  }

  @override
  String get accountTypeDialogTitle => 'Type de compte';

  @override
  String get removeAccountTitle => 'Supprimer le compte ?';

  @override
  String removeAccountBody(String label) {
    return 'Retirer « $label » de la configuration enregistrée sur cet appareil ?';
  }

  @override
  String removedAccount(String label) {
    return 'Compte « $label » supprimé';
  }

  @override
  String get accountsListTitle => 'Comptes';

  @override
  String get accountsListSubtitle =>
      'Appuyez sur un compte pour le modifier, ou ajoutez-en un ci-dessous.';

  @override
  String get deleteTooltip => 'Supprimer';

  @override
  String get addAccount => 'Ajouter un compte';

  @override
  String get noAccountsYet =>
      'Aucun compte pour l’instant. Appuyez sur « Ajouter un compte ».';

  @override
  String get discardChangesTitle => 'Abandonner les modifications ?';

  @override
  String get discardChangesBody =>
      'Vos modifications seront perdues, comme en quittant sans enregistrer.';

  @override
  String get keepEditing => 'Continuer l’édition';

  @override
  String get pickNotSupportedWeb =>
      'Le choix de fichiers ou de dossiers n’est pas pris en charge dans la version web';

  @override
  String get chooseMaildirFolderTitle => 'Choisir le dossier racine Maildir';

  @override
  String get chooseMboxFileTitle => 'Choisir le fichier mbox';

  @override
  String get validationAccountNameRequired =>
      'Le nom du compte est obligatoire';

  @override
  String get validationLocalPathRequired =>
      'Le chemin de la boîte locale est obligatoire';

  @override
  String get validationUsernameRequired =>
      'Nom d’utilisateur ou adresse e-mail obligatoire pour ce type de compte';

  @override
  String get validationEmailAddressRequired =>
      'L’adresse e-mail est obligatoire';

  @override
  String get validationMatrixUserIdRequired =>
      'L’identifiant Matrix est obligatoire';

  @override
  String get accountEmailAddressLabel => 'Adresse e-mail';

  @override
  String get accountMatrixUserIdLabel => 'Identifiant Matrix (MXID)';

  @override
  String get accountMatrixMxidHelper =>
      'Exemple : @you:matrix.org — l’URL du serveur d’accueil est dérivée du domaine après les deux-points.';

  @override
  String get validationMatrixMxidInvalid =>
      'Saisissez un identifiant Matrix du type @user:serveur';

  @override
  String get accountNntpDefaultFromLabel => 'Expéditeur par défaut (Usenet)';

  @override
  String get accountNntpDefaultFromHelper =>
      'Affiché lors de la rédaction ; ce compte NNTP publie via sa propre connexion serveur.';

  @override
  String get accountEmailOptionalLabel => 'Adresse e-mail (facultatif)';

  @override
  String get accountTcpLoginHelper =>
      'Identité de connexion pour ce serveur (souvent votre adresse e-mail).';

  @override
  String get validationHostRequired =>
      'Le nom d’hôte du serveur est obligatoire';

  @override
  String get validationPortRequired =>
      'Un numéro de port valide est obligatoire';

  @override
  String get accountSaved => 'Compte enregistré';

  @override
  String get createTransportFirst =>
      'Créez d’abord un transport dans l’onglet Envoi';

  @override
  String get addTransportDialogTitle => 'Ajouter un transport';

  @override
  String get accountTypeLabel => 'Type de compte';

  @override
  String get accountTypeHelper =>
      'Choisi à l’ajout du compte ; ne peut pas être modifié ici.';

  @override
  String get accountNameLabel => 'Nom du compte';

  @override
  String get usernameEmailOptional => 'Nom d’utilisateur / e-mail (facultatif)';

  @override
  String get usernameEmailRequired => 'Nom d’utilisateur / e-mail';

  @override
  String get avatarUrlLabel => 'URL d’avatar ou chemin de fichier (facultatif)';

  @override
  String get avatarUrlHelper =>
      'Image ou fichier local facultatif pour la bande de comptes';

  @override
  String get localMailboxSection => 'Boîte locale';

  @override
  String get pathMboxFile => 'Chemin du fichier mbox';

  @override
  String get pathMaildirRoot => 'Chemin racine Maildir';

  @override
  String get helperMboxPath =>
      'Utilisez le bouton fichier pour parcourir, ou saisissez un chemin absolu';

  @override
  String get helperMaildirPath =>
      'Utilisez le bouton dossier pour parcourir, ou saisissez un chemin absolu';

  @override
  String get chooseMboxTooltip => 'Choisir le fichier mbox';

  @override
  String get chooseMaildirTooltip => 'Choisir le dossier Maildir';

  @override
  String get imapServerSection => 'Serveur IMAP';

  @override
  String get pop3ServerSection => 'Serveur POP3';

  @override
  String get nntpServerSection => 'Serveur NNTP';

  @override
  String get hostLabel => 'Hôte';

  @override
  String get serverHostLabel => 'Hôte du serveur';

  @override
  String get portLabel => 'Port';

  @override
  String get portHelperImap => 'En général 993 (IMAPS) ou 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'En général 995 (POP3S, TLS implicite)';

  @override
  String get portHelperNntp => 'En général 563 (NNTPS, TLS implicite)';

  @override
  String get securityLabel => 'Sécurité';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (TLS implicite)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (TLS implicite)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (TLS implicite)';

  @override
  String get mailSecurityImplicitTlsNntp => 'NNTPS (TLS implicite)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'Pas de chiffrement';

  @override
  String get outgoingTransportsSection => 'Transports sortants';

  @override
  String get noTransportsHintLinked =>
      'Aucun transport sélectionné : la rédaction et les réponses restent désactivées tant que vous n’en avez pas choisi au moins un. Utilisez l’onglet Envoi, puis sélectionnez-les ici.';

  @override
  String get transportsOrderHint =>
      'Le premier de la liste est utilisé par défaut pour l’envoi. Créez les transports dans l’onglet Envoi.';

  @override
  String get unknownTransport => 'Transport inconnu';

  @override
  String get moveUpTooltip => 'Monter';

  @override
  String get moveDownTooltip => 'Descendre';

  @override
  String get removeFromAccountTooltip => 'Retirer du compte';

  @override
  String get addTransportToAccount => 'Ajouter un transport au compte';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Serveur d’accueil (homeserver)';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'URL des relais';

  @override
  String get relayUrlsHelper =>
      'Chaque ligne est l’URL WebSocket d’un relais. Appuyez sur Entrée quand vous avez fini de modifier une URL.';

  @override
  String get relayAddFieldHint => 'Nouvelle URL de relais';

  @override
  String get relayAddTooltip => 'Ajouter un relais';

  @override
  String get relayRemoveTooltip => 'Retirer le relais';

  @override
  String get nostrNewIdentityTooltip => 'Créer une identité Nostr';

  @override
  String get nostrRelayUrlsRequired => 'Saisissez au moins une URL de relais.';

  @override
  String storeUriLabel(String uri) {
    return 'Connexion : $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'Ancienne URI de transport : $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'Nouveau $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return 'Modifier $label';
  }

  @override
  String foldersLoadError(String error) {
    return 'Dossiers : $error';
  }

  @override
  String get sortMessagesTooltip => 'Trier les messages';

  @override
  String get sort => 'Trier';

  @override
  String get sortFromAz => 'Expéditeur A → Z';

  @override
  String get sortFromZa => 'Expéditeur Z → A';

  @override
  String get sortSubjectAz => 'Objet A → Z';

  @override
  String get sortSubjectZa => 'Objet Z → A';

  @override
  String get sortDateOldest => 'Date : plus anciens d’abord';

  @override
  String get sortDateNewest => 'Date : plus récents d’abord';

  @override
  String get removeTransportTitle => 'Supprimer le transport ?';

  @override
  String removeTransportBody(String name) {
    return '« $name » sera retiré des listes d’envoi de tous les comptes.';
  }

  @override
  String removedTransport(String name) {
    return 'Transport « $name » supprimé';
  }

  @override
  String get outgoingListTitle => 'Envoi';

  @override
  String get outgoingListSubtitle =>
      'SMTP et autres transports d’envoi. Associez-les aux comptes dans l’onglet Comptes.';

  @override
  String get addTransport => 'Ajouter un transport';

  @override
  String get noTransportsYet =>
      'Aucun transport sortant. Appuyez sur « Ajouter un transport ».';

  @override
  String get transportDisplayHostRequired =>
      'Le nom d’affichage et l’hôte sont obligatoires.';

  @override
  String get transportSaved => 'Transport enregistré';

  @override
  String get transportSavedAndVerified =>
      'Transport enregistré et SMTP vérifié';

  @override
  String get transportSavedVerifyPending =>
      'Transport enregistré, mais le serveur est injoignable ou l’authentification a échoué. Vérifiez l’hôte, la sécurité et les identifiants, puis enregistrez à nouveau.';

  @override
  String get transportTypeDialogTitle => 'Type de transport sortant';

  @override
  String get transportTypeFixedHelper =>
      'Choisi à l’ajout ; ne peut pas être modifié ici.';

  @override
  String get transportDisplayNameRequired =>
      'Le nom d’affichage est obligatoire.';

  @override
  String get transportKindLabel => 'Type d’envoi';

  @override
  String get transportKindSmtp => 'SMTP';

  @override
  String get transportKindGmail => 'Gmail (Google)';

  @override
  String get gmailTransportPresetHelper =>
      'Utilise smtp.gmail.com avec OAuth (XOAUTH2). Enregistrez le transport, puis connectez-vous avec le même compte Google que pour l’IMAP Gmail.';

  @override
  String get newTransport => 'Nouveau transport';

  @override
  String get editTransport => 'Modifier le transport';

  @override
  String get displayNameLabel => 'Nom d’affichage';

  @override
  String get smtpHostLabel => 'Hôte SMTP';

  @override
  String get smtpPortHelper => 'En général 587 (STARTTLS) ou 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'Connexion IMAP';

  @override
  String get matrixSignInTitle => 'Connexion Matrix';

  @override
  String get gmailSignInTitle => 'Se connecter avec Google';

  @override
  String get gmailSignInBody =>
      'Le navigateur s’ouvrira pour autoriser Gmail (IMAP). L’application nécessite TAGLIACARTE_GOOGLE_CLIENT_ID (et en général TAGLIACARTE_GOOGLE_CLIENT_SECRET).';

  @override
  String get gmailSignInBrowserButton => 'Continuer dans le navigateur';

  @override
  String get smtpSignInTitle => 'Connexion SMTP';

  @override
  String smtpSignInSubtitle(String transportName, String host) {
    return 'Saisissez le nom d’utilisateur et le mot de passe pour « $transportName » ($host).';
  }

  @override
  String get composeSendCancelledNoSmtpCredentials =>
      'Message non envoyé : les identifiants SMTP n’ont pas été enregistrés.';

  @override
  String get enterUsernameAndPassword =>
      'Saisissez le nom d’utilisateur et le mot de passe.';

  @override
  String get usernameLabel => 'Nom d’utilisateur';

  @override
  String get passwordLabel => 'Mot de passe';

  @override
  String get showPasswordTooltip => 'Afficher le mot de passe';

  @override
  String get hidePasswordTooltip => 'Masquer le mot de passe';

  @override
  String get fieldFrom => 'De';

  @override
  String get composeOutgoingTransport => 'Transport sortant';

  @override
  String get composeSendSucceeded => 'Message envoyé';

  @override
  String get composeMissingFrom => 'Saisissez une adresse d\'expéditeur.';

  @override
  String get composeMissingTo => 'Saisissez au moins un destinataire.';

  @override
  String get composeMissingNewsgroups =>
      'Saisissez au moins un nom de groupe de nouvelles.';

  @override
  String get composeNntpPostingBlurb =>
      'Les messages sont envoyés via le serveur NNTP de ce compte (pas de transport séparé).';

  @override
  String get fieldNewsgroups => 'Groupes de nouvelles';

  @override
  String get fieldTo => 'À';

  @override
  String get fieldCc => 'Cc';

  @override
  String get fieldBcc => 'Cci';

  @override
  String get fieldSubject => 'Objet';

  @override
  String get fieldBody => 'Corps';

  @override
  String get attach => 'Joindre';

  @override
  String get composeRemoveAttachment => 'Retirer la pièce jointe';

  @override
  String get defaultFromLabel => 'Adresse d’expéditeur par défaut';

  @override
  String get defaultFromHelper =>
      'p. ex. Votre nom <you@example.com> ou you@example.com';

  @override
  String get dsnLabel => 'Notifications de remise';

  @override
  String get dsnUseTransportDefault => 'Valeur par défaut du transport';

  @override
  String get dsnNever => 'Jamais';

  @override
  String get dsnFailure => 'En cas d’échec';

  @override
  String get dsnSuccess => 'En cas de succès';

  @override
  String get dsnDelay => 'En cas de retard';

  @override
  String get dsnFailureAndSuccess => 'En cas d’échec et de succès';

  @override
  String get dsnNotifyLabel => 'Notification DSN';

  @override
  String get folderNewSubfolder => 'Nouveau sous-dossier';

  @override
  String get folderRename => 'Renommer…';

  @override
  String get folderDelete => 'Supprimer…';

  @override
  String get folderNewTooltip => 'Nouveau dossier';

  @override
  String get folderNewDialogTitle => 'Nouveau dossier';

  @override
  String get folderNameLabel => 'Nom du dossier';

  @override
  String get folderNewTopLevelHelper => 'Crée une boîte à la racine';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Sous-dossier de $parent';
  }

  @override
  String get subfolderNameLabel => 'Nom du sous-dossier';

  @override
  String subfolderPathHelper(String path) {
    return 'Chemin : $path';
  }

  @override
  String folderCreated(String name) {
    return 'Dossier « $name » créé';
  }

  @override
  String get renameFolderTitle => 'Renommer le dossier';

  @override
  String get newFolderPathLabel => 'Nouveau chemin du dossier';

  @override
  String get folderRenamed => 'Dossier renommé';

  @override
  String get deleteFolderTitle => 'Supprimer le dossier ?';

  @override
  String deleteFolderBody(String name) {
    return 'Supprimer « $name » et ses messages sur le serveur (si pris en charge) ? Cette action est irréversible.';
  }

  @override
  String get folderDeleted => 'Dossier supprimé';

  @override
  String get licenseTitle => 'Licence';

  @override
  String get copyrightTitle => 'Droits d’auteur';

  @override
  String get chatHintTypeMessage => 'Saisissez un message';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Le chat ne peut pas encore envoyer de pièces jointes. Retirez-les pour envoyer le message, ou utilisez la rédaction d’e-mail pour les fichiers.';

  @override
  String operationFailed(String error) {
    return 'Un problème est survenu : $error';
  }

  @override
  String get expandFolder => 'Développer';

  @override
  String get collapseFolder => 'Replier';

  @override
  String get noTextBody => '(Aucun corps en texte brut)';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Déplacer ici';

  @override
  String get folderCopyHere => 'Copier ici';

  @override
  String get folderExpunge => 'Purger les messages supprimés';

  @override
  String get folderExpungeDone => 'Purge terminée';

  @override
  String get folderTabSubscribed => 'Abonnés';

  @override
  String get folderTabAvailable => 'Disponibles';

  @override
  String get folderActionSubscribe => 'S’abonner';

  @override
  String get folderActionUnsubscribe => 'Se désabonner';

  @override
  String get folderActionJoinRoom => 'Rejoindre le salon';

  @override
  String get folderActionLeaveRoom => 'Quitter le salon';

  @override
  String get nntpWildmatHint => 'Motif (p. ex. comp.os.linux.*)';

  @override
  String get nntpWildmatQuery => 'Lister';

  @override
  String pendingMoveTagged(int count) {
    return 'Choisissez un dossier, puis Déplacer ici ($count messages)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Choisissez un dossier, puis Copier ici ($count messages)';
  }

  @override
  String transferResultOk(int count) {
    return 'Terminé : $count message(s).';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok réussis, $failed échecs.';
  }

  @override
  String transferFailed(String error) {
    return 'Échec du transfert : $error';
  }

  @override
  String deleteMessagesFailed(String error) {
    return 'Échec de la suppression : $error';
  }

  @override
  String get settingsNotifyNewMessages => 'Notifications de nouveaux messages';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Snackbar lorsque l’app est ouverte ; notification système en arrière-plan (IMAP).';

  @override
  String get newMailNotificationTitle => 'Nouveau courrier';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count nouveau(x) message(s) dans $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel =>
      'Secondes d’inactivité avant IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Vide = défaut (120). Minimum 15. Après inactivité de la connexion.';

  @override
  String get validationImapMinIdleSeconds =>
      'Entier entre 15 et 864000, ou vide.';
}
