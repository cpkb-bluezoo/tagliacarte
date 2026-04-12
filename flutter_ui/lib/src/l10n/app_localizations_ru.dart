// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Russian (`ru`).
class AppLocalizationsRu extends AppLocalizations {
  AppLocalizationsRu([String locale = 'ru']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => 'Настройки';

  @override
  String get compose => 'Написать';

  @override
  String get send => 'Отправить';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'Отмена';

  @override
  String get save => 'Сохранить';

  @override
  String get remove => 'Убрать';

  @override
  String get delete => 'Удалить';

  @override
  String get discard => 'Отменить';

  @override
  String get back => 'Назад';

  @override
  String get create => 'Создать';

  @override
  String get rename => 'Переименовать';

  @override
  String get folderLabel => 'Папка';

  @override
  String get messageTitle => 'Письмо';

  @override
  String get selectFolder => 'Выберите папку';

  @override
  String get selectMessage => 'Выберите письмо';

  @override
  String get selectMessageToRead => 'Выберите письмо для чтения.';

  @override
  String get noMessages => 'Нет писем';

  @override
  String get attachments => 'Вложения';

  @override
  String get saveAttachment => 'Сохранить вложение';

  @override
  String savedToPath(String path) {
    return 'Сохранено: $path';
  }

  @override
  String saveFailed(String error) {
    return 'Не удалось сохранить: $error';
  }

  @override
  String get cannotDownloadAttachment => 'Невозможно скачать это вложение';

  @override
  String get emptyAttachmentData => 'Пустые данные вложения';

  @override
  String downloadFailed(String error) {
    return 'Ошибка загрузки: $error';
  }

  @override
  String get saveVerb => 'Сохранить';

  @override
  String get loadImages => 'Загрузить изображения';

  @override
  String get remoteImagesBlocked =>
      'Удалённые изображения заблокированы в целях конфиденциальности.';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'Не удалось открыть HTML-текст: $error';
  }

  @override
  String webViewError(String error) {
    return 'Ошибка WebView: $error';
  }

  @override
  String get linkHoverMisleadingCaption =>
      'Видимый текст ссылки показывает другой адрес, чем реальное назначение.';

  @override
  String get headerFrom => 'От:';

  @override
  String get headerTo => 'Кому:';

  @override
  String get headerCc => 'Копия:';

  @override
  String get headerDate => 'Дата:';

  @override
  String get folderInbox => 'Входящие';

  @override
  String get messageActionReply => 'Ответить';

  @override
  String get messageActionReplyAll => 'Ответить всем';

  @override
  String get messageActionForward => 'Переслать';

  @override
  String get messageActionDelete => 'Удалить';

  @override
  String get messageActionJunk => 'Спам';

  @override
  String get messageActionMove => 'Переместить';

  @override
  String get messageActionCopy => 'Копировать';

  @override
  String get messageMenuTooltip => 'Действия с письмом';

  @override
  String get settingsViewMinimalHeaders => 'Минимум заголовков письма';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'Если включено, скрывается только «Копия»; «От», «Кому» и дата показываются, когда есть.';

  @override
  String get settingsTooltip => 'Настройки';

  @override
  String get accountsAndFoldersTooltip => 'Учётные записи и папки';

  @override
  String get cancelSelectionTooltip => 'Снять выделение';

  @override
  String multiSelectCount(int count) {
    return 'Выбрано: $count';
  }

  @override
  String get composeTooltip => 'Написать';

  @override
  String get composeNeedTransportTooltip =>
      'Добавьте исходящий транспорт в настройках';

  @override
  String get mailToolbarMoreTooltip => 'Ещё';

  @override
  String mailToolbarSelectedCount(int count) {
    return 'Выбрано: $count';
  }

  @override
  String get settingsTabAccounts => 'Учётные записи';

  @override
  String get settingsTabOutgoing => 'Исходящие';

  @override
  String get settingsTabSecurity => 'Безопасность';

  @override
  String get settingsTabViewing => 'Просмотр';

  @override
  String get settingsTabComposing => 'Написание';

  @override
  String get settingsTabContacts => 'Contacts';

  @override
  String get settingsTabAbout => 'О программе';

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
  String get settingsLoadFailed => 'Не удалось загрузить настройки с диска.';

  @override
  String get settingsLoadRetry => 'Повторить';

  @override
  String get useSystemKeychain => 'Использовать системную связку ключей';

