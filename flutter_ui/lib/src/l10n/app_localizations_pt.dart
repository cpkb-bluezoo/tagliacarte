// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Portuguese (`pt`).
class AppLocalizationsPt extends AppLocalizations {
  AppLocalizationsPt([String locale = 'pt']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Definições';

  @override
  String get compose => 'Redigir';

  @override
  String get send => 'Enviar';

  @override
  String get cancel => 'Cancelar';

  @override
  String get save => 'Guardar';

  @override
  String get remove => 'Remover';

  @override
  String get delete => 'Eliminar';

  @override
  String get discard => 'Descartar';

  @override
  String get back => 'Voltar';

  @override
  String get create => 'Criar';

  @override
  String get rename => 'Mudar o nome';

  @override
  String get folderLabel => 'Pasta';

  @override
  String get messageTitle => 'Mensagem';

  @override
  String get selectFolder => 'Selecione uma pasta';

  @override
  String get selectMessage => 'Selecione uma mensagem';

  @override
  String get selectMessageToRead => 'Selecione uma mensagem para ler.';

  @override
  String get noMessages => 'Sem mensagens';

  @override
  String get attachments => 'Anexos';

  @override
  String get saveAttachment => 'Guardar anexo';

  @override
  String savedToPath(String path) {
    return 'Guardado em $path';
  }

  @override
  String saveFailed(String error) {
    return 'Falha ao guardar: $error';
  }

  @override
  String get cannotDownloadAttachment => 'Não é possível transferir este anexo';

  @override
  String get emptyAttachmentData => 'Dados do anexo vazios';

  @override
  String downloadFailed(String error) {
    return 'Falha na transferência: $error';
  }

  @override
  String get saveVerb => 'Guardar';

  @override
  String get loadImages => 'Carregar imagens';

  @override
  String get remoteImagesBlocked =>
      'Imagens remotas bloqueadas por privacidade.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'Não foi possível abrir o corpo HTML: $error';
  }

  @override
  String webViewError(String error) {
    return 'Erro do WebView: $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'O texto visível do link mostra um endereço diferente do destino real.';

  @override
  String get headerFrom => 'De:';

  @override
  String get headerTo => 'Para:';

  @override
  String get headerCc => 'Cc:';

  @override
  String get headerDate => 'Data:';

  @override
  String get folderInbox => 'Caixa de entrada';

  @override
  String get messageActionReply => 'Responder';

  @override
  String get messageActionReplyAll => 'Responder a todos';

  @override
  String get messageActionForward => 'Reencaminhar';

  @override
  String get messageActionDelete => 'Eliminar';

  @override
  String get messageActionJunk => 'Lixo';

  @override
  String get messageActionMove => 'Mover';

  @override
  String get messageActionCopy => 'Copiar';

  @override
  String get messageMenuTooltip => 'Ações da mensagem';

  @override
  String get settingsViewMinimalHeaders => 'Cabeçalhos mínimos';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'Quando ativo, oculta apenas Cc; De, Para e data continuam visíveis quando existirem.';

  @override
  String get settingsTooltip => 'Definições';

  @override
  String get accountsAndFoldersTooltip => 'Contas e pastas';

  @override
  String get cancelSelectionTooltip => 'Cancelar seleção';

  @override
  String multiSelectCount(int count) {
    return '$count selecionado(s)';
  }

  @override
  String get composeTooltip => 'Redigir';

  @override
  String get composeNeedTransportTooltip =>
      'Adicione um transporte de saída nas Definições';

  @override
  String get mailToolbarMoreTooltip => 'Mais';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count selecionado(s)';
  }

  @override
  String get settingsTabAccounts => 'Contas';

  @override
  String get settingsTabOutgoing => 'Saída';

  @override
  String get settingsTabSecurity => 'Segurança';

  @override
  String get settingsTabViewing => 'Visualização';

  @override
  String get settingsTabComposing => 'Redação';

  @override
  String get settingsTabAbout => 'Acerca';

  @override
  String get useSystemKeychain => 'Usar chaveiro do sistema';

