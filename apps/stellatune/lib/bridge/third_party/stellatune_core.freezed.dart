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
    required TResult Function(String path) playbackEnded,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
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
    required TResult Function(String path) playbackEnded,
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
    TResult? Function(String path)? playbackEnded,
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
    TResult Function(String path)? playbackEnded,
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
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
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
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
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
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
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
    required TResult Function(String path) playbackEnded,
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
    TResult? Function(String path)? playbackEnded,
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
    TResult Function(String path)? playbackEnded,
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
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
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
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
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
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
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
    required TResult Function(String path) playbackEnded,
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
    TResult? Function(String path)? playbackEnded,
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
    TResult Function(String path)? playbackEnded,
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
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
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
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
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
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
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
abstract class _$$Event_PlaybackEndedImplCopyWith<$Res> {
  factory _$$Event_PlaybackEndedImplCopyWith(
    _$Event_PlaybackEndedImpl value,
    $Res Function(_$Event_PlaybackEndedImpl) then,
  ) = __$$Event_PlaybackEndedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String path});
}

/// @nodoc
class __$$Event_PlaybackEndedImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_PlaybackEndedImpl>
    implements _$$Event_PlaybackEndedImplCopyWith<$Res> {
  __$$Event_PlaybackEndedImplCopyWithImpl(
    _$Event_PlaybackEndedImpl _value,
    $Res Function(_$Event_PlaybackEndedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? path = null}) {
    return _then(
      _$Event_PlaybackEndedImpl(
        path: null == path
            ? _value.path
            : path // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$Event_PlaybackEndedImpl extends Event_PlaybackEnded {
  const _$Event_PlaybackEndedImpl({required this.path}) : super._();

  @override
  final String path;

  @override
  String toString() {
    return 'Event.playbackEnded(path: $path)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_PlaybackEndedImpl &&
            (identical(other.path, path) || other.path == path));
  }

  @override
  int get hashCode => Object.hash(runtimeType, path);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_PlaybackEndedImplCopyWith<_$Event_PlaybackEndedImpl> get copyWith =>
      __$$Event_PlaybackEndedImplCopyWithImpl<_$Event_PlaybackEndedImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return playbackEnded(path);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return playbackEnded?.call(path);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (playbackEnded != null) {
      return playbackEnded(path);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
  }) {
    return playbackEnded(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
  }) {
    return playbackEnded?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    required TResult orElse(),
  }) {
    if (playbackEnded != null) {
      return playbackEnded(this);
    }
    return orElse();
  }
}

abstract class Event_PlaybackEnded extends Event {
  const factory Event_PlaybackEnded({required final String path}) =
      _$Event_PlaybackEndedImpl;
  const Event_PlaybackEnded._() : super._();