  @override
  String get storeCredentialsInKeychain =>
      'Хранить учётные данные в связке ключей платформы';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Войти через Google';

  @override
  String get authenticateMicrosoft => 'Войти через Microsoft';

  @override
  String get reloadOAuthToken => 'Обновить токен OAuth';

  @override
  String get matrixE2eeSection => 'Сквозное шифрование Matrix';

  @override
  String get initCrypto => 'Инициализировать шифрование';

  @override
  String get setupBackup => 'Настроить резервную копию';

  @override
  String get restoreBackup => 'Восстановить из резервной копии';

  @override
  String get showDeviceFingerprint => 'Показать отпечаток устройства';

  @override
  String get messageDetailInlineDesktopTitle =>
      'Письмо под списком (настольный режим)';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'Если выключено, открытие письма открывает отдельный полноэкранный вид.';

  @override
  String get loadRemoteImages => 'Загружать удалённые изображения';

  @override
  String get loadRemoteImagesSubtitle =>
      'Разрешать внешние изображения в HTML-почте';

  @override
  String get threadedView => 'Цепочки писем';

  @override
  String get threadedViewSubtitle => 'Группировать письма по теме обсуждения';

  @override
  String get deletionAndTrashSection => 'Удаление и корзина';

  @override
  String get deletionAppliesGlobally =>
      'Действует для всех почтовых учётных записей.';

  @override
  String get deleteModeLabel => 'Режим удаления';

  @override
  String get trashFolderNameLabel => 'Имя папки корзины';

  @override
  String get junkFolderNameLabel => 'Имя папки нежелательной почты';

  @override
  String get exchangeTrashFolderHelper =>
      'Оставьте пустым для «Deleted Items» (ящик на английском). Укажите точное имя папки из Outlook, если оно другое.';

  @override
  String get exchangeJunkFolderHelper =>
      'Оставьте пустым для «Junk Email» (ящик на английском). Укажите точное имя папки из Outlook, если оно другое.';

  @override
  String get deleteModeDeleteImmediately => 'Удалить сразу';

  @override
  String get deleteModeMoveToTrash => 'В корзину';

  @override
  String get deleteModeMarkDeleted => 'Пометить удалённым';

  @override
  String get quoteOriginalOnReply => 'Цитировать исходное письмо в ответе';

  @override
  String get quoteOriginalOnReplySubtitle =>
      'Добавляет оригинал под заголовком ответа. Форматированный редактор помещает его в блок цитаты; простой текст добавляет префикс к каждой строке. Часть text/plain по-прежнему включает оригинал, если параметр включён.';

  @override
  String get composingReplySection => 'Цитирование в ответах';

  @override
  String get replyHeaderTemplateLabel => 'Строка заголовка ответа';

  @override
  String get replyHeaderTemplateHelp =>
      'Показывается над цитируемым оригиналом. Укажите слова date, time и sender с символом \$ перед каждым (см. предпросмотр). При ответе они заменяются датой, временем и отправителем.';

  @override
  String get replyHeaderPreviewLabel => 'Предпросмотр';

  @override
  String get replyDateFormatLabel => 'Дата (в заголовке)';

  @override
  String get replyTimeFormatLabel => 'Время (в заголовке)';

  @override
  String get replyDatePresetLocale => 'Как в системе (длинная дата)';

  @override
  String get replyDatePresetIso => 'ISO: 2026-04-08';

  @override
  String get replyDatePresetUs => 'США: 04/08/2026';

  @override
  String get replyDatePresetEu => 'День/месяц/год: 08/04/2026';

  @override
  String get replyDatePresetMedium => 'Средний: 8 апр. 2026';

  @override
  String get replyDatePresetWeekday => 'С днём недели: ср, 8 апр. 2026';

  @override
  String replyDatePresetCustom(String pattern) {
    return 'Свой формат ($pattern)';
  }

  @override
  String get replyTimePresetLocale => 'Как в системе';

  @override
  String get replyTimePreset12h => '12-часовой (напр. 13:30)';

  @override
  String get replyTimePreset24h => '24-часовой (15:30)';

  @override
  String get replyTimePreset24hSeconds => '24-часовой с секундами';

  @override
  String replyTimePresetCustom(String pattern) {
    return 'Свой формат ($pattern)';
  }

  @override
  String get replyLinePrefixLabel => 'Префикс цитируемой строки';

