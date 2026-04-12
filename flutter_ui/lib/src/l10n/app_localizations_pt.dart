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
  String get dialogOk => 'OK';

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
  String get settingsTabContacts => 'Contacts';

  @override
  String get settingsTabAbout => 'Acerca';

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
      'Não foi possível carregar as definições a partir do disco.';

  @override
  String get settingsLoadRetry => 'Tentar novamente';

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
  String get junkFolderNameLabel => 'Nome da pasta de spam';

  @override
  String get exchangeTrashFolderHelper =>
      'Deixe vazio para usar «Itens eliminados» (caixa em inglês). Use o nome exato mostrado no Outlook se for diferente.';

  @override
  String get exchangeJunkFolderHelper =>
      'Deixe vazio para usar «Correio não solicitado» (caixa em inglês). Use o nome exato mostrado no Outlook se for diferente.';

  @override
  String get deleteModeDeleteImmediately => 'Eliminar de imediato';

  @override
  String get deleteModeMoveToTrash => 'Mover para o lixo';

  @override
  String get deleteModeMarkDeleted => 'Marcar como eliminado';

  @override
  String get quoteOriginalOnReply => 'Citar a mensagem original na resposta';

  @override
  String get quoteOriginalOnReplySubtitle =>
      'Inclui o original por baixo do cabeçalho de resposta em novas respostas. A composição rica envolve-o num bloco de citação; o texto simples prefixa cada linha. A parte text/plain ainda inclui o original se isto estiver ativo.';

  @override
  String get composingReplySection => 'Citações nas respostas';

  @override
  String get replyHeaderTemplateLabel => 'Linha de cabeçalho da resposta';

  @override
  String get replyHeaderTemplateHelp =>
      'Mostrado acima do original citado. Inclua as palavras date, time e sender, cada uma com um dólar à frente (veja a pré-visualização). Ao responder, são substituídas pela data, hora e remetente.';

  @override
  String get replyHeaderPreviewLabel => 'Pré-visualização';

  @override
  String get replyDateFormatLabel => 'Data (no cabeçalho)';

  @override
  String get replyTimeFormatLabel => 'Hora (no cabeçalho)';

  @override
  String get replyDatePresetLocale => 'Igual ao sistema (data longa)';

  @override
  String get replyDatePresetIso => 'ISO: 2026-04-08';

  @override
  String get replyDatePresetUs => 'EUA: 04/08/2026';

  @override
  String get replyDatePresetEu => 'Dia/mês/ano: 08/04/2026';

  @override
  String get replyDatePresetMedium => 'Médio: 8 de abr. de 2026';

  @override
  String get replyDatePresetWeekday =>
      'Com dia da semana: qua., 8 de abr. de 2026';

  @override
  String replyDatePresetCustom(String pattern) {
    return 'Personalizado ($pattern)';
  }

  @override
  String get replyTimePresetLocale => 'Igual ao sistema';

  @override
  String get replyTimePreset12h => '12 horas (ex.: 13:30)';

  @override
  String get replyTimePreset24h => '24 horas (15:30)';

  @override
  String get replyTimePreset24hSeconds => '24 horas com segundos';

  @override
  String replyTimePresetCustom(String pattern) {
    return 'Personalizado ($pattern)';
  }

  @override
  String get replyLinePrefixLabel => 'Prefixo da linha citada';

  @override
  String get replyLinePrefixSubtitle =>
      'Anteposto a cada linha do original em citações de texto simples (clássico «> »). Só quando a citação está ativa.';

  @override
  String get replyPlainPositionLabel => 'Ordem da resposta e da citação';

  @override
  String get replyPlainPositionBefore => 'Resposta antes do texto citado';

  @override
  String get replyPlainPositionAfter => 'Resposta depois do texto citado';

  @override
  String get replyPlainPositionSubtitle =>
      'Texto simples ou rico: duas linhas em branco e cursor antes do cabeçalho de resposta, ou duas linhas em branco e cursor após o bloco citado. A parte text/plain no envio segue o mesmo esquema.';

  @override
  String get replyQuoteModeLabel => 'Partes HTML SMTP';

  @override
  String get replyQuoteModePlain => 'Só original em citação de texto simples';

  @override
  String get replyQuoteModeHtmlSmtp =>
      'Incluir também o original como HTML separado (SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'Adiciona uma segunda parte HTML que preserva a formatação da mensagem de origem. Clientes só de texto continuam a ver o corpo citado em claro. As publicações NNTP usam sempre citações em texto simples.';

  @override
  String get settingsComposeRichText => 'Texto rico ao escrever e-mail';

  @override
  String get settingsComposeRichTextSubtitle =>
      'Editor formatado para e-mails novos e respostas. Usenet (NNTP) permanece em texto simples.';

  @override
  String get settingsMatrixChatRichText => 'Texto rico em conversas Matrix';

  @override
  String get settingsMatrixChatRichTextSubtitle =>
      'Enviar mensagens formatadas em salas Matrix (com texto simples como alternativa).';

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
  String get validationEmailAddressRequired =>
      'O endereço de e-mail é obrigatório';

  @override
  String get validationMatrixUserIdRequired =>
      'O ID de utilizador Matrix é obrigatório';

  @override
  String get accountEmailAddressLabel => 'Endereço de e-mail';

  @override
  String get accountMatrixUserIdLabel => 'ID Matrix (MXID)';

  @override
  String get accountMatrixMxidHelper =>
      'Exemplo: @you:matrix.org — o URL do homeserver é derivado do domínio após os dois pontos.';

  @override
  String get validationMatrixMxidInvalid =>
      'Introduza um ID Matrix como @user:servidor';

  @override
  String get accountNntpDefaultFromLabel => 'Remetente predefinido (Usenet)';

  @override
  String get accountNntpDefaultFromHelper =>
      'Mostrado ao compor; esta conta NNTP publica através da sua própria ligação ao servidor.';

  @override
  String get accountEmailOptionalLabel => 'Endereço de e-mail (opcional)';

  @override
  String get accountTcpLoginHelper =>
      'Identidade de início de sessão neste servidor (normalmente o seu e-mail).';

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
  String get accountTypeHelper =>
      'Escolhido ao adicionar a conta; não pode ser alterado aqui.';

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
  String get mailSecurityImplicitTlsNntp => 'NNTPS (TLS implícito)';

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
  String get relayUrlsLabel => 'URLs de relay';

  @override
  String get relayUrlsHelper =>
      'Cada linha é um URL WebSocket de relay. Prima Enter quando terminar de editar um URL.';

  @override
  String get relayAddFieldHint => 'Novo URL de relay';

  @override
  String get relayAddTooltip => 'Adicionar relay';

  @override
  String get relayRemoveTooltip => 'Remover relay';

  @override
  String get nostrNewIdentityTooltip => 'Criar identidade Nostr nova';

  @override
  String get nostrRelayUrlsRequired => 'Introduza pelo menos um URL de relay.';

  @override
  String storeUriLabel(String uri) {
    return 'Conexão: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'URI de transporte legado: $uri';
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
  String get transportSavedAndVerified =>
      'Transporte guardado e SMTP verificado';

  @override
  String get transportSavedVerifyPending =>
      'Transporte guardado, mas o servidor não foi alcançado ou a autenticação falhou. Verifique o anfitrião, a segurança e as credenciais e guarde novamente.';

  @override
  String get transportTypeDialogTitle => 'Tipo de transporte de saída';

  @override
  String get transportTypeFixedHelper =>
      'Escolhido ao adicionar; não pode ser alterado aqui.';

  @override
  String get transportDisplayNameRequired =>
      'O nome a apresentar é obrigatório.';

  @override
  String get transportKindLabel => 'Tipo de saída';

  @override
  String get transportKindSmtp => 'SMTP';

  @override
  String get transportKindGmail => 'Gmail (Google)';

  @override
  String get gmailTransportPresetHelper =>
      'Usa smtp.gmail.com com OAuth (XOAUTH2). Guarde o transporte e inicie sessão com a mesma conta Google do IMAP Gmail.';

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
  String get matrixSignInTitle => 'Início de sessão Matrix';

  @override
  String get gmailSignInTitle => 'Iniciar sessão com Google';

  @override
  String get gmailSignInBody =>
      'O navegador abrirá para você iniciar sessão com Google e autorizar o acesso ao Gmail (IMAP).';

  @override
  String get gmailSignInBrowserButton => 'Continuar no navegador';

  @override
  String get smtpSignInTitle => 'Início de sessão SMTP';

  @override
  String smtpSignInSubtitle(String transportName, String host) {
    return 'Introduza o nome de utilizador e a palavra-passe para «$transportName» ($host).';
  }

  @override
  String get composeSendCancelledNoSmtpCredentials =>
      'Mensagem não enviada: as credenciais SMTP não foram guardadas.';

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
  String get fieldFrom => 'De';

  @override
  String get composeOutgoingTransport => 'Transporte de saída';

  @override
  String get composeSendSucceeded => 'Mensagem enviada';

  @override
  String get composeMissingFrom => 'Insira um endereço de remetente.';

  @override
  String get composeMissingTo => 'Insira pelo menos um destinatário.';

  @override
  String get composeMissingNewsgroups =>
      'Insira pelo menos um nome de newsgroup.';

  @override
  String get composeNntpPostingBlurb =>
      'As mensagens são enviadas pelo servidor NNTP desta conta (sem transporte separado).';

  @override
  String get fieldNewsgroups => 'Newsgroups';

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
  String get attach => 'Anexar';

  @override
  String get composeRemoveAttachment => 'Remover anexo';

  @override
  String get defaultFromLabel => 'Endereço De predefinido';

  @override
  String get defaultFromHelper =>
      'ex.: O seu nome <you@example.com> ou you@example.com';

  @override
  String get dsnLabel => 'Notificações de entrega';

  @override
  String get dsnUseTransportDefault => 'Predefinição do transporte';

  @override
  String get dsnNever => 'Nunca';

  @override
  String get dsnFailure => 'Em caso de falha';

  @override
  String get dsnSuccess => 'Em caso de sucesso';

  @override
  String get dsnDelay => 'Em caso de atraso';

  @override
  String get dsnFailureAndSuccess => 'Em caso de falha e de sucesso';

  @override
  String get dsnNotifyLabel => 'Notificação DSN';

  @override
  String get composeCryptoLabel => 'Assinatura / encriptação';

  @override
  String get composeCryptoTitle => 'Assinatura e encriptação de saída';

  @override
  String get composeCryptoNone => 'Sem encriptação';

  @override
  String get composeCryptoSign => 'Assinar';

  @override
  String get composeCryptoEncrypt => 'Encriptar';

  @override
  String get composeCryptoSignEncrypt => 'Assinar e encriptar';

  @override
  String get settingsMailCryptoSection => 'Assinatura de e-mail (saída)';

  @override
  String get settingsMailCryptoStackSubtitle =>
      'Pilha criptográfica para assinatura e encriptação de saída (composição).';

  @override
  String get settingsMailCryptoStackOpenpgp => 'OpenPGP';

  @override
  String get settingsMailCryptoStackSmime => 'S/MIME';

  @override
  String get settingsMailCryptoPgpSecretKeyPath =>
      'Pasta home do GnuPG (opcional)';

  @override
  String get settingsMailCryptoPgpPassphrase =>
      'ID ou impressão digital da chave OpenPGP de assinatura';

  @override
  String get settingsMailCryptoSmimeCert =>
      'Certificado de assinatura S/MIME (caminho PEM)';

  @override
  String get settingsMailCryptoSmimeKey =>
      'Chave privada de assinatura S/MIME (caminho PEM)';

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
  String get chatAttachmentsNotSentInChat =>
      'O chat ainda não pode enviar anexos. Remova-os para enviar a mensagem ou use a redação de e-mail para ficheiros.';

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
  String get matrixE2eeUndecryptableTitle =>
      'Esta mensagem ainda não pode ser decifrada';

  @override
  String get matrixE2eeUndecryptableHelp =>
      'Esta conversa está protegida por encriptação ponta a ponta Matrix. O Tagliacarte não tem a chave da sala para esta mensagem.\n\nO que pode fazer:\n• No Element (ou outro cliente Matrix): Definições → Segurança → Cópia de segurança segura — desbloqueie com a chave ou frase de recuperação. Se o Tagliacarte oferecer restauro da cópia de chaves, use a mesma chave aí.\n• Noutro dispositivo onde já leu esta conversa (ex. Element no telemóvel ou no computador): inicie sessão, verifique esta sessão Tagliacarte se for pedido, mantenha o dispositivo ligado e abra esta mensagem direta para as chaves poderem ser reencaminhadas.\n• Depois dos dispositivos se confiarem, peça ao contacto uma mensagem nova — isso só ajuda mensagens novas; as antigas ainda precisam de chaves da cópia ou doutro dispositivo.\n\nSem cópia de segurança segura e sem outro cliente com sessão iniciada, o histórico encriptado antigo pode ficar ilegível — é assim por desenho no Matrix.';

  @override
  String get matrixE2eeUndecryptableListPreview =>
      'Ainda não é possível desencriptar — abra a mensagem para os passos';

  @override
  String get matrixE2eeUndecryptableChatSnippet =>
      'Ainda não é possível desencriptar';

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
  String get folderTabSubscribed => 'Subscritos';

  @override
  String get folderTabAvailable => 'Disponíveis';

  @override
  String get matrixFolderTabRooms => 'Salas';

  @override
  String get matrixFolderTabDirectMessages => 'Mensagens diretas';

  @override
  String get folderActionSubscribe => 'Subscrever';

  @override
  String get folderActionUnsubscribe => 'Cancelar subscrição';

  @override
  String get folderActionJoinRoom => 'Entrar na sala';

  @override
  String get folderActionLeaveRoom => 'Sair da sala';

  @override
  String get nntpWildmatHint => 'Padrão (ex.: comp.os.linux.*)';

  @override
  String get nntpWildmatQuery => 'Listar';

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

  @override
  String deleteMessagesFailed(String error) {
    return 'Falha ao excluir: $error';
  }

  @override
  String get settingsNotifyNewMessages => 'Notificações de mensagens novas';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Snack bar com o app aberto; notificação do sistema em segundo plano (IMAP).';

  @override
  String get newMailNotificationTitle => 'Correio novo';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count mensagem(ns) nova(s) em $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel =>
      'Segundos em silêncio antes do IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Vazio = padrão (120). Mínimo 15. Após inatividade da conexão.';

  @override
  String get validationImapMinIdleSeconds =>
      'Número inteiro entre 15 e 864000, ou vazio.';
}
