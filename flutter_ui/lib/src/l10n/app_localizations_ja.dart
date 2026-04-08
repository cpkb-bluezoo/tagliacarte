// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Japanese (`ja`).
class AppLocalizationsJa extends AppLocalizations {
  AppLocalizationsJa([String locale = 'ja']) : super(locale);

  @override
  String get appTitle => 'Tagliacarte';

  @override
  String get settings => '設定';

  @override
  String get compose => '作成';

  @override
  String get send => '送信';

  @override
  String get dialogOk => 'OK';

  @override
  String get cancel => 'キャンセル';

  @override
  String get save => '保存';

  @override
  String get remove => '削除';

  @override
  String get delete => '削除';

  @override
  String get discard => '破棄';

  @override
  String get back => '戻る';

  @override
  String get create => '作成';

  @override
  String get rename => '名前を変更';

  @override
  String get folderLabel => 'フォルダ';

  @override
  String get messageTitle => 'メッセージ';

  @override
  String get selectFolder => 'フォルダを選択';

  @override
  String get selectMessage => 'メッセージを選択';

  @override
  String get selectMessageToRead => '読むメッセージを選択してください。';

  @override
  String get noMessages => 'メッセージはありません';

  @override
  String get attachments => '添付ファイル';

  @override
  String get saveAttachment => '添付を保存';

  @override
  String savedToPath(String path) {
    return '$path に保存しました';
  }

  @override
  String saveFailed(String error) {
    return '保存に失敗しました: $error';
  }

  @override
  String get cannotDownloadAttachment => 'この添付はダウンロードできません';

  @override
  String get emptyAttachmentData => '添付データが空です';

  @override
  String downloadFailed(String error) {
    return 'ダウンロードに失敗しました: $error';
  }

  @override
  String get saveVerb => '保存';

  @override
  String get loadImages => '画像を読み込む';

  @override
  String get remoteImagesBlocked => 'プライバシーのためリモート画像をブロックしています。';

  @override
  String couldNotOpenHtmlBody(String error) {
    return 'HTML 本文を開けませんでした: $error';
  }

  @override
  String webViewError(String error) {
    return 'WebView エラー: $error';
  }

  @override
  String get linkHoverMisleadingCaption => '表示されているリンクの文字は、実際の移動先とは異なるアドレスです。';

  @override
  String get headerFrom => '差出人:';

  @override
  String get headerTo => '宛先:';

  @override
  String get headerCc => 'Cc:';

  @override
  String get headerDate => '日付:';

  @override
  String get folderInbox => '受信トレイ';

  @override
  String get messageActionReply => '返信';

  @override
  String get messageActionReplyAll => '全員に返信';

  @override
  String get messageActionForward => '転送';

  @override
  String get messageActionDelete => '削除';

  @override
  String get messageActionJunk => '迷惑メール';

  @override
  String get messageActionMove => '移動';

  @override
  String get messageActionCopy => 'コピー';

  @override
  String get messageMenuTooltip => 'メッセージ操作';

  @override
  String get settingsViewMinimalHeaders => 'メッセージヘッダーを最小表示';

  @override
  String get settingsViewMinimalHeadersSubtitle =>
      'オンにすると Cc のみ非表示。差出人・宛先・日付はある場合は表示されます。';

  @override
  String get settingsTooltip => '設定';

  @override
  String get accountsAndFoldersTooltip => 'アカウントとフォルダ';

  @override
  String get cancelSelectionTooltip => '選択を解除';

  @override
  String multiSelectCount(int count) {
    return '$count 件選択中';
  }

  @override
  String get composeTooltip => '作成';

  @override
  String get composeNeedTransportTooltip => '設定で送信（アウトバウンド）トランスポートを追加してください';

  @override
  String get mailToolbarMoreTooltip => 'その他';

  @override
  String mailToolbarSelectedCount(int count) {
    return '$count 件選択中';
  }

  @override
  String get settingsTabAccounts => 'アカウント';

  @override
  String get settingsTabOutgoing => '送信';

  @override
  String get settingsTabSecurity => 'セキュリティ';

  @override
  String get settingsTabViewing => '表示';

  @override
  String get settingsTabComposing => '作成';

  @override
  String get settingsTabAbout => 'このアプリについて';

  @override
  String get settingsLoadFailed => 'Could not load settings from disk.';

  @override
  String get settingsLoadRetry => 'Retry';

  @override
  String get useSystemKeychain => 'システムのキーチェーンを使う';