  @override
  String get replyLinePrefixSubtitle =>
      'Добавляется к каждой строке оригинала в цитатах простого текста (классика «> »). Только при включённом цитировании.';

  @override
  String get replyPlainPositionLabel => 'Порядок ответа и цитаты';

  @override
  String get replyPlainPositionBefore => 'Ответ перед цитируемым текстом';

  @override
  String get replyPlainPositionAfter => 'Ответ после цитируемого текста';

  @override
  String get replyPlainPositionSubtitle =>
      'Простой или форматированный текст: две пустые строки и курсор перед заголовком ответа или после блока цитаты. Часть text/plain при отправке следует той же вёрстке.';

  @override
  String get replyQuoteModeLabel => 'HTML-части SMTP';

  @override
  String get replyQuoteModePlain => 'Только оригинал в цитате простого текста';

  @override
  String get replyQuoteModeHtmlSmtp =>
      'Также включить оригинал отдельной HTML-частью (SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'Добавляет вторую HTML-часть с сохранением форматирования исходного письма. Текстовые клиенты по-прежнему видят цитируемое тело. Публикации NNTP всегда в простом тексте.';

  @override
  String get settingsComposeRichText =>
      'Форматированный текст при написании писем';

  @override
  String get settingsComposeRichTextSubtitle =>
      'Редактор с форматированием для новых писем и ответов. Публикации в Usenet (NNTP) остаются простым текстом.';

  @override
  String get settingsMatrixChatRichText =>
      'Форматированный текст в чатах Matrix';

  @override
  String get settingsMatrixChatRichTextSubtitle =>
      'Отправка форматированных сообщений в комнаты Matrix (с запасным вариантом в виде простого текста).';

  @override
  String get testSend => 'Тестовая отправка';

  @override
  String get openSignatureEditor => 'Редактор подписи';

  @override
  String get aboutSubtitle => 'Кроссплатформенная почта и обмен сообщениями';

  @override
  String get supportedBackends => 'Поддерживаемые бэкенды';

