// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Spanish Castilian (`es`).
class AppLocalizationsEs extends AppLocalizations {
  AppLocalizationsEs([String locale = 'es']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Ajustes';

  @override
  String get compose => 'Redactar';

  @override
  String get send => 'Enviar';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'Cancelar';

  @override
  String get save => 'Guardar';

  @override
  String get remove => 'Quitar';

  @override
  String get delete => 'Eliminar';

  @override
  String get discard => 'Descartar';

  @override
  String get back => 'Atrás';

  @override
  String get create => 'Crear';

  @override
  String get rename => 'Renombrar';

  @override
  String get folderLabel => 'Carpeta';

  @override
  String get messageTitle => 'Mensaje';

  @override
  String get selectFolder => 'Seleccione una carpeta';

  @override
  String get selectMessage => 'Seleccione un mensaje';

  @override
  String get selectMessageToRead => 'Seleccione un mensaje para leer.';

  @override
  String get noMessages => 'No hay mensajes';

  @override
  String get attachments => 'Adjuntos';

  @override
  String get saveAttachment => 'Guardar adjunto';

  @override
  String savedToPath(String path) {
    return 'Guardado en $path';
  }

  @override
  String saveFailed(String error) {
    return 'Error al guardar: $error';
  }

  @override
  String get cannotDownloadAttachment => 'No se puede descargar este adjunto';

  @override
  String get emptyAttachmentData => 'Datos del adjunto vacíos';

  @override
  String downloadFailed(String error) {
    return 'Error al descargar: $error';
  }

  @override
  String get saveVerb => 'Guardar';

  @override
  String get loadImages => 'Cargar imágenes';

  @override
  String get remoteImagesBlocked =>
      'Las imágenes remotas están bloqueadas por privacidad.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'No se pudo abrir el cuerpo HTML: $error';
  }

  @override
  String webViewError(String error) {
    return 'Error de WebView: $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'El texto del enlace muestra una dirección distinta a la de destino.';

  @override
  String get headerFrom => 'De:';

  @override
  String get headerTo => 'Para:';

  @override
  String get headerCc => 'Cc:';

  @override
  String get headerDate => 'Fecha:';

  @override
  String get folderInbox => 'Bandeja de entrada';

  @override
  String get messageActionReply => 'Responder';

  @override
  String get messageActionReplyAll => 'Responder a todos';

  @override
  String get messageActionForward => 'Reenviar';

  @override
  String get messageActionDelete => 'Eliminar';

  @override
  String get messageActionJunk => 'Correo no deseado';

  @override
  String get messageActionMove => 'Mover';

  @override
  String get messageActionCopy => 'Copiar';

  @override
  String get messageMenuTooltip => 'Acciones del mensaje';

  @override
  String get settingsViewMinimalHeaders => 'Cabeceras de mensaje mínimas';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'Si está activo, solo se oculta Cc; De, Para y la fecha siguen mostrándose cuando existan.';

  @override
  String get settingsTooltip => 'Ajustes';

  @override
  String get accountsAndFoldersTooltip => 'Cuentas y carpetas';

  @override
  String get cancelSelectionTooltip => 'Cancelar selección';

  @override
  String multiSelectCount(int count) {
    return '$count seleccionado(s)';
  }

  @override
  String get composeTooltip => 'Redactar';

  @override
  String get composeNeedTransportTooltip =>
      'Añada un transporte saliente en Ajustes';

  @override
  String get mailToolbarMoreTooltip => 'Más';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count seleccionado(s)';
  }

  @override
  String get settingsTabAccounts => 'Cuentas';

  @override
  String get settingsTabOutgoing => 'Saliente';

  @override
  String get settingsTabSecurity => 'Seguridad';

  @override
  String get settingsTabViewing => 'Visualización';

  @override
  String get settingsTabComposing => 'Redacción';

  @override
  String get settingsTabAbout => 'Acerca de';

  @override
  String get useSystemKeychain => 'Usar llavero del sistema';

  @override
  String get storeCredentialsInKeychain =>
      'Guardar credenciales en el llavero del sistema';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Autenticar con Google';

  @override
  String get authenticateMicrosoft => 'Autenticar con Microsoft';

  @override
  String get reloadOAuthToken => 'Recargar token OAuth';

  @override
  String get matrixE2eeSection => 'Cifrado extremo a extremo de Matrix';

  @override
  String get initCrypto => 'Inicializar cifrado';