  @override
  String get storeCredentialsInKeychain =>
      'Guardar credenciais no chaveiro da plataforma';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Autenticar com Google';

  @override
  String get authenticateMicrosoft => 'Autenticar com Microsoft';

  @override
  String get reloadOAuthToken => 'Recarregar token OAuth';

  @override
  String get matrixE2eeSection => 'Cifra ponta a ponta Matrix';

  @override
  String get initCrypto => 'Inicializar cifra';

  @override
  String get setupBackup => 'Configurar cópia de segurança';

  @override
  String get restoreBackup => 'Restaurar cópia de segurança';

  @override
  String get showDeviceFingerprint =>
      'Mostrar impressão digital do dispositivo';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Detalhe da mensagem por baixo da lista (ambiente de trabalho)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'Quando desligado, abrir uma mensagem usa um ecrã completo separado.';

  @override
  String get loadRemoteImages => 'Carregar imagens remotas';

  @override
  String get loadRemoteImagesSubtitle =>
      'Permitir imagens externas no correio HTML';

  @override
  String get threadedView => 'Vista por conversação';

  @override
  String get threadedViewSubtitle => 'Agrupar mensagens por tópico';

  @override
  String get deletionAndTrashSection => 'Eliminação e lixo';

  @override
  String get deletionAppliesGlobally =>
      'Aplica-se a todas as contas de correio.';

  @override
  String get deleteModeLabel => 'Modo de eliminação';

  @override
  String get trashFolderNameLabel => 'Nome da pasta de lixo';

  @override
  String get deleteModeMoveToTrash => 'Mover para o lixo';

  @override
  String get deleteModeMarkDeleted => 'Marcar como eliminado';

  @override
  String get quoteOriginalOnReply => 'Citar a mensagem original na resposta';

  @override
  String get testSend => 'Teste de envio';

  @override
  String get openSignatureEditor => 'Abrir editor de assinatura';

  @override
  String get aboutSubtitle => 'Correio e mensagens multiplataforma';

