// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'commands.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

/// @nodoc
mixin _$AppCommand {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )
    markRead,
    required TResult Function(String accountId) refreshFolders,
    required TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )
    transferMessages,
    required TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )
    sendChatMessage,
    required TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )
    listMessagesWindow,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult? Function(String accountId)? refreshFolders,
    TResult? Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult? Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult? Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult Function(String accountId)? refreshFolders,
    TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppCommand_MarkRead value) markRead,
    required TResult Function(AppCommand_RefreshFolders value) refreshFolders,
    required TResult Function(AppCommand_TransferMessages value)
    transferMessages,
    required TResult Function(AppCommand_SendChatMessage value) sendChatMessage,
    required TResult Function(AppCommand_ListMessagesWindow value)
    listMessagesWindow,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppCommand_MarkRead value)? markRead,
    TResult? Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult? Function(AppCommand_TransferMessages value)? transferMessages,
    TResult? Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult? Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppCommand_MarkRead value)? markRead,
    TResult Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult Function(AppCommand_TransferMessages value)? transferMessages,
    TResult Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $AppCommandCopyWith<$Res> {
  factory $AppCommandCopyWith(
    AppCommand value,
    $Res Function(AppCommand) then,
  ) = _$AppCommandCopyWithImpl<$Res, AppCommand>;
}

/// @nodoc
class _$AppCommandCopyWithImpl<$Res, $Val extends AppCommand>
    implements $AppCommandCopyWith<$Res> {
  _$AppCommandCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$AppCommand_MarkReadImplCopyWith<$Res> {
  factory _$$AppCommand_MarkReadImplCopyWith(
    _$AppCommand_MarkReadImpl value,
    $Res Function(_$AppCommand_MarkReadImpl) then,
  ) = __$$AppCommand_MarkReadImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String accountId,
    String folder,
    String messageId,
    String? requestId,
  });
}

/// @nodoc
class __$$AppCommand_MarkReadImplCopyWithImpl<$Res>
    extends _$AppCommandCopyWithImpl<$Res, _$AppCommand_MarkReadImpl>
    implements _$$AppCommand_MarkReadImplCopyWith<$Res> {
  __$$AppCommand_MarkReadImplCopyWithImpl(
    _$AppCommand_MarkReadImpl _value,
    $Res Function(_$AppCommand_MarkReadImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? folder = null,
    Object? messageId = null,
    Object? requestId = freezed,
  }) {
    return _then(
      _$AppCommand_MarkReadImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folder: null == folder
            ? _value.folder
            : folder // ignore: cast_nullable_to_non_nullable
                  as String,
        messageId: null == messageId
            ? _value.messageId
            : messageId // ignore: cast_nullable_to_non_nullable
                  as String,
        requestId: freezed == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AppCommand_MarkReadImpl extends AppCommand_MarkRead {
  const _$AppCommand_MarkReadImpl({
    required this.accountId,
    required this.folder,
    required this.messageId,
    this.requestId,
  }) : super._();

  @override
  final String accountId;
  @override
  final String folder;
  @override
  final String messageId;
  @override
  final String? requestId;

  @override
  String toString() {
    return 'AppCommand.markRead(accountId: $accountId, folder: $folder, messageId: $messageId, requestId: $requestId)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppCommand_MarkReadImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folder, folder) || other.folder == folder) &&
            (identical(other.messageId, messageId) ||
                other.messageId == messageId) &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, accountId, folder, messageId, requestId);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppCommand_MarkReadImplCopyWith<_$AppCommand_MarkReadImpl> get copyWith =>
      __$$AppCommand_MarkReadImplCopyWithImpl<_$AppCommand_MarkReadImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )
    markRead,
    required TResult Function(String accountId) refreshFolders,
    required TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )
    transferMessages,
    required TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )
    sendChatMessage,
    required TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )
    listMessagesWindow,
  }) {
    return markRead(accountId, folder, messageId, requestId);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult? Function(String accountId)? refreshFolders,
    TResult? Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult? Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult? Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
  }) {
    return markRead?.call(accountId, folder, messageId, requestId);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult Function(String accountId)? refreshFolders,
    TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
    required TResult orElse(),
  }) {
    if (markRead != null) {
      return markRead(accountId, folder, messageId, requestId);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppCommand_MarkRead value) markRead,
    required TResult Function(AppCommand_RefreshFolders value) refreshFolders,
    required TResult Function(AppCommand_TransferMessages value)
    transferMessages,
    required TResult Function(AppCommand_SendChatMessage value) sendChatMessage,
    required TResult Function(AppCommand_ListMessagesWindow value)
    listMessagesWindow,
  }) {
    return markRead(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppCommand_MarkRead value)? markRead,
    TResult? Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult? Function(AppCommand_TransferMessages value)? transferMessages,
    TResult? Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult? Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
  }) {
    return markRead?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppCommand_MarkRead value)? markRead,
    TResult Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult Function(AppCommand_TransferMessages value)? transferMessages,
    TResult Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
    required TResult orElse(),
  }) {
    if (markRead != null) {
      return markRead(this);
    }
    return orElse();
  }
}

