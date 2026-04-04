// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'frb_json.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

/// @nodoc
mixin _$AccountParseState {
  FrbAccount get acc => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      FrbAccount acc,
      String? key,
      bool inLegacyTransportIds,
    )
    top,
    required TResult Function(FrbAccount acc, String? key) inAttrs,
    required TResult Function(FrbAccount acc, String? listKey, bool inArray)
    inLists,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult? Function(FrbAccount acc, String? key)? inAttrs,
    TResult? Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult Function(FrbAccount acc, String? key)? inAttrs,
    TResult Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AccountParseState_Top value) top,
    required TResult Function(AccountParseState_InAttrs value) inAttrs,
    required TResult Function(AccountParseState_InLists value) inLists,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AccountParseState_Top value)? top,
    TResult? Function(AccountParseState_InAttrs value)? inAttrs,
    TResult? Function(AccountParseState_InLists value)? inLists,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AccountParseState_Top value)? top,
    TResult Function(AccountParseState_InAttrs value)? inAttrs,
    TResult Function(AccountParseState_InLists value)? inLists,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  $AccountParseStateCopyWith<AccountParseState> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $AccountParseStateCopyWith<$Res> {
  factory $AccountParseStateCopyWith(
    AccountParseState value,
    $Res Function(AccountParseState) then,
  ) = _$AccountParseStateCopyWithImpl<$Res, AccountParseState>;
  @useResult
  $Res call({FrbAccount acc});
}

/// @nodoc
class _$AccountParseStateCopyWithImpl<$Res, $Val extends AccountParseState>
    implements $AccountParseStateCopyWith<$Res> {
  _$AccountParseStateCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? acc = null}) {
    return _then(
      _value.copyWith(
            acc: null == acc
                ? _value.acc
                : acc // ignore: cast_nullable_to_non_nullable
                      as FrbAccount,
          )
          as $Val,
    );
  }
}

/// @nodoc
abstract class _$$AccountParseState_TopImplCopyWith<$Res>
    implements $AccountParseStateCopyWith<$Res> {
  factory _$$AccountParseState_TopImplCopyWith(
    _$AccountParseState_TopImpl value,
    $Res Function(_$AccountParseState_TopImpl) then,
  ) = __$$AccountParseState_TopImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({FrbAccount acc, String? key, bool inLegacyTransportIds});
}

/// @nodoc
class __$$AccountParseState_TopImplCopyWithImpl<$Res>
    extends _$AccountParseStateCopyWithImpl<$Res, _$AccountParseState_TopImpl>
    implements _$$AccountParseState_TopImplCopyWith<$Res> {
  __$$AccountParseState_TopImplCopyWithImpl(
    _$AccountParseState_TopImpl _value,
    $Res Function(_$AccountParseState_TopImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? acc = null,
    Object? key = freezed,
    Object? inLegacyTransportIds = null,
  }) {
    return _then(
      _$AccountParseState_TopImpl(
        acc: null == acc
            ? _value.acc
            : acc // ignore: cast_nullable_to_non_nullable
                  as FrbAccount,
        key: freezed == key
            ? _value.key
            : key // ignore: cast_nullable_to_non_nullable
                  as String?,
        inLegacyTransportIds: null == inLegacyTransportIds
            ? _value.inLegacyTransportIds
            : inLegacyTransportIds // ignore: cast_nullable_to_non_nullable
                  as bool,
      ),
    );
  }
}

/// @nodoc

class _$AccountParseState_TopImpl extends AccountParseState_Top {
  const _$AccountParseState_TopImpl({
    required this.acc,
    this.key,
    required this.inLegacyTransportIds,
  }) : super._();

  @override
  final FrbAccount acc;
  @override
  final String? key;
  @override
  final bool inLegacyTransportIds;

