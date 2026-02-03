// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'stellatune_core.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

/// @nodoc
mixin _$Event {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $EventCopyWith<$Res> {
  factory $EventCopyWith(Event value, $Res Function(Event) then) =
      _$EventCopyWithImpl<$Res, Event>;
}

/// @nodoc
class _$EventCopyWithImpl<$Res, $Val extends Event>
    implements $EventCopyWith<$Res> {
  _$EventCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$Event_StateChangedImplCopyWith<$Res> {
  factory _$$Event_StateChangedImplCopyWith(
    _$Event_StateChangedImpl value,
    $Res Function(_$Event_StateChangedImpl) then,
  ) = __$$Event_StateChangedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({PlayerState state});
}

/// @nodoc
class __$$Event_StateChangedImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_StateChangedImpl>
    implements _$$Event_StateChangedImplCopyWith<$Res> {
  __$$Event_StateChangedImplCopyWithImpl(
    _$Event_StateChangedImpl _value,
    $Res Function(_$Event_StateChangedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? state = null}) {
    return _then(
      _$Event_StateChangedImpl(
        state: null == state
            ? _value.state
            : state // ignore: cast_nullable_to_non_nullable
                  as PlayerState,
      ),
    );
  }
}

/// @nodoc

class _$Event_StateChangedImpl extends Event_StateChanged {
  const _$Event_StateChangedImpl({required this.state}) : super._();

  @override
  final PlayerState state;

  @override
  String toString() {
    return 'Event.stateChanged(state: $state)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_StateChangedImpl &&
            (identical(other.state, state) || other.state == state));
  }

  @override
  int get hashCode => Object.hash(runtimeType, state);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_StateChangedImplCopyWith<_$Event_StateChangedImpl> get copyWith =>
      __$$Event_StateChangedImplCopyWithImpl<_$Event_StateChangedImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return stateChanged(state);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return stateChanged?.call(state);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (stateChanged != null) {
      return stateChanged(state);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) {
    return stateChanged(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) {
    return stateChanged?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    required TResult orElse(),
  }) {
    if (stateChanged != null) {
      return stateChanged(this);
    }
    return orElse();
  }
}

abstract class Event_StateChanged extends Event {
  const factory Event_StateChanged({required final PlayerState state}) =
      _$Event_StateChangedImpl;
  const Event_StateChanged._() : super._();

  PlayerState get state;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_StateChangedImplCopyWith<_$Event_StateChangedImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Event_PositionImplCopyWith<$Res> {
  factory _$$Event_PositionImplCopyWith(
    _$Event_PositionImpl value,
    $Res Function(_$Event_PositionImpl) then,
  ) = __$$Event_PositionImplCopyWithImpl<$Res>;
  @useResult
  $Res call({int ms});
}

/// @nodoc
class __$$Event_PositionImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_PositionImpl>
    implements _$$Event_PositionImplCopyWith<$Res> {
  __$$Event_PositionImplCopyWithImpl(
    _$Event_PositionImpl _value,
    $Res Function(_$Event_PositionImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? ms = null}) {
    return _then(
      _$Event_PositionImpl(
        ms: null == ms
            ? _value.ms
            : ms // ignore: cast_nullable_to_non_nullable
                  as int,
      ),
    );
  }
}

/// @nodoc

class _$Event_PositionImpl extends Event_Position {
  const _$Event_PositionImpl({required this.ms}) : super._();

  @override
  final int ms;

  @override
  String toString() {
    return 'Event.position(ms: $ms)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_PositionImpl &&
            (identical(other.ms, ms) || other.ms == ms));
  }

  @override
  int get hashCode => Object.hash(runtimeType, ms);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_PositionImplCopyWith<_$Event_PositionImpl> get copyWith =>
      __$$Event_PositionImplCopyWithImpl<_$Event_PositionImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return position(ms);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return position?.call(ms);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (position != null) {
      return position(ms);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) {
    return position(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) {
    return position?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    required TResult orElse(),
  }) {
    if (position != null) {
      return position(this);
    }
    return orElse();
  }
}

abstract class Event_Position extends Event {
  const factory Event_Position({required final int ms}) = _$Event_PositionImpl;
  const Event_Position._() : super._();

  int get ms;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_PositionImplCopyWith<_$Event_PositionImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Event_TrackChangedImplCopyWith<$Res> {
  factory _$$Event_TrackChangedImplCopyWith(
    _$Event_TrackChangedImpl value,
    $Res Function(_$Event_TrackChangedImpl) then,
  ) = __$$Event_TrackChangedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String path});
}