  @override
  String get supportedBackendsList =>
      'IMAP, POP3, SMTP, NNTP, Matrix, Nostr, Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return 'Вызвано: $operation';
  }

  @override
  String get accountTypeDialogTitle => 'Тип учётной записи';

  @override
  String get removeAccountTitle => 'Удалить учётную запись?';

  @override
  String removeAccountBody(String label) {
    return 'Удалить «$label» из сохранённой конфигурации на этом устройстве?';
  }

  @override
  String removedAccount(String label) {
    return 'Удалено: $label';
  }

  @override
  String get accountsListTitle => 'Учётные записи';

  @override
  String get accountsListSubtitle =>
      'Нажмите учётную запись для редактирования или добавьте новую ниже.';

  @override
  String get deleteTooltip => 'Удалить';

  @override
  String get addAccount => 'Добавить учётную запись';

  @override
  String get noAccountsYet =>
      'Учётных записей пока нет. Нажмите «Добавить учётную запись».';

  @override
  String get discardChangesTitle => 'Отменить изменения?';

  @override
  String get discardChangesBody =>
      'Правки будут потеряны — как при выходе без сохранения.';

  @override
  String get keepEditing => 'Продолжить редактирование';

  @override
  String get pickNotSupportedWeb =>
      'Выбор файлов или папок не поддерживается в веб-сборке';

  @override
  String get chooseMaildirFolderTitle => 'Выберите корневую папку Maildir';

  @override
  String get chooseMboxFileTitle => 'Выберите файл mbox';

  @override
  String get validationAccountNameRequired => 'Укажите имя учётной записи';

  @override
  String get validationLocalPathRequired =>
      'Укажите путь к локальному почтовому ящику';

  @override
  String get validationUsernameRequired =>
      'Для этого типа учётной записи нужны имя пользователя или email';

  @override
  String get validationEmailAddressRequired =>
      'Требуется адрес электронной почты';

  @override
  String get validationMatrixUserIdRequired =>
      'Требуется идентификатор пользователя Matrix';

  @override
  String get accountEmailAddressLabel => 'Адрес электронной почты';

  @override
  String get accountMatrixUserIdLabel => 'Идентификатор Matrix (MXID)';

  @override
  String get accountMatrixMxidHelper =>
      'Пример: @you:matrix.org — URL homeserver выводится из домена после двоеточия.';

  @override
  String get validationMatrixMxidInvalid =>
      'Введите Matrix ID вида @user:server';

  @override
  String get accountNntpDefaultFromLabel =>
      'Адрес отправителя по умолчанию (Usenet)';

  @override
  String get accountNntpDefaultFromHelper =>
      'Показывается при написании; эта учётная запись NNTP публикует через своё подключение к серверу.';

  @override
  String get accountEmailOptionalLabel =>
      'Адрес электронной почты (необязательно)';

  @override
  String get accountTcpLoginHelper =>
      'Учётная запись для входа на сервер (обычно ваш email).';

  @override
  String get validationHostRequired => 'Укажите хост сервера';

  @override
  String get validationPortRequired => 'Укажите корректный номер порта';

  @override
  String get accountSaved => 'Учётная запись сохранена';

  @override
  String get createTransportFirst =>
      'Сначала создайте транспорт на вкладке «Исходящие»';

  @override
  String get addTransportDialogTitle => 'Добавить транспорт';

  @override
  String get accountTypeLabel => 'Тип учётной записи';

  @override
  String get accountTypeHelper =>
      'Выбирается при добавлении учётной записи; здесь не меняется.';

  @override
  String get accountNameLabel => 'Имя учётной записи';

  @override
  String get usernameEmailOptional =>
      'Имя пользователя / email (необязательно)';

  @override
  String get usernameEmailRequired => 'Имя пользователя / email';

  @override
  String get avatarUrlLabel => 'URL аватара или путь к файлу (необязательно)';

  @override
  String get avatarUrlHelper =>
      'Необязательный URL изображения или локальный путь для полосы учётных записей';

  @override
  String get localMailboxSection => 'Локальный почтовый ящик';

  @override
  String get pathMboxFile => 'Путь к файлу mbox';

  @override
  String get pathMaildirRoot => 'Путь к корню Maildir';

  @override
  String get helperMboxPath => 'Кнопка «Файл» или введите абсолютный путь';

  @override
  String get helperMaildirPath => 'Кнопка «Папка» или введите абсолютный путь';

  @override
  String get chooseMboxTooltip => 'Выбрать файл mbox';

  @override
  String get chooseMaildirTooltip => 'Выбрать папку Maildir';

  @override
  String get imapServerSection => 'Сервер IMAP';

  @override
  String get pop3ServerSection => 'Сервер POP3';

  @override
  String get nntpServerSection => 'Сервер NNTP';

  @override
  String get hostLabel => 'Хост';

  @override
  String get serverHostLabel => 'Хост сервера';

  @override
  String get portLabel => 'Порт';

  @override
  String get portHelperImap => 'Обычно 993 (IMAPS) или 143 (STARTTLS)';

  @override
  String get portHelperPop3 => 'Обычно 995 (POP3S, неявный TLS)';

  @override
  String get portHelperNntp => 'Обычно 563 (NNTPS, неявный TLS)';

  @override
  String get securityLabel => 'Безопасность';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS (неявный TLS)';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS (неявный TLS)';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S (неявный TLS)';

  @override
  String get mailSecurityImplicitTlsNntp => 'NNTPS (неявный TLS)';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => 'Без шифрования';

  @override
  String get outgoingTransportsSection => 'Исходящие транспорты';

  @override
  String get noTransportsHintLinked =>
      'Транспорты не выбраны — написание и ответы отключены, пока не выберете хотя бы один. Создайте на вкладке «Исходящие» и укажите здесь.';

  @override
  String get transportsOrderHint =>
      'Первый в списке — транспорт по умолчанию для отправки. Создавайте транспорты в «Исходящие».';

  @override
  String get unknownTransport => 'Неизвестный транспорт';

  @override
  String get moveUpTooltip => 'Выше';

  @override
  String get moveDownTooltip => 'Ниже';

  @override
  String get removeFromAccountTooltip => 'Убрать из учётной записи';

  @override
  String get addTransportToAccount => 'Добавить транспорт к учётной записи';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'Homeserver';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'URL релеев';

  @override
  String get relayUrlsHelper =>
      'Каждая строка — WebSocket URL релея. Нажмите Enter, когда закончите редактировать URL.';

  @override
  String get relayAddFieldHint => 'Новый URL релея';

  @override
  String get relayAddTooltip => 'Добавить релей';

  @override
  String get relayRemoveTooltip => 'Удалить релей';

  @override
  String get nostrNewIdentityTooltip => 'Создать новую личность Nostr';

  @override
  String get nostrRelayUrlsRequired => 'Укажите хотя бы один URL релея.';

  @override
  String storeUriLabel(String uri) {
    return 'Подключение: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'Устаревший URI исходящей доставки: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return 'Новая: $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return 'Изменить: $label';
  }

  @override
  String foldersLoadError(String error) {
    return 'Папки: $error';
  }

  @override
  String get sortMessagesTooltip => 'Сортировка писем';

  @override
  String get sort => 'Сортировка';

  @override
  String get sortFromAz => 'От А до Я';

  @override
  String get sortFromZa => 'От Я до А';

  @override
  String get sortSubjectAz => 'Тема А–Я';

  @override
  String get sortSubjectZa => 'Тема Я–А';

  @override
  String get sortDateOldest => 'Дата: сначала старые';

  @override
  String get sortDateNewest => 'Дата: сначала новые';

  @override
  String get removeTransportTitle => 'Удалить транспорт?';

  @override
  String removeTransportBody(String name) {
    return '«$name» будет удалён из списков исходящих у всех учётных записей.';
  }

  @override
  String removedTransport(String name) {
    return 'Удалено: $name';
  }

  @override
  String get outgoingListTitle => 'Исходящие';

  @override
  String get outgoingListSubtitle =>
      'SMTP и другие транспорты отправки. Свяжите их с почтовыми учётными записями на вкладке «Учётные записи».';

  @override
  String get addTransport => 'Добавить транспорт';

  @override
  String get noTransportsYet =>
      'Исходящих транспортов пока нет. Нажмите «Добавить транспорт».';

  @override
  String get transportDisplayHostRequired => 'Нужны отображаемое имя и хост.';

  @override
  String get transportSaved => 'Транспорт сохранён';

  @override
  String get transportSavedAndVerified => 'Транспорт сохранён, SMTP проверен';

  @override
  String get transportSavedVerifyPending =>
      'Транспорт сохранён, но сервер недоступен или ошибка аутентификации. Проверьте хост, безопасность и учётные данные и нажмите «Сохранить» снова.';

  @override
  String get transportTypeDialogTitle => 'Тип исходящего транспорта';

  @override
  String get transportTypeFixedHelper =>
      'Выбирается при добавлении; здесь не меняется.';

  @override
  String get transportDisplayNameRequired => 'Нужно указать отображаемое имя.';

  @override
  String get transportKindLabel => 'Тип исходящей доставки';

  @override
  String get transportKindSmtp => 'SMTP';

  @override
  String get transportKindGmail => 'Gmail (Google)';

  @override
  String get gmailTransportPresetHelper =>
      'Использует smtp.gmail.com с OAuth (XOAUTH2). Сохраните транспорт и войдите тем же аккаунтом Google, что и для Gmail IMAP.';

  @override
  String get newTransport => 'Новый транспорт';

  @override
  String get editTransport => 'Изменить транспорт';

  @override
  String get displayNameLabel => 'Отображаемое имя';

  @override
  String get smtpHostLabel => 'Хост SMTP';

  @override
  String get smtpPortHelper => 'Обычно 587 (STARTTLS) или 465 (SMTPS)';

  @override
  String get imapSignInTitle => 'Вход IMAP';

  @override
  String get matrixSignInTitle => 'Вход Matrix';

  @override
  String get gmailSignInTitle => 'Войти через Google';

  @override
  String get gmailSignInBody =>
      'Браузер откроется для входа через Google и авторизации доступа к Gmail (IMAP).';

  @override
  String get gmailSignInBrowserButton => 'Продолжить в браузере';

  @override
  String get smtpSignInTitle => 'Вход SMTP';

  @override
  String smtpSignInSubtitle(String transportName, String host) {
    return 'Введите имя пользователя и пароль для «$transportName» ($host).';
  }

  @override
  String get composeSendCancelledNoSmtpCredentials =>
      'Сообщение не отправлено: учётные данные SMTP не сохранены.';

  @override
  String get enterUsernameAndPassword => 'Введите имя пользователя и пароль.';

  @override
  String get usernameLabel => 'Имя пользователя';

  @override
  String get passwordLabel => 'Пароль';

  @override
  String get showPasswordTooltip => 'Показать пароль';

  @override
  String get hidePasswordTooltip => 'Скрыть пароль';

  @override
  String get fieldFrom => 'От';

  @override
  String get composeOutgoingTransport => 'Исходящая доставка';

  @override
  String get composeSendSucceeded => 'Сообщение отправлено';

  @override
  String get composeMissingFrom => 'Введите адрес отправителя.';

  @override
  String get composeMissingTo => 'Укажите хотя бы одного получателя.';

  @override
  String get composeMissingNewsgroups =>
      'Укажите хотя бы одну группу новостей.';

  @override
  String get composeNntpPostingBlurb =>
      'Сообщения отправляются через NNTP-сервер этой учётной записи (отдельный транспорт не используется).';

  @override
  String get fieldNewsgroups => 'Группы новостей';

  @override
  String get fieldTo => 'Кому';

  @override
  String get fieldCc => 'Копия';

  @override
  String get fieldBcc => 'Скрытая копия';

  @override
  String get fieldSubject => 'Тема';

  @override
  String get fieldBody => 'Текст';

  @override
  String get attach => 'Вложить';

  @override
  String get composeRemoveAttachment => 'Удалить вложение';

  @override
  String get defaultFromLabel => 'Адрес отправителя по умолчанию';

  @override
  String get defaultFromHelper =>
      'напр. Ваше имя <you@example.com> или you@example.com';

  @override
  String get dsnLabel => 'Уведомления о доставке';

  @override
  String get dsnUseTransportDefault => 'По умолчанию для транспорта';

  @override
  String get dsnNever => 'Никогда';

  @override
  String get dsnFailure => 'При ошибке';

  @override
  String get dsnSuccess => 'При успехе';

  @override
  String get dsnDelay => 'При задержке';

  @override
  String get dsnFailureAndSuccess => 'При ошибке и успехе';

  @override
  String get dsnNotifyLabel => 'Уведомление DSN';

  @override
  String get composeCryptoLabel => 'Подпись / шифрование';

  @override
  String get composeCryptoTitle => 'Исходящая подпись и шифрование';

  @override
  String get composeCryptoNone => 'Без шифрования';

  @override
  String get composeCryptoSign => 'Подписать';

  @override
  String get composeCryptoEncrypt => 'Зашифровать';

  @override
  String get composeCryptoSignEncrypt => 'Подписать и зашифровать';

  @override
  String get settingsMailCryptoSection => 'Подпись почты (исходящие)';

  @override
  String get settingsMailCryptoStackSubtitle =>
      'Стек для исходящей подписи и шифрования (черновик/отправка).';

  @override
  String get settingsMailCryptoStackOpenpgp => 'OpenPGP';

  @override
  String get settingsMailCryptoStackSmime => 'S/MIME';

  @override
  String get settingsMailCryptoPgpSecretKeyPath =>
      'Каталог GnuPG (необязательно)';

  @override
  String get settingsMailCryptoPgpPassphrase =>
      'ID ключа или отпечаток ключа подписи OpenPGP';

  @override
  String get settingsMailCryptoSmimeCert =>
      'Сертификат подписи S/MIME (путь к PEM)';

  @override
  String get settingsMailCryptoSmimeKey =>
      'Закрытый ключ подписи S/MIME (путь к PEM)';

  @override
  String get folderNewSubfolder => 'Новая вложенная папка';

  @override
  String get folderRename => 'Переименовать…';

  @override
  String get folderDelete => 'Удалить…';

  @override
  String get folderNewTooltip => 'Новая папка';

  @override
  String get folderNewDialogTitle => 'Новая папка';

  @override
  String get folderNameLabel => 'Имя папки';

  @override
  String get folderNewTopLevelHelper =>
      'Создаёт почтовый ящик на верхнем уровне';

  @override
  String subfolderDialogTitle(String parent) {
    return 'Вложенная папка: $parent';
  }

  @override
  String get subfolderNameLabel => 'Имя вложенной папки';

  @override
  String subfolderPathHelper(String path) {
    return 'Путь: $path';
  }

  @override
  String folderCreated(String name) {
    return 'Создана папка «$name»';
  }

  @override
  String get renameFolderTitle => 'Переименовать папку';

  @override
  String get newFolderPathLabel => 'Новый путь папки';

  @override
  String get folderRenamed => 'Папка переименована';

  @override
  String get deleteFolderTitle => 'Удалить папку?';

  @override
  String deleteFolderBody(String name) {
    return 'Удалить «$name» и её письма с сервера (если поддерживается)? Это нельзя отменить.';
  }

  @override
  String get folderDeleted => 'Папка удалена';

  @override
  String get licenseTitle => 'Лицензия';

  @override
  String get copyrightTitle => 'Авторские права';

  @override
  String get chatHintTypeMessage => 'Введите сообщение';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Чат пока не может отправлять вложения. Удалите их, чтобы отправить сообщение, или используйте написание письма для файлов.';

  @override
  String operationFailed(String error) {
    return 'Что-то пошло не так: $error';
  }

  @override
  String get expandFolder => 'Развернуть';

  @override
  String get collapseFolder => 'Свернуть';

  @override
  String get noTextBody => '(Нет текстового тела)';

  @override
  String get matrixE2eeUndecryptableTitle =>
      'Это сообщение пока нельзя расшифровать';

  @override
  String get matrixE2eeUndecryptableHelp =>
      'Этот чат защищён сквозным шифрованием Matrix. У Tagliacarte нет ключей сессии комнаты для этого сообщения.\n\nЧто можно сделать:\n• В Element (или другом клиенте Matrix): Настройки → Безопасность → Защищённая резервная копия — разблокируйте ключом восстановления или парольной фразой. Если в Tagliacarte есть восстановление резервной копии ключей, используйте там тот же ключ.\n• На другом устройстве, где вы уже читали этот чат (например Element на телефоне или ПК): войдите, при запросе подтвердите эту сессию Tagliacarte, оставьте устройство в сети и откройте этот личный чат, чтобы ключи могли переслаться.\n• Когда устройства доверят друг другу, попросите собеседника отправить новое сообщение — это помогает только новым сообщениям; старым по-прежнему нужны ключи из резервной копии или с другого устройства.\n\nБез защищённой резервной копии и без другого вошедшего клиента старая зашифрованная история может остаться нечитаемой — так задумано в Matrix.';

  @override
  String get matrixE2eeUndecryptableListPreview =>
      'Пока нельзя расшифровать — откройте сообщение для шагов';

  @override
  String get matrixE2eeUndecryptableChatSnippet => 'Пока нельзя расшифровать';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'Переместить сюда';

  @override
  String get folderCopyHere => 'Копировать сюда';

  @override
  String get folderExpunge => 'Удалить помеченные навсегда';

  @override
  String get folderExpungeDone => 'Очистка выполнена';

  @override
  String get folderTabSubscribed => 'Подписки';

  @override
  String get folderTabAvailable => 'Доступные';

  @override
  String get matrixFolderTabRooms => 'Комнаты';

  @override
  String get matrixFolderTabDirectMessages => 'Личные сообщения';

  @override
  String get folderActionSubscribe => 'Подписаться';

  @override
  String get folderActionUnsubscribe => 'Отписаться';

  @override
  String get folderActionJoinRoom => 'Войти в комнату';

  @override
  String get folderActionLeaveRoom => 'Покинуть комнату';

  @override
  String get nntpWildmatHint => 'Шаблон (напр. comp.os.linux.*)';

  @override
  String get nntpWildmatQuery => 'Список';

  @override
  String pendingMoveTagged(int count) {
    return 'Выберите папку, затем «Переместить сюда» ($count писем)';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'Выберите папку, затем «Копировать сюда» ($count писем)';
  }

  @override
  String transferResultOk(int count) {
    return 'Готово: $count писем.';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok успешно, $failed с ошибкой.';
  }

  @override
  String transferFailed(String error) {
    return 'Ошибка переноса: $error';
  }

  @override
  String deleteMessagesFailed(String error) {
    return 'Не удалось удалить: $error';
  }

  @override
  String get settingsNotifyNewMessages => 'Уведомления о новых сообщениях';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'Snackbar при открытом приложении; системное уведомление в фоне (IMAP).';

  @override
  String get newMailNotificationTitle => 'Новая почта';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$count новых сообщ. в $folder';
  }

  @override
  String get accountImapMinIdleSecondsLabel => 'Мин. сек. тишины перед IDLE';

  @override
  String get accountImapMinIdleSecondsHelper =>
      'Пусто — по умолчанию (120). Минимум 15. После простоя соединения.';

  @override
  String get validationImapMinIdleSeconds => 'Целое от 15 до 864000 или пусто.';
}