  @override
  String toString() {
    return 'AccountParseState.top(acc: $acc, key: $key, inLegacyTransportIds: $inLegacyTransportIds)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AccountParseState_TopImpl &&
            (identical(other.acc, acc) || other.acc == acc) &&
            (identical(other.key, key) || other.key == key) &&
            (identical(other.inLegacyTransportIds, inLegacyTransportIds) ||
                other.inLegacyTransportIds == inLegacyTransportIds));
  }

  @override
  int get hashCode => Object.hash(runtimeType, acc, key, inLegacyTransportIds);

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AccountParseState_TopImplCopyWith<_$AccountParseState_TopImpl>
  get copyWith =>
      __$$AccountParseState_TopImplCopyWithImpl<_$AccountParseState_TopImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      FrbAccount acc,
      String? key,
      bool inLegacyTransportIds,
    )
    top,
    required TResult Function(FrbAccount acc, String? key) inAttrs,
    required TResult Function(FrbAccount acc, String? listKey, bool inArray)
    inLists,
  }) {
    return top(acc, key, inLegacyTransportIds);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult? Function(FrbAccount acc, String? key)? inAttrs,
    TResult? Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
  }) {
    return top?.call(acc, key, inLegacyTransportIds);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult Function(FrbAccount acc, String? key)? inAttrs,
    TResult Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
    required TResult orElse(),
  }) {
    if (top != null) {
      return top(acc, key, inLegacyTransportIds);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AccountParseState_Top value) top,
    required TResult Function(AccountParseState_InAttrs value) inAttrs,
    required TResult Function(AccountParseState_InLists value) inLists,
  }) {
    return top(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AccountParseState_Top value)? top,
    TResult? Function(AccountParseState_InAttrs value)? inAttrs,
    TResult? Function(AccountParseState_InLists value)? inLists,
  }) {
    return top?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AccountParseState_Top value)? top,
    TResult Function(AccountParseState_InAttrs value)? inAttrs,
    TResult Function(AccountParseState_InLists value)? inLists,
    required TResult orElse(),
  }) {
    if (top != null) {
      return top(this);
    }
    return orElse();
  }
}

abstract class AccountParseState_Top extends AccountParseState {
  const factory AccountParseState_Top({
    required final FrbAccount acc,
    final String? key,
    required final bool inLegacyTransportIds,
  }) = _$AccountParseState_TopImpl;
  const AccountParseState_Top._() : super._();

  @override
  FrbAccount get acc;
  String? get key;
  bool get inLegacyTransportIds;

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AccountParseState_TopImplCopyWith<_$AccountParseState_TopImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AccountParseState_InAttrsImplCopyWith<$Res>
    implements $AccountParseStateCopyWith<$Res> {
  factory _$$AccountParseState_InAttrsImplCopyWith(
    _$AccountParseState_InAttrsImpl value,
    $Res Function(_$AccountParseState_InAttrsImpl) then,
  ) = __$$AccountParseState_InAttrsImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({FrbAccount acc, String? key});
}

/// @nodoc
class __$$AccountParseState_InAttrsImplCopyWithImpl<$Res>
    extends
        _$AccountParseStateCopyWithImpl<$Res, _$AccountParseState_InAttrsImpl>
    implements _$$AccountParseState_InAttrsImplCopyWith<$Res> {
  __$$AccountParseState_InAttrsImplCopyWithImpl(
    _$AccountParseState_InAttrsImpl _value,
    $Res Function(_$AccountParseState_InAttrsImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? acc = null, Object? key = freezed}) {
    return _then(
      _$AccountParseState_InAttrsImpl(
        acc: null == acc
            ? _value.acc
            : acc // ignore: cast_nullable_to_non_nullable
                  as FrbAccount,
        key: freezed == key
            ? _value.key
            : key // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$AccountParseState_InAttrsImpl extends AccountParseState_InAttrs {
  const _$AccountParseState_InAttrsImpl({required this.acc, this.key})
    : super._();

  @override
  final FrbAccount acc;
  @override
  final String? key;

  @override
  String toString() {
    return 'AccountParseState.inAttrs(acc: $acc, key: $key)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AccountParseState_InAttrsImpl &&
            (identical(other.acc, acc) || other.acc == acc) &&
            (identical(other.key, key) || other.key == key));
  }

  @override
  int get hashCode => Object.hash(runtimeType, acc, key);

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AccountParseState_InAttrsImplCopyWith<_$AccountParseState_InAttrsImpl>
  get copyWith =>
      __$$AccountParseState_InAttrsImplCopyWithImpl<
        _$AccountParseState_InAttrsImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      FrbAccount acc,
      String? key,
      bool inLegacyTransportIds,
    )
    top,
    required TResult Function(FrbAccount acc, String? key) inAttrs,
    required TResult Function(FrbAccount acc, String? listKey, bool inArray)
    inLists,
  }) {
    return inAttrs(acc, key);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult? Function(FrbAccount acc, String? key)? inAttrs,
    TResult? Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
  }) {
    return inAttrs?.call(acc, key);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult Function(FrbAccount acc, String? key)? inAttrs,
    TResult Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
    required TResult orElse(),
  }) {
    if (inAttrs != null) {
      return inAttrs(acc, key);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AccountParseState_Top value) top,
    required TResult Function(AccountParseState_InAttrs value) inAttrs,
    required TResult Function(AccountParseState_InLists value) inLists,
  }) {
    return inAttrs(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AccountParseState_Top value)? top,
    TResult? Function(AccountParseState_InAttrs value)? inAttrs,
    TResult? Function(AccountParseState_InLists value)? inLists,
  }) {
    return inAttrs?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AccountParseState_Top value)? top,
    TResult Function(AccountParseState_InAttrs value)? inAttrs,
    TResult Function(AccountParseState_InLists value)? inLists,
    required TResult orElse(),
  }) {
    if (inAttrs != null) {
      return inAttrs(this);
    }
    return orElse();
  }
}