  @override
  String get setupBackup => 'Configurar copia de seguridad';

  @override
  String get restoreBackup => 'Restaurar copia de seguridad';

  @override
  String get showDeviceFingerprint => 'Mostrar huella del dispositivo';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Detalle del mensaje bajo la lista (escritorio)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'Si está desactivado, al abrir un mensaje se usa una vista a pantalla completa separada.';

  @override
  String get loadRemoteImages => 'Cargar imágenes remotas';

  @override
  String get loadRemoteImagesSubtitle =>
      'Permitir imágenes externas en el correo HTML';

  @override
  String get threadedView => 'Vista por conversaciones';

  @override
  String get threadedViewSubtitle => 'Agrupar mensajes por hilo';

  @override
  String get deletionAndTrashSection => 'Eliminación y papelera';

  @override
  String get deletionAppliesGlobally =>
      'Se aplica a todas las cuentas de correo.';

  @override
  String get deleteModeLabel => 'Modo de eliminación';

  @override
  String get trashFolderNameLabel => 'Nombre de la carpeta de papelera';

  @override
  String get deleteModeMoveToTrash => 'Mover a la papelera';

  @override
  String get deleteModeMarkDeleted => 'Marcar como eliminado';

  @override
  String get quoteOriginalOnReply => 'Citar el mensaje original al responder';

  @override
  String get testSend => 'Prueba de envío';

  @override
  String get openSignatureEditor => 'Abrir editor de firma';

  @override
  String get aboutSubtitle => 'Correo y mensajería multiplataforma';