  @override
  String get storeCredentialsInKeychain => '認証情報をプラットフォームのキーチェーンに保存';

  @override
  String get oauthSection => 'OAuth';

  @override
  String get authenticateGoogle => 'Google で認証';

  @override
  String get authenticateMicrosoft => 'Microsoft で認証';

  @override
  String get reloadOAuthToken => 'OAuth トークンを再読み込み';

  @override
  String get matrixE2eeSection => 'Matrix エンドツーエンド暗号化';

  @override
  String get initCrypto => '暗号を初期化';

  @override
  String get setupBackup => 'バックアップを設定';

  @override
  String get restoreBackup => 'バックアップから復元';

  @override
  String get showDeviceFingerprint => 'デバイスフィンガープリントを表示';

  @override
  String get messageDetailInlineDesktopTitle => '一覧の下にメッセージ詳細（デスクトップ）';

  @override
  String get messageDetailInlineDesktopSubtitle =>
      'オフにすると、メッセージを開いたとき別の全画面表示になります。';

  @override
  String get loadRemoteImages => 'リモート画像を読み込む';

  @override
  String get loadRemoteImagesSubtitle => 'HTML メールの外部画像を許可';

  @override
  String get threadedView => 'スレッド表示';

  @override
  String get threadedViewSubtitle => '会話ごとにメールをグループ化';

  @override
  String get deletionAndTrashSection => '削除とゴミ箱';

  @override
  String get deletionAppliesGlobally => 'メール形式のアカウントすべてに適用されます。';

  @override
  String get deleteModeLabel => '削除モード';

  @override
  String get trashFolderNameLabel => 'ゴミ箱フォルダ名';

  @override
  String get junkFolderNameLabel => '迷惑メールフォルダ名';

  @override
  String get exchangeTrashFolderHelper =>
      'Leave empty to use “Deleted Items” (English mailbox). Use the exact folder name shown in Outlook if yours differs.';

  @override
  String get exchangeJunkFolderHelper =>
      'Leave empty to use “Junk Email” (English mailbox). Use the exact folder name shown in Outlook if yours differs.';

  @override
  String get deleteModeDeleteImmediately => 'すぐに削除';

  @override
  String get deleteModeMoveToTrash => 'ゴミ箱へ移動';

  @override
  String get deleteModeMarkDeleted => '削除済みとしてマーク';

  @override
  String get quoteOriginalOnReply => 'Quote original message on reply';

  @override
  String get quoteOriginalOnReplySubtitle =>
      'Adds the original under the reply header in new replies. Rich compose wraps it in a marked quote block; plain compose prefixes each line of the original. The text/plain part of the message still includes the original when this is on.';

  @override
  String get composingReplySection => 'Reply quoting';

  @override
  String get replyHeaderTemplateLabel => 'Reply header line';

  @override
  String get replyHeaderTemplateHelp =>
      'Shown above the quoted original. Include the three words date, time, and sender, each with a dollar sign immediately in front (see preview). They are replaced with the message’s date, time, and From when you reply.';

  @override
  String get replyHeaderPreviewLabel => 'Preview';

  @override
  String get replyDateFormatLabel => 'Reply date (in header)';

  @override
  String get replyTimeFormatLabel => 'Reply time (in header)';

  @override
  String get replyDatePresetLocale => 'Same as system (long date)';

  @override
  String get replyDatePresetIso => 'ISO: 2026-04-08';

  @override
  String get replyDatePresetUs => 'US: 04/08/2026';

  @override
  String get replyDatePresetEu => 'Day/month/year: 08/04/2026';

  @override
  String get replyDatePresetMedium => 'Medium: Apr 8, 2026';

  @override
  String get replyDatePresetWeekday => 'With weekday: Wed, Apr 8, 2026';

  @override
  String replyDatePresetCustom(String pattern) {
    return 'Custom ($pattern)';
  }

  @override
  String get replyTimePresetLocale => 'Same as system';

  @override
  String get replyTimePreset12h => '12-hour (e.g. 1:30 PM)';

  @override
  String get replyTimePreset24h => '24-hour (15:30)';

  @override
  String get replyTimePreset24hSeconds => '24-hour with seconds';

  @override
  String replyTimePresetCustom(String pattern) {
    return 'Custom ($pattern)';
  }

  @override
  String get replyLinePrefixLabel => 'Quoted line prefix';

  @override
  String get replyLinePrefixSubtitle =>
      'Prepended to each line of the original in plain-text quotes (classic “> ” quoting). Only used when quoting the original is enabled.';