abstract class AccountParseState_InAttrs extends AccountParseState {
  const factory AccountParseState_InAttrs({
    required final FrbAccount acc,
    final String? key,
  }) = _$AccountParseState_InAttrsImpl;
  const AccountParseState_InAttrs._() : super._();

  @override
  FrbAccount get acc;
  String? get key;

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AccountParseState_InAttrsImplCopyWith<_$AccountParseState_InAttrsImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$AccountParseState_InListsImplCopyWith<$Res>
    implements $AccountParseStateCopyWith<$Res> {
  factory _$$AccountParseState_InListsImplCopyWith(
    _$AccountParseState_InListsImpl value,
    $Res Function(_$AccountParseState_InListsImpl) then,
  ) = __$$AccountParseState_InListsImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({FrbAccount acc, String? listKey, bool inArray});
}

/// @nodoc
class __$$AccountParseState_InListsImplCopyWithImpl<$Res>
    extends
        _$AccountParseStateCopyWithImpl<$Res, _$AccountParseState_InListsImpl>
    implements _$$AccountParseState_InListsImplCopyWith<$Res> {
  __$$AccountParseState_InListsImplCopyWithImpl(
    _$AccountParseState_InListsImpl _value,
    $Res Function(_$AccountParseState_InListsImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? acc = null,
    Object? listKey = freezed,
    Object? inArray = null,
  }) {
    return _then(
      _$AccountParseState_InListsImpl(
        acc: null == acc
            ? _value.acc
            : acc // ignore: cast_nullable_to_non_nullable
                  as FrbAccount,
        listKey: freezed == listKey
            ? _value.listKey
            : listKey // ignore: cast_nullable_to_non_nullable
                  as String?,
        inArray: null == inArray
            ? _value.inArray
            : inArray // ignore: cast_nullable_to_non_nullable
                  as bool,
      ),
    );
  }
}

/// @nodoc

class _$AccountParseState_InListsImpl extends AccountParseState_InLists {
  const _$AccountParseState_InListsImpl({
    required this.acc,
    this.listKey,
    required this.inArray,
  }) : super._();

  @override
  final FrbAccount acc;
  @override
  final String? listKey;
  @override
  final bool inArray;