abstract class AppCommand_MarkRead extends AppCommand {
  const factory AppCommand_MarkRead({
    required final String accountId,
    required final String folder,
    required final String messageId,
    final String? requestId,
  }) = _$AppCommand_MarkReadImpl;
  const AppCommand_MarkRead._() : super._();

  String get accountId;
  String get folder;
  String get messageId;
  String? get requestId;

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppCommand_MarkReadImplCopyWith<_$AppCommand_MarkReadImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppCommand_RefreshFoldersImplCopyWith<$Res> {
  factory _$$AppCommand_RefreshFoldersImplCopyWith(
    _$AppCommand_RefreshFoldersImpl value,
    $Res Function(_$AppCommand_RefreshFoldersImpl) then,
  ) = __$$AppCommand_RefreshFoldersImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String accountId});
}

/// @nodoc
class __$$AppCommand_RefreshFoldersImplCopyWithImpl<$Res>
    extends _$AppCommandCopyWithImpl<$Res, _$AppCommand_RefreshFoldersImpl>
    implements _$$AppCommand_RefreshFoldersImplCopyWith<$Res> {
  __$$AppCommand_RefreshFoldersImplCopyWithImpl(
    _$AppCommand_RefreshFoldersImpl _value,
    $Res Function(_$AppCommand_RefreshFoldersImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? accountId = null}) {
    return _then(
      _$AppCommand_RefreshFoldersImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$AppCommand_RefreshFoldersImpl extends AppCommand_RefreshFolders {
  const _$AppCommand_RefreshFoldersImpl({required this.accountId}) : super._();

  @override
  final String accountId;

  @override
  String toString() {
    return 'AppCommand.refreshFolders(accountId: $accountId)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppCommand_RefreshFoldersImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId));
  }

  @override
  int get hashCode => Object.hash(runtimeType, accountId);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppCommand_RefreshFoldersImplCopyWith<_$AppCommand_RefreshFoldersImpl>
  get copyWith =>
      __$$AppCommand_RefreshFoldersImplCopyWithImpl<
        _$AppCommand_RefreshFoldersImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )
    markRead,
    required TResult Function(String accountId) refreshFolders,
    required TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )
    transferMessages,
    required TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )
    sendChatMessage,
    required TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )
    listMessagesWindow,
  }) {
    return refreshFolders(accountId);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult? Function(String accountId)? refreshFolders,
    TResult? Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult? Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult? Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
  }) {
    return refreshFolders?.call(accountId);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult Function(String accountId)? refreshFolders,
    TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
    required TResult orElse(),
  }) {
    if (refreshFolders != null) {
      return refreshFolders(accountId);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppCommand_MarkRead value) markRead,
    required TResult Function(AppCommand_RefreshFolders value) refreshFolders,
    required TResult Function(AppCommand_TransferMessages value)
    transferMessages,
    required TResult Function(AppCommand_SendChatMessage value) sendChatMessage,
    required TResult Function(AppCommand_ListMessagesWindow value)
    listMessagesWindow,
  }) {
    return refreshFolders(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppCommand_MarkRead value)? markRead,
    TResult? Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult? Function(AppCommand_TransferMessages value)? transferMessages,
    TResult? Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult? Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
  }) {
    return refreshFolders?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppCommand_MarkRead value)? markRead,
    TResult Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult Function(AppCommand_TransferMessages value)? transferMessages,
    TResult Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
    required TResult orElse(),
  }) {
    if (refreshFolders != null) {
      return refreshFolders(this);
    }
    return orElse();
  }
}

