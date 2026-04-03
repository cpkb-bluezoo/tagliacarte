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
  String get settingsTabAbout => 'О программе';

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
  String get deleteModeMoveToTrash => 'В корзину';

  @override
  String get deleteModeMarkDeleted => 'Пометить удалённым';

  @override
  String get quoteOriginalOnReply => 'Цитировать исходное письмо в ответе';

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
  String get accountTypeHelper => 'Тип нельзя изменить при редактировании';

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
  String get relayUrlsLabel => 'URL релеев (через запятую)';

  @override
  String storeUriLabel(String uri) {
    return 'URI хранилища: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return 'URI транспорта: $uri';
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
}