  @override
  String toString() {
    return 'AccountParseState.inLists(acc: $acc, listKey: $listKey, inArray: $inArray)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$AccountParseState_InListsImpl &&
            (identical(other.acc, acc) || other.acc == acc) &&
            (identical(other.listKey, listKey) || other.listKey == listKey) &&
            (identical(other.inArray, inArray) || other.inArray == inArray));
  }

  @override
  int get hashCode => Object.hash(runtimeType, acc, listKey, inArray);

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$AccountParseState_InListsImplCopyWith<_$AccountParseState_InListsImpl>
  get copyWith =>
      __$$AccountParseState_InListsImplCopyWithImpl<
        _$AccountParseState_InListsImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      FrbAccount acc,
      String? key,
      bool inLegacyTransportIds,
    )
    top,
    required TResult Function(FrbAccount acc, String? key) inAttrs,
    required TResult Function(FrbAccount acc, String? listKey, bool inArray)
    inLists,
  }) {
    return inLists(acc, listKey, inArray);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult? Function(FrbAccount acc, String? key)? inAttrs,
    TResult? Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
  }) {
    return inLists?.call(acc, listKey, inArray);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(FrbAccount acc, String? key, bool inLegacyTransportIds)?
    top,
    TResult Function(FrbAccount acc, String? key)? inAttrs,
    TResult Function(FrbAccount acc, String? listKey, bool inArray)? inLists,
    required TResult orElse(),
  }) {
    if (inLists != null) {
      return inLists(acc, listKey, inArray);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(AccountParseState_Top value) top,
    required TResult Function(AccountParseState_InAttrs value) inAttrs,
    required TResult Function(AccountParseState_InLists value) inLists,
  }) {
    return inLists(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(AccountParseState_Top value)? top,
    TResult? Function(AccountParseState_InAttrs value)? inAttrs,
    TResult? Function(AccountParseState_InLists value)? inLists,
  }) {
    return inLists?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(AccountParseState_Top value)? top,
    TResult Function(AccountParseState_InAttrs value)? inAttrs,
    TResult Function(AccountParseState_InLists value)? inLists,
    required TResult orElse(),
  }) {
    if (inLists != null) {
      return inLists(this);
    }
    return orElse();
  }
}

abstract class AccountParseState_InLists extends AccountParseState {
  const factory AccountParseState_InLists({
    required final FrbAccount acc,
    final String? listKey,
    required final bool inArray,
  }) = _$AccountParseState_InListsImpl;
  const AccountParseState_InLists._() : super._();

  @override
  FrbAccount get acc;
  String? get listKey;
  bool get inArray;

  /// Create a copy of AccountParseState
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$AccountParseState_InListsImplCopyWith<_$AccountParseState_InListsImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
mixin _$CfgStack {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String? key) root,
    required TResult Function() inAccountsArray,
    required TResult Function(AccountParseState field0) inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(AccountParseState field0)? inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(AccountParseState field0)? inAccount,
    TResult Function()? inTransportsArray,
    TResult Function(FrbTransport t, String? key)? inTransport,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(CfgStack_Root value) root,
    required TResult Function(CfgStack_InAccountsArray value) inAccountsArray,
    required TResult Function(CfgStack_InAccount value) inAccount,
    required TResult Function(CfgStack_InTransportsArray value)
    inTransportsArray,
    required TResult Function(CfgStack_InTransport value) inTransport,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(CfgStack_Root value)? root,
    TResult? Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult? Function(CfgStack_InAccount value)? inAccount,
    TResult? Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult? Function(CfgStack_InTransport value)? inTransport,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(CfgStack_Root value)? root,
    TResult Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult Function(CfgStack_InAccount value)? inAccount,
    TResult Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult Function(CfgStack_InTransport value)? inTransport,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $CfgStackCopyWith<$Res> {
  factory $CfgStackCopyWith(CfgStack value, $Res Function(CfgStack) then) =
      _$CfgStackCopyWithImpl<$Res, CfgStack>;
}

/// @nodoc
class _$CfgStackCopyWithImpl<$Res, $Val extends CfgStack>
    implements $CfgStackCopyWith<$Res> {
  _$CfgStackCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$CfgStack_RootImplCopyWith<$Res> {
  factory _$$CfgStack_RootImplCopyWith(
    _$CfgStack_RootImpl value,
    $Res Function(_$CfgStack_RootImpl) then,
  ) = __$$CfgStack_RootImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String? key});
}