abstract class AppCommand_RefreshFolders extends AppCommand {
  const factory AppCommand_RefreshFolders({required final String accountId}) =
      _$AppCommand_RefreshFoldersImpl;
  const AppCommand_RefreshFolders._() : super._();

  String get accountId;

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppCommand_RefreshFoldersImplCopyWith<_$AppCommand_RefreshFoldersImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppCommand_TransferMessagesImplCopyWith<$Res> {
  factory _$$AppCommand_TransferMessagesImplCopyWith(
    _$AppCommand_TransferMessagesImpl value,
    $Res Function(_$AppCommand_TransferMessagesImpl) then,
  ) = __$$AppCommand_TransferMessagesImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String sourceAccountId,
    String sourceFolder,
    String destAccountId,
    String destFolder,
    List<String> messageIds,
    bool isMove,
    String? requestId,
  });
}

/// @nodoc
class __$$AppCommand_TransferMessagesImplCopyWithImpl<$Res>
    extends _$AppCommandCopyWithImpl<$Res, _$AppCommand_TransferMessagesImpl>
    implements _$$AppCommand_TransferMessagesImplCopyWith<$Res> {
  __$$AppCommand_TransferMessagesImplCopyWithImpl(
    _$AppCommand_TransferMessagesImpl _value,
    $Res Function(_$AppCommand_TransferMessagesImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? sourceAccountId = null,
    Object? sourceFolder = null,
    Object? destAccountId = null,
    Object? destFolder = null,
    Object? messageIds = null,
    Object? isMove = null,
    Object? requestId = freezed,
  }) {
    return _then(
      _$AppCommand_TransferMessagesImpl(
        sourceAccountId: null == sourceAccountId
            ? _value.sourceAccountId
            : sourceAccountId // ignore: cast_nullable_to_non_nullable
                  as String,
        sourceFolder: null == sourceFolder
            ? _value.sourceFolder
            : sourceFolder // ignore: cast_nullable_to_non_nullable
                  as String,
        destAccountId: null == destAccountId
            ? _value.destAccountId
            : destAccountId // ignore: cast_nullable_to_non_nullable
                  as String,
        destFolder: null == destFolder
            ? _value.destFolder
            : destFolder // ignore: cast_nullable_to_non_nullable
                  as String,
        messageIds: null == messageIds
            ? _value._messageIds
            : messageIds // ignore: cast_nullable_to_non_nullable
                  as List<String>,
        isMove: null == isMove
            ? _value.isMove
            : isMove // ignore: cast_nullable_to_non_nullable
                  as bool,
        requestId: freezed == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AppCommand_TransferMessagesImpl extends AppCommand_TransferMessages {
  const _$AppCommand_TransferMessagesImpl({
    required this.sourceAccountId,
    required this.sourceFolder,
    required this.destAccountId,
    required this.destFolder,
    required final List<String> messageIds,
    required this.isMove,
    this.requestId,
  }) : _messageIds = messageIds,
       super._();

  @override
  final String sourceAccountId;
  @override
  final String sourceFolder;
  @override
  final String destAccountId;
  @override
  final String destFolder;
  final List<String> _messageIds;
  @override
  List<String> get messageIds {
    if (_messageIds is EqualUnmodifiableListView) return _messageIds;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_messageIds);
  }

  @override
  final bool isMove;
  @override
  final String? requestId;

  @override
  String toString() {
    return 'AppCommand.transferMessages(sourceAccountId: $sourceAccountId, sourceFolder: $sourceFolder, destAccountId: $destAccountId, destFolder: $destFolder, messageIds: $messageIds, isMove: $isMove, requestId: $requestId)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppCommand_TransferMessagesImpl &&
            (identical(other.sourceAccountId, sourceAccountId) ||
                other.sourceAccountId == sourceAccountId) &&
            (identical(other.sourceFolder, sourceFolder) ||
                other.sourceFolder == sourceFolder) &&
            (identical(other.destAccountId, destAccountId) ||
                other.destAccountId == destAccountId) &&
            (identical(other.destFolder, destFolder) ||
                other.destFolder == destFolder) &&
            const DeepCollectionEquality().equals(
              other._messageIds,
              _messageIds,
            ) &&
            (identical(other.isMove, isMove) || other.isMove == isMove) &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    sourceAccountId,
    sourceFolder,
    destAccountId,
    destFolder,
    const DeepCollectionEquality().hash(_messageIds),
    isMove,
    requestId,
  );

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppCommand_TransferMessagesImplCopyWith<_$AppCommand_TransferMessagesImpl>
  get copyWith =>
      __$$AppCommand_TransferMessagesImplCopyWithImpl<
        _$AppCommand_TransferMessagesImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )
    markRead,
    required TResult Function(String accountId) refreshFolders,
    required TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )
    transferMessages,
    required TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )
    sendChatMessage,
    required TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )
    listMessagesWindow,
  }) {
    return transferMessages(
      sourceAccountId,
      sourceFolder,
      destAccountId,
      destFolder,
      messageIds,
      isMove,
      requestId,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult? Function(String accountId)? refreshFolders,
    TResult? Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult? Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult? Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
  }) {
    return transferMessages?.call(
      sourceAccountId,
      sourceFolder,
      destAccountId,
      destFolder,
      messageIds,
      isMove,
      requestId,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult Function(String accountId)? refreshFolders,
    TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
    required TResult orElse(),
  }) {
    if (transferMessages != null) {
      return transferMessages(
        sourceAccountId,
        sourceFolder,
        destAccountId,
        destFolder,
        messageIds,
        isMove,
        requestId,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppCommand_MarkRead value) markRead,
    required TResult Function(AppCommand_RefreshFolders value) refreshFolders,
    required TResult Function(AppCommand_TransferMessages value)
    transferMessages,
    required TResult Function(AppCommand_SendChatMessage value) sendChatMessage,
    required TResult Function(AppCommand_ListMessagesWindow value)
    listMessagesWindow,
  }) {
    return transferMessages(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppCommand_MarkRead value)? markRead,
    TResult? Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult? Function(AppCommand_TransferMessages value)? transferMessages,
    TResult? Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult? Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
  }) {
    return transferMessages?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppCommand_MarkRead value)? markRead,
    TResult Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult Function(AppCommand_TransferMessages value)? transferMessages,
    TResult Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
    required TResult orElse(),
  }) {
    if (transferMessages != null) {
      return transferMessages(this);
    }
    return orElse();
  }
}

