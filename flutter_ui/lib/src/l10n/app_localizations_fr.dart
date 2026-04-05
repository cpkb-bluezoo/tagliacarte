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
  String get deleteModeMoveToTrash => 'Déplacer vers la corbeille';

  @override
  String get deleteModeMarkDeleted => 'Marquer comme supprimé';

  @override
  String get quoteOriginalOnReply =>
      'Citer le message d’origine dans la réponse';

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
      'Le type ne peut pas être modifié lors de l’édition d’un compte existant';

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
    return 'URI du magasin : $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'URI du transport : $uri';
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