/// @nodoc
class __$$CfgStack_RootImplCopyWithImpl<$Res>
    extends _$CfgStackCopyWithImpl<$Res, _$CfgStack_RootImpl>
    implements _$$CfgStack_RootImplCopyWith<$Res> {
  __$$CfgStack_RootImplCopyWithImpl(
    _$CfgStack_RootImpl _value,
    $Res Function(_$CfgStack_RootImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? key = freezed}) {
    return _then(
      _$CfgStack_RootImpl(
        key: freezed == key
            ? _value.key
            : key // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$CfgStack_RootImpl extends CfgStack_Root {
  const _$CfgStack_RootImpl({this.key}) : super._();

  @override
  final String? key;

  @override
  String toString() {
    return 'CfgStack.root(key: $key)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$CfgStack_RootImpl &&
            (identical(other.key, key) || other.key == key));
  }

  @override
  int get hashCode => Object.hash(runtimeType, key);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$CfgStack_RootImplCopyWith<_$CfgStack_RootImpl> get copyWith =>
      __$$CfgStack_RootImplCopyWithImpl<_$CfgStack_RootImpl>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String? key) root,
    required TResult Function() inAccountsArray,
    required TResult Function(AccountParseState field0) inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) {
    return root(key);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(AccountParseState field0)? inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) {
    return root?.call(key);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(AccountParseState field0)? inAccount,
    TResult Function()? inTransportsArray,
    TResult Function(FrbTransport t, String? key)? inTransport,
    required TResult orElse(),
  }) {
    if (root != null) {
      return root(key);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(CfgStack_Root value) root,
    required TResult Function(CfgStack_InAccountsArray value) inAccountsArray,
    required TResult Function(CfgStack_InAccount value) inAccount,
    required TResult Function(CfgStack_InTransportsArray value)
    inTransportsArray,
    required TResult Function(CfgStack_InTransport value) inTransport,
  }) {
    return root(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(CfgStack_Root value)? root,
    TResult? Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult? Function(CfgStack_InAccount value)? inAccount,
    TResult? Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult? Function(CfgStack_InTransport value)? inTransport,
  }) {
    return root?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(CfgStack_Root value)? root,
    TResult Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult Function(CfgStack_InAccount value)? inAccount,
    TResult Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult Function(CfgStack_InTransport value)? inTransport,
    required TResult orElse(),
  }) {
    if (root != null) {
      return root(this);
    }
    return orElse();
  }
}

abstract class CfgStack_Root extends CfgStack {
  const factory CfgStack_Root({final String? key}) = _$CfgStack_RootImpl;
  const CfgStack_Root._() : super._();