/// @nodoc
class __$$Event_TrackChangedImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_TrackChangedImpl>
    implements _$$Event_TrackChangedImplCopyWith<$Res> {
  __$$Event_TrackChangedImplCopyWithImpl(
    _$Event_TrackChangedImpl _value,
    $Res Function(_$Event_TrackChangedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? path = null}) {
    return _then(
      _$Event_TrackChangedImpl(
        path: null == path
            ? _value.path
            : path // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$Event_TrackChangedImpl extends Event_TrackChanged {
  const _$Event_TrackChangedImpl({required this.path}) : super._();

  @override
  final String path;

  @override
  String toString() {
    return 'Event.trackChanged(path: $path)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_TrackChangedImpl &&
            (identical(other.path, path) || other.path == path));
  }

  @override
  int get hashCode => Object.hash(runtimeType, path);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_TrackChangedImplCopyWith<_$Event_TrackChangedImpl> get copyWith =>
      __$$Event_TrackChangedImplCopyWithImpl<_$Event_TrackChangedImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return trackChanged(path);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return trackChanged?.call(path);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (trackChanged != null) {
      return trackChanged(path);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) {
    return trackChanged(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) {
    return trackChanged?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    required TResult orElse(),
  }) {
    if (trackChanged != null) {
      return trackChanged(this);
    }
    return orElse();
  }
}

abstract class Event_TrackChanged extends Event {
  const factory Event_TrackChanged({required final String path}) =
      _$Event_TrackChangedImpl;
  const Event_TrackChanged._() : super._();

  String get path;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_TrackChangedImplCopyWith<_$Event_TrackChangedImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Event_ErrorImplCopyWith<$Res> {
  factory _$$Event_ErrorImplCopyWith(
    _$Event_ErrorImpl value,
    $Res Function(_$Event_ErrorImpl) then,
  ) = __$$Event_ErrorImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String message});
}

/// @nodoc
class __$$Event_ErrorImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_ErrorImpl>
    implements _$$Event_ErrorImplCopyWith<$Res> {
  __$$Event_ErrorImplCopyWithImpl(
    _$Event_ErrorImpl _value,
    $Res Function(_$Event_ErrorImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? message = null}) {
    return _then(
      _$Event_ErrorImpl(
        message: null == message
            ? _value.message
            : message // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$Event_ErrorImpl extends Event_Error {
  const _$Event_ErrorImpl({required this.message}) : super._();

  @override
  final String message;

  @override
  String toString() {
    return 'Event.error(message: $message)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_ErrorImpl &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode => Object.hash(runtimeType, message);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_ErrorImplCopyWith<_$Event_ErrorImpl> get copyWith =>
      __$$Event_ErrorImplCopyWithImpl<_$Event_ErrorImpl>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return error(message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return error?.call(message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(message);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) {
    return error(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) {
    return error?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(this);
    }
    return orElse();
  }
}

abstract class Event_Error extends Event {
  const factory Event_Error({required final String message}) =
      _$Event_ErrorImpl;
  const Event_Error._() : super._();

  String get message;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_ErrorImplCopyWith<_$Event_ErrorImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Event_LogImplCopyWith<$Res> {
  factory _$$Event_LogImplCopyWith(
    _$Event_LogImpl value,
    $Res Function(_$Event_LogImpl) then,
  ) = __$$Event_LogImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String message});
}

/// @nodoc
class __$$Event_LogImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_LogImpl>
    implements _$$Event_LogImplCopyWith<$Res> {
  __$$Event_LogImplCopyWithImpl(
    _$Event_LogImpl _value,
    $Res Function(_$Event_LogImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? message = null}) {
    return _then(
      _$Event_LogImpl(
        message: null == message
            ? _value.message
            : message // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$Event_LogImpl extends Event_Log {
  const _$Event_LogImpl({required this.message}) : super._();

  @override
  final String message;

  @override
  String toString() {
    return 'Event.log(message: $message)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_LogImpl &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode => Object.hash(runtimeType, message);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_LogImplCopyWith<_$Event_LogImpl> get copyWith =>
      __$$Event_LogImplCopyWithImpl<_$Event_LogImpl>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return log(message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return log?.call(message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (log != null) {
      return log(message);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) {
    return log(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) {
    return log?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    required TResult orElse(),
  }) {
    if (log != null) {
      return log(this);
    }
    return orElse();
  }
}

abstract class Event_Log extends Event {
  const factory Event_Log({required final String message}) = _$Event_LogImpl;
  const Event_Log._() : super._();

  String get message;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_LogImplCopyWith<_$Event_LogImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