  @override
  String get supportedBackends => 'Backends admitidos';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation (demostración)';
  }

  @override
  String get accountTypeDialogTitle => 'Tipo de cuenta';

  @override
  String get removeAccountTitle => '¿Eliminar cuenta?';

  @override
  String removeAccountBody(String label) {
    return '¿Quitar «$label» de la configuración guardada en este dispositivo?';
  }

  @override
  String removedAccount(String label) {
    return 'Se eliminó «$label»';
  }

  @override
  String get accountsListTitle => 'Cuentas';

  @override
  String get accountsListSubtitle =>
      'Pulse una cuenta para editarla o añada una nueva abajo.';

  @override
  String get deleteTooltip => 'Eliminar';

  @override
  String get addAccount => 'Añadir cuenta';

  @override
  String get noAccountsYet => 'Aún no hay cuentas. Pulse «Añadir cuenta».';

  @override
  String get discardChangesTitle => '¿Descartar cambios?';

  @override
  String get discardChangesBody =>
      'Se perderán los cambios, como si saliera sin guardar.';

  @override
  String get keepEditing => 'Seguir editando';

  @override
  String get pickNotSupportedWeb =>
      'Elegir archivos o carpetas no está soportado en la versión web';

  @override
  String get chooseMaildirFolderTitle => 'Elegir carpeta raíz Maildir';

  @override
  String get chooseMboxFileTitle => 'Elegir archivo mbox';

  @override
  String get validationAccountNameRequired =>
      'El nombre de la cuenta es obligatorio';

  @override
  String get validationLocalPathRequired =>
      'La ruta del buzón local es obligatoria';

  @override
  String get validationUsernameRequired =>
      'Usuario o correo obligatorio para este tipo de cuenta';

  @override
  String get validationHostRequired => 'El host del servidor es obligatorio';

  @override
  String get validationPortRequired => 'Se requiere un número de puerto válido';

  @override
  String get accountSaved => 'Cuenta guardada';

  @override
  String get createTransportFirst =>
      'Cree primero un transporte en la pestaña Saliente';

  @override
  String get addTransportDialogTitle => 'Añadir transporte';

  @override
  String get accountTypeLabel => 'Tipo de cuenta';

  @override
  String get accountTypeHelper =>
      'El tipo no se puede cambiar al editar una cuenta existente';

  @override
  String get accountNameLabel => 'Nombre de la cuenta';

  @override
  String get usernameEmailOptional => 'Usuario / correo (opcional)';

  @override
  String get usernameEmailRequired => 'Usuario / correo';

  @override
  String get avatarUrlLabel => 'URL del avatar o ruta de archivo (opcional)';

  @override
  String get avatarUrlHelper =>
      'Imagen o ruta local opcional para la barra de cuentas';

  @override
  String get localMailboxSection => 'Buzón local';

  @override
  String get pathMboxFile => 'Ruta al archivo mbox';

  @override
  String get pathMaildirRoot => 'Ruta raíz Maildir';

  @override
  String get helperMboxPath =>
      'Use el botón de archivo para examinar, o escriba una ruta absoluta';

  @override
  String get helperMaildirPath =>
      'Use el botón de carpeta para examinar, o escriba una ruta absoluta';

  @override
  String get chooseMboxTooltip => 'Elegir archivo mbox';

  @override
  String get chooseMaildirTooltip => 'Elegir carpeta Maildir';

  @override
  String get imapServerSection => 'Servidor IMAP';

  @override
  String get pop3ServerSection => 'Servidor POP3';

  @override
  String get nntpServerSection => 'Servidor NNTP';

  @override
  String get hostLabel => 'Host';

  @override
  String get serverHostLabel => 'Host del servidor';

  @override
  String get portLabel => 'Puerto';

  @override
  String get portHelperImap => 'Suele ser 993 (IMAPS) o 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'Suele ser 995 (POP3S, TLS implícito)';

  @override
  String get portHelperNntp => 'Suele ser 563 (NNTPS, TLS implícito)';

  @override
  String get securityLabel => 'Seguridad';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (TLS implícito)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (TLS implícito)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (TLS implícito)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'Sin cifrado';

  @override
  String get outgoingTransportsSection => 'Transportes salientes';

  @override
  String get noTransportsHintLinked =>
      'Ningún transporte seleccionado: redactar y responder siguen desactivados hasta que elija al menos uno. Use la pestaña Saliente y selecciónelo aquí.';

  @override
  String get transportsOrderHint =>
      'El primero de la lista es el predeterminado para enviar. Cree transportes en Saliente.';

  @override
  String get unknownTransport => 'Transporte desconocido';

  @override
  String get moveUpTooltip => 'Subir';

  @override
  String get moveDownTooltip => 'Bajar';

  @override
  String get removeFromAccountTooltip => 'Quitar de la cuenta';

  @override
  String get addTransportToAccount => 'Añadir transporte a la cuenta';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Servidor de origen';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'URL de relés';

  @override
  String get relayUrlsHelper =>
      'Cada fila es una URL WebSocket de relé. Pulse Intro al terminar de editar una URL.';

  @override
  String get relayAddFieldHint => 'Nueva URL de relé';

  @override
  String get relayAddTooltip => 'Añadir relé';

  @override
  String get relayRemoveTooltip => 'Quitar relé';

  @override
  String get nostrNewIdentityTooltip => 'Crear identidad Nostr nueva';

  @override
  String get nostrRelayUrlsRequired => 'Introduzca al menos una URL de relé.';

  @override
  String storeUriLabel(String uri) {
    return 'URI del almacén: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'URI del transporte: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'Nuevo $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return 'Editar $label';
  }

  @override
  String foldersLoadError(String error) {
    return 'Carpetas: $error';
  }

  @override
  String get sortMessagesTooltip => 'Ordenar mensajes';

  @override
  String get sort => 'Ordenar';

  @override
  String get sortFromAz => 'De la A a la Z';

  @override
  String get sortFromZa => 'De la Z a la A';

  @override
  String get sortSubjectAz => 'Asunto A–Z';

  @override
  String get sortSubjectZa => 'Asunto Z–A';

  @override
  String get sortDateOldest => 'Fecha: más antiguos primero';

  @override
  String get sortDateNewest => 'Fecha: más recientes primero';

  @override
  String get removeTransportTitle => '¿Eliminar transporte?';

  @override
  String removeTransportBody(String name) {
    return '«$name» se quitará de las listas salientes de todas las cuentas.';
  }

  @override
  String removedTransport(String name) {
    return 'Se eliminó «$name»';
  }

  @override
  String get outgoingListTitle => 'Saliente';

  @override
  String get outgoingListSubtitle =>
      'SMTP y otros transportes de envío. Enlácelos con las cuentas en la pestaña Cuentas.';

  @override
  String get addTransport => 'Añadir transporte';

  @override
  String get noTransportsYet =>
      'Aún no hay transportes salientes. Pulse «Añadir transporte».';

  @override
  String get transportDisplayHostRequired =>
      'El nombre para mostrar y el host son obligatorios.';

  @override
  String get transportSaved => 'Transporte guardado';

  @override
  String get newTransport => 'Nuevo transporte';

  @override
  String get editTransport => 'Editar transporte';

  @override
  String get displayNameLabel => 'Nombre para mostrar';

  @override
  String get smtpHostLabel => 'Host SMTP';

  @override
  String get smtpPortHelper => 'Suele ser 587 (STARTTLS) o 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'Inicio de sesión IMAP';

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
  String get enterUsernameAndPassword => 'Introduzca usuario y contraseña.';

  @override
  String get usernameLabel => 'Usuario';

  @override
  String get passwordLabel => 'Contraseña';

  @override
  String get showPasswordTooltip => 'Mostrar contraseña';

  @override
  String get hidePasswordTooltip => 'Ocultar contraseña';

  @override
  String get fieldFrom => 'De';

  @override
  String get composeOutgoingTransport => 'Transporte saliente';

  @override
  String get composeSendSucceeded => 'Mensaje enviado';

  @override
  String get composeMissingFrom => 'Introduzca una dirección De.';

  @override
  String get composeMissingTo => 'Introduzca al menos un destinatario.';

  @override
  String get fieldTo => 'Para';

  @override
  String get fieldCc => 'Cc';

  @override
  String get fieldBcc => 'Cco';

  @override
  String get fieldSubject => 'Asunto';

  @override
  String get fieldBody => 'Cuerpo';

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
  String get folderNewSubfolder => 'Nueva subcarpeta';

  @override
  String get folderRename => 'Renombrar…';

  @override
  String get folderDelete => 'Eliminar…';

  @override
  String get folderNewTooltip => 'Nueva carpeta';

  @override
  String get folderNewDialogTitle => 'Nueva carpeta';

  @override
  String get folderNameLabel => 'Nombre de la carpeta';

  @override
  String get folderNewTopLevelHelper => 'Crea un buzón en el nivel superior';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Subcarpeta de $parent';
  }

  @override
  String get subfolderNameLabel => 'Nombre de la subcarpeta';

  @override
  String subfolderPathHelper(String path) {
    return 'Ruta: $path';
  }

  @override
  String folderCreated(String name) {
    return 'Carpeta «$name» creada';
  }

  @override
  String get renameFolderTitle => 'Renombrar carpeta';

  @override
  String get newFolderPathLabel => 'Nueva ruta de carpeta';

  @override
  String get folderRenamed => 'Carpeta renombrada';

  @override
  String get deleteFolderTitle => '¿Eliminar carpeta?';

  @override
  String deleteFolderBody(String name) {
    return '¿Eliminar «$name» y sus mensajes del servidor (si está soportado)? No se puede deshacer.';
  }

  @override
  String get folderDeleted => 'Carpeta eliminada';

  @override
  String get licenseTitle => 'Licencia';

  @override
  String get copyrightTitle => 'Copyright';

  @override
  String get chatHintTypeMessage => 'Escribe un mensaje';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.';

  @override
  String operationFailed(String error) {
    return 'Algo salió mal: $error';
  }

  @override
  String get expandFolder => 'Expandir';

  @override
  String get collapseFolder => 'Contraer';

  @override
  String get noTextBody => '(Sin cuerpo de texto)';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Mover aquí';

  @override
  String get folderCopyHere => 'Copiar aquí';

  @override
  String get folderExpunge => 'Purgar mensajes eliminados';

  @override
  String get folderExpungeDone => 'Purgado completado';

  @override
  String pendingMoveTagged(int count) {
    return 'Elija una carpeta y luego Mover aquí ($count mensajes)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Elija una carpeta y luego Copiar aquí ($count mensajes)';
  }

  @override
  String transferResultOk(int count) {
    return 'Hecho: $count mensaje(s).';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok correctos, $failed fallidos.';
  }

  @override
  String transferFailed(String error) {
    return 'Error en la transferencia: $error';
  }

  @override
  String get settingsNotifyNewMessages => 'Notificaciones de mensajes nuevos';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Barra emergente con la app abierta; notificación del sistema en segundo plano (IMAP).';

  @override
  String get newMailNotificationTitle => 'Correo nuevo';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count mensaje(s) nuevo(s) en $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel =>
      'Segundos en silencio antes de IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Vacío = predeterminado (120). Mínimo 15. Tras inactividad de la conexión.';

  @override
  String get validationImapMinIdleSeconds =>
      'Número entero entre 15 y 864000, o vacío.';
}