  String? get key;

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$CfgStack_RootImplCopyWith<_$CfgStack_RootImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$CfgStack_InAccountsArrayImplCopyWith<$Res> {
  factory _$$CfgStack_InAccountsArrayImplCopyWith(
    _$CfgStack_InAccountsArrayImpl value,
    $Res Function(_$CfgStack_InAccountsArrayImpl) then,
  ) = __$$CfgStack_InAccountsArrayImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$CfgStack_InAccountsArrayImplCopyWithImpl<$Res>
    extends _$CfgStackCopyWithImpl<$Res, _$CfgStack_InAccountsArrayImpl>
    implements _$$CfgStack_InAccountsArrayImplCopyWith<$Res> {
  __$$CfgStack_InAccountsArrayImplCopyWithImpl(
    _$CfgStack_InAccountsArrayImpl _value,
    $Res Function(_$CfgStack_InAccountsArrayImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$CfgStack_InAccountsArrayImpl extends CfgStack_InAccountsArray {
  const _$CfgStack_InAccountsArrayImpl() : super._();

  @override
  String toString() {
    return 'CfgStack.inAccountsArray()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$CfgStack_InAccountsArrayImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String? key) root,
    required TResult Function() inAccountsArray,
    required TResult Function(AccountParseState field0) inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) {
    return inAccountsArray();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(AccountParseState field0)? inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) {
    return inAccountsArray?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(AccountParseState field0)? inAccount,
    TResult Function()? inTransportsArray,
    TResult Function(FrbTransport t, String? key)? inTransport,
    required TResult orElse(),
  }) {
    if (inAccountsArray != null) {
      return inAccountsArray();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(CfgStack_Root value) root,
    required TResult Function(CfgStack_InAccountsArray value) inAccountsArray,
    required TResult Function(CfgStack_InAccount value) inAccount,
    required TResult Function(CfgStack_InTransportsArray value)
    inTransportsArray,
    required TResult Function(CfgStack_InTransport value) inTransport,
  }) {
    return inAccountsArray(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(CfgStack_Root value)? root,
    TResult? Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult? Function(CfgStack_InAccount value)? inAccount,
    TResult? Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult? Function(CfgStack_InTransport value)? inTransport,
  }) {
    return inAccountsArray?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(CfgStack_Root value)? root,
    TResult Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult Function(CfgStack_InAccount value)? inAccount,
    TResult Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult Function(CfgStack_InTransport value)? inTransport,
    required TResult orElse(),
  }) {
    if (inAccountsArray != null) {
      return inAccountsArray(this);
    }
    return orElse();
  }
}

abstract class CfgStack_InAccountsArray extends CfgStack {
  const factory CfgStack_InAccountsArray() = _$CfgStack_InAccountsArrayImpl;
  const CfgStack_InAccountsArray._() : super._();
}

/// @nodoc
abstract class _$$CfgStack_InAccountImplCopyWith<$Res> {
  factory _$$CfgStack_InAccountImplCopyWith(
    _$CfgStack_InAccountImpl value,
    $Res Function(_$CfgStack_InAccountImpl) then,
  ) = __$$CfgStack_InAccountImplCopyWithImpl<$Res>;
  @useResult
  $Res call({AccountParseState field0});

  $AccountParseStateCopyWith<$Res> get field0;
}

/// @nodoc
class __$$CfgStack_InAccountImplCopyWithImpl<$Res>
    extends _$CfgStackCopyWithImpl<$Res, _$CfgStack_InAccountImpl>
    implements _$$CfgStack_InAccountImplCopyWith<$Res> {
  __$$CfgStack_InAccountImplCopyWithImpl(
    _$CfgStack_InAccountImpl _value,
    $Res Function(_$CfgStack_InAccountImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? field0 = null}) {
    return _then(
      _$CfgStack_InAccountImpl(
        null == field0
            ? _value.field0
            : field0 // ignore: cast_nullable_to_non_nullable
                  as AccountParseState,
      ),
    );
  }

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $AccountParseStateCopyWith<$Res> get field0 {
    return $AccountParseStateCopyWith<$Res>(_value.field0, (value) {
      return _then(_value.copyWith(field0: value));
    });
  }
}

/// @nodoc

class _$CfgStack_InAccountImpl extends CfgStack_InAccount {
  const _$CfgStack_InAccountImpl(this.field0) : super._();

  @override
  final AccountParseState field0;

  @override
  String toString() {
    return 'CfgStack.inAccount(field0: $field0)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$CfgStack_InAccountImpl &&
            (identical(other.field0, field0) || other.field0 == field0));
  }

  @override
  int get hashCode => Object.hash(runtimeType, field0);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$CfgStack_InAccountImplCopyWith<_$CfgStack_InAccountImpl> get copyWith =>
      __$$CfgStack_InAccountImplCopyWithImpl<_$CfgStack_InAccountImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String? key) root,
    required TResult Function() inAccountsArray,
    required TResult Function(AccountParseState field0) inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) {
    return inAccount(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(AccountParseState field0)? inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) {
    return inAccount?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(AccountParseState field0)? inAccount,
    TResult Function()? inTransportsArray,
    TResult Function(FrbTransport t, String? key)? inTransport,
    required TResult orElse(),
  }) {
    if (inAccount != null) {
      return inAccount(field0);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(CfgStack_Root value) root,
    required TResult Function(CfgStack_InAccountsArray value) inAccountsArray,
    required TResult Function(CfgStack_InAccount value) inAccount,
    required TResult Function(CfgStack_InTransportsArray value)
    inTransportsArray,
    required TResult Function(CfgStack_InTransport value) inTransport,
  }) {
    return inAccount(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(CfgStack_Root value)? root,
    TResult? Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult? Function(CfgStack_InAccount value)? inAccount,
    TResult? Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult? Function(CfgStack_InTransport value)? inTransport,
  }) {
    return inAccount?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(CfgStack_Root value)? root,
    TResult Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult Function(CfgStack_InAccount value)? inAccount,
    TResult Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult Function(CfgStack_InTransport value)? inTransport,
    required TResult orElse(),
  }) {
    if (inAccount != null) {
      return inAccount(this);
    }
    return orElse();
  }
}