  @override
  String get supportedBackends => 'Backends suportados';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation (demonstração)';
  }

  @override
  String get accountTypeDialogTitle => 'Tipo de conta';

  @override
  String get removeAccountTitle => 'Remover conta?';

  @override
  String removeAccountBody(String label) {
    return 'Remover «$label» da configuração guardada neste dispositivo?';
  }

  @override
  String removedAccount(String label) {
    return 'Removido «$label»';
  }

  @override
  String get accountsListTitle => 'Contas';

  @override
  String get accountsListSubtitle =>
      'Toque numa conta para editar ou adicione uma abaixo.';

  @override
  String get deleteTooltip => 'Eliminar';

  @override
  String get addAccount => 'Adicionar conta';

  @override
  String get noAccountsYet => 'Ainda sem contas. Toque em «Adicionar conta».';

  @override
  String get discardChangesTitle => 'Descartar alterações?';

  @override
  String get discardChangesBody =>
      'As alterações serão perdidas, como sair sem guardar.';

  @override
  String get keepEditing => 'Continuar a editar';

  @override
  String get pickNotSupportedWeb =>
      'Escolher ficheiros ou pastas não é suportado na versão web';

  @override
  String get chooseMaildirFolderTitle => 'Escolher pasta raiz Maildir';

  @override
  String get chooseMboxFileTitle => 'Escolher ficheiro mbox';

  @override
  String get validationAccountNameRequired => 'O nome da conta é obrigatório';

  @override
  String get validationLocalPathRequired =>
      'O caminho da caixa local é obrigatório';

  @override
  String get validationUsernameRequired =>
      'Nome de utilizador ou e-mail obrigatório para este tipo de conta';

  @override
  String get validationHostRequired => 'O anfitrião do servidor é obrigatório';

  @override
  String get validationPortRequired => 'É necessário um número de porta válido';

  @override
  String get accountSaved => 'Conta guardada';

  @override
  String get createTransportFirst =>
      'Crie primeiro um transporte no separador Saída';

  @override
  String get addTransportDialogTitle => 'Adicionar transporte';

  @override
  String get accountTypeLabel => 'Tipo de conta';

  @override
  String get accountTypeHelper => 'O tipo é fixo ao editar uma conta existente';

  @override
  String get accountNameLabel => 'Nome da conta';

  @override
  String get usernameEmailOptional => 'Nome de utilizador / e-mail (opcional)';

  @override
  String get usernameEmailRequired => 'Nome de utilizador / e-mail';

  @override
  String get avatarUrlLabel =>
      'URL do avatar ou caminho do ficheiro (opcional)';

  @override
  String get avatarUrlHelper =>
      'Imagem ou caminho local opcional para a barra de contas';

  @override
  String get localMailboxSection => 'Caixa local';

  @override
  String get pathMboxFile => 'Caminho para o ficheiro mbox';

  @override
  String get pathMaildirRoot => 'Caminho raiz Maildir';

  @override
  String get helperMboxPath =>
      'Use o botão de ficheiro para procurar ou escreva um caminho absoluto';

  @override
  String get helperMaildirPath =>
      'Use o botão de pasta para procurar ou escreva um caminho absoluto';

  @override
  String get chooseMboxTooltip => 'Escolher ficheiro mbox';

  @override
  String get chooseMaildirTooltip => 'Escolher pasta Maildir';

  @override
  String get imapServerSection => 'Servidor IMAP';

  @override
  String get pop3ServerSection => 'Servidor POP3';

  @override
  String get nntpServerSection => 'Servidor NNTP';

  @override
  String get hostLabel => 'Anfitrião';

  @override
  String get serverHostLabel => 'Anfitrião do servidor';

  @override
  String get portLabel => 'Porta';

  @override
  String get portHelperImap => 'Normalmente 993 (IMAPS) ou 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'Normalmente 995 (POP3S, TLS implícito)';

  @override
  String get portHelperNntp => 'Normalmente 563 (NNTPS, TLS implícito)';

  @override
  String get securityLabel => 'Segurança';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (TLS implícito)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (TLS implícito)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (TLS implícito)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'Sem encriptação';

  @override
  String get outgoingTransportsSection => 'Transportes de saída';

  @override
  String get noTransportsHintLinked =>
      'Nenhum transporte selecionado — redigir e responder ficam desativados até escolher pelo menos um. Use o separador Saída e selecione aqui.';

  @override
  String get transportsOrderHint =>
      'O primeiro da lista é o predefinido para envio. Crie transportes em Saída.';

  @override
  String get unknownTransport => 'Transporte desconhecido';

  @override
  String get moveUpTooltip => 'Subir';

  @override
  String get moveDownTooltip => 'Descer';

  @override
  String get removeFromAccountTooltip => 'Remover da conta';

  @override
  String get addTransportToAccount => 'Adicionar transporte à conta';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Servidor de origem';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'URLs de relay (separados por vírgulas)';

  @override
  String storeUriLabel(String uri) {
    return 'URI do arquivo: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'URI do transporte: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'Novo $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return 'Editar $label';
  }

  @override
  String foldersLoadError(String error) {
    return 'Pastas: $error';
  }

  @override
  String get sortMessagesTooltip => 'Ordenar mensagens';

  @override
  String get sort => 'Ordenar';

  @override
  String get sortFromAz => 'De A a Z';

  @override
  String get sortFromZa => 'De Z a A';

  @override
  String get sortSubjectAz => 'Assunto A–Z';

  @override
  String get sortSubjectZa => 'Assunto Z–A';

  @override
  String get sortDateOldest => 'Data: mais antigas primeiro';

  @override
  String get sortDateNewest => 'Data: mais recentes primeiro';

  @override
  String get removeTransportTitle => 'Remover transporte?';

  @override
  String removeTransportBody(String name) {
    return '«$name» será removido das listas de saída de todas as contas.';
  }

  @override
  String removedTransport(String name) {
    return 'Removido «$name»';
  }

  @override
  String get outgoingListTitle => 'Saída';

  @override
  String get outgoingListSubtitle =>
      'SMTP e outros transportes de envio. Associe-os às contas no separador Contas.';

  @override
  String get addTransport => 'Adicionar transporte';

  @override
  String get noTransportsYet =>
      'Ainda sem transportes de saída. Toque em «Adicionar transporte».';

  @override
  String get transportDisplayHostRequired =>
      'O nome a apresentar e o anfitrião são obrigatórios.';

  @override
  String get transportSaved => 'Transporte guardado';

  @override
  String get newTransport => 'Novo transporte';

  @override
  String get editTransport => 'Editar transporte';

  @override
  String get displayNameLabel => 'Nome a apresentar';

  @override
  String get smtpHostLabel => 'Anfitrião SMTP';

  @override
  String get smtpPortHelper => 'Normalmente 587 (STARTTLS) ou 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'Início de sessão IMAP';

  @override
  String get enterUsernameAndPassword =>
      'Introduza nome de utilizador e palavra-passe.';

  @override
  String get usernameLabel => 'Nome de utilizador';

  @override
  String get passwordLabel => 'Palavra-passe';

  @override
  String get showPasswordTooltip => 'Mostrar palavra-passe';

  @override
  String get hidePasswordTooltip => 'Ocultar palavra-passe';

  @override
  String get fieldTo => 'Para';

  @override
  String get fieldCc => 'Cc';

  @override
  String get fieldBcc => 'Bcc';

  @override
  String get fieldSubject => 'Assunto';

  @override
  String get fieldBody => 'Corpo';

  @override
  String get folderNewSubfolder => 'Nova subpasta';

  @override
  String get folderRename => 'Mudar o nome…';

  @override
  String get folderDelete => 'Eliminar…';

  @override
  String get folderNewTooltip => 'Nova pasta';

  @override
  String get folderNewDialogTitle => 'Nova pasta';

  @override
  String get folderNameLabel => 'Nome da pasta';

  @override
  String get folderNewTopLevelHelper => 'Cria uma caixa no nível superior';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Subpasta de $parent';
  }

  @override
  String get subfolderNameLabel => 'Nome da subpasta';

  @override
  String subfolderPathHelper(String path) {
    return 'Caminho: $path';
  }

  @override
  String folderCreated(String name) {
    return 'Pasta «$name» criada';
  }

  @override
  String get renameFolderTitle => 'Mudar o nome da pasta';

  @override
  String get newFolderPathLabel => 'Novo caminho da pasta';

  @override
  String get folderRenamed => 'Pasta renomeada';

  @override
  String get deleteFolderTitle => 'Eliminar pasta?';

  @override
  String deleteFolderBody(String name) {
    return 'Remover «$name» e as suas mensagens do servidor (se suportado)? Não pode ser anulado.';
  }

  @override
  String get folderDeleted => 'Pasta eliminada';

  @override
  String get licenseTitle => 'Licença';

  @override
  String get copyrightTitle => 'Direitos de autor';

  @override
  String get chatHintTypeMessage => 'Escreva uma mensagem';

  @override
  String operationFailed(String error) {
    return 'Ocorreu um problema: $error';
  }

  @override
  String get expandFolder => 'Expandir';

  @override
  String get collapseFolder => 'Recolher';

  @override
  String get noTextBody => '(Sem corpo de texto)';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Mover para aqui';

  @override
  String get folderCopyHere => 'Copiar para aqui';

  @override
  String get folderExpunge => 'Expurgar mensagens eliminadas';

  @override
  String get folderExpungeDone => 'Expurgo concluído';

  @override
  String pendingMoveTagged(int count) {
    return 'Escolha uma pasta e depois Mover para aqui ($count mensagens)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Escolha uma pasta e depois Copiar para aqui ($count mensagens)';
  }

  @override
  String transferResultOk(int count) {
    return 'Concluído: $count mensagem(ns).';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok com sucesso, $failed falharam.';
  }

  @override
  String transferFailed(String error) {
    return 'Falha na transferência: $error';
  }
}