  @override
  String get replyPlainPositionLabel => 'Ordering of quoted text';

  @override
  String get replyPlainPositionBefore => 'Reply before quoted text';

  @override
  String get replyPlainPositionAfter => 'Reply after quoted text';

  @override
  String get replyPlainPositionSubtitle =>
      'Plain or rich compose: two blank lines and caret before the reply header, or two blank lines and caret after the quoted block. The text/plain part when sending follows the same layout.';

  @override
  String get replyQuoteModeLabel => 'SMTP HTML parts';

  @override
  String get replyQuoteModePlain => 'Original only in plain-text quote';

  @override
  String get replyQuoteModeHtmlSmtp =>
      'Also include original as separate HTML (SMTP)';

  @override
  String get replyQuoteModeHtmlSmtpSubtitle =>
      'Adds a second HTML part preserving the source message’s formatting for HTML-capable clients. Plain-text-only clients still see the quoted plain body. NNTP posting always uses plain quoting.';

  @override
  String get settingsComposeRichText => 'メール作成時のリッチテキスト';

  @override
  String get settingsComposeRichTextSubtitle =>
      '新規メールと返信に書式付きエディターを使用。Usenet（NNTP）の投稿は常にプレーンテキスト。';

  @override
  String get settingsMatrixChatRichText => 'Matrix チャットのリッチテキスト';

  @override
  String get settingsMatrixChatRichTextSubtitle =>
      'Matrix ルームで書式付きメッセージを送信（プレーンテキストのフォールバック付き）。';

  @override
  String get testSend => 'テスト送信';

  @override
  String get openSignatureEditor => '署名エディタを開く';

  @override
  String get aboutSubtitle => 'クロスプラットフォームのメールとメッセージング';

  @override
  String get supportedBackends => '対応バックエンド';

  @override
  String get supportedBackendsList => 'IMAP、POP3、SMTP、NNTP、Matrix、Nostr、Graph';

  @override
  String get licenseGpl => 'GPLv3';

  @override
  String get copyrightLine => 'Copyright (C) 2026 Chris Burdess';

  @override
  String stubInvoked(String operation) {
    return '$operation を呼び出しました';
  }

  @override
  String get accountTypeDialogTitle => 'アカウントの種類';

  @override
  String get removeAccountTitle => 'アカウントを削除しますか？';

  @override
  String removeAccountBody(String label) {
    return 'この端末に保存された設定から「$label」を削除しますか？';
  }

  @override
  String removedAccount(String label) {
    return '「$label」を削除しました';
  }

  @override
  String get accountsListTitle => 'アカウント';

  @override
  String get accountsListSubtitle => 'アカウントをタップして編集するか、下から追加してください。';

  @override
  String get deleteTooltip => '削除';

  @override
  String get addAccount => 'アカウントを追加';

  @override
  String get noAccountsYet => 'アカウントはまだありません。「アカウントを追加」から作成してください。';

  @override
  String get discardChangesTitle => '変更を破棄しますか？';

  @override
  String get discardChangesBody => '編集内容は失われます（保存せずに終了したのと同じです）。';

  @override
  String get keepEditing => '編集を続ける';

  @override
  String get pickNotSupportedWeb => 'Web ビルドではファイルやフォルダの選択はできません';

  @override
  String get chooseMaildirFolderTitle => 'Maildir のルートフォルダを選択';

  @override
  String get chooseMboxFileTitle => 'mbox ファイルを選択';

  @override
  String get validationAccountNameRequired => 'アカウント名が必要です';

  @override
  String get validationLocalPathRequired => 'ローカルメールボックスのパスが必要です';

  @override
  String get validationUsernameRequired => 'このアカウント種別ではユーザー名 / メールが必要です';

  @override
  String get validationEmailAddressRequired => 'メールアドレスが必要です';

  @override
  String get validationMatrixUserIdRequired => 'Matrix ユーザー ID が必要です';

  @override
  String get accountEmailAddressLabel => 'メールアドレス';

  @override
  String get accountMatrixUserIdLabel => 'Matrix ID（MXID）';

  @override
  String get accountMatrixMxidHelper =>
      '例: @you:matrix.org — コロン以降のドメインからホームサーバーURLを導出します。';

  @override
  String get validationMatrixMxidInvalid =>
      '@user:server の形式の Matrix ID を入力してください';

  @override
  String get accountNntpDefaultFromLabel => '既定の From（Usenet）';

