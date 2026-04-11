// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'events.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

/// @nodoc
mixin _$AppEvent {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $AppEventCopyWith<$Res> {
  factory $AppEventCopyWith(AppEvent value, $Res Function(AppEvent) then) =
      _$AppEventCopyWithImpl<$Res, AppEvent>;
}

/// @nodoc
class _$AppEventCopyWithImpl<$Res, $Val extends AppEvent>
    implements $AppEventCopyWith<$Res> {
  _$AppEventCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$AppEvent_AccountConnectionChangedImplCopyWith<$Res> {
  factory _$$AppEvent_AccountConnectionChangedImplCopyWith(
    _$AppEvent_AccountConnectionChangedImpl value,
    $Res Function(_$AppEvent_AccountConnectionChangedImpl) then,
  ) = __$$AppEvent_AccountConnectionChangedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String accountId,
    String storeKind,
    String connectionState,
    String? message,
  });
}

/// @nodoc
class __$$AppEvent_AccountConnectionChangedImplCopyWithImpl<$Res>
    extends
        _$AppEventCopyWithImpl<$Res, _$AppEvent_AccountConnectionChangedImpl>
    implements _$$AppEvent_AccountConnectionChangedImplCopyWith<$Res> {
  __$$AppEvent_AccountConnectionChangedImplCopyWithImpl(
    _$AppEvent_AccountConnectionChangedImpl _value,
    $Res Function(_$AppEvent_AccountConnectionChangedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? storeKind = null,
    Object? connectionState = null,
    Object? message = freezed,
  }) {
    return _then(
      _$AppEvent_AccountConnectionChangedImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        storeKind: null == storeKind
            ? _value.storeKind
            : storeKind // ignore: cast_nullable_to_non_nullable
                  as String,
        connectionState: null == connectionState
            ? _value.connectionState
            : connectionState // ignore: cast_nullable_to_non_nullable
                  as String,
        message: freezed == message
            ? _value.message
            : message // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_AccountConnectionChangedImpl
    extends AppEvent_AccountConnectionChanged {
  const _$AppEvent_AccountConnectionChangedImpl({
    required this.accountId,
    required this.storeKind,
    required this.connectionState,
    this.message,
  }) : super._();

  @override
  final String accountId;

  /// `email` | `nostr` | `matrix` (UI picks list vs conversation chrome).
  @override
  final String storeKind;
  @override
  final String connectionState;
  @override
  final String? message;

  @override
  String toString() {
    return 'AppEvent.accountConnectionChanged(accountId: $accountId, storeKind: $storeKind, connectionState: $connectionState, message: $message)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_AccountConnectionChangedImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.storeKind, storeKind) ||
                other.storeKind == storeKind) &&
            (identical(other.connectionState, connectionState) ||
                other.connectionState == connectionState) &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, accountId, storeKind, connectionState, message);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_AccountConnectionChangedImplCopyWith<
    _$AppEvent_AccountConnectionChangedImpl
  >
  get copyWith =>
      __$$AppEvent_AccountConnectionChangedImplCopyWithImpl<
        _$AppEvent_AccountConnectionChangedImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return accountConnectionChanged(
      accountId,
      storeKind,
      connectionState,
      message,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return accountConnectionChanged?.call(
      accountId,
      storeKind,
      connectionState,
      message,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (accountConnectionChanged != null) {
      return accountConnectionChanged(
        accountId,
        storeKind,
        connectionState,
        message,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return accountConnectionChanged(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return accountConnectionChanged?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (accountConnectionChanged != null) {
      return accountConnectionChanged(this);
    }
    return orElse();
  }
}

abstract class AppEvent_AccountConnectionChanged extends AppEvent {
  const factory AppEvent_AccountConnectionChanged({
    required final String accountId,
    required final String storeKind,
    required final String connectionState,
    final String? message,
  }) = _$AppEvent_AccountConnectionChangedImpl;
  const AppEvent_AccountConnectionChanged._() : super._();

  String get accountId;

  /// `email` | `nostr` | `matrix` (UI picks list vs conversation chrome).
  String get storeKind;
  String get connectionState;
  String? get message;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_AccountConnectionChangedImplCopyWith<
    _$AppEvent_AccountConnectionChangedImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_FolderListUpdatedImplCopyWith<$Res> {
  factory _$$AppEvent_FolderListUpdatedImplCopyWith(
    _$AppEvent_FolderListUpdatedImpl value,
    $Res Function(_$AppEvent_FolderListUpdatedImpl) then,
  ) = __$$AppEvent_FolderListUpdatedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String accountId,
    List<String> folders,
    String? hierarchyDelimiter,
    Map<String, int> unreadByFolder,
    Map<String, String> folderDisplayNames,
    List<SubscriptionAvailableRow>? subscriptionAvailable,
  });
}

/// @nodoc
class __$$AppEvent_FolderListUpdatedImplCopyWithImpl<$Res>
    extends _$AppEventCopyWithImpl<$Res, _$AppEvent_FolderListUpdatedImpl>
    implements _$$AppEvent_FolderListUpdatedImplCopyWith<$Res> {
  __$$AppEvent_FolderListUpdatedImplCopyWithImpl(
    _$AppEvent_FolderListUpdatedImpl _value,
    $Res Function(_$AppEvent_FolderListUpdatedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? folders = null,
    Object? hierarchyDelimiter = freezed,
    Object? unreadByFolder = null,
    Object? folderDisplayNames = null,
    Object? subscriptionAvailable = freezed,
  }) {
    return _then(
      _$AppEvent_FolderListUpdatedImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folders: null == folders
            ? _value._folders
            : folders // ignore: cast_nullable_to_non_nullable
                  as List<String>,
        hierarchyDelimiter: freezed == hierarchyDelimiter
            ? _value.hierarchyDelimiter
            : hierarchyDelimiter // ignore: cast_nullable_to_non_nullable
                  as String?,
        unreadByFolder: null == unreadByFolder
            ? _value._unreadByFolder
            : unreadByFolder // ignore: cast_nullable_to_non_nullable
                  as Map<String, int>,
        folderDisplayNames: null == folderDisplayNames
            ? _value._folderDisplayNames
            : folderDisplayNames // ignore: cast_nullable_to_non_nullable
                  as Map<String, String>,
        subscriptionAvailable: freezed == subscriptionAvailable
            ? _value._subscriptionAvailable
            : subscriptionAvailable // ignore: cast_nullable_to_non_nullable
                  as List<SubscriptionAvailableRow>?,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_FolderListUpdatedImpl extends AppEvent_FolderListUpdated {
  const _$AppEvent_FolderListUpdatedImpl({
    required this.accountId,
    required final List<String> folders,
    this.hierarchyDelimiter,
    required final Map<String, int> unreadByFolder,
    required final Map<String, String> folderDisplayNames,
    final List<SubscriptionAvailableRow>? subscriptionAvailable,
  }) : _folders = folders,
       _unreadByFolder = unreadByFolder,
       _folderDisplayNames = folderDisplayNames,
       _subscriptionAvailable = subscriptionAvailable,
       super._();

  @override
  final String accountId;
  final List<String> _folders;
  @override
  List<String> get folders {
    if (_folders is EqualUnmodifiableListView) return _folders;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_folders);
  }

  @override
  final String? hierarchyDelimiter;
  final Map<String, int> _unreadByFolder;
  @override
  Map<String, int> get unreadByFolder {
    if (_unreadByFolder is EqualUnmodifiableMapView) return _unreadByFolder;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(_unreadByFolder);
  }

  /// Optional UI labels keyed by folder id (e.g. Matrix room id → room / peer display name).
  final Map<String, String> _folderDisplayNames;

  /// Optional UI labels keyed by folder id (e.g. Matrix room id → room / peer display name).
  @override
  Map<String, String> get folderDisplayNames {
    if (_folderDisplayNames is EqualUnmodifiableMapView)
      return _folderDisplayNames;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(_folderDisplayNames);
  }

  /// IMAP / NNTP / Matrix: **Available** tab rows (Subscribed tab is `folders`).
  final List<SubscriptionAvailableRow>? _subscriptionAvailable;

  /// IMAP / NNTP / Matrix: **Available** tab rows (Subscribed tab is `folders`).
  @override
  List<SubscriptionAvailableRow>? get subscriptionAvailable {
    final value = _subscriptionAvailable;
    if (value == null) return null;
    if (_subscriptionAvailable is EqualUnmodifiableListView)
      return _subscriptionAvailable;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(value);
  }

  @override
  String toString() {
    return 'AppEvent.folderListUpdated(accountId: $accountId, folders: $folders, hierarchyDelimiter: $hierarchyDelimiter, unreadByFolder: $unreadByFolder, folderDisplayNames: $folderDisplayNames, subscriptionAvailable: $subscriptionAvailable)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_FolderListUpdatedImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            const DeepCollectionEquality().equals(other._folders, _folders) &&
            (identical(other.hierarchyDelimiter, hierarchyDelimiter) ||
                other.hierarchyDelimiter == hierarchyDelimiter) &&
            const DeepCollectionEquality().equals(
              other._unreadByFolder,
              _unreadByFolder,
            ) &&
            const DeepCollectionEquality().equals(
              other._folderDisplayNames,
              _folderDisplayNames,
            ) &&
            const DeepCollectionEquality().equals(
              other._subscriptionAvailable,
              _subscriptionAvailable,
            ));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    accountId,
    const DeepCollectionEquality().hash(_folders),
    hierarchyDelimiter,
    const DeepCollectionEquality().hash(_unreadByFolder),
    const DeepCollectionEquality().hash(_folderDisplayNames),
    const DeepCollectionEquality().hash(_subscriptionAvailable),
  );

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_FolderListUpdatedImplCopyWith<_$AppEvent_FolderListUpdatedImpl>
  get copyWith =>
      __$$AppEvent_FolderListUpdatedImplCopyWithImpl<
        _$AppEvent_FolderListUpdatedImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return folderListUpdated(
      accountId,
      folders,
      hierarchyDelimiter,
      unreadByFolder,
      folderDisplayNames,
      subscriptionAvailable,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return folderListUpdated?.call(
      accountId,
      folders,
      hierarchyDelimiter,
      unreadByFolder,
      folderDisplayNames,
      subscriptionAvailable,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (folderListUpdated != null) {
      return folderListUpdated(
        accountId,
        folders,
        hierarchyDelimiter,
        unreadByFolder,
        folderDisplayNames,
        subscriptionAvailable,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return folderListUpdated(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return folderListUpdated?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (folderListUpdated != null) {
      return folderListUpdated(this);
    }
    return orElse();
  }
}

abstract class AppEvent_FolderListUpdated extends AppEvent {
  const factory AppEvent_FolderListUpdated({
    required final String accountId,
    required final List<String> folders,
    final String? hierarchyDelimiter,
    required final Map<String, int> unreadByFolder,
    required final Map<String, String> folderDisplayNames,
    final List<SubscriptionAvailableRow>? subscriptionAvailable,
  }) = _$AppEvent_FolderListUpdatedImpl;
  const AppEvent_FolderListUpdated._() : super._();

  String get accountId;
  List<String> get folders;
  String? get hierarchyDelimiter;
  Map<String, int> get unreadByFolder;

  /// Optional UI labels keyed by folder id (e.g. Matrix room id → room / peer display name).
  Map<String, String> get folderDisplayNames;

  /// IMAP / NNTP / Matrix: **Available** tab rows (Subscribed tab is `folders`).
  List<SubscriptionAvailableRow>? get subscriptionAvailable;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_FolderListUpdatedImplCopyWith<_$AppEvent_FolderListUpdatedImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_FolderFoundImplCopyWith<$Res> {
  factory _$$AppEvent_FolderFoundImplCopyWith(
    _$AppEvent_FolderFoundImpl value,
    $Res Function(_$AppEvent_FolderFoundImpl) then,
  ) = __$$AppEvent_FolderFoundImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String accountId, String folderName, int unread});
}

/// @nodoc
class __$$AppEvent_FolderFoundImplCopyWithImpl<$Res>
    extends _$AppEventCopyWithImpl<$Res, _$AppEvent_FolderFoundImpl>
    implements _$$AppEvent_FolderFoundImplCopyWith<$Res> {
  __$$AppEvent_FolderFoundImplCopyWithImpl(
    _$AppEvent_FolderFoundImpl _value,
    $Res Function(_$AppEvent_FolderFoundImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? folderName = null,
    Object? unread = null,
  }) {
    return _then(
      _$AppEvent_FolderFoundImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folderName: null == folderName
            ? _value.folderName
            : folderName // ignore: cast_nullable_to_non_nullable
                  as String,
        unread: null == unread
            ? _value.unread
            : unread // ignore: cast_nullable_to_non_nullable
                  as int,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_FolderFoundImpl extends AppEvent_FolderFound {
  const _$AppEvent_FolderFoundImpl({
    required this.accountId,
    required this.folderName,
    required this.unread,
  }) : super._();

  @override
  final String accountId;
  @override
  final String folderName;
  @override
  final int unread;

  @override
  String toString() {
    return 'AppEvent.folderFound(accountId: $accountId, folderName: $folderName, unread: $unread)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_FolderFoundImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folderName, folderName) ||
                other.folderName == folderName) &&
            (identical(other.unread, unread) || other.unread == unread));
  }

  @override
  int get hashCode => Object.hash(runtimeType, accountId, folderName, unread);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_FolderFoundImplCopyWith<_$AppEvent_FolderFoundImpl>
  get copyWith =>
      __$$AppEvent_FolderFoundImplCopyWithImpl<_$AppEvent_FolderFoundImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return folderFound(accountId, folderName, unread);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return folderFound?.call(accountId, folderName, unread);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (folderFound != null) {
      return folderFound(accountId, folderName, unread);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return folderFound(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return folderFound?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (folderFound != null) {
      return folderFound(this);
    }
    return orElse();
  }
}

abstract class AppEvent_FolderFound extends AppEvent {
  const factory AppEvent_FolderFound({
    required final String accountId,
    required final String folderName,
    required final int unread,
  }) = _$AppEvent_FolderFoundImpl;
  const AppEvent_FolderFound._() : super._();

  String get accountId;
  String get folderName;
  int get unread;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_FolderFoundImplCopyWith<_$AppEvent_FolderFoundImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_MessageFlagsChangedImplCopyWith<$Res> {
  factory _$$AppEvent_MessageFlagsChangedImplCopyWith(
    _$AppEvent_MessageFlagsChangedImpl value,
    $Res Function(_$AppEvent_MessageFlagsChangedImpl) then,
  ) = __$$AppEvent_MessageFlagsChangedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String accountId, String folder, String messageId, bool isRead});
}

/// @nodoc
class __$$AppEvent_MessageFlagsChangedImplCopyWithImpl<$Res>
    extends _$AppEventCopyWithImpl<$Res, _$AppEvent_MessageFlagsChangedImpl>
    implements _$$AppEvent_MessageFlagsChangedImplCopyWith<$Res> {
  __$$AppEvent_MessageFlagsChangedImplCopyWithImpl(
    _$AppEvent_MessageFlagsChangedImpl _value,
    $Res Function(_$AppEvent_MessageFlagsChangedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? folder = null,
    Object? messageId = null,
    Object? isRead = null,
  }) {
    return _then(
      _$AppEvent_MessageFlagsChangedImpl(
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
        isRead: null == isRead
            ? _value.isRead
            : isRead // ignore: cast_nullable_to_non_nullable
                  as bool,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_MessageFlagsChangedImpl extends AppEvent_MessageFlagsChanged {
  const _$AppEvent_MessageFlagsChangedImpl({
    required this.accountId,
    required this.folder,
    required this.messageId,
    required this.isRead,
  }) : super._();

  @override
  final String accountId;
  @override
  final String folder;
  @override
  final String messageId;
  @override
  final bool isRead;

  @override
  String toString() {
    return 'AppEvent.messageFlagsChanged(accountId: $accountId, folder: $folder, messageId: $messageId, isRead: $isRead)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_MessageFlagsChangedImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folder, folder) || other.folder == folder) &&
            (identical(other.messageId, messageId) ||
                other.messageId == messageId) &&
            (identical(other.isRead, isRead) || other.isRead == isRead));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, accountId, folder, messageId, isRead);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_MessageFlagsChangedImplCopyWith<
    _$AppEvent_MessageFlagsChangedImpl
  >
  get copyWith =>
      __$$AppEvent_MessageFlagsChangedImplCopyWithImpl<
        _$AppEvent_MessageFlagsChangedImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return messageFlagsChanged(accountId, folder, messageId, isRead);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return messageFlagsChanged?.call(accountId, folder, messageId, isRead);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageFlagsChanged != null) {
      return messageFlagsChanged(accountId, folder, messageId, isRead);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return messageFlagsChanged(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return messageFlagsChanged?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageFlagsChanged != null) {
      return messageFlagsChanged(this);
    }
    return orElse();
  }
}

abstract class AppEvent_MessageFlagsChanged extends AppEvent {
  const factory AppEvent_MessageFlagsChanged({
    required final String accountId,
    required final String folder,
    required final String messageId,
    required final bool isRead,
  }) = _$AppEvent_MessageFlagsChangedImpl;
  const AppEvent_MessageFlagsChanged._() : super._();

  String get accountId;
  String get folder;
  String get messageId;
  bool get isRead;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_MessageFlagsChangedImplCopyWith<
    _$AppEvent_MessageFlagsChangedImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_MessageListWindowStartedImplCopyWith<$Res> {
  factory _$$AppEvent_MessageListWindowStartedImplCopyWith(
    _$AppEvent_MessageListWindowStartedImpl value,
    $Res Function(_$AppEvent_MessageListWindowStartedImpl) then,
  ) = __$$AppEvent_MessageListWindowStartedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String requestId,
    String accountId,
    String folderName,
    String messageListSort,
    BigInt total,
    BigInt startIndex,
    String listStrategy,
    int rowCount,
    bool listReady,
  });
}

/// @nodoc
class __$$AppEvent_MessageListWindowStartedImplCopyWithImpl<$Res>
    extends
        _$AppEventCopyWithImpl<$Res, _$AppEvent_MessageListWindowStartedImpl>
    implements _$$AppEvent_MessageListWindowStartedImplCopyWith<$Res> {
  __$$AppEvent_MessageListWindowStartedImplCopyWithImpl(
    _$AppEvent_MessageListWindowStartedImpl _value,
    $Res Function(_$AppEvent_MessageListWindowStartedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? requestId = null,
    Object? accountId = null,
    Object? folderName = null,
    Object? messageListSort = null,
    Object? total = null,
    Object? startIndex = null,
    Object? listStrategy = null,
    Object? rowCount = null,
    Object? listReady = null,
  }) {
    return _then(
      _$AppEvent_MessageListWindowStartedImpl(
        requestId: null == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String,
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folderName: null == folderName
            ? _value.folderName
            : folderName // ignore: cast_nullable_to_non_nullable
                  as String,
        messageListSort: null == messageListSort
            ? _value.messageListSort
            : messageListSort // ignore: cast_nullable_to_non_nullable
                  as String,
        total: null == total
            ? _value.total
            : total // ignore: cast_nullable_to_non_nullable
                  as BigInt,
        startIndex: null == startIndex
            ? _value.startIndex
            : startIndex // ignore: cast_nullable_to_non_nullable
                  as BigInt,
        listStrategy: null == listStrategy
            ? _value.listStrategy
            : listStrategy // ignore: cast_nullable_to_non_nullable
                  as String,
        rowCount: null == rowCount
            ? _value.rowCount
            : rowCount // ignore: cast_nullable_to_non_nullable
                  as int,
        listReady: null == listReady
            ? _value.listReady
            : listReady // ignore: cast_nullable_to_non_nullable
                  as bool,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_MessageListWindowStartedImpl
    extends AppEvent_MessageListWindowStarted {
  const _$AppEvent_MessageListWindowStartedImpl({
    required this.requestId,
    required this.accountId,
    required this.folderName,
    required this.messageListSort,
    required this.total,
    required this.startIndex,
    required this.listStrategy,
    required this.rowCount,
    required this.listReady,
  }) : super._();

  @override
  final String requestId;
  @override
  final String accountId;
  @override
  final String folderName;
  @override
  final String messageListSort;
  @override
  final BigInt total;
  @override
  final BigInt startIndex;
  @override
  final String listStrategy;
  @override
  final int rowCount;
  @override
  final bool listReady;

  @override
  String toString() {
    return 'AppEvent.messageListWindowStarted(requestId: $requestId, accountId: $accountId, folderName: $folderName, messageListSort: $messageListSort, total: $total, startIndex: $startIndex, listStrategy: $listStrategy, rowCount: $rowCount, listReady: $listReady)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_MessageListWindowStartedImpl &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folderName, folderName) ||
                other.folderName == folderName) &&
            (identical(other.messageListSort, messageListSort) ||
                other.messageListSort == messageListSort) &&
            (identical(other.total, total) || other.total == total) &&
            (identical(other.startIndex, startIndex) ||
                other.startIndex == startIndex) &&
            (identical(other.listStrategy, listStrategy) ||
                other.listStrategy == listStrategy) &&
            (identical(other.rowCount, rowCount) ||
                other.rowCount == rowCount) &&
            (identical(other.listReady, listReady) ||
                other.listReady == listReady));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    requestId,
    accountId,
    folderName,
    messageListSort,
    total,
    startIndex,
    listStrategy,
    rowCount,
    listReady,
  );

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_MessageListWindowStartedImplCopyWith<
    _$AppEvent_MessageListWindowStartedImpl
  >
  get copyWith =>
      __$$AppEvent_MessageListWindowStartedImplCopyWithImpl<
        _$AppEvent_MessageListWindowStartedImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return messageListWindowStarted(
      requestId,
      accountId,
      folderName,
      messageListSort,
      total,
      startIndex,
      listStrategy,
      rowCount,
      listReady,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return messageListWindowStarted?.call(
      requestId,
      accountId,
      folderName,
      messageListSort,
      total,
      startIndex,
      listStrategy,
      rowCount,
      listReady,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageListWindowStarted != null) {
      return messageListWindowStarted(
        requestId,
        accountId,
        folderName,
        messageListSort,
        total,
        startIndex,
        listStrategy,
        rowCount,
        listReady,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return messageListWindowStarted(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return messageListWindowStarted?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageListWindowStarted != null) {
      return messageListWindowStarted(this);
    }
    return orElse();
  }
}

abstract class AppEvent_MessageListWindowStarted extends AppEvent {
  const factory AppEvent_MessageListWindowStarted({
    required final String requestId,
    required final String accountId,
    required final String folderName,
    required final String messageListSort,
    required final BigInt total,
    required final BigInt startIndex,
    required final String listStrategy,
    required final int rowCount,
    required final bool listReady,
  }) = _$AppEvent_MessageListWindowStartedImpl;
  const AppEvent_MessageListWindowStarted._() : super._();

  String get requestId;
  String get accountId;
  String get folderName;
  String get messageListSort;
  BigInt get total;
  BigInt get startIndex;
  String get listStrategy;
  int get rowCount;
  bool get listReady;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_MessageListWindowStartedImplCopyWith<
    _$AppEvent_MessageListWindowStartedImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_MessageListRowFoundImplCopyWith<$Res> {
  factory _$$AppEvent_MessageListRowFoundImplCopyWith(
    _$AppEvent_MessageListRowFoundImpl value,
    $Res Function(_$AppEvent_MessageListRowFoundImpl) then,
  ) = __$$AppEvent_MessageListRowFoundImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String requestId,
    String accountId,
    String folderName,
    String messageListSort,
    BigInt rank,
    MessageListRowSummary summary,
  });
}

/// @nodoc
class __$$AppEvent_MessageListRowFoundImplCopyWithImpl<$Res>
    extends _$AppEventCopyWithImpl<$Res, _$AppEvent_MessageListRowFoundImpl>
    implements _$$AppEvent_MessageListRowFoundImplCopyWith<$Res> {
  __$$AppEvent_MessageListRowFoundImplCopyWithImpl(
    _$AppEvent_MessageListRowFoundImpl _value,
    $Res Function(_$AppEvent_MessageListRowFoundImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? requestId = null,
    Object? accountId = null,
    Object? folderName = null,
    Object? messageListSort = null,
    Object? rank = null,
    Object? summary = null,
  }) {
    return _then(
      _$AppEvent_MessageListRowFoundImpl(
        requestId: null == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String,
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folderName: null == folderName
            ? _value.folderName
            : folderName // ignore: cast_nullable_to_non_nullable
                  as String,
        messageListSort: null == messageListSort
            ? _value.messageListSort
            : messageListSort // ignore: cast_nullable_to_non_nullable
                  as String,
        rank: null == rank
            ? _value.rank
            : rank // ignore: cast_nullable_to_non_nullable
                  as BigInt,
        summary: null == summary
            ? _value.summary
            : summary // ignore: cast_nullable_to_non_nullable
                  as MessageListRowSummary,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_MessageListRowFoundImpl extends AppEvent_MessageListRowFound {
  const _$AppEvent_MessageListRowFoundImpl({
    required this.requestId,
    required this.accountId,
    required this.folderName,
    required this.messageListSort,
    required this.rank,
    required this.summary,
  }) : super._();

  @override
  final String requestId;
  @override
  final String accountId;
  @override
  final String folderName;
  @override
  final String messageListSort;
  @override
  final BigInt rank;
  @override
  final MessageListRowSummary summary;

  @override
  String toString() {
    return 'AppEvent.messageListRowFound(requestId: $requestId, accountId: $accountId, folderName: $folderName, messageListSort: $messageListSort, rank: $rank, summary: $summary)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_MessageListRowFoundImpl &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folderName, folderName) ||
                other.folderName == folderName) &&
            (identical(other.messageListSort, messageListSort) ||
                other.messageListSort == messageListSort) &&
            (identical(other.rank, rank) || other.rank == rank) &&
            (identical(other.summary, summary) || other.summary == summary));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    requestId,
    accountId,
    folderName,
    messageListSort,
    rank,
    summary,
  );

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_MessageListRowFoundImplCopyWith<
    _$AppEvent_MessageListRowFoundImpl
  >
  get copyWith =>
      __$$AppEvent_MessageListRowFoundImplCopyWithImpl<
        _$AppEvent_MessageListRowFoundImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return messageListRowFound(
      requestId,
      accountId,
      folderName,
      messageListSort,
      rank,
      summary,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return messageListRowFound?.call(
      requestId,
      accountId,
      folderName,
      messageListSort,
      rank,
      summary,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageListRowFound != null) {
      return messageListRowFound(
        requestId,
        accountId,
        folderName,
        messageListSort,
        rank,
        summary,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return messageListRowFound(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return messageListRowFound?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageListRowFound != null) {
      return messageListRowFound(this);
    }
    return orElse();
  }
}

abstract class AppEvent_MessageListRowFound extends AppEvent {
  const factory AppEvent_MessageListRowFound({
    required final String requestId,
    required final String accountId,
    required final String folderName,
    required final String messageListSort,
    required final BigInt rank,
    required final MessageListRowSummary summary,
  }) = _$AppEvent_MessageListRowFoundImpl;
  const AppEvent_MessageListRowFound._() : super._();

  String get requestId;
  String get accountId;
  String get folderName;
  String get messageListSort;
  BigInt get rank;
  MessageListRowSummary get summary;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_MessageListRowFoundImplCopyWith<
    _$AppEvent_MessageListRowFoundImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_MessageListWindowCompleteImplCopyWith<$Res> {
  factory _$$AppEvent_MessageListWindowCompleteImplCopyWith(
    _$AppEvent_MessageListWindowCompleteImpl value,
    $Res Function(_$AppEvent_MessageListWindowCompleteImpl) then,
  ) = __$$AppEvent_MessageListWindowCompleteImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String requestId,
    String accountId,
    String folderName,
    String messageListSort,
    String? error,
  });
}

/// @nodoc
class __$$AppEvent_MessageListWindowCompleteImplCopyWithImpl<$Res>
    extends
        _$AppEventCopyWithImpl<$Res, _$AppEvent_MessageListWindowCompleteImpl>
    implements _$$AppEvent_MessageListWindowCompleteImplCopyWith<$Res> {
  __$$AppEvent_MessageListWindowCompleteImplCopyWithImpl(
    _$AppEvent_MessageListWindowCompleteImpl _value,
    $Res Function(_$AppEvent_MessageListWindowCompleteImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? requestId = null,
    Object? accountId = null,
    Object? folderName = null,
    Object? messageListSort = null,
    Object? error = freezed,
  }) {
    return _then(
      _$AppEvent_MessageListWindowCompleteImpl(
        requestId: null == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String,
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        folderName: null == folderName
            ? _value.folderName
            : folderName // ignore: cast_nullable_to_non_nullable
                  as String,
        messageListSort: null == messageListSort
            ? _value.messageListSort
            : messageListSort // ignore: cast_nullable_to_non_nullable
                  as String,
        error: freezed == error
            ? _value.error
            : error // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_MessageListWindowCompleteImpl
    extends AppEvent_MessageListWindowComplete {
  const _$AppEvent_MessageListWindowCompleteImpl({
    required this.requestId,
    required this.accountId,
    required this.folderName,
    required this.messageListSort,
    this.error,
  }) : super._();

  @override
  final String requestId;
  @override
  final String accountId;
  @override
  final String folderName;
  @override
  final String messageListSort;
  @override
  final String? error;

  @override
  String toString() {
    return 'AppEvent.messageListWindowComplete(requestId: $requestId, accountId: $accountId, folderName: $folderName, messageListSort: $messageListSort, error: $error)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_MessageListWindowCompleteImpl &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.folderName, folderName) ||
                other.folderName == folderName) &&
            (identical(other.messageListSort, messageListSort) ||
                other.messageListSort == messageListSort) &&
            (identical(other.error, error) || other.error == error));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    requestId,
    accountId,
    folderName,
    messageListSort,
    error,
  );

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_MessageListWindowCompleteImplCopyWith<
    _$AppEvent_MessageListWindowCompleteImpl
  >
  get copyWith =>
      __$$AppEvent_MessageListWindowCompleteImplCopyWithImpl<
        _$AppEvent_MessageListWindowCompleteImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return messageListWindowComplete(
      requestId,
      accountId,
      folderName,
      messageListSort,
      error,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return messageListWindowComplete?.call(
      requestId,
      accountId,
      folderName,
      messageListSort,
      error,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageListWindowComplete != null) {
      return messageListWindowComplete(
        requestId,
        accountId,
        folderName,
        messageListSort,
        error,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return messageListWindowComplete(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return messageListWindowComplete?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (messageListWindowComplete != null) {
      return messageListWindowComplete(this);
    }
    return orElse();
  }
}

abstract class AppEvent_MessageListWindowComplete extends AppEvent {
  const factory AppEvent_MessageListWindowComplete({
    required final String requestId,
    required final String accountId,
    required final String folderName,
    required final String messageListSort,
    final String? error,
  }) = _$AppEvent_MessageListWindowCompleteImpl;
  const AppEvent_MessageListWindowComplete._() : super._();

  String get requestId;
  String get accountId;
  String get folderName;
  String get messageListSort;
  String? get error;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_MessageListWindowCompleteImplCopyWith<
    _$AppEvent_MessageListWindowCompleteImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_CommandResultImplCopyWith<$Res> {
  factory _$$AppEvent_CommandResultImplCopyWith(
    _$AppEvent_CommandResultImpl value,
    $Res Function(_$AppEvent_CommandResultImpl) then,
  ) = __$$AppEvent_CommandResultImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String? requestId, bool ok, String? error});
}

/// @nodoc
class __$$AppEvent_CommandResultImplCopyWithImpl<$Res>
    extends _$AppEventCopyWithImpl<$Res, _$AppEvent_CommandResultImpl>
    implements _$$AppEvent_CommandResultImplCopyWith<$Res> {
  __$$AppEvent_CommandResultImplCopyWithImpl(
    _$AppEvent_CommandResultImpl _value,
    $Res Function(_$AppEvent_CommandResultImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? requestId = freezed,
    Object? ok = null,
    Object? error = freezed,
  }) {
    return _then(
      _$AppEvent_CommandResultImpl(
        requestId: freezed == requestId
            ? _value.requestId
            : requestId // ignore: cast_nullable_to_non_nullable
                  as String?,
        ok: null == ok
            ? _value.ok
            : ok // ignore: cast_nullable_to_non_nullable
                  as bool,
        error: freezed == error
            ? _value.error
            : error // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_CommandResultImpl extends AppEvent_CommandResult {
  const _$AppEvent_CommandResultImpl({
    this.requestId,
    required this.ok,
    this.error,
  }) : super._();

  @override
  final String? requestId;
  @override
  final bool ok;
  @override
  final String? error;

  @override
  String toString() {
    return 'AppEvent.commandResult(requestId: $requestId, ok: $ok, error: $error)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_CommandResultImpl &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.ok, ok) || other.ok == ok) &&
            (identical(other.error, error) || other.error == error));
  }

  @override
  int get hashCode => Object.hash(runtimeType, requestId, ok, error);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_CommandResultImplCopyWith<_$AppEvent_CommandResultImpl>
  get copyWith =>
      __$$AppEvent_CommandResultImplCopyWithImpl<_$AppEvent_CommandResultImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return commandResult(requestId, ok, error);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return commandResult?.call(requestId, ok, error);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (commandResult != null) {
      return commandResult(requestId, ok, error);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return commandResult(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return commandResult?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (commandResult != null) {
      return commandResult(this);
    }
    return orElse();
  }
}

abstract class AppEvent_CommandResult extends AppEvent {
  const factory AppEvent_CommandResult({
    final String? requestId,
    required final bool ok,
    final String? error,
  }) = _$AppEvent_CommandResultImpl;
  const AppEvent_CommandResult._() : super._();

  String? get requestId;
  bool get ok;
  String? get error;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_CommandResultImplCopyWith<_$AppEvent_CommandResultImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AppEvent_NostrProfileUpdatedImplCopyWith<$Res> {
  factory _$$AppEvent_NostrProfileUpdatedImplCopyWith(
    _$AppEvent_NostrProfileUpdatedImpl value,
    $Res Function(_$AppEvent_NostrProfileUpdatedImpl) then,
  ) = __$$AppEvent_NostrProfileUpdatedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String accountId,
    String pubkeyHex,
    String npub,
    String? displayName,
    String? nip05,
    String? picture,
  });
}

/// @nodoc
class __$$AppEvent_NostrProfileUpdatedImplCopyWithImpl<$Res>
    extends _$AppEventCopyWithImpl<$Res, _$AppEvent_NostrProfileUpdatedImpl>
    implements _$$AppEvent_NostrProfileUpdatedImplCopyWith<$Res> {
  __$$AppEvent_NostrProfileUpdatedImplCopyWithImpl(
    _$AppEvent_NostrProfileUpdatedImpl _value,
    $Res Function(_$AppEvent_NostrProfileUpdatedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? accountId = null,
    Object? pubkeyHex = null,
    Object? npub = null,
    Object? displayName = freezed,
    Object? nip05 = freezed,
    Object? picture = freezed,
  }) {
    return _then(
      _$AppEvent_NostrProfileUpdatedImpl(
        accountId: null == accountId
            ? _value.accountId
            : accountId // ignore: cast_nullable_to_non_nullable
                  as String,
        pubkeyHex: null == pubkeyHex
            ? _value.pubkeyHex
            : pubkeyHex // ignore: cast_nullable_to_non_nullable
                  as String,
        npub: null == npub
            ? _value.npub
            : npub // ignore: cast_nullable_to_non_nullable
                  as String,
        displayName: freezed == displayName
            ? _value.displayName
            : displayName // ignore: cast_nullable_to_non_nullable
                  as String?,
        nip05: freezed == nip05
            ? _value.nip05
            : nip05 // ignore: cast_nullable_to_non_nullable
                  as String?,
        picture: freezed == picture
            ? _value.picture
            : picture // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AppEvent_NostrProfileUpdatedImpl extends AppEvent_NostrProfileUpdated {
  const _$AppEvent_NostrProfileUpdatedImpl({
    required this.accountId,
    required this.pubkeyHex,
    required this.npub,
    this.displayName,
    this.nip05,
    this.picture,
  }) : super._();

  @override
  final String accountId;

  /// Lowercase hex pubkey (folder id for DM conversations).
  @override
  final String pubkeyHex;

  /// npub (bech32) for display fallback.
  @override
  final String npub;
  @override
  final String? displayName;
  @override
  final String? nip05;
  @override
  final String? picture;

  @override
  String toString() {
    return 'AppEvent.nostrProfileUpdated(accountId: $accountId, pubkeyHex: $pubkeyHex, npub: $npub, displayName: $displayName, nip05: $nip05, picture: $picture)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AppEvent_NostrProfileUpdatedImpl &&
            (identical(other.accountId, accountId) ||
                other.accountId == accountId) &&
            (identical(other.pubkeyHex, pubkeyHex) ||
                other.pubkeyHex == pubkeyHex) &&
            (identical(other.npub, npub) || other.npub == npub) &&
            (identical(other.displayName, displayName) ||
                other.displayName == displayName) &&
            (identical(other.nip05, nip05) || other.nip05 == nip05) &&
            (identical(other.picture, picture) || other.picture == picture));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    accountId,
    pubkeyHex,
    npub,
    displayName,
    nip05,
    picture,
  );

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AppEvent_NostrProfileUpdatedImplCopyWith<
    _$AppEvent_NostrProfileUpdatedImpl
  >
  get copyWith =>
      __$$AppEvent_NostrProfileUpdatedImplCopyWithImpl<
        _$AppEvent_NostrProfileUpdatedImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )
    accountConnectionChanged,
    required TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )
    folderListUpdated,
    required TResult Function(String accountId, String folderName, int unread)
    folderFound,
    required TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )
    messageFlagsChanged,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )
    messageListWindowStarted,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )
    messageListRowFound,
    required TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )
    messageListWindowComplete,
    required TResult Function(String? requestId, bool ok, String? error)
    commandResult,
    required TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )
    nostrProfileUpdated,
  }) {
    return nostrProfileUpdated(
      accountId,
      pubkeyHex,
      npub,
      displayName,
      nip05,
      picture,
    );
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult? Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult? Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult? Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult? Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult? Function(String? requestId, bool ok, String? error)? commandResult,
    TResult? Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
  }) {
    return nostrProfileUpdated?.call(
      accountId,
      pubkeyHex,
      npub,
      displayName,
      nip05,
      picture,
    );
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String accountId,
      String storeKind,
      String connectionState,
      String? message,
    )?
    accountConnectionChanged,
    TResult Function(
      String accountId,
      List<String> folders,
      String? hierarchyDelimiter,
      Map<String, int> unreadByFolder,
      Map<String, String> folderDisplayNames,
      List<SubscriptionAvailableRow>? subscriptionAvailable,
    )?
    folderListUpdated,
    TResult Function(String accountId, String folderName, int unread)?
    folderFound,
    TResult Function(
      String accountId,
      String folder,
      String messageId,
      bool isRead,
    )?
    messageFlagsChanged,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt total,
      BigInt startIndex,
      String listStrategy,
      int rowCount,
      bool listReady,
    )?
    messageListWindowStarted,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      BigInt rank,
      MessageListRowSummary summary,
    )?
    messageListRowFound,
    TResult Function(
      String requestId,
      String accountId,
      String folderName,
      String messageListSort,
      String? error,
    )?
    messageListWindowComplete,
    TResult Function(String? requestId, bool ok, String? error)? commandResult,
    TResult Function(
      String accountId,
      String pubkeyHex,
      String npub,
      String? displayName,
      String? nip05,
      String? picture,
    )?
    nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (nostrProfileUpdated != null) {
      return nostrProfileUpdated(
        accountId,
        pubkeyHex,
        npub,
        displayName,
        nip05,
        picture,
      );
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AppEvent_AccountConnectionChanged value)
    accountConnectionChanged,
    required TResult Function(AppEvent_FolderListUpdated value)
    folderListUpdated,
    required TResult Function(AppEvent_FolderFound value) folderFound,
    required TResult Function(AppEvent_MessageFlagsChanged value)
    messageFlagsChanged,
    required TResult Function(AppEvent_MessageListWindowStarted value)
    messageListWindowStarted,
    required TResult Function(AppEvent_MessageListRowFound value)
    messageListRowFound,
    required TResult Function(AppEvent_MessageListWindowComplete value)
    messageListWindowComplete,
    required TResult Function(AppEvent_CommandResult value) commandResult,
    required TResult Function(AppEvent_NostrProfileUpdated value)
    nostrProfileUpdated,
  }) {
    return nostrProfileUpdated(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult? Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult? Function(AppEvent_FolderFound value)? folderFound,
    TResult? Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult? Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult? Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult? Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult? Function(AppEvent_CommandResult value)? commandResult,
    TResult? Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
  }) {
    return nostrProfileUpdated?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AppEvent_AccountConnectionChanged value)?
    accountConnectionChanged,
    TResult Function(AppEvent_FolderListUpdated value)? folderListUpdated,
    TResult Function(AppEvent_FolderFound value)? folderFound,
    TResult Function(AppEvent_MessageFlagsChanged value)? messageFlagsChanged,
    TResult Function(AppEvent_MessageListWindowStarted value)?
    messageListWindowStarted,
    TResult Function(AppEvent_MessageListRowFound value)? messageListRowFound,
    TResult Function(AppEvent_MessageListWindowComplete value)?
    messageListWindowComplete,
    TResult Function(AppEvent_CommandResult value)? commandResult,
    TResult Function(AppEvent_NostrProfileUpdated value)? nostrProfileUpdated,
    required TResult orElse(),
  }) {
    if (nostrProfileUpdated != null) {
      return nostrProfileUpdated(this);
    }
    return orElse();
  }
}

abstract class AppEvent_NostrProfileUpdated extends AppEvent {
  const factory AppEvent_NostrProfileUpdated({
    required final String accountId,
    required final String pubkeyHex,
    required final String npub,
    final String? displayName,
    final String? nip05,
    final String? picture,
  }) = _$AppEvent_NostrProfileUpdatedImpl;
  const AppEvent_NostrProfileUpdated._() : super._();

  String get accountId;

  /// Lowercase hex pubkey (folder id for DM conversations).
  String get pubkeyHex;

  /// npub (bech32) for display fallback.
  String get npub;
  String? get displayName;
  String? get nip05;
  String? get picture;

  /// Create a copy of AppEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AppEvent_NostrProfileUpdatedImplCopyWith<
    _$AppEvent_NostrProfileUpdatedImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}