abstract class AppCommand_TransferMessages extends AppCommand {
  const factory AppCommand_TransferMessages({
    required final String sourceAccountId,
    required final String sourceFolder,
    required final String destAccountId,
    required final String destFolder,
    required final List<String> messageIds,
    required final bool isMove,
    final String? requestId,
  }) = _$AppCommand_TransferMessagesImpl;
  const AppCommand_TransferMessages._() : super._();

  String get sourceAccountId;
  String get sourceFolder;
  String get destAccountId;
  String get destFolder;
  List<String> get messageIds;
  bool get isMove;
  String? get requestId;

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppCommand_TransferMessagesImplCopyWith<_$AppCommand_TransferMessagesImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppCommand_SendChatMessageImplCopyWith<$Res> {
  factory _$$AppCommand_SendChatMessageImplCopyWith(
    _$AppCommand_SendChatMessageImpl value,
    $Res Function(_$AppCommand_SendChatMessageImpl) then,
  ) = __$$AppCommand_SendChatMessageImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String accountId,
    String folder,
    String text,
    String? bodyHtml,
    String? requestId,
  });
}

/// @nodoc
class __$$AppCommand_SendChatMessageImplCopyWithImpl<$Res>
    extends _$AppCommandCopyWithImpl<$Res, _$AppCommand_SendChatMessageImpl>
    implements _$$AppCommand_SendChatMessageImplCopyWith<$Res> {
  __$$AppCommand_SendChatMessageImplCopyWithImpl(
    _$AppCommand_SendChatMessageImpl _value,
    $Res Function(_$AppCommand_SendChatMessageImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? folder = null,
    Object? text = null,
    Object? bodyHtml = freezed,
    Object? requestId = freezed,
  }) {
    return _then(
      _$AppCommand_SendChatMessageImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folder: null == folder
            ? _value.folder
            : folder // ignore: cast_nullable_to_non_nullable
                  as String,
        text: null == text
            ? _value.text
            : text // ignore: cast_nullable_to_non_nullable
                  as String,
        bodyHtml: freezed == bodyHtml
            ? _value.bodyHtml
            : bodyHtml // ignore: cast_nullable_to_non_nullable
                  as String?,
        requestId: freezed == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AppCommand_SendChatMessageImpl extends AppCommand_SendChatMessage {
  const _$AppCommand_SendChatMessageImpl({
    required this.accountId,
    required this.folder,
    required this.text,
    this.bodyHtml,
    this.requestId,
  }) : super._();

  @override
  final String accountId;
  @override
  final String folder;
  @override
  final String text;

  /// Optional HTML (Matrix rich text). Ignored for Nostr.
  @override
  final String? bodyHtml;
  @override
  final String? requestId;

  @override
  String toString() {
    return 'AppCommand.sendChatMessage(accountId: $accountId, folder: $folder, text: $text, bodyHtml: $bodyHtml, requestId: $requestId)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppCommand_SendChatMessageImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folder, folder) || other.folder == folder) &&
            (identical(other.text, text) || other.text == text) &&
            (identical(other.bodyHtml, bodyHtml) ||
                other.bodyHtml == bodyHtml) &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, accountId, folder, text, bodyHtml, requestId);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppCommand_SendChatMessageImplCopyWith<_$AppCommand_SendChatMessageImpl>
  get copyWith =>
      __$$AppCommand_SendChatMessageImplCopyWithImpl<
        _$AppCommand_SendChatMessageImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )
    markRead,
    required TResult Function(String accountId) refreshFolders,
    required TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )
    transferMessages,
    required TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )
    sendChatMessage,
    required TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )
    listMessagesWindow,
  }) {
    return sendChatMessage(accountId, folder, text, bodyHtml, requestId);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult? Function(String accountId)? refreshFolders,
    TResult? Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult? Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult? Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
  }) {
    return sendChatMessage?.call(accountId, folder, text, bodyHtml, requestId);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult Function(String accountId)? refreshFolders,
    TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
    required TResult orElse(),
  }) {
    if (sendChatMessage != null) {
      return sendChatMessage(accountId, folder, text, bodyHtml, requestId);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppCommand_MarkRead value) markRead,
    required TResult Function(AppCommand_RefreshFolders value) refreshFolders,
    required TResult Function(AppCommand_TransferMessages value)
    transferMessages,
    required TResult Function(AppCommand_SendChatMessage value) sendChatMessage,
    required TResult Function(AppCommand_ListMessagesWindow value)
    listMessagesWindow,
  }) {
    return sendChatMessage(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppCommand_MarkRead value)? markRead,
    TResult? Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult? Function(AppCommand_TransferMessages value)? transferMessages,
    TResult? Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult? Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
  }) {
    return sendChatMessage?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppCommand_MarkRead value)? markRead,
    TResult Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult Function(AppCommand_TransferMessages value)? transferMessages,
    TResult Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
    required TResult orElse(),
  }) {
    if (sendChatMessage != null) {
      return sendChatMessage(this);
    }
    return orElse();
  }
}