  @override
  String get accountNntpDefaultFromHelper =>
      '投稿作成時に表示されます。この NNTP アカウントは独自のサーバー接続で投稿します。';

  @override
  String get accountEmailOptionalLabel => 'メールアドレス（任意）';

  @override
  String get accountTcpLoginHelper => 'このサーバーへのサインインに使う識別子（多くはメールアドレス）。';

  @override
  String get validationHostRequired => 'サーバーホストが必要です';

  @override
  String get validationPortRequired => '有効なポート番号が必要です';

  @override
  String get accountSaved => 'アカウントを保存しました';

  @override
  String get createTransportFirst => '先に「送信」タブでトランスポートを作成してください';

  @override
  String get addTransportDialogTitle => 'トランスポートを追加';

  @override
  String get accountTypeLabel => 'アカウントの種類';

  @override
  String get accountTypeHelper => '追加時に選びます。ここでは変更できません。';

  @override
  String get accountNameLabel => 'アカウント名';

  @override
  String get usernameEmailOptional => 'ユーザー名 / メール（任意）';

  @override
  String get usernameEmailRequired => 'ユーザー名 / メール';

  @override
  String get avatarUrlLabel => 'アバター URL またはファイルパス（任意）';

  @override
  String get avatarUrlHelper => 'アカウントストリップ用の画像 URL またはローカルパス（任意）';

  @override
  String get localMailboxSection => 'ローカルメールボックス';

  @override
  String get pathMboxFile => 'mbox ファイルへのパス';

  @override
  String get pathMaildirRoot => 'Maildir ルートへのパス';

  @override
  String get helperMboxPath => 'ファイルボタンで参照するか、絶対パスを入力';

  @override
  String get helperMaildirPath => 'フォルダボタンで参照するか、絶対パスを入力';

  @override
  String get chooseMboxTooltip => 'mbox ファイルを選択';

  @override
  String get chooseMaildirTooltip => 'Maildir フォルダを選択';

  @override
  String get imapServerSection => 'IMAP サーバー';

  @override
  String get pop3ServerSection => 'POP3 サーバー';

  @override
  String get nntpServerSection => 'NNTP サーバー';

  @override
  String get hostLabel => 'ホスト';

  @override
  String get serverHostLabel => 'サーバーホスト';

  @override
  String get portLabel => 'ポート';

  @override
  String get portHelperImap => '通常 993（IMAPS）または 143（STARTTLS）';

  @override
  String get portHelperPop3 => '通常 995（POP3S、暗黙の TLS）';

  @override
  String get portHelperNntp => '通常 563（NNTPS、暗黙の TLS）';

  @override
  String get securityLabel => 'セキュリティ';

  @override
  String get mailSecurityImplicitTlsImap => 'IMAPS（暗黙の TLS）';

  @override
  String get mailSecurityImplicitTlsSmtp => 'SMTPS（暗黙の TLS）';

  @override
  String get mailSecurityImplicitTlsPop3 => 'POP3S（暗黙の TLS）';

  @override
  String get mailSecurityImplicitTlsNntp => 'NNTPS（暗黙の TLS）';

  @override
  String get mailSecurityStarttls => 'STARTTLS';

  @override
  String get mailSecurityNoEncryption => '暗号化なし';

  @override
  String get outgoingTransportsSection => '送信トランスポート';

  @override
  String get noTransportsHintLinked =>
      'トランスポートが未選択です。少なくとも 1 つ選ぶまで作成・返信は無効です。「送信」タブで作成し、ここで選択してください。';

  @override
  String get transportsOrderHint => 'リストの先頭が送信の既定です。送信タブでトランスポートを作成してください。';

  @override
  String get unknownTransport => '不明なトランスポート';

  @override
  String get moveUpTooltip => '上へ';

  @override
  String get moveDownTooltip => '下へ';

  @override
  String get removeFromAccountTooltip => 'アカウントから外す';

  @override
  String get addTransportToAccount => 'アカウントにトランスポートを追加';

  @override
  String get matrixSection => 'Matrix';

  @override
  String get homeserverLabel => 'ホームサーバー';

  @override
  String get nostrSection => 'Nostr';

  @override
  String get relayUrlsLabel => 'リレー URL';

  @override
  String get relayUrlsHelper =>
      '各行に1つのリレー WebSocket URL を入力します。編集が終わったら Enter を押してください。';

  @override
  String get relayAddFieldHint => '新しいリレー URL';

  @override
  String get relayAddTooltip => 'リレーを追加';