abstract class CfgStack_InAccount extends CfgStack {
  const factory CfgStack_InAccount(final AccountParseState field0) =
      _$CfgStack_InAccountImpl;
  const CfgStack_InAccount._() : super._();

  AccountParseState get field0;

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$CfgStack_InAccountImplCopyWith<_$CfgStack_InAccountImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$CfgStack_InTransportsArrayImplCopyWith<$Res> {
  factory _$$CfgStack_InTransportsArrayImplCopyWith(
    _$CfgStack_InTransportsArrayImpl value,
    $Res Function(_$CfgStack_InTransportsArrayImpl) then,
  ) = __$$CfgStack_InTransportsArrayImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$CfgStack_InTransportsArrayImplCopyWithImpl<$Res>
    extends _$CfgStackCopyWithImpl<$Res, _$CfgStack_InTransportsArrayImpl>
    implements _$$CfgStack_InTransportsArrayImplCopyWith<$Res> {
  __$$CfgStack_InTransportsArrayImplCopyWithImpl(
    _$CfgStack_InTransportsArrayImpl _value,
    $Res Function(_$CfgStack_InTransportsArrayImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$CfgStack_InTransportsArrayImpl extends CfgStack_InTransportsArray {
  const _$CfgStack_InTransportsArrayImpl() : super._();

  @override
  String toString() {
    return 'CfgStack.inTransportsArray()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$CfgStack_InTransportsArrayImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String? key) root,
    required TResult Function() inAccountsArray,
    required TResult Function(AccountParseState field0) inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) {
    return inTransportsArray();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(AccountParseState field0)? inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) {
    return inTransportsArray?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(AccountParseState field0)? inAccount,
    TResult Function()? inTransportsArray,
    TResult Function(FrbTransport t, String? key)? inTransport,
    required TResult orElse(),
  }) {
    if (inTransportsArray != null) {
      return inTransportsArray();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(CfgStack_Root value) root,
    required TResult Function(CfgStack_InAccountsArray value) inAccountsArray,
    required TResult Function(CfgStack_InAccount value) inAccount,
    required TResult Function(CfgStack_InTransportsArray value)
    inTransportsArray,
    required TResult Function(CfgStack_InTransport value) inTransport,
  }) {
    return inTransportsArray(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(CfgStack_Root value)? root,
    TResult? Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult? Function(CfgStack_InAccount value)? inAccount,
    TResult? Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult? Function(CfgStack_InTransport value)? inTransport,
  }) {
    return inTransportsArray?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(CfgStack_Root value)? root,
    TResult Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult Function(CfgStack_InAccount value)? inAccount,
    TResult Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult Function(CfgStack_InTransport value)? inTransport,
    required TResult orElse(),
  }) {
    if (inTransportsArray != null) {
      return inTransportsArray(this);
    }
    return orElse();
  }
}

abstract class CfgStack_InTransportsArray extends CfgStack {
  const factory CfgStack_InTransportsArray() = _$CfgStack_InTransportsArrayImpl;
  const CfgStack_InTransportsArray._() : super._();
}

/// @nodoc
abstract class _$$CfgStack_InTransportImplCopyWith<$Res> {
  factory _$$CfgStack_InTransportImplCopyWith(
    _$CfgStack_InTransportImpl value,
    $Res Function(_$CfgStack_InTransportImpl) then,
  ) = __$$CfgStack_InTransportImplCopyWithImpl<$Res>;
  @useResult
  $Res call({FrbTransport t, String? key});
}

/// @nodoc
class __$$CfgStack_InTransportImplCopyWithImpl<$Res>
    extends _$CfgStackCopyWithImpl<$Res, _$CfgStack_InTransportImpl>
    implements _$$CfgStack_InTransportImplCopyWith<$Res> {
  __$$CfgStack_InTransportImplCopyWithImpl(
    _$CfgStack_InTransportImpl _value,
    $Res Function(_$CfgStack_InTransportImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? t = null, Object? key = freezed}) {
    return _then(
      _$CfgStack_InTransportImpl(
        t: null == t
            ? _value.t
            : t // ignore: cast_nullable_to_non_nullable
                  as FrbTransport,
        key: freezed == key
            ? _value.key
            : key // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc

class _$CfgStack_InTransportImpl extends CfgStack_InTransport {
  const _$CfgStack_InTransportImpl({required this.t, this.key}) : super._();

  @override
  final FrbTransport t;
  @override
  final String? key;

  @override
  String toString() {
    return 'CfgStack.inTransport(t: $t, key: $key)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$CfgStack_InTransportImpl &&
            (identical(other.t, t) || other.t == t) &&
            (identical(other.key, key) || other.key == key));
  }

  @override
  int get hashCode => Object.hash(runtimeType, t, key);

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$CfgStack_InTransportImplCopyWith<_$CfgStack_InTransportImpl>
  get copyWith =>
      __$$CfgStack_InTransportImplCopyWithImpl<_$CfgStack_InTransportImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String? key) root,
    required TResult Function() inAccountsArray,
    required TResult Function(AccountParseState field0) inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) {
    return inTransport(t, key);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(AccountParseState field0)? inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) {
    return inTransport?.call(t, key);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(AccountParseState field0)? inAccount,
    TResult Function()? inTransportsArray,
    TResult Function(FrbTransport t, String? key)? inTransport,
    required TResult orElse(),
  }) {
    if (inTransport != null) {
      return inTransport(t, key);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(CfgStack_Root value) root,
    required TResult Function(CfgStack_InAccountsArray value) inAccountsArray,
    required TResult Function(CfgStack_InAccount value) inAccount,
    required TResult Function(CfgStack_InTransportsArray value)
    inTransportsArray,
    required TResult Function(CfgStack_InTransport value) inTransport,
  }) {
    return inTransport(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(CfgStack_Root value)? root,
    TResult? Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult? Function(CfgStack_InAccount value)? inAccount,
    TResult? Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult? Function(CfgStack_InTransport value)? inTransport,
  }) {
    return inTransport?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(CfgStack_Root value)? root,
    TResult Function(CfgStack_InAccountsArray value)? inAccountsArray,
    TResult Function(CfgStack_InAccount value)? inAccount,
    TResult Function(CfgStack_InTransportsArray value)? inTransportsArray,
    TResult Function(CfgStack_InTransport value)? inTransport,
    required TResult orElse(),
  }) {
    if (inTransport != null) {
      return inTransport(this);
    }
    return orElse();
  }
}

abstract class CfgStack_InTransport extends CfgStack {
  const factory CfgStack_InTransport({
    required final FrbTransport t,
    final String? key,
  }) = _$CfgStack_InTransportImpl;
  const CfgStack_InTransport._() : super._();

  FrbTransport get t;
  String? get key;

  /// Create a copy of CfgStack
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$CfgStack_InTransportImplCopyWith<_$CfgStack_InTransportImpl>
  get copyWith => throw _privateConstructorUsedError;
}
