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
mixin _$CfgStack {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String? key) root,
    required TResult Function() inAccountsArray,
    required TResult Function(FrbAccount acc, String? key, bool inTransportIds)
    inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
    required TResult Function(FrbAccount acc, String? key, bool inTransportIds)
    inAccount,
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
    TResult? Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
    TResult Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
    required TResult Function(FrbAccount acc, String? key, bool inTransportIds)
    inAccount,
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
    TResult? Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
    TResult Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
  $Res call({FrbAccount acc, String? key, bool inTransportIds});
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
  $Res call({
    Object? acc = null,
    Object? key = freezed,
    Object? inTransportIds = null,
  }) {
    return _then(
      _$CfgStack_InAccountImpl(
        acc: null == acc
            ? _value.acc
            : acc // ignore: cast_nullable_to_non_nullable
                  as FrbAccount,
        key: freezed == key
            ? _value.key
            : key // ignore: cast_nullable_to_non_nullable
                  as String?,
        inTransportIds: null == inTransportIds
            ? _value.inTransportIds
            : inTransportIds // ignore: cast_nullable_to_non_nullable
                  as bool,
      ),
    );
  }
}

/// @nodoc

class _$CfgStack_InAccountImpl extends CfgStack_InAccount {
  const _$CfgStack_InAccountImpl({
    required this.acc,
    this.key,
    required this.inTransportIds,
  }) : super._();

  @override
  final FrbAccount acc;
  @override
  final String? key;
  @override
  final bool inTransportIds;

  @override
  String toString() {
    return 'CfgStack.inAccount(acc: $acc, key: $key, inTransportIds: $inTransportIds)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$CfgStack_InAccountImpl &&
            (identical(other.acc, acc) || other.acc == acc) &&
            (identical(other.key, key) || other.key == key) &&
            (identical(other.inTransportIds, inTransportIds) ||
                other.inTransportIds == inTransportIds));
  }

  @override
  int get hashCode => Object.hash(runtimeType, acc, key, inTransportIds);

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
    required TResult Function(FrbAccount acc, String? key, bool inTransportIds)
    inAccount,
    required TResult Function() inTransportsArray,
    required TResult Function(FrbTransport t, String? key) inTransport,
  }) {
    return inAccount(acc, key, inTransportIds);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String? key)? root,
    TResult? Function()? inAccountsArray,
    TResult? Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
    TResult? Function()? inTransportsArray,
    TResult? Function(FrbTransport t, String? key)? inTransport,
  }) {
    return inAccount?.call(acc, key, inTransportIds);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String? key)? root,
    TResult Function()? inAccountsArray,
    TResult Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
    TResult Function()? inTransportsArray,
    TResult Function(FrbTransport t, String? key)? inTransport,
    required TResult orElse(),
  }) {
    if (inAccount != null) {
      return inAccount(acc, key, inTransportIds);
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
  const factory CfgStack_InAccount({
    required final FrbAccount acc,
    final String? key,
    required final bool inTransportIds,
  }) = _$CfgStack_InAccountImpl;
  const CfgStack_InAccount._() : super._();

  FrbAccount get acc;
  String? get key;
  bool get inTransportIds;

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
    required TResult Function(FrbAccount acc, String? key, bool inTransportIds)
    inAccount,
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
    TResult? Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
    TResult Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
    required TResult Function(FrbAccount acc, String? key, bool inTransportIds)
    inAccount,
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
    TResult? Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
    TResult Function(FrbAccount acc, String? key, bool inTransportIds)?
    inAccount,
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