  @override
  String get relayRemoveTooltip => 'リレーを削除';

  @override
  String get nostrNewIdentityTooltip => '新しい Nostr ID を作成';

  @override
  String get nostrRelayUrlsRequired => 'リレー URL を1つ以上入力してください。';

  @override
  String storeUriLabel(String uri) {
    return '接続: $uri';
  }

  @override
  String transportUriLabel(String uri) {
    return '旧形式の送信 URI: $uri';
  }

  @override
  String accountDetailTitleNew(String type) {
    return '新規 $type';
  }

  @override
  String accountDetailTitleEdit(String label) {
    return '$label を編集';
  }

  @override
  String foldersLoadError(String error) {
    return 'フォルダ: $error';
  }

  @override
  String get sortMessagesTooltip => 'メッセージを並べ替え';

  @override
  String get sort => '並べ替え';

  @override
  String get sortFromAz => '差出人 A→Z';

  @override
  String get sortFromZa => '差出人 Z→A';

  @override
  String get sortSubjectAz => '件名 A→Z';

  @override
  String get sortSubjectZa => '件名 Z→A';

  @override
  String get sortDateOldest => '日付: 古い順';

  @override
  String get sortDateNewest => '日付: 新しい順';

  @override
  String get removeTransportTitle => 'トランスポートを削除しますか？';

  @override
  String removeTransportBody(String name) {
    return '「$name」はすべてのアカウントの送信リストから削除されます。';
  }

  @override
  String removedTransport(String name) {
    return '「$name」を削除しました';
  }

  @override
  String get outgoingListTitle => '送信';

  @override
  String get outgoingListSubtitle =>
      'SMTP などの送信トランスポート。「アカウント」タブでメールアカウントに関連付けます。';

  @override
  String get addTransport => 'トランスポートを追加';

  @override
  String get noTransportsYet => '送信トランスポートはまだありません。「トランスポートを追加」から作成してください。';

  @override
  String get transportDisplayHostRequired => '表示名とホストが必要です。';

  @override
  String get transportSaved => 'トランスポートを保存しました';

  @override
  String get transportSavedAndVerified => 'トランスポートを保存し、SMTPを確認しました';

  @override
  String get transportSavedVerifyPending =>
      'トランスポートは保存されましたが、サーバーに接続できないか認証に失敗しました。ホスト・セキュリティ・認証情報を確認し、再度保存してください。';

  @override
  String get transportTypeDialogTitle => '送信トランスポートの種類';

  @override
  String get transportTypeFixedHelper => '追加時に選びます。ここでは変更できません。';

  @override
  String get transportDisplayNameRequired => '表示名が必要です。';

  @override
  String get transportKindLabel => '送信の種類';

  @override
  String get transportKindSmtp => 'SMTP';

  @override
  String get transportKindGmail => 'Gmail (Google)';

  @override
  String get gmailTransportPresetHelper =>
      'smtp.gmail.com で OAuth (XOAUTH2) を使います。トランスポートを保存し、Gmail IMAP と同じ Google アカウントでサインインしてください。';

  @override
  String get newTransport => '新規トランスポート';

  @override
  String get editTransport => 'トランスポートを編集';

  @override
  String get displayNameLabel => '表示名';

  @override
  String get smtpHostLabel => 'SMTP ホスト';

  @override
  String get smtpPortHelper => '通常 587（STARTTLS）または 465（SMTPS）';

  @override
  String get imapSignInTitle => 'IMAP サインイン';

  @override
  String get matrixSignInTitle => 'Matrix サインイン';

  @override
  String get gmailSignInTitle => 'Google でサインイン';

  @override
  String get gmailSignInBody =>
      'ブラウザが開き Gmail（IMAP）の認可を行います。TAGLIACARTE_GOOGLE_CLIENT_ID（通常は TAGLIACARTE_GOOGLE_CLIENT_SECRET も）が必要です。';

  @override
  String get gmailSignInBrowserButton => 'ブラウザで続行';

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
  String get enterUsernameAndPassword => 'ユーザー名とパスワードを入力してください。';

  @override
  String get usernameLabel => 'ユーザー名';

  @override
  String get passwordLabel => 'パスワード';

  @override
  String get showPasswordTooltip => 'パスワードを表示';

  @override
  String get hidePasswordTooltip => 'パスワードを隠す';

  @override
  String get fieldFrom => '差出人';

  @override
  String get composeOutgoingTransport => '送信経路';

  @override
  String get composeSendSucceeded => '送信しました';

