// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => '设置';

  @override
  String get compose => '撰写';

  @override
  String get send => '发送';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => '取消';

  @override
  String get save => '保存';

  @override
  String get remove => '移除';

  @override
  String get delete => '删除';

  @override
  String get discard => '放弃';

  @override
  String get back => '返回';

  @override
  String get create => '创建';

  @override
  String get rename => '重命名';

  @override
  String get folderLabel => '文件夹';

  @override
  String get messageTitle => '邮件';

  @override
  String get selectFolder => '请选择文件夹';

  @override
  String get selectMessage => '请选择邮件';

  @override
  String get selectMessageToRead => '请选择要阅读的邮件。';

  @override
  String get noMessages => '没有邮件';

  @override
  String get attachments => '附件';

  @override
  String get saveAttachment => '保存附件';

  @override
  String savedToPath(String path) {
    return '已保存到 $path';
  }

  @override
  String saveFailed(String error) {
    return '保存失败：$error';
  }

  @override
  String get cannotDownloadAttachment => '无法下载此附件';

  @override
  String get emptyAttachmentData => '附件数据为空';

  @override
  String downloadFailed(String error) {
    return '下载失败：$error';
  }

  @override
  String get saveVerb => '保存';

  @override
  String get loadImages => '加载图片';

  @override
  String get remoteImagesBlocked => '为保护隐私，已阻止远程图片。';

  @override
  String couldNotOpenHtmlBody(String error) {
    return '无法打开 HTML 正文：$error';
  }

  @override
  String webViewError(String error) {
    return 'WebView 错误：$error';
  }

  @override
  String get linkHoverMisleadingCaption => '可见的链接文字显示的地址与实际目标不同。';

  @override
  String get headerFrom => '发件人：';

  @override
  String get headerTo => '收件人：';

  @override
  String get headerCc => '抄送：';

  @override
  String get headerDate => '日期：';

  @override
  String get folderInbox => '收件箱';

  @override
  String get messageActionReply => '回复';

  @override
  String get messageActionReplyAll => '全部回复';

  @override
  String get messageActionForward => '转发';

  @override
  String get messageActionDelete => '删除';

  @override
  String get messageActionJunk => '垃圾邮件';

  @override
  String get messageActionMove => '移动';

  @override
  String get messageActionCopy => '复制';

  @override
  String get messageMenuTooltip => '邮件操作';

  @override
  String get settingsViewMinimalHeaders => '精简邮件头';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      '开启后仅隐藏抄送；有数据时仍显示发件人、收件人和日期。';

  @override
  String get settingsTooltip => '设置';

  @override
  String get accountsAndFoldersTooltip => '帐户与文件夹';

  @override
  String get cancelSelectionTooltip => '取消选择';

  @override
  String multiSelectCount(int count) {
    return '已选 $count 项';
  }

  @override
  String get composeTooltip => '撰写';

  @override
  String get composeNeedTransportTooltip => '请在设置中添加外发传输';

  @override
  String get mailToolbarMoreTooltip => '更多';

  @override
  String mailToolbarSelectedCount(int count) {
    return '已选 $count 项';
  }

  @override
  String get settingsTabAccounts => '帐户';

  @override
  String get settingsTabOutgoing => '外发';

  @override
  String get settingsTabSecurity => '安全';

  @override
  String get settingsTabViewing => '阅读';

  @override
  String get settingsTabComposing => '撰写';

  @override
  String get settingsTabAbout => '关于';

  @override
  String get useSystemKeychain => '使用系统钥匙串';

  @override
  String get storeCredentialsInKeychain => '将凭据保存在平台钥匙串中';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => '使用 Google 登录';

  @override
  String get authenticateMicrosoft => '使用 Microsoft 登录';

  @override
  String get reloadOAuthToken => '重新加载 OAuth 令牌';

  @override
  String get matrixE2eeSection => 'Matrix 端到端加密';

  @override
  String get initCrypto => '初始化加密';

  @override
  String get setupBackup => '设置备份';

  @override
  String get restoreBackup => '恢复备份';

  @override
  String get showDeviceFingerprint => '显示设备指纹';

  @override
  String get messageDetailInlineDesktopTitle => '在列表下方显示邮件详情（桌面）';

  @override
  String get messageDetailInlineDesktopSubtitle => '关闭后，打开邮件将使用独立全屏视图。';

  @override
  String get loadRemoteImages => '加载远程图片';

  @override
  String get loadRemoteImagesSubtitle => '允许 HTML 邮件中的外部图片';

  @override
  String get threadedView => '会话视图';

  @override
  String get threadedViewSubtitle => '按会话分组邮件';

  @override
  String get deletionAndTrashSection => '删除与废纸篓';

  @override
  String get deletionAppliesGlobally => '对所有邮件式帐户生效。';

  @override
  String get deleteModeLabel => '删除方式';

  @override
  String get trashFolderNameLabel => '废纸篓文件夹名称';

  @override
  String get deleteModeMoveToTrash => '移到废纸篓';

  @override
  String get deleteModeMarkDeleted => '标记为已删除';

  @override
  String get quoteOriginalOnReply => '回复时引用原邮件';

  @override
  String get testSend => '测试发送';

  @override
  String get openSignatureEditor => '打开签名编辑器';

  @override
  String get aboutSubtitle => '跨平台电子邮件与消息';

  @override
  String get supportedBackends => '支持的后端';

  @override
  String get supportedBackendsList => 'IMAP、POP3、SMTP、NNTP、Matrix、Nostr、Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '已调用 $operation';
  }

  @override
  String get accountTypeDialogTitle => '帐户类型';

  @override
  String get removeAccountTitle => '移除帐户？';

  @override
  String removeAccountBody(String label) {
    return '要从本机已保存的配置中移除「$label」吗？';
  }

  @override
  String removedAccount(String label) {
    return '已移除 $label';
  }

  @override
  String get accountsListTitle => '帐户';

  @override
  String get accountsListSubtitle => '点按帐户可编辑，或在下方添加新帐户。';

  @override
  String get deleteTooltip => '删除';

  @override
  String get addAccount => '添加帐户';

  @override
  String get noAccountsYet => '尚无帐户。点按「添加帐户」创建。';

  @override
  String get discardChangesTitle => '放弃更改？';

  @override
  String get discardChangesBody => '未保存的编辑将丢失，与直接退出相同。';

  @override
  String get keepEditing => '继续编辑';

  @override
  String get pickNotSupportedWeb => '网页版不支持选择文件或文件夹';

  @override
  String get chooseMaildirFolderTitle => '选择 Maildir 根文件夹';

  @override
  String get chooseMboxFileTitle => '选择 mbox 文件';

  @override
  String get validationAccountNameRequired => '需要填写帐户名称';

  @override
  String get validationLocalPathRequired => '需要填写本地邮箱路径';

  @override
  String get validationUsernameRequired => '此帐户类型需要用户名或电子邮件';

  @override
  String get validationHostRequired => '需要填写服务器主机';

  @override
  String get validationPortRequired => '需要有效的端口号';

  @override
  String get accountSaved => '帐户已保存';

  @override
  String get createTransportFirst => '请先在「外发」标签页创建传输';

  @override
  String get addTransportDialogTitle => '添加传输';

  @override
  String get accountTypeLabel => '帐户类型';

  @override
  String get accountTypeHelper => '编辑现有帐户时类型不可更改';

  @override
  String get accountNameLabel => '帐户名称';

  @override
  String get usernameEmailOptional => '用户名 / 电子邮件（可选）';

  @override
  String get usernameEmailRequired => '用户名 / 电子邮件';

  @override
  String get avatarUrlLabel => '头像 URL 或文件路径（可选）';

  @override
  String get avatarUrlHelper => '可选的图片 URL 或本地路径，用于帐户条展示';

  @override
  String get localMailboxSection => '本地邮箱';

  @override
  String get pathMboxFile => 'mbox 文件路径';

  @override
  String get pathMaildirRoot => 'Maildir 根路径';

  @override
  String get helperMboxPath => '使用文件按钮浏览，或输入绝对路径';

  @override
  String get helperMaildirPath => '使用文件夹按钮浏览，或输入绝对路径';

  @override
  String get chooseMboxTooltip => '选择 mbox 文件';

  @override
  String get chooseMaildirTooltip => '选择 Maildir 文件夹';

  @override
  String get imapServerSection => 'IMAP 服务器';

  @override
  String get pop3ServerSection => 'POP3 服务器';

  @override
  String get nntpServerSection => 'NNTP 服务器';

  @override
  String get hostLabel => '主机';

  @override
  String get serverHostLabel => '服务器主机';

  @override
  String get portLabel => '端口';

  @override
  String get portHelperImap => '通常为 993（IMAPS）或 143（STARTTLS）';

  @override
  String get portHelperPop3 => '通常为 995（POP3S，隐式 TLS）';

  @override
  String get portHelperNntp => '通常为 563（NNTPS，隐式 TLS）';

  @override
  String get securityLabel => '安全';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS（隐式 TLS）';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS（隐式 TLS）';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S（隐式 TLS）';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => '不加密';

  @override
  String get outgoingTransportsSection => '外发传输';

  @override
  String get noTransportsHintLinked =>
      '未选择传输 — 在至少选择一个之前，撰写与回复将不可用。请在外发标签页创建并在此处选择。';

  @override
  String get transportsOrderHint => '列表第一项为默认发送传输。请在外发中创建传输。';

  @override
  String get unknownTransport => '未知传输';

  @override
  String get moveUpTooltip => '上移';

  @override
  String get moveDownTooltip => '下移';

  @override
  String get removeFromAccountTooltip => '从帐户中移除';

  @override
  String get addTransportToAccount => '向帐户添加传输';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Homeserver';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => '中继 URL';

  @override
  String get relayUrlsHelper => '每行一个中继 WebSocket URL。编辑完成后按 Enter。';

  @override
  String get relayAddFieldHint => '新中继 URL';

  @override
  String get relayAddTooltip => '添加中继';

  @override
  String get relayRemoveTooltip => '移除中继';

  @override
  String get nostrNewIdentityTooltip => '创建新的 Nostr 身份';

  @override
  String get nostrRelayUrlsRequired => '请至少填写一个中继 URL。';

  @override
  String storeUriLabel(String uri) {
    return '存储 URI：$uri';
  }

  @override
  String transportUriLabel(String uri) {
    return '传输 URI：$uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return '新建 $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return '编辑 $label';
  }

  @override
  String foldersLoadError(String error) {
    return '文件夹：$error';
  }

  @override
  String get sortMessagesTooltip => '排序邮件';

  @override
  String get sort => '排序';

  @override
  String get sortFromAz => '发件人 A–Z';

  @override
  String get sortFromZa => '发件人 Z–A';

  @override
  String get sortSubjectAz => '主题 A–Z';

  @override
  String get sortSubjectZa => '主题 Z–A';

  @override
  String get sortDateOldest => '日期：最旧在前';

  @override
  String get sortDateNewest => '日期：最新在前';

  @override
  String get removeTransportTitle => '移除传输？';

  @override
  String removeTransportBody(String name) {
    return '「$name」将从所有帐户的外发列表中移除。';
  }

  @override
  String removedTransport(String name) {
    return '已移除 $name';
  }

  @override
  String get outgoingListTitle => '外发';

  @override
  String get outgoingListSubtitle => 'SMTP 等发送传输。在「帐户」标签页中关联到邮件帐户。';

  @override
  String get addTransport => '添加传输';

  @override
  String get noTransportsYet => '尚无外发传输。点按「添加传输」创建。';

  @override
  String get transportDisplayHostRequired => '需要显示名称和主机。';

  @override
  String get transportSaved => '传输已保存';

  @override
  String get newTransport => '新建传输';

  @override
  String get editTransport => '编辑传输';

  @override
  String get displayNameLabel => '显示名称';

  @override
  String get smtpHostLabel => 'SMTP 主机';

  @override
  String get smtpPortHelper => '通常为 587（STARTTLS）或 465（SMTPS）';

  @override
  String get imapSignInTitle => 'IMAP 登录';

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
  String get enterUsernameAndPassword => '请输入用户名和密码。';

  @override
  String get usernameLabel => '用户名';

  @override
  String get passwordLabel => '密码';

  @override
  String get showPasswordTooltip => '显示密码';

  @override
  String get hidePasswordTooltip => '隐藏密码';

  @override
  String get fieldFrom => '发件人';

  @override
  String get composeOutgoingTransport => '发件传输';

  @override
  String get composeSendSucceeded => '邮件已发送';

  @override
  String get composeMissingFrom => '请输入发件人地址。';

  @override
  String get composeMissingTo => '请至少填写一位收件人。';

  @override
  String get fieldTo => '收件人';

  @override
  String get fieldCc => '抄送';

  @override
  String get fieldBcc => '密送';

  @override
  String get fieldSubject => '主题';

  @override
  String get fieldBody => '正文';

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
  String get folderNewSubfolder => '新建子文件夹';

  @override
  String get folderRename => '重命名…';

  @override
  String get folderDelete => '删除…';

  @override
  String get folderNewTooltip => '新建文件夹';

  @override
  String get folderNewDialogTitle => '新建文件夹';

  @override
  String get folderNameLabel => '文件夹名称';

  @override
  String get folderNewTopLevelHelper => '在顶层创建邮箱';

  @override
  String subfolderDialogTitle(String parent) {
    return '$parent 的子文件夹';
  }

  @override
  String get subfolderNameLabel => '子文件夹名称';

  @override
  String subfolderPathHelper(String path) {
    return '路径：$path';
  }

  @override
  String folderCreated(String name) {
    return '已创建文件夹「$name」';
  }

  @override
  String get renameFolderTitle => '重命名文件夹';

  @override
  String get newFolderPathLabel => '新文件夹路径';

  @override
  String get folderRenamed => '文件夹已重命名';

  @override
  String get deleteFolderTitle => '删除文件夹？';

  @override
  String deleteFolderBody(String name) {
    return '从服务器删除「$name」及其邮件（若支持）？此操作无法撤销。';
  }

  @override
  String get folderDeleted => '文件夹已删除';

  @override
  String get licenseTitle => '许可协议';

  @override
  String get copyrightTitle => '版权';

  @override
  String get chatHintTypeMessage => '输入消息';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.';

  @override
  String operationFailed(String error) {
    return '出现问题：$error';
  }

  @override
  String get expandFolder => '展开';

  @override
  String get collapseFolder => '折叠';

  @override
  String get noTextBody => '（无纯文本正文）';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => '移动到此';

  @override
  String get folderCopyHere => '复制到此';

  @override
  String get folderExpunge => '清除已删除邮件';

  @override
  String get folderExpungeDone => '清除完成';

  @override
  String pendingMoveTagged(int count) {
    return '选择文件夹，然后点「移动到此」（$count 封）';
  }

  @override
  String pendingCopyTagged(int count) {
    return '选择文件夹，然后点「复制到此」（$count 封）';
  }

  @override
  String transferResultOk(int count) {
    return '完成：$count 封。';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok 成功，$failed 失败。';
  }

  @override
  String transferFailed(String error) {
    return '传输失败：$error';
  }

  @override
  String get settingsNotifyNewMessages => '新邮件通知';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      '应用在前台时显示 Snackbar；在后台时显示系统通知（IMAP）。';

  @override
  String get newMailNotificationTitle => '新邮件';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$folder 中有 $count 封新邮件';
  }

  @override
  String get accountImapMinIdleSecondsLabel => '进入 IDLE 前的最短空闲秒数';

  @override
  String get accountImapMinIdleSecondsHelper => '留空为默认（120）。最少 15。在连接空闲时生效。';

  @override
  String get validationImapMinIdleSeconds => '请输入 15–864000 的整数，或留空。';
}