abstract class AppCommand_SendChatMessage extends AppCommand {
  const factory AppCommand_SendChatMessage({
    required final String accountId,
    required final String folder,
    required final String text,
    final String? bodyHtml,
    final String? requestId,
  }) = _$AppCommand_SendChatMessageImpl;
  const AppCommand_SendChatMessage._() : super._();

  String get accountId;
  String get folder;
  String get text;

  /// Optional HTML (Matrix rich text). Ignored for Nostr.
  String? get bodyHtml;
  String? get requestId;

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppCommand_SendChatMessageImplCopyWith<_$AppCommand_SendChatMessageImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppCommand_ListMessagesWindowImplCopyWith<$Res> {
  factory _$$AppCommand_ListMessagesWindowImplCopyWith(
    _$AppCommand_ListMessagesWindowImpl value,
    $Res Function(_$AppCommand_ListMessagesWindowImpl) then,
  ) = __$$AppCommand_ListMessagesWindowImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String accountId,
    String folderName,
    BigInt startIndex,
    BigInt limit,
    String messageListSort,
    String requestId,
    bool listReady,
    BigInt? visibleFirstRank,
    BigInt? visibleLastRank,
  });
}

/// @nodoc
class __$$AppCommand_ListMessagesWindowImplCopyWithImpl<$Res>
    extends _$AppCommandCopyWithImpl<$Res, _$AppCommand_ListMessagesWindowImpl>
    implements _$$AppCommand_ListMessagesWindowImplCopyWith<$Res> {
  __$$AppCommand_ListMessagesWindowImplCopyWithImpl(
    _$AppCommand_ListMessagesWindowImpl _value,
    $Res Function(_$AppCommand_ListMessagesWindowImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? folderName = null,
    Object? startIndex = null,
    Object? limit = null,
    Object? messageListSort = null,
    Object? requestId = null,
    Object? listReady = null,
    Object? visibleFirstRank = freezed,
    Object? visibleLastRank = freezed,
  }) {
    return _then(
      _$AppCommand_ListMessagesWindowImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folderName: null == folderName
            ? _value.folderName
            : folderName // ignore: cast_nullable_to_non_nullable
                  as String,
        startIndex: null == startIndex
            ? _value.startIndex
            : startIndex // ignore: cast_nullable_to_non_nullable
                  as BigInt,
        limit: null == limit
            ? _value.limit
            : limit // ignore: cast_nullable_to_non_nullable
                  as BigInt,
        messageListSort: null == messageListSort
            ? _value.messageListSort
            : messageListSort // ignore: cast_nullable_to_non_nullable
                  as String,
        requestId: null == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String,
        listReady: null == listReady
            ? _value.listReady
            : listReady // ignore: cast_nullable_to_non_nullable
                  as bool,
        visibleFirstRank: freezed == visibleFirstRank
            ? _value.visibleFirstRank
            : visibleFirstRank // ignore: cast_nullable_to_non_nullable
                  as BigInt?,
        visibleLastRank: freezed == visibleLastRank
            ? _value.visibleLastRank
            : visibleLastRank // ignore: cast_nullable_to_non_nullable
                  as BigInt?,
      ),
    );
  }
}