  @override
  String get composeMissingFrom => '送信元アドレスを入力してください。';

  @override
  String get composeMissingTo => '宛先を1件以上入力してください。';

  @override
  String get composeMissingNewsgroups => 'ニュースグループを1つ以上入力してください。';

  @override
  String get composeNntpPostingBlurb =>
      '投稿はこのアカウントの NNTP サーバー経由です（別の送信経路はありません）。';

  @override
  String get fieldNewsgroups => 'ニュースグループ';

  @override
  String get fieldTo => '宛先';

  @override
  String get fieldCc => 'Cc';

  @override
  String get fieldBcc => 'Bcc';

  @override
  String get fieldSubject => '件名';

  @override
  String get fieldBody => '本文';

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
  String get folderNewSubfolder => '新しいサブフォルダ';

  @override
  String get folderRename => '名前を変更…';

  @override
  String get folderDelete => '削除…';

  @override
  String get folderNewTooltip => '新しいフォルダ';

  @override
  String get folderNewDialogTitle => '新しいフォルダ';

  @override
  String get folderNameLabel => 'フォルダ名';

  @override
  String get folderNewTopLevelHelper => 'トップレベルにメールボックスを作成します';

  @override
  String subfolderDialogTitle(String parent) {
    return '$parent のサブフォルダ';
  }

  @override
  String get subfolderNameLabel => 'サブフォルダ名';

  @override
  String subfolderPathHelper(String path) {
    return 'パス: $path';
  }

  @override
  String folderCreated(String name) {
    return 'フォルダ「$name」を作成しました';
  }

  @override
  String get renameFolderTitle => 'フォルダ名を変更';

  @override
  String get newFolderPathLabel => '新しいフォルダパス';

  @override
  String get folderRenamed => 'フォルダ名を変更しました';

  @override
  String get deleteFolderTitle => 'フォルダを削除しますか？';

  @override
  String deleteFolderBody(String name) {
    return 'サーバーから「$name」とそのメッセージを削除しますか（対応している場合）？元に戻せません。';
  }

  @override
  String get folderDeleted => 'フォルダを削除しました';

  @override
  String get licenseTitle => 'ライセンス';

  @override
  String get copyrightTitle => '著作権';

  @override
  String get chatHintTypeMessage => 'メッセージを入力';

  @override
  String get chatAttachmentsNotSentInChat =>
      'Chat cannot send file attachments yet. Remove them to send your message, or use mail compose for files.';

  @override
  String operationFailed(String error) {
    return '問題が発生しました: $error';
  }

  @override
  String get expandFolder => '展開';

  @override
  String get collapseFolder => '折りたたむ';

  @override
  String get noTextBody => '（テキスト本文なし）';

  @override
  String messageActionFeedback(String label, String messageId) {
    return '$label · $messageId';
  }

  @override
  String get folderMoveHere => 'ここへ移動';

  @override
  String get folderCopyHere => 'ここへコピー';

  @override
  String get folderExpunge => '削除済みを完全削除';

  @override
  String get folderExpungeDone => '完全削除しました';

  @override
  String pendingMoveTagged(int count) {
    return 'フォルダを選び「ここへ移動」（$count 件）';
  }

  @override
  String pendingCopyTagged(int count) {
    return 'フォルダを選び「ここへコピー」（$count 件）';
  }

  @override
  String transferResultOk(int count) {
    return '完了: $count 件。';
  }

  @override
  String transferResultMixed(int ok, int failed) {
    return '$ok 件成功、$failed 件失敗。';
  }

  @override
  String transferFailed(String error) {
    return '転送に失敗: $error';
  }

  @override
  String deleteMessagesFailed(String error) {
    return '削除に失敗しました: $error';
  }

  @override
  String get settingsNotifyNewMessages => '新着メールの通知';

  @override
  String get settingsNotifyNewMessagesSubtitle =>
      'アプリ表示中はスナックバー、バックグラウンドではシステム通知（IMAP）。';

  @override
  String get newMailNotificationTitle => '新着メール';

  @override
  String newMailNotificationBody(int count, String folder) {
    return '$folder に新しいメッセージが $count 件';
  }

  @override
  String get accountImapMinIdleSecondsLabel => 'IDLE 前の無通信秒数（最小）';

  @override
  String get accountImapMinIdleSecondsHelper =>
      '空欄で既定（120）。最小 15。接続が無通信のときに適用。';

  @override
  String get validationImapMinIdleSeconds => '15〜864000 の整数、または空欄。';
}