  String get path;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_PlaybackEndedImplCopyWith<_$Event_PlaybackEndedImpl> get copyWith =>
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
    required TResult Function(String path) playbackEnded,
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
    TResult? Function(String path)? playbackEnded,
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
    TResult Function(String path)? playbackEnded,
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
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
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
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
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
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
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
    required TResult Function(String path) playbackEnded,
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
    TResult? Function(String path)? playbackEnded,
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
    TResult Function(String path)? playbackEnded,
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
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
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
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
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
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
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

/// @nodoc
mixin _$LibraryEvent {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $LibraryEventCopyWith<$Res> {
  factory $LibraryEventCopyWith(
    LibraryEvent value,
    $Res Function(LibraryEvent) then,
  ) = _$LibraryEventCopyWithImpl<$Res, LibraryEvent>;
}

/// @nodoc
class _$LibraryEventCopyWithImpl<$Res, $Val extends LibraryEvent>
    implements $LibraryEventCopyWith<$Res> {
  _$LibraryEventCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$LibraryEvent_RootsImplCopyWith<$Res> {
  factory _$$LibraryEvent_RootsImplCopyWith(
    _$LibraryEvent_RootsImpl value,
    $Res Function(_$LibraryEvent_RootsImpl) then,
  ) = __$$LibraryEvent_RootsImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<String> paths});
}

/// @nodoc
class __$$LibraryEvent_RootsImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_RootsImpl>
    implements _$$LibraryEvent_RootsImplCopyWith<$Res> {
  __$$LibraryEvent_RootsImplCopyWithImpl(
    _$LibraryEvent_RootsImpl _value,
    $Res Function(_$LibraryEvent_RootsImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? paths = null}) {
    return _then(
      _$LibraryEvent_RootsImpl(
        paths: null == paths
            ? _value._paths
            : paths // ignore: cast_nullable_to_non_nullable
                  as List<String>,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_RootsImpl extends LibraryEvent_Roots {
  const _$LibraryEvent_RootsImpl({required final List<String> paths})
    : _paths = paths,
      super._();

  final List<String> _paths;
  @override
  List<String> get paths {
    if (_paths is EqualUnmodifiableListView) return _paths;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_paths);
  }

  @override
  String toString() {
    return 'LibraryEvent.roots(paths: $paths)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_RootsImpl &&
            const DeepCollectionEquality().equals(other._paths, _paths));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_paths));

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_RootsImplCopyWith<_$LibraryEvent_RootsImpl> get copyWith =>
      __$$LibraryEvent_RootsImplCopyWithImpl<_$LibraryEvent_RootsImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return roots(paths);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return roots?.call(paths);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (roots != null) {
      return roots(paths);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return roots(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return roots?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (roots != null) {
      return roots(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_Roots extends LibraryEvent {
  const factory LibraryEvent_Roots({required final List<String> paths}) =
      _$LibraryEvent_RootsImpl;
  const LibraryEvent_Roots._() : super._();

  List<String> get paths;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_RootsImplCopyWith<_$LibraryEvent_RootsImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_FoldersImplCopyWith<$Res> {
  factory _$$LibraryEvent_FoldersImplCopyWith(
    _$LibraryEvent_FoldersImpl value,
    $Res Function(_$LibraryEvent_FoldersImpl) then,
  ) = __$$LibraryEvent_FoldersImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<String> paths});
}

/// @nodoc
class __$$LibraryEvent_FoldersImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_FoldersImpl>
    implements _$$LibraryEvent_FoldersImplCopyWith<$Res> {
  __$$LibraryEvent_FoldersImplCopyWithImpl(
    _$LibraryEvent_FoldersImpl _value,
    $Res Function(_$LibraryEvent_FoldersImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? paths = null}) {
    return _then(
      _$LibraryEvent_FoldersImpl(
        paths: null == paths
            ? _value._paths
            : paths // ignore: cast_nullable_to_non_nullable
                  as List<String>,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_FoldersImpl extends LibraryEvent_Folders {
  const _$LibraryEvent_FoldersImpl({required final List<String> paths})
    : _paths = paths,
      super._();

  final List<String> _paths;
  @override
  List<String> get paths {
    if (_paths is EqualUnmodifiableListView) return _paths;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_paths);
  }

  @override
  String toString() {
    return 'LibraryEvent.folders(paths: $paths)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_FoldersImpl &&
            const DeepCollectionEquality().equals(other._paths, _paths));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_paths));

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_FoldersImplCopyWith<_$LibraryEvent_FoldersImpl>
  get copyWith =>
      __$$LibraryEvent_FoldersImplCopyWithImpl<_$LibraryEvent_FoldersImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return folders(paths);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return folders?.call(paths);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (folders != null) {
      return folders(paths);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return folders(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return folders?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (folders != null) {
      return folders(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_Folders extends LibraryEvent {
  const factory LibraryEvent_Folders({required final List<String> paths}) =
      _$LibraryEvent_FoldersImpl;
  const LibraryEvent_Folders._() : super._();

  List<String> get paths;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_FoldersImplCopyWith<_$LibraryEvent_FoldersImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_TracksImplCopyWith<$Res> {
  factory _$$LibraryEvent_TracksImplCopyWith(
    _$LibraryEvent_TracksImpl value,
    $Res Function(_$LibraryEvent_TracksImpl) then,
  ) = __$$LibraryEvent_TracksImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    String folder,
    bool recursive,
    String query,
    List<TrackLite> items,
  });
}

/// @nodoc
class __$$LibraryEvent_TracksImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_TracksImpl>
    implements _$$LibraryEvent_TracksImplCopyWith<$Res> {
  __$$LibraryEvent_TracksImplCopyWithImpl(
    _$LibraryEvent_TracksImpl _value,
    $Res Function(_$LibraryEvent_TracksImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? folder = null,
    Object? recursive = null,
    Object? query = null,
    Object? items = null,
  }) {
    return _then(
      _$LibraryEvent_TracksImpl(
        folder: null == folder
            ? _value.folder
            : folder // ignore: cast_nullable_to_non_nullable
                  as String,
        recursive: null == recursive
            ? _value.recursive
            : recursive // ignore: cast_nullable_to_non_nullable
                  as bool,
        query: null == query
            ? _value.query
            : query // ignore: cast_nullable_to_non_nullable
                  as String,
        items: null == items
            ? _value._items
            : items // ignore: cast_nullable_to_non_nullable
                  as List<TrackLite>,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_TracksImpl extends LibraryEvent_Tracks {
  const _$LibraryEvent_TracksImpl({
    required this.folder,
    required this.recursive,
    required this.query,
    required final List<TrackLite> items,
  }) : _items = items,
       super._();

  @override
  final String folder;
  @override
  final bool recursive;
  @override
  final String query;
  final List<TrackLite> _items;
  @override
  List<TrackLite> get items {
    if (_items is EqualUnmodifiableListView) return _items;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_items);
  }

  @override
  String toString() {
    return 'LibraryEvent.tracks(folder: $folder, recursive: $recursive, query: $query, items: $items)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_TracksImpl &&
            (identical(other.folder, folder) || other.folder == folder) &&
            (identical(other.recursive, recursive) ||
                other.recursive == recursive) &&
            (identical(other.query, query) || other.query == query) &&
            const DeepCollectionEquality().equals(other._items, _items));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    folder,
    recursive,
    query,
    const DeepCollectionEquality().hash(_items),
  );

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_TracksImplCopyWith<_$LibraryEvent_TracksImpl> get copyWith =>
      __$$LibraryEvent_TracksImplCopyWithImpl<_$LibraryEvent_TracksImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return tracks(folder, recursive, query, items);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return tracks?.call(folder, recursive, query, items);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (tracks != null) {
      return tracks(folder, recursive, query, items);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return tracks(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return tracks?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (tracks != null) {
      return tracks(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_Tracks extends LibraryEvent {
  const factory LibraryEvent_Tracks({
    required final String folder,
    required final bool recursive,
    required final String query,
    required final List<TrackLite> items,
  }) = _$LibraryEvent_TracksImpl;
  const LibraryEvent_Tracks._() : super._();

  String get folder;
  bool get recursive;
  String get query;
  List<TrackLite> get items;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_TracksImplCopyWith<_$LibraryEvent_TracksImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_ScanProgressImplCopyWith<$Res> {
  factory _$$LibraryEvent_ScanProgressImplCopyWith(
    _$LibraryEvent_ScanProgressImpl value,
    $Res Function(_$LibraryEvent_ScanProgressImpl) then,
  ) = __$$LibraryEvent_ScanProgressImplCopyWithImpl<$Res>;
  @useResult
  $Res call({int scanned, int updated, int skipped, int errors});
}

/// @nodoc
class __$$LibraryEvent_ScanProgressImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_ScanProgressImpl>
    implements _$$LibraryEvent_ScanProgressImplCopyWith<$Res> {
  __$$LibraryEvent_ScanProgressImplCopyWithImpl(
    _$LibraryEvent_ScanProgressImpl _value,
    $Res Function(_$LibraryEvent_ScanProgressImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? scanned = null,
    Object? updated = null,
    Object? skipped = null,
    Object? errors = null,
  }) {
    return _then(
      _$LibraryEvent_ScanProgressImpl(
        scanned: null == scanned
            ? _value.scanned
            : scanned // ignore: cast_nullable_to_non_nullable
                  as int,
        updated: null == updated
            ? _value.updated
            : updated // ignore: cast_nullable_to_non_nullable
                  as int,
        skipped: null == skipped
            ? _value.skipped
            : skipped // ignore: cast_nullable_to_non_nullable
                  as int,
        errors: null == errors
            ? _value.errors
            : errors // ignore: cast_nullable_to_non_nullable
                  as int,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_ScanProgressImpl extends LibraryEvent_ScanProgress {
  const _$LibraryEvent_ScanProgressImpl({
    required this.scanned,
    required this.updated,
    required this.skipped,
    required this.errors,
  }) : super._();

  @override
  final int scanned;
  @override
  final int updated;
  @override
  final int skipped;
  @override
  final int errors;

  @override
  String toString() {
    return 'LibraryEvent.scanProgress(scanned: $scanned, updated: $updated, skipped: $skipped, errors: $errors)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_ScanProgressImpl &&
            (identical(other.scanned, scanned) || other.scanned == scanned) &&
            (identical(other.updated, updated) || other.updated == updated) &&
            (identical(other.skipped, skipped) || other.skipped == skipped) &&
            (identical(other.errors, errors) || other.errors == errors));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, scanned, updated, skipped, errors);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_ScanProgressImplCopyWith<_$LibraryEvent_ScanProgressImpl>
  get copyWith =>
      __$$LibraryEvent_ScanProgressImplCopyWithImpl<
        _$LibraryEvent_ScanProgressImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return scanProgress(scanned, updated, skipped, errors);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return scanProgress?.call(scanned, updated, skipped, errors);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (scanProgress != null) {
      return scanProgress(scanned, updated, skipped, errors);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return scanProgress(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return scanProgress?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (scanProgress != null) {
      return scanProgress(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_ScanProgress extends LibraryEvent {
  const factory LibraryEvent_ScanProgress({
    required final int scanned,
    required final int updated,
    required final int skipped,
    required final int errors,
  }) = _$LibraryEvent_ScanProgressImpl;
  const LibraryEvent_ScanProgress._() : super._();

  int get scanned;
  int get updated;
  int get skipped;
  int get errors;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_ScanProgressImplCopyWith<_$LibraryEvent_ScanProgressImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_ScanFinishedImplCopyWith<$Res> {
  factory _$$LibraryEvent_ScanFinishedImplCopyWith(
    _$LibraryEvent_ScanFinishedImpl value,
    $Res Function(_$LibraryEvent_ScanFinishedImpl) then,
  ) = __$$LibraryEvent_ScanFinishedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({
    int durationMs,
    int scanned,
    int updated,
    int skipped,
    int errors,
  });
}

/// @nodoc
class __$$LibraryEvent_ScanFinishedImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_ScanFinishedImpl>
    implements _$$LibraryEvent_ScanFinishedImplCopyWith<$Res> {
  __$$LibraryEvent_ScanFinishedImplCopyWithImpl(
    _$LibraryEvent_ScanFinishedImpl _value,
    $Res Function(_$LibraryEvent_ScanFinishedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? durationMs = null,
    Object? scanned = null,
    Object? updated = null,
    Object? skipped = null,
    Object? errors = null,
  }) {
    return _then(
      _$LibraryEvent_ScanFinishedImpl(
        durationMs: null == durationMs
            ? _value.durationMs
            : durationMs // ignore: cast_nullable_to_non_nullable
                  as int,
        scanned: null == scanned
            ? _value.scanned
            : scanned // ignore: cast_nullable_to_non_nullable
                  as int,
        updated: null == updated
            ? _value.updated
            : updated // ignore: cast_nullable_to_non_nullable
                  as int,
        skipped: null == skipped
            ? _value.skipped
            : skipped // ignore: cast_nullable_to_non_nullable
                  as int,
        errors: null == errors
            ? _value.errors
            : errors // ignore: cast_nullable_to_non_nullable
                  as int,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_ScanFinishedImpl extends LibraryEvent_ScanFinished {
  const _$LibraryEvent_ScanFinishedImpl({
    required this.durationMs,
    required this.scanned,
    required this.updated,
    required this.skipped,
    required this.errors,
  }) : super._();

  @override
  final int durationMs;
  @override
  final int scanned;
  @override
  final int updated;
  @override
  final int skipped;
  @override
  final int errors;

  @override
  String toString() {
    return 'LibraryEvent.scanFinished(durationMs: $durationMs, scanned: $scanned, updated: $updated, skipped: $skipped, errors: $errors)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_ScanFinishedImpl &&
            (identical(other.durationMs, durationMs) ||
                other.durationMs == durationMs) &&
            (identical(other.scanned, scanned) || other.scanned == scanned) &&
            (identical(other.updated, updated) || other.updated == updated) &&
            (identical(other.skipped, skipped) || other.skipped == skipped) &&
            (identical(other.errors, errors) || other.errors == errors));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, durationMs, scanned, updated, skipped, errors);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_ScanFinishedImplCopyWith<_$LibraryEvent_ScanFinishedImpl>
  get copyWith =>
      __$$LibraryEvent_ScanFinishedImplCopyWithImpl<
        _$LibraryEvent_ScanFinishedImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return scanFinished(durationMs, scanned, updated, skipped, errors);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return scanFinished?.call(durationMs, scanned, updated, skipped, errors);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (scanFinished != null) {
      return scanFinished(durationMs, scanned, updated, skipped, errors);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return scanFinished(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return scanFinished?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (scanFinished != null) {
      return scanFinished(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_ScanFinished extends LibraryEvent {
  const factory LibraryEvent_ScanFinished({
    required final int durationMs,
    required final int scanned,
    required final int updated,
    required final int skipped,
    required final int errors,
  }) = _$LibraryEvent_ScanFinishedImpl;
  const LibraryEvent_ScanFinished._() : super._();

  int get durationMs;
  int get scanned;
  int get updated;
  int get skipped;
  int get errors;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_ScanFinishedImplCopyWith<_$LibraryEvent_ScanFinishedImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_SearchResultImplCopyWith<$Res> {
  factory _$$LibraryEvent_SearchResultImplCopyWith(
    _$LibraryEvent_SearchResultImpl value,
    $Res Function(_$LibraryEvent_SearchResultImpl) then,
  ) = __$$LibraryEvent_SearchResultImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String query, List<TrackLite> items});
}

/// @nodoc
class __$$LibraryEvent_SearchResultImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_SearchResultImpl>
    implements _$$LibraryEvent_SearchResultImplCopyWith<$Res> {
  __$$LibraryEvent_SearchResultImplCopyWithImpl(
    _$LibraryEvent_SearchResultImpl _value,
    $Res Function(_$LibraryEvent_SearchResultImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? query = null, Object? items = null}) {
    return _then(
      _$LibraryEvent_SearchResultImpl(
        query: null == query
            ? _value.query
            : query // ignore: cast_nullable_to_non_nullable
                  as String,
        items: null == items
            ? _value._items
            : items // ignore: cast_nullable_to_non_nullable
                  as List<TrackLite>,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_SearchResultImpl extends LibraryEvent_SearchResult {
  const _$LibraryEvent_SearchResultImpl({
    required this.query,
    required final List<TrackLite> items,
  }) : _items = items,
       super._();

  @override
  final String query;
  final List<TrackLite> _items;
  @override
  List<TrackLite> get items {
    if (_items is EqualUnmodifiableListView) return _items;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_items);
  }

  @override
  String toString() {
    return 'LibraryEvent.searchResult(query: $query, items: $items)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_SearchResultImpl &&
            (identical(other.query, query) || other.query == query) &&
            const DeepCollectionEquality().equals(other._items, _items));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    query,
    const DeepCollectionEquality().hash(_items),
  );

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_SearchResultImplCopyWith<_$LibraryEvent_SearchResultImpl>
  get copyWith =>
      __$$LibraryEvent_SearchResultImplCopyWithImpl<
        _$LibraryEvent_SearchResultImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return searchResult(query, items);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return searchResult?.call(query, items);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (searchResult != null) {
      return searchResult(query, items);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return searchResult(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return searchResult?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (searchResult != null) {
      return searchResult(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_SearchResult extends LibraryEvent {
  const factory LibraryEvent_SearchResult({
    required final String query,
    required final List<TrackLite> items,
  }) = _$LibraryEvent_SearchResultImpl;
  const LibraryEvent_SearchResult._() : super._();

  String get query;
  List<TrackLite> get items;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_SearchResultImplCopyWith<_$LibraryEvent_SearchResultImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_ErrorImplCopyWith<$Res> {
  factory _$$LibraryEvent_ErrorImplCopyWith(
    _$LibraryEvent_ErrorImpl value,
    $Res Function(_$LibraryEvent_ErrorImpl) then,
  ) = __$$LibraryEvent_ErrorImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String message});
}

/// @nodoc
class __$$LibraryEvent_ErrorImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_ErrorImpl>
    implements _$$LibraryEvent_ErrorImplCopyWith<$Res> {
  __$$LibraryEvent_ErrorImplCopyWithImpl(
    _$LibraryEvent_ErrorImpl _value,
    $Res Function(_$LibraryEvent_ErrorImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? message = null}) {
    return _then(
      _$LibraryEvent_ErrorImpl(
        message: null == message
            ? _value.message
            : message // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_ErrorImpl extends LibraryEvent_Error {
  const _$LibraryEvent_ErrorImpl({required this.message}) : super._();

  @override
  final String message;

  @override
  String toString() {
    return 'LibraryEvent.error(message: $message)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_ErrorImpl &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode => Object.hash(runtimeType, message);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_ErrorImplCopyWith<_$LibraryEvent_ErrorImpl> get copyWith =>
      __$$LibraryEvent_ErrorImplCopyWithImpl<_$LibraryEvent_ErrorImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return error(message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return error?.call(message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
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
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return error(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return error?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_Error extends LibraryEvent {
  const factory LibraryEvent_Error({required final String message}) =
      _$LibraryEvent_ErrorImpl;
  const LibraryEvent_Error._() : super._();

  String get message;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_ErrorImplCopyWith<_$LibraryEvent_ErrorImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_LogImplCopyWith<$Res> {
  factory _$$LibraryEvent_LogImplCopyWith(
    _$LibraryEvent_LogImpl value,
    $Res Function(_$LibraryEvent_LogImpl) then,
  ) = __$$LibraryEvent_LogImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String message});
}

/// @nodoc
class __$$LibraryEvent_LogImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_LogImpl>
    implements _$$LibraryEvent_LogImplCopyWith<$Res> {
  __$$LibraryEvent_LogImplCopyWithImpl(
    _$LibraryEvent_LogImpl _value,
    $Res Function(_$LibraryEvent_LogImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? message = null}) {
    return _then(
      _$LibraryEvent_LogImpl(
        message: null == message
            ? _value.message
            : message // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_LogImpl extends LibraryEvent_Log {
  const _$LibraryEvent_LogImpl({required this.message}) : super._();

  @override
  final String message;

  @override
  String toString() {
    return 'LibraryEvent.log(message: $message)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_LogImpl &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode => Object.hash(runtimeType, message);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_LogImplCopyWith<_$LibraryEvent_LogImpl> get copyWith =>
      __$$LibraryEvent_LogImplCopyWithImpl<_$LibraryEvent_LogImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )
    tracks,
    required TResult Function(int scanned, int updated, int skipped, int errors)
    scanProgress,
    required TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )
    scanFinished,
    required TResult Function(String query, List<TrackLite> items) searchResult,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return log(message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult? Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult? Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult? Function(String query, List<TrackLite> items)? searchResult,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return log?.call(message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(
      String folder,
      bool recursive,
      String query,
      List<TrackLite> items,
    )?
    tracks,
    TResult Function(int scanned, int updated, int skipped, int errors)?
    scanProgress,
    TResult Function(
      int durationMs,
      int scanned,
      int updated,
      int skipped,
      int errors,
    )?
    scanFinished,
    TResult Function(String query, List<TrackLite> items)? searchResult,
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
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return log(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return log?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (log != null) {
      return log(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_Log extends LibraryEvent {
  const factory LibraryEvent_Log({required final String message}) =
      _$LibraryEvent_LogImpl;
  const LibraryEvent_Log._() : super._();

  String get message;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_LogImplCopyWith<_$LibraryEvent_LogImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