/// @nodoc

class _$AppCommand_ListMessagesWindowImpl
    extends AppCommand_ListMessagesWindow {
  const _$AppCommand_ListMessagesWindowImpl({
    required this.accountId,
    required this.folderName,
    required this.startIndex,
    required this.limit,
    required this.messageListSort,
    required this.requestId,
    required this.listReady,
    this.visibleFirstRank,
    this.visibleLastRank,
  }) : super._();

  @override
  final String accountId;
  @override
  final String folderName;
  @override
  final BigInt startIndex;
  @override
  final BigInt limit;
  @override
  final String messageListSort;
  @override
  final String requestId;
  @override
  final bool listReady;

  /// Inclusive oldest-first rank of the first visible list row (viewport), if known.
  @override
  final BigInt? visibleFirstRank;

  /// Inclusive oldest-first rank of the last visible list row (viewport), if known.
  @override
  final BigInt? visibleLastRank;

  @override
  String toString() {
    return 'AppCommand.listMessagesWindow(accountId: $accountId, folderName: $folderName, startIndex: $startIndex, limit: $limit, messageListSort: $messageListSort, requestId: $requestId, listReady: $listReady, visibleFirstRank: $visibleFirstRank, visibleLastRank: $visibleLastRank)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppCommand_ListMessagesWindowImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folderName, folderName) ||
                other.folderName == folderName) &&
            (identical(other.startIndex, startIndex) ||
                other.startIndex == startIndex) &&
            (identical(other.limit, limit) || other.limit == limit) &&
            (identical(other.messageListSort, messageListSort) ||
                other.messageListSort == messageListSort) &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.listReady, listReady) ||
                other.listReady == listReady) &&
            (identical(other.visibleFirstRank, visibleFirstRank) ||
                other.visibleFirstRank == visibleFirstRank) &&
            (identical(other.visibleLastRank, visibleLastRank) ||
                other.visibleLastRank == visibleLastRank));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    accountId,
    folderName,
    startIndex,
    limit,
    messageListSort,
    requestId,
    listReady,
    visibleFirstRank,
    visibleLastRank,
  );

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppCommand_ListMessagesWindowImplCopyWith<
    _$AppCommand_ListMessagesWindowImpl
  >
  get copyWith =>
      __$$AppCommand_ListMessagesWindowImplCopyWithImpl<
        _$AppCommand_ListMessagesWindowImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )
    markRead,
    required TResult Function(String accountId) refreshFolders,
    required TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )
    transferMessages,
    required TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )
    sendChatMessage,
    required TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )
    listMessagesWindow,
  }) {
    return listMessagesWindow(
      accountId,
      folderName,
      startIndex,
      limit,
      messageListSort,
      requestId,
      listReady,
      visibleFirstRank,
      visibleLastRank,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult? Function(String accountId)? refreshFolders,
    TResult? Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult? Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult? Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
  }) {
    return listMessagesWindow?.call(
      accountId,
      folderName,
      startIndex,
      limit,
      messageListSort,
      requestId,
      listReady,
      visibleFirstRank,
      visibleLastRank,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      String? requestId,
    )?
    markRead,
    TResult Function(String accountId)? refreshFolders,
    TResult Function(
      String sourceAccountId,
      String sourceFolder,
      String destAccountId,
      String destFolder,
      List<String> messageIds,
      bool isMove,
      String? requestId,
    )?
    transferMessages,
    TResult Function(
      String accountId,
      String folder,
      String text,
      String? bodyHtml,
      String? requestId,
    )?
    sendChatMessage,
    TResult Function(
      String accountId,
      String folderName,
      BigInt startIndex,
      BigInt limit,
      String messageListSort,
      String requestId,
      bool listReady,
      BigInt? visibleFirstRank,
      BigInt? visibleLastRank,
    )?
    listMessagesWindow,
    required TResult orElse(),
  }) {
    if (listMessagesWindow != null) {
      return listMessagesWindow(
        accountId,
        folderName,
        startIndex,
        limit,
        messageListSort,
        requestId,
        listReady,
        visibleFirstRank,
        visibleLastRank,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppCommand_MarkRead value) markRead,
    required TResult Function(AppCommand_RefreshFolders value) refreshFolders,
    required TResult Function(AppCommand_TransferMessages value)
    transferMessages,
    required TResult Function(AppCommand_SendChatMessage value) sendChatMessage,
    required TResult Function(AppCommand_ListMessagesWindow value)
    listMessagesWindow,
  }) {
    return listMessagesWindow(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppCommand_MarkRead value)? markRead,
    TResult? Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult? Function(AppCommand_TransferMessages value)? transferMessages,
    TResult? Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult? Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
  }) {
    return listMessagesWindow?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppCommand_MarkRead value)? markRead,
    TResult Function(AppCommand_RefreshFolders value)? refreshFolders,
    TResult Function(AppCommand_TransferMessages value)? transferMessages,
    TResult Function(AppCommand_SendChatMessage value)? sendChatMessage,
    TResult Function(AppCommand_ListMessagesWindow value)? listMessagesWindow,
    required TResult orElse(),
  }) {
    if (listMessagesWindow != null) {
      return listMessagesWindow(this);
    }
    return orElse();
  }
}

abstract class AppCommand_ListMessagesWindow extends AppCommand {
  const factory AppCommand_ListMessagesWindow({
    required final String accountId,
    required final String folderName,
    required final BigInt startIndex,
    required final BigInt limit,
    required final String messageListSort,
    required final String requestId,
    required final bool listReady,
    final BigInt? visibleFirstRank,
    final BigInt? visibleLastRank,
  }) = _$AppCommand_ListMessagesWindowImpl;
  const AppCommand_ListMessagesWindow._() : super._();

  String get accountId;
  String get folderName;
  BigInt get startIndex;
  BigInt get limit;
  String get messageListSort;
  String get requestId;
  bool get listReady;

  /// Inclusive oldest-first rank of the first visible list row (viewport), if known.
  BigInt? get visibleFirstRank;

  /// Inclusive oldest-first rank of the last visible list row (viewport), if known.
  BigInt? get visibleLastRank;

  /// Create a copy of AppCommand
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppCommand_ListMessagesWindowImplCopyWith<
    _$AppCommand_ListMessagesWindowImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}
