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
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event_StateChanged value) stateChanged,
    required TResult Function(Event_Position value) position,
    required TResult Function(Event_TrackChanged value) trackChanged,
    required TResult Function(Event_PlaybackEnded value) playbackEnded,
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return stateChanged(state);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return stateChanged?.call(state);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
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
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
  $Res call({int ms, String path, BigInt sessionId});
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
  $Res call({
    Object? ms = null,
    Object? path = null,
    Object? sessionId = null,
  }) {
    return _then(
      _$Event_PositionImpl(
        ms: null == ms
            ? _value.ms
            : ms // ignore: cast_nullable_to_non_nullable
                  as int,
        path: null == path
            ? _value.path
            : path // ignore: cast_nullable_to_non_nullable
                  as String,
        sessionId: null == sessionId
            ? _value.sessionId
            : sessionId // ignore: cast_nullable_to_non_nullable
                  as BigInt,
      ),
    );
  }
}

/// @nodoc

class _$Event_PositionImpl extends Event_Position {
  const _$Event_PositionImpl({
    required this.ms,
    required this.path,
    required this.sessionId,
  }) : super._();

  @override
  final int ms;
  @override
  final String path;
  @override
  final BigInt sessionId;

  @override
  String toString() {
    return 'Event.position(ms: $ms, path: $path, sessionId: $sessionId)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_PositionImpl &&
            (identical(other.ms, ms) || other.ms == ms) &&
            (identical(other.path, path) || other.path == path) &&
            (identical(other.sessionId, sessionId) ||
                other.sessionId == sessionId));
  }

  @override
  int get hashCode => Object.hash(runtimeType, ms, path, sessionId);

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
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return position(ms, path, sessionId);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return position?.call(ms, path, sessionId);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
    required TResult orElse(),
  }) {
    if (position != null) {
      return position(ms, path, sessionId);
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
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
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
    required TResult orElse(),
  }) {
    if (position != null) {
      return position(this);
    }
    return orElse();
  }
}

abstract class Event_Position extends Event {
  const factory Event_Position({
    required final int ms,
    required final String path,
    required final BigInt sessionId,
  }) = _$Event_PositionImpl;
  const Event_Position._() : super._();

  int get ms;
  String get path;
  BigInt get sessionId;

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
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return trackChanged(path);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return trackChanged?.call(path);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
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
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return playbackEnded(path);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return playbackEnded?.call(path);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
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
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
abstract class _$$Event_VolumeChangedImplCopyWith<$Res> {
  factory _$$Event_VolumeChangedImplCopyWith(
    _$Event_VolumeChangedImpl value,
    $Res Function(_$Event_VolumeChangedImpl) then,
  ) = __$$Event_VolumeChangedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({double volume});
}

/// @nodoc
class __$$Event_VolumeChangedImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_VolumeChangedImpl>
    implements _$$Event_VolumeChangedImplCopyWith<$Res> {
  __$$Event_VolumeChangedImplCopyWithImpl(
    _$Event_VolumeChangedImpl _value,
    $Res Function(_$Event_VolumeChangedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? volume = null}) {
    return _then(
      _$Event_VolumeChangedImpl(
        volume: null == volume
            ? _value.volume
            : volume // ignore: cast_nullable_to_non_nullable
                  as double,
      ),
    );
  }
}

/// @nodoc

class _$Event_VolumeChangedImpl extends Event_VolumeChanged {
  const _$Event_VolumeChangedImpl({required this.volume}) : super._();

  @override
  final double volume;

  @override
  String toString() {
    return 'Event.volumeChanged(volume: $volume)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_VolumeChangedImpl &&
            (identical(other.volume, volume) || other.volume == volume));
  }

  @override
  int get hashCode => Object.hash(runtimeType, volume);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_VolumeChangedImplCopyWith<_$Event_VolumeChangedImpl> get copyWith =>
      __$$Event_VolumeChangedImplCopyWithImpl<_$Event_VolumeChangedImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return volumeChanged(volume);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return volumeChanged?.call(volume);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
    required TResult orElse(),
  }) {
    if (volumeChanged != null) {
      return volumeChanged(volume);
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
  }) {
    return volumeChanged(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
  }) {
    return volumeChanged?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
    required TResult orElse(),
  }) {
    if (volumeChanged != null) {
      return volumeChanged(this);
    }
    return orElse();
  }
}

abstract class Event_VolumeChanged extends Event {
  const factory Event_VolumeChanged({required final double volume}) =
      _$Event_VolumeChangedImpl;
  const Event_VolumeChanged._() : super._();

  double get volume;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_VolumeChangedImplCopyWith<_$Event_VolumeChangedImpl> get copyWith =>
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
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return error(message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return error?.call(message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
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
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return log(message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return log?.call(message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
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
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
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
abstract class _$$Event_OutputDevicesChangedImplCopyWith<$Res> {
  factory _$$Event_OutputDevicesChangedImplCopyWith(
    _$Event_OutputDevicesChangedImpl value,
    $Res Function(_$Event_OutputDevicesChangedImpl) then,
  ) = __$$Event_OutputDevicesChangedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<AudioDevice> devices});
}

/// @nodoc
class __$$Event_OutputDevicesChangedImplCopyWithImpl<$Res>
    extends _$EventCopyWithImpl<$Res, _$Event_OutputDevicesChangedImpl>
    implements _$$Event_OutputDevicesChangedImplCopyWith<$Res> {
  __$$Event_OutputDevicesChangedImplCopyWithImpl(
    _$Event_OutputDevicesChangedImpl _value,
    $Res Function(_$Event_OutputDevicesChangedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? devices = null}) {
    return _then(
      _$Event_OutputDevicesChangedImpl(
        devices: null == devices
            ? _value._devices
            : devices // ignore: cast_nullable_to_non_nullable
                  as List<AudioDevice>,
      ),
    );
  }
}

/// @nodoc

class _$Event_OutputDevicesChangedImpl extends Event_OutputDevicesChanged {
  const _$Event_OutputDevicesChangedImpl({
    required final List<AudioDevice> devices,
  }) : _devices = devices,
       super._();

  final List<AudioDevice> _devices;
  @override
  List<AudioDevice> get devices {
    if (_devices is EqualUnmodifiableListView) return _devices;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_devices);
  }

  @override
  String toString() {
    return 'Event.outputDevicesChanged(devices: $devices)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event_OutputDevicesChangedImpl &&
            const DeepCollectionEquality().equals(other._devices, _devices));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_devices));

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Event_OutputDevicesChangedImplCopyWith<_$Event_OutputDevicesChangedImpl>
  get copyWith =>
      __$$Event_OutputDevicesChangedImplCopyWithImpl<
        _$Event_OutputDevicesChangedImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(PlayerState state) stateChanged,
    required TResult Function(int ms, String path, BigInt sessionId) position,
    required TResult Function(String path) trackChanged,
    required TResult Function(String path) playbackEnded,
    required TResult Function(double volume) volumeChanged,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
    required TResult Function(List<AudioDevice> devices) outputDevicesChanged,
  }) {
    return outputDevicesChanged(devices);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(PlayerState state)? stateChanged,
    TResult? Function(int ms, String path, BigInt sessionId)? position,
    TResult? Function(String path)? trackChanged,
    TResult? Function(String path)? playbackEnded,
    TResult? Function(double volume)? volumeChanged,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
    TResult? Function(List<AudioDevice> devices)? outputDevicesChanged,
  }) {
    return outputDevicesChanged?.call(devices);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(PlayerState state)? stateChanged,
    TResult Function(int ms, String path, BigInt sessionId)? position,
    TResult Function(String path)? trackChanged,
    TResult Function(String path)? playbackEnded,
    TResult Function(double volume)? volumeChanged,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    TResult Function(List<AudioDevice> devices)? outputDevicesChanged,
    required TResult orElse(),
  }) {
    if (outputDevicesChanged != null) {
      return outputDevicesChanged(devices);
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
    required TResult Function(Event_VolumeChanged value) volumeChanged,
    required TResult Function(Event_Error value) error,
    required TResult Function(Event_Log value) log,
    required TResult Function(Event_OutputDevicesChanged value)
    outputDevicesChanged,
  }) {
    return outputDevicesChanged(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Event_StateChanged value)? stateChanged,
    TResult? Function(Event_Position value)? position,
    TResult? Function(Event_TrackChanged value)? trackChanged,
    TResult? Function(Event_PlaybackEnded value)? playbackEnded,
    TResult? Function(Event_VolumeChanged value)? volumeChanged,
    TResult? Function(Event_Error value)? error,
    TResult? Function(Event_Log value)? log,
    TResult? Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
  }) {
    return outputDevicesChanged?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event_StateChanged value)? stateChanged,
    TResult Function(Event_Position value)? position,
    TResult Function(Event_TrackChanged value)? trackChanged,
    TResult Function(Event_PlaybackEnded value)? playbackEnded,
    TResult Function(Event_VolumeChanged value)? volumeChanged,
    TResult Function(Event_Error value)? error,
    TResult Function(Event_Log value)? log,
    TResult Function(Event_OutputDevicesChanged value)? outputDevicesChanged,
    required TResult orElse(),
  }) {
    if (outputDevicesChanged != null) {
      return outputDevicesChanged(this);
    }
    return orElse();
  }
}

abstract class Event_OutputDevicesChanged extends Event {
  const factory Event_OutputDevicesChanged({
    required final List<AudioDevice> devices,
  }) = _$Event_OutputDevicesChangedImpl;
  const Event_OutputDevicesChanged._() : super._();

  List<AudioDevice> get devices;

  /// Create a copy of Event
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Event_OutputDevicesChangedImplCopyWith<_$Event_OutputDevicesChangedImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
mixin _$LibraryEvent {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
abstract class _$$LibraryEvent_ExcludedFoldersImplCopyWith<$Res> {
  factory _$$LibraryEvent_ExcludedFoldersImplCopyWith(
    _$LibraryEvent_ExcludedFoldersImpl value,
    $Res Function(_$LibraryEvent_ExcludedFoldersImpl) then,
  ) = __$$LibraryEvent_ExcludedFoldersImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<String> paths});
}

/// @nodoc
class __$$LibraryEvent_ExcludedFoldersImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_ExcludedFoldersImpl>
    implements _$$LibraryEvent_ExcludedFoldersImplCopyWith<$Res> {
  __$$LibraryEvent_ExcludedFoldersImplCopyWithImpl(
    _$LibraryEvent_ExcludedFoldersImpl _value,
    $Res Function(_$LibraryEvent_ExcludedFoldersImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? paths = null}) {
    return _then(
      _$LibraryEvent_ExcludedFoldersImpl(
        paths: null == paths
            ? _value._paths
            : paths // ignore: cast_nullable_to_non_nullable
                  as List<String>,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_ExcludedFoldersImpl extends LibraryEvent_ExcludedFolders {
  const _$LibraryEvent_ExcludedFoldersImpl({required final List<String> paths})
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
    return 'LibraryEvent.excludedFolders(paths: $paths)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_ExcludedFoldersImpl &&
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
  _$$LibraryEvent_ExcludedFoldersImplCopyWith<
    _$LibraryEvent_ExcludedFoldersImpl
  >
  get copyWith =>
      __$$LibraryEvent_ExcludedFoldersImplCopyWithImpl<
        _$LibraryEvent_ExcludedFoldersImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return excludedFolders(paths);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return excludedFolders?.call(paths);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (excludedFolders != null) {
      return excludedFolders(paths);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return excludedFolders(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return excludedFolders?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (excludedFolders != null) {
      return excludedFolders(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_ExcludedFolders extends LibraryEvent {
  const factory LibraryEvent_ExcludedFolders({
    required final List<String> paths,
  }) = _$LibraryEvent_ExcludedFoldersImpl;
  const LibraryEvent_ExcludedFolders._() : super._();

  List<String> get paths;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_ExcludedFoldersImplCopyWith<
    _$LibraryEvent_ExcludedFoldersImpl
  >
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_ChangedImplCopyWith<$Res> {
  factory _$$LibraryEvent_ChangedImplCopyWith(
    _$LibraryEvent_ChangedImpl value,
    $Res Function(_$LibraryEvent_ChangedImpl) then,
  ) = __$$LibraryEvent_ChangedImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$LibraryEvent_ChangedImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_ChangedImpl>
    implements _$$LibraryEvent_ChangedImplCopyWith<$Res> {
  __$$LibraryEvent_ChangedImplCopyWithImpl(
    _$LibraryEvent_ChangedImpl _value,
    $Res Function(_$LibraryEvent_ChangedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$LibraryEvent_ChangedImpl extends LibraryEvent_Changed {
  const _$LibraryEvent_ChangedImpl() : super._();

  @override
  String toString() {
    return 'LibraryEvent.changed()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_ChangedImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return changed();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return changed?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (changed != null) {
      return changed();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return changed(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return changed?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (changed != null) {
      return changed(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_Changed extends LibraryEvent {
  const factory LibraryEvent_Changed() = _$LibraryEvent_ChangedImpl;
  const LibraryEvent_Changed._() : super._();
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
abstract class _$$LibraryEvent_PlaylistsImplCopyWith<$Res> {
  factory _$$LibraryEvent_PlaylistsImplCopyWith(
    _$LibraryEvent_PlaylistsImpl value,
    $Res Function(_$LibraryEvent_PlaylistsImpl) then,
  ) = __$$LibraryEvent_PlaylistsImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<PlaylistLite> items});
}

/// @nodoc
class __$$LibraryEvent_PlaylistsImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_PlaylistsImpl>
    implements _$$LibraryEvent_PlaylistsImplCopyWith<$Res> {
  __$$LibraryEvent_PlaylistsImplCopyWithImpl(
    _$LibraryEvent_PlaylistsImpl _value,
    $Res Function(_$LibraryEvent_PlaylistsImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? items = null}) {
    return _then(
      _$LibraryEvent_PlaylistsImpl(
        items: null == items
            ? _value._items
            : items // ignore: cast_nullable_to_non_nullable
                  as List<PlaylistLite>,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_PlaylistsImpl extends LibraryEvent_Playlists {
  const _$LibraryEvent_PlaylistsImpl({required final List<PlaylistLite> items})
    : _items = items,
      super._();

  final List<PlaylistLite> _items;
  @override
  List<PlaylistLite> get items {
    if (_items is EqualUnmodifiableListView) return _items;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_items);
  }

  @override
  String toString() {
    return 'LibraryEvent.playlists(items: $items)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_PlaylistsImpl &&
            const DeepCollectionEquality().equals(other._items, _items));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_items));

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_PlaylistsImplCopyWith<_$LibraryEvent_PlaylistsImpl>
  get copyWith =>
      __$$LibraryEvent_PlaylistsImplCopyWithImpl<_$LibraryEvent_PlaylistsImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return playlists(items);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return playlists?.call(items);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (playlists != null) {
      return playlists(items);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return playlists(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return playlists?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (playlists != null) {
      return playlists(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_Playlists extends LibraryEvent {
  const factory LibraryEvent_Playlists({
    required final List<PlaylistLite> items,
  }) = _$LibraryEvent_PlaylistsImpl;
  const LibraryEvent_Playlists._() : super._();

  List<PlaylistLite> get items;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_PlaylistsImplCopyWith<_$LibraryEvent_PlaylistsImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_PlaylistTracksImplCopyWith<$Res> {
  factory _$$LibraryEvent_PlaylistTracksImplCopyWith(
    _$LibraryEvent_PlaylistTracksImpl value,
    $Res Function(_$LibraryEvent_PlaylistTracksImpl) then,
  ) = __$$LibraryEvent_PlaylistTracksImplCopyWithImpl<$Res>;
  @useResult
  $Res call({int playlistId, String query, List<TrackLite> items});
}

/// @nodoc
class __$$LibraryEvent_PlaylistTracksImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_PlaylistTracksImpl>
    implements _$$LibraryEvent_PlaylistTracksImplCopyWith<$Res> {
  __$$LibraryEvent_PlaylistTracksImplCopyWithImpl(
    _$LibraryEvent_PlaylistTracksImpl _value,
    $Res Function(_$LibraryEvent_PlaylistTracksImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? playlistId = null,
    Object? query = null,
    Object? items = null,
  }) {
    return _then(
      _$LibraryEvent_PlaylistTracksImpl(
        playlistId: null == playlistId
            ? _value.playlistId
            : playlistId // ignore: cast_nullable_to_non_nullable
                  as int,
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

class _$LibraryEvent_PlaylistTracksImpl extends LibraryEvent_PlaylistTracks {
  const _$LibraryEvent_PlaylistTracksImpl({
    required this.playlistId,
    required this.query,
    required final List<TrackLite> items,
  }) : _items = items,
       super._();

  @override
  final int playlistId;
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
    return 'LibraryEvent.playlistTracks(playlistId: $playlistId, query: $query, items: $items)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_PlaylistTracksImpl &&
            (identical(other.playlistId, playlistId) ||
                other.playlistId == playlistId) &&
            (identical(other.query, query) || other.query == query) &&
            const DeepCollectionEquality().equals(other._items, _items));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    playlistId,
    query,
    const DeepCollectionEquality().hash(_items),
  );

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_PlaylistTracksImplCopyWith<_$LibraryEvent_PlaylistTracksImpl>
  get copyWith =>
      __$$LibraryEvent_PlaylistTracksImplCopyWithImpl<
        _$LibraryEvent_PlaylistTracksImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return playlistTracks(playlistId, query, items);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return playlistTracks?.call(playlistId, query, items);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (playlistTracks != null) {
      return playlistTracks(playlistId, query, items);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return playlistTracks(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return playlistTracks?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (playlistTracks != null) {
      return playlistTracks(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_PlaylistTracks extends LibraryEvent {
  const factory LibraryEvent_PlaylistTracks({
    required final int playlistId,
    required final String query,
    required final List<TrackLite> items,
  }) = _$LibraryEvent_PlaylistTracksImpl;
  const LibraryEvent_PlaylistTracks._() : super._();

  int get playlistId;
  String get query;
  List<TrackLite> get items;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_PlaylistTracksImplCopyWith<_$LibraryEvent_PlaylistTracksImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LibraryEvent_LikedTrackIdsImplCopyWith<$Res> {
  factory _$$LibraryEvent_LikedTrackIdsImplCopyWith(
    _$LibraryEvent_LikedTrackIdsImpl value,
    $Res Function(_$LibraryEvent_LikedTrackIdsImpl) then,
  ) = __$$LibraryEvent_LikedTrackIdsImplCopyWithImpl<$Res>;
  @useResult
  $Res call({Int64List trackIds});
}

/// @nodoc
class __$$LibraryEvent_LikedTrackIdsImplCopyWithImpl<$Res>
    extends _$LibraryEventCopyWithImpl<$Res, _$LibraryEvent_LikedTrackIdsImpl>
    implements _$$LibraryEvent_LikedTrackIdsImplCopyWith<$Res> {
  __$$LibraryEvent_LikedTrackIdsImplCopyWithImpl(
    _$LibraryEvent_LikedTrackIdsImpl _value,
    $Res Function(_$LibraryEvent_LikedTrackIdsImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? trackIds = null}) {
    return _then(
      _$LibraryEvent_LikedTrackIdsImpl(
        trackIds: null == trackIds
            ? _value.trackIds
            : trackIds // ignore: cast_nullable_to_non_nullable
                  as Int64List,
      ),
    );
  }
}

/// @nodoc

class _$LibraryEvent_LikedTrackIdsImpl extends LibraryEvent_LikedTrackIds {
  const _$LibraryEvent_LikedTrackIdsImpl({required this.trackIds}) : super._();

  @override
  final Int64List trackIds;

  @override
  String toString() {
    return 'LibraryEvent.likedTrackIds(trackIds: $trackIds)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LibraryEvent_LikedTrackIdsImpl &&
            const DeepCollectionEquality().equals(other.trackIds, trackIds));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(trackIds));

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LibraryEvent_LikedTrackIdsImplCopyWith<_$LibraryEvent_LikedTrackIdsImpl>
  get copyWith =>
      __$$LibraryEvent_LikedTrackIdsImplCopyWithImpl<
        _$LibraryEvent_LikedTrackIdsImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> paths) roots,
    required TResult Function(List<String> paths) folders,
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
    required TResult Function(String message) error,
    required TResult Function(String message) log,
  }) {
    return likedTrackIds(trackIds);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> paths)? roots,
    TResult? Function(List<String> paths)? folders,
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
    TResult? Function(String message)? error,
    TResult? Function(String message)? log,
  }) {
    return likedTrackIds?.call(trackIds);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> paths)? roots,
    TResult Function(List<String> paths)? folders,
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
    TResult Function(String message)? error,
    TResult Function(String message)? log,
    required TResult orElse(),
  }) {
    if (likedTrackIds != null) {
      return likedTrackIds(trackIds);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LibraryEvent_Roots value) roots,
    required TResult Function(LibraryEvent_Folders value) folders,
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
    required TResult Function(LibraryEvent_Error value) error,
    required TResult Function(LibraryEvent_Log value) log,
  }) {
    return likedTrackIds(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LibraryEvent_Roots value)? roots,
    TResult? Function(LibraryEvent_Folders value)? folders,
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult? Function(LibraryEvent_Error value)? error,
    TResult? Function(LibraryEvent_Log value)? log,
  }) {
    return likedTrackIds?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LibraryEvent_Roots value)? roots,
    TResult Function(LibraryEvent_Folders value)? folders,
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
    TResult Function(LibraryEvent_Error value)? error,
    TResult Function(LibraryEvent_Log value)? log,
    required TResult orElse(),
  }) {
    if (likedTrackIds != null) {
      return likedTrackIds(this);
    }
    return orElse();
  }
}

abstract class LibraryEvent_LikedTrackIds extends LibraryEvent {
  const factory LibraryEvent_LikedTrackIds({
    required final Int64List trackIds,
  }) = _$LibraryEvent_LikedTrackIdsImpl;
  const LibraryEvent_LikedTrackIds._() : super._();

  Int64List get trackIds;

  /// Create a copy of LibraryEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LibraryEvent_LikedTrackIdsImplCopyWith<_$LibraryEvent_LikedTrackIdsImpl>
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    required TResult Function(List<String> paths) excludedFolders,
    required TResult Function() changed,
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
    required TResult Function(List<PlaylistLite> items) playlists,
    required TResult Function(
      int playlistId,
      String query,
      List<TrackLite> items,
    )
    playlistTracks,
    required TResult Function(Int64List trackIds) likedTrackIds,
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
    TResult? Function(List<String> paths)? excludedFolders,
    TResult? Function()? changed,
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
    TResult? Function(List<PlaylistLite> items)? playlists,
    TResult? Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult? Function(Int64List trackIds)? likedTrackIds,
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
    TResult Function(List<String> paths)? excludedFolders,
    TResult Function()? changed,
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
    TResult Function(List<PlaylistLite> items)? playlists,
    TResult Function(int playlistId, String query, List<TrackLite> items)?
    playlistTracks,
    TResult Function(Int64List trackIds)? likedTrackIds,
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
    required TResult Function(LibraryEvent_ExcludedFolders value)
    excludedFolders,
    required TResult Function(LibraryEvent_Changed value) changed,
    required TResult Function(LibraryEvent_Tracks value) tracks,
    required TResult Function(LibraryEvent_ScanProgress value) scanProgress,
    required TResult Function(LibraryEvent_ScanFinished value) scanFinished,
    required TResult Function(LibraryEvent_SearchResult value) searchResult,
    required TResult Function(LibraryEvent_Playlists value) playlists,
    required TResult Function(LibraryEvent_PlaylistTracks value) playlistTracks,
    required TResult Function(LibraryEvent_LikedTrackIds value) likedTrackIds,
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
    TResult? Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult? Function(LibraryEvent_Changed value)? changed,
    TResult? Function(LibraryEvent_Tracks value)? tracks,
    TResult? Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult? Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult? Function(LibraryEvent_SearchResult value)? searchResult,
    TResult? Function(LibraryEvent_Playlists value)? playlists,
    TResult? Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult? Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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
    TResult Function(LibraryEvent_ExcludedFolders value)? excludedFolders,
    TResult Function(LibraryEvent_Changed value)? changed,
    TResult Function(LibraryEvent_Tracks value)? tracks,
    TResult Function(LibraryEvent_ScanProgress value)? scanProgress,
    TResult Function(LibraryEvent_ScanFinished value)? scanFinished,
    TResult Function(LibraryEvent_SearchResult value)? searchResult,
    TResult Function(LibraryEvent_Playlists value)? playlists,
    TResult Function(LibraryEvent_PlaylistTracks value)? playlistTracks,
    TResult Function(LibraryEvent_LikedTrackIds value)? likedTrackIds,
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

/// @nodoc
mixin _$LyricsEvent {
  String get trackKey => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String trackKey) loading,
    required TResult Function(String trackKey, LyricsDoc doc) ready,
    required TResult Function(String trackKey, int lineIndex) cursor,
    required TResult Function(String trackKey) empty,
    required TResult Function(String trackKey, String message) error,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String trackKey)? loading,
    TResult? Function(String trackKey, LyricsDoc doc)? ready,
    TResult? Function(String trackKey, int lineIndex)? cursor,
    TResult? Function(String trackKey)? empty,
    TResult? Function(String trackKey, String message)? error,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String trackKey)? loading,
    TResult Function(String trackKey, LyricsDoc doc)? ready,
    TResult Function(String trackKey, int lineIndex)? cursor,
    TResult Function(String trackKey)? empty,
    TResult Function(String trackKey, String message)? error,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LyricsEvent_Loading value) loading,
    required TResult Function(LyricsEvent_Ready value) ready,
    required TResult Function(LyricsEvent_Cursor value) cursor,
    required TResult Function(LyricsEvent_Empty value) empty,
    required TResult Function(LyricsEvent_Error value) error,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LyricsEvent_Loading value)? loading,
    TResult? Function(LyricsEvent_Ready value)? ready,
    TResult? Function(LyricsEvent_Cursor value)? cursor,
    TResult? Function(LyricsEvent_Empty value)? empty,
    TResult? Function(LyricsEvent_Error value)? error,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LyricsEvent_Loading value)? loading,
    TResult Function(LyricsEvent_Ready value)? ready,
    TResult Function(LyricsEvent_Cursor value)? cursor,
    TResult Function(LyricsEvent_Empty value)? empty,
    TResult Function(LyricsEvent_Error value)? error,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  $LyricsEventCopyWith<LyricsEvent> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $LyricsEventCopyWith<$Res> {
  factory $LyricsEventCopyWith(
    LyricsEvent value,
    $Res Function(LyricsEvent) then,
  ) = _$LyricsEventCopyWithImpl<$Res, LyricsEvent>;
  @useResult
  $Res call({String trackKey});
}

/// @nodoc
class _$LyricsEventCopyWithImpl<$Res, $Val extends LyricsEvent>
    implements $LyricsEventCopyWith<$Res> {
  _$LyricsEventCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? trackKey = null}) {
    return _then(
      _value.copyWith(
            trackKey: null == trackKey
                ? _value.trackKey
                : trackKey // ignore: cast_nullable_to_non_nullable
                      as String,
          )
          as $Val,
    );
  }
}

/// @nodoc
abstract class _$$LyricsEvent_LoadingImplCopyWith<$Res>
    implements $LyricsEventCopyWith<$Res> {
  factory _$$LyricsEvent_LoadingImplCopyWith(
    _$LyricsEvent_LoadingImpl value,
    $Res Function(_$LyricsEvent_LoadingImpl) then,
  ) = __$$LyricsEvent_LoadingImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({String trackKey});
}

/// @nodoc
class __$$LyricsEvent_LoadingImplCopyWithImpl<$Res>
    extends _$LyricsEventCopyWithImpl<$Res, _$LyricsEvent_LoadingImpl>
    implements _$$LyricsEvent_LoadingImplCopyWith<$Res> {
  __$$LyricsEvent_LoadingImplCopyWithImpl(
    _$LyricsEvent_LoadingImpl _value,
    $Res Function(_$LyricsEvent_LoadingImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? trackKey = null}) {
    return _then(
      _$LyricsEvent_LoadingImpl(
        trackKey: null == trackKey
            ? _value.trackKey
            : trackKey // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$LyricsEvent_LoadingImpl extends LyricsEvent_Loading {
  const _$LyricsEvent_LoadingImpl({required this.trackKey}) : super._();

  @override
  final String trackKey;

  @override
  String toString() {
    return 'LyricsEvent.loading(trackKey: $trackKey)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LyricsEvent_LoadingImpl &&
            (identical(other.trackKey, trackKey) ||
                other.trackKey == trackKey));
  }

  @override
  int get hashCode => Object.hash(runtimeType, trackKey);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LyricsEvent_LoadingImplCopyWith<_$LyricsEvent_LoadingImpl> get copyWith =>
      __$$LyricsEvent_LoadingImplCopyWithImpl<_$LyricsEvent_LoadingImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String trackKey) loading,
    required TResult Function(String trackKey, LyricsDoc doc) ready,
    required TResult Function(String trackKey, int lineIndex) cursor,
    required TResult Function(String trackKey) empty,
    required TResult Function(String trackKey, String message) error,
  }) {
    return loading(trackKey);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String trackKey)? loading,
    TResult? Function(String trackKey, LyricsDoc doc)? ready,
    TResult? Function(String trackKey, int lineIndex)? cursor,
    TResult? Function(String trackKey)? empty,
    TResult? Function(String trackKey, String message)? error,
  }) {
    return loading?.call(trackKey);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String trackKey)? loading,
    TResult Function(String trackKey, LyricsDoc doc)? ready,
    TResult Function(String trackKey, int lineIndex)? cursor,
    TResult Function(String trackKey)? empty,
    TResult Function(String trackKey, String message)? error,
    required TResult orElse(),
  }) {
    if (loading != null) {
      return loading(trackKey);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LyricsEvent_Loading value) loading,
    required TResult Function(LyricsEvent_Ready value) ready,
    required TResult Function(LyricsEvent_Cursor value) cursor,
    required TResult Function(LyricsEvent_Empty value) empty,
    required TResult Function(LyricsEvent_Error value) error,
  }) {
    return loading(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LyricsEvent_Loading value)? loading,
    TResult? Function(LyricsEvent_Ready value)? ready,
    TResult? Function(LyricsEvent_Cursor value)? cursor,
    TResult? Function(LyricsEvent_Empty value)? empty,
    TResult? Function(LyricsEvent_Error value)? error,
  }) {
    return loading?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LyricsEvent_Loading value)? loading,
    TResult Function(LyricsEvent_Ready value)? ready,
    TResult Function(LyricsEvent_Cursor value)? cursor,
    TResult Function(LyricsEvent_Empty value)? empty,
    TResult Function(LyricsEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (loading != null) {
      return loading(this);
    }
    return orElse();
  }
}

abstract class LyricsEvent_Loading extends LyricsEvent {
  const factory LyricsEvent_Loading({required final String trackKey}) =
      _$LyricsEvent_LoadingImpl;
  const LyricsEvent_Loading._() : super._();

  @override
  String get trackKey;

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LyricsEvent_LoadingImplCopyWith<_$LyricsEvent_LoadingImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LyricsEvent_ReadyImplCopyWith<$Res>
    implements $LyricsEventCopyWith<$Res> {
  factory _$$LyricsEvent_ReadyImplCopyWith(
    _$LyricsEvent_ReadyImpl value,
    $Res Function(_$LyricsEvent_ReadyImpl) then,
  ) = __$$LyricsEvent_ReadyImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({String trackKey, LyricsDoc doc});
}

/// @nodoc
class __$$LyricsEvent_ReadyImplCopyWithImpl<$Res>
    extends _$LyricsEventCopyWithImpl<$Res, _$LyricsEvent_ReadyImpl>
    implements _$$LyricsEvent_ReadyImplCopyWith<$Res> {
  __$$LyricsEvent_ReadyImplCopyWithImpl(
    _$LyricsEvent_ReadyImpl _value,
    $Res Function(_$LyricsEvent_ReadyImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? trackKey = null, Object? doc = null}) {
    return _then(
      _$LyricsEvent_ReadyImpl(
        trackKey: null == trackKey
            ? _value.trackKey
            : trackKey // ignore: cast_nullable_to_non_nullable
                  as String,
        doc: null == doc
            ? _value.doc
            : doc // ignore: cast_nullable_to_non_nullable
                  as LyricsDoc,
      ),
    );
  }
}

/// @nodoc

class _$LyricsEvent_ReadyImpl extends LyricsEvent_Ready {
  const _$LyricsEvent_ReadyImpl({required this.trackKey, required this.doc})
    : super._();

  @override
  final String trackKey;
  @override
  final LyricsDoc doc;

  @override
  String toString() {
    return 'LyricsEvent.ready(trackKey: $trackKey, doc: $doc)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LyricsEvent_ReadyImpl &&
            (identical(other.trackKey, trackKey) ||
                other.trackKey == trackKey) &&
            (identical(other.doc, doc) || other.doc == doc));
  }

  @override
  int get hashCode => Object.hash(runtimeType, trackKey, doc);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LyricsEvent_ReadyImplCopyWith<_$LyricsEvent_ReadyImpl> get copyWith =>
      __$$LyricsEvent_ReadyImplCopyWithImpl<_$LyricsEvent_ReadyImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String trackKey) loading,
    required TResult Function(String trackKey, LyricsDoc doc) ready,
    required TResult Function(String trackKey, int lineIndex) cursor,
    required TResult Function(String trackKey) empty,
    required TResult Function(String trackKey, String message) error,
  }) {
    return ready(trackKey, doc);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String trackKey)? loading,
    TResult? Function(String trackKey, LyricsDoc doc)? ready,
    TResult? Function(String trackKey, int lineIndex)? cursor,
    TResult? Function(String trackKey)? empty,
    TResult? Function(String trackKey, String message)? error,
  }) {
    return ready?.call(trackKey, doc);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String trackKey)? loading,
    TResult Function(String trackKey, LyricsDoc doc)? ready,
    TResult Function(String trackKey, int lineIndex)? cursor,
    TResult Function(String trackKey)? empty,
    TResult Function(String trackKey, String message)? error,
    required TResult orElse(),
  }) {
    if (ready != null) {
      return ready(trackKey, doc);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LyricsEvent_Loading value) loading,
    required TResult Function(LyricsEvent_Ready value) ready,
    required TResult Function(LyricsEvent_Cursor value) cursor,
    required TResult Function(LyricsEvent_Empty value) empty,
    required TResult Function(LyricsEvent_Error value) error,
  }) {
    return ready(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LyricsEvent_Loading value)? loading,
    TResult? Function(LyricsEvent_Ready value)? ready,
    TResult? Function(LyricsEvent_Cursor value)? cursor,
    TResult? Function(LyricsEvent_Empty value)? empty,
    TResult? Function(LyricsEvent_Error value)? error,
  }) {
    return ready?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LyricsEvent_Loading value)? loading,
    TResult Function(LyricsEvent_Ready value)? ready,
    TResult Function(LyricsEvent_Cursor value)? cursor,
    TResult Function(LyricsEvent_Empty value)? empty,
    TResult Function(LyricsEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (ready != null) {
      return ready(this);
    }
    return orElse();
  }
}

abstract class LyricsEvent_Ready extends LyricsEvent {
  const factory LyricsEvent_Ready({
    required final String trackKey,
    required final LyricsDoc doc,
  }) = _$LyricsEvent_ReadyImpl;
  const LyricsEvent_Ready._() : super._();

  @override
  String get trackKey;
  LyricsDoc get doc;

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LyricsEvent_ReadyImplCopyWith<_$LyricsEvent_ReadyImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LyricsEvent_CursorImplCopyWith<$Res>
    implements $LyricsEventCopyWith<$Res> {
  factory _$$LyricsEvent_CursorImplCopyWith(
    _$LyricsEvent_CursorImpl value,
    $Res Function(_$LyricsEvent_CursorImpl) then,
  ) = __$$LyricsEvent_CursorImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({String trackKey, int lineIndex});
}

/// @nodoc
class __$$LyricsEvent_CursorImplCopyWithImpl<$Res>
    extends _$LyricsEventCopyWithImpl<$Res, _$LyricsEvent_CursorImpl>
    implements _$$LyricsEvent_CursorImplCopyWith<$Res> {
  __$$LyricsEvent_CursorImplCopyWithImpl(
    _$LyricsEvent_CursorImpl _value,
    $Res Function(_$LyricsEvent_CursorImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? trackKey = null, Object? lineIndex = null}) {
    return _then(
      _$LyricsEvent_CursorImpl(
        trackKey: null == trackKey
            ? _value.trackKey
            : trackKey // ignore: cast_nullable_to_non_nullable
                  as String,
        lineIndex: null == lineIndex
            ? _value.lineIndex
            : lineIndex // ignore: cast_nullable_to_non_nullable
                  as int,
      ),
    );
  }
}

/// @nodoc

class _$LyricsEvent_CursorImpl extends LyricsEvent_Cursor {
  const _$LyricsEvent_CursorImpl({
    required this.trackKey,
    required this.lineIndex,
  }) : super._();

  @override
  final String trackKey;
  @override
  final int lineIndex;

  @override
  String toString() {
    return 'LyricsEvent.cursor(trackKey: $trackKey, lineIndex: $lineIndex)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LyricsEvent_CursorImpl &&
            (identical(other.trackKey, trackKey) ||
                other.trackKey == trackKey) &&
            (identical(other.lineIndex, lineIndex) ||
                other.lineIndex == lineIndex));
  }

  @override
  int get hashCode => Object.hash(runtimeType, trackKey, lineIndex);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LyricsEvent_CursorImplCopyWith<_$LyricsEvent_CursorImpl> get copyWith =>
      __$$LyricsEvent_CursorImplCopyWithImpl<_$LyricsEvent_CursorImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String trackKey) loading,
    required TResult Function(String trackKey, LyricsDoc doc) ready,
    required TResult Function(String trackKey, int lineIndex) cursor,
    required TResult Function(String trackKey) empty,
    required TResult Function(String trackKey, String message) error,
  }) {
    return cursor(trackKey, lineIndex);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String trackKey)? loading,
    TResult? Function(String trackKey, LyricsDoc doc)? ready,
    TResult? Function(String trackKey, int lineIndex)? cursor,
    TResult? Function(String trackKey)? empty,
    TResult? Function(String trackKey, String message)? error,
  }) {
    return cursor?.call(trackKey, lineIndex);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String trackKey)? loading,
    TResult Function(String trackKey, LyricsDoc doc)? ready,
    TResult Function(String trackKey, int lineIndex)? cursor,
    TResult Function(String trackKey)? empty,
    TResult Function(String trackKey, String message)? error,
    required TResult orElse(),
  }) {
    if (cursor != null) {
      return cursor(trackKey, lineIndex);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LyricsEvent_Loading value) loading,
    required TResult Function(LyricsEvent_Ready value) ready,
    required TResult Function(LyricsEvent_Cursor value) cursor,
    required TResult Function(LyricsEvent_Empty value) empty,
    required TResult Function(LyricsEvent_Error value) error,
  }) {
    return cursor(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LyricsEvent_Loading value)? loading,
    TResult? Function(LyricsEvent_Ready value)? ready,
    TResult? Function(LyricsEvent_Cursor value)? cursor,
    TResult? Function(LyricsEvent_Empty value)? empty,
    TResult? Function(LyricsEvent_Error value)? error,
  }) {
    return cursor?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LyricsEvent_Loading value)? loading,
    TResult Function(LyricsEvent_Ready value)? ready,
    TResult Function(LyricsEvent_Cursor value)? cursor,
    TResult Function(LyricsEvent_Empty value)? empty,
    TResult Function(LyricsEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (cursor != null) {
      return cursor(this);
    }
    return orElse();
  }
}

abstract class LyricsEvent_Cursor extends LyricsEvent {
  const factory LyricsEvent_Cursor({
    required final String trackKey,
    required final int lineIndex,
  }) = _$LyricsEvent_CursorImpl;
  const LyricsEvent_Cursor._() : super._();

  @override
  String get trackKey;
  int get lineIndex;

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LyricsEvent_CursorImplCopyWith<_$LyricsEvent_CursorImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LyricsEvent_EmptyImplCopyWith<$Res>
    implements $LyricsEventCopyWith<$Res> {
  factory _$$LyricsEvent_EmptyImplCopyWith(
    _$LyricsEvent_EmptyImpl value,
    $Res Function(_$LyricsEvent_EmptyImpl) then,
  ) = __$$LyricsEvent_EmptyImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({String trackKey});
}

/// @nodoc
class __$$LyricsEvent_EmptyImplCopyWithImpl<$Res>
    extends _$LyricsEventCopyWithImpl<$Res, _$LyricsEvent_EmptyImpl>
    implements _$$LyricsEvent_EmptyImplCopyWith<$Res> {
  __$$LyricsEvent_EmptyImplCopyWithImpl(
    _$LyricsEvent_EmptyImpl _value,
    $Res Function(_$LyricsEvent_EmptyImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? trackKey = null}) {
    return _then(
      _$LyricsEvent_EmptyImpl(
        trackKey: null == trackKey
            ? _value.trackKey
            : trackKey // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$LyricsEvent_EmptyImpl extends LyricsEvent_Empty {
  const _$LyricsEvent_EmptyImpl({required this.trackKey}) : super._();

  @override
  final String trackKey;

  @override
  String toString() {
    return 'LyricsEvent.empty(trackKey: $trackKey)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LyricsEvent_EmptyImpl &&
            (identical(other.trackKey, trackKey) ||
                other.trackKey == trackKey));
  }

  @override
  int get hashCode => Object.hash(runtimeType, trackKey);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LyricsEvent_EmptyImplCopyWith<_$LyricsEvent_EmptyImpl> get copyWith =>
      __$$LyricsEvent_EmptyImplCopyWithImpl<_$LyricsEvent_EmptyImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String trackKey) loading,
    required TResult Function(String trackKey, LyricsDoc doc) ready,
    required TResult Function(String trackKey, int lineIndex) cursor,
    required TResult Function(String trackKey) empty,
    required TResult Function(String trackKey, String message) error,
  }) {
    return empty(trackKey);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String trackKey)? loading,
    TResult? Function(String trackKey, LyricsDoc doc)? ready,
    TResult? Function(String trackKey, int lineIndex)? cursor,
    TResult? Function(String trackKey)? empty,
    TResult? Function(String trackKey, String message)? error,
  }) {
    return empty?.call(trackKey);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String trackKey)? loading,
    TResult Function(String trackKey, LyricsDoc doc)? ready,
    TResult Function(String trackKey, int lineIndex)? cursor,
    TResult Function(String trackKey)? empty,
    TResult Function(String trackKey, String message)? error,
    required TResult orElse(),
  }) {
    if (empty != null) {
      return empty(trackKey);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LyricsEvent_Loading value) loading,
    required TResult Function(LyricsEvent_Ready value) ready,
    required TResult Function(LyricsEvent_Cursor value) cursor,
    required TResult Function(LyricsEvent_Empty value) empty,
    required TResult Function(LyricsEvent_Error value) error,
  }) {
    return empty(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LyricsEvent_Loading value)? loading,
    TResult? Function(LyricsEvent_Ready value)? ready,
    TResult? Function(LyricsEvent_Cursor value)? cursor,
    TResult? Function(LyricsEvent_Empty value)? empty,
    TResult? Function(LyricsEvent_Error value)? error,
  }) {
    return empty?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LyricsEvent_Loading value)? loading,
    TResult Function(LyricsEvent_Ready value)? ready,
    TResult Function(LyricsEvent_Cursor value)? cursor,
    TResult Function(LyricsEvent_Empty value)? empty,
    TResult Function(LyricsEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (empty != null) {
      return empty(this);
    }
    return orElse();
  }
}

abstract class LyricsEvent_Empty extends LyricsEvent {
  const factory LyricsEvent_Empty({required final String trackKey}) =
      _$LyricsEvent_EmptyImpl;
  const LyricsEvent_Empty._() : super._();

  @override
  String get trackKey;

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LyricsEvent_EmptyImplCopyWith<_$LyricsEvent_EmptyImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$LyricsEvent_ErrorImplCopyWith<$Res>
    implements $LyricsEventCopyWith<$Res> {
  factory _$$LyricsEvent_ErrorImplCopyWith(
    _$LyricsEvent_ErrorImpl value,
    $Res Function(_$LyricsEvent_ErrorImpl) then,
  ) = __$$LyricsEvent_ErrorImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({String trackKey, String message});
}

/// @nodoc
class __$$LyricsEvent_ErrorImplCopyWithImpl<$Res>
    extends _$LyricsEventCopyWithImpl<$Res, _$LyricsEvent_ErrorImpl>
    implements _$$LyricsEvent_ErrorImplCopyWith<$Res> {
  __$$LyricsEvent_ErrorImplCopyWithImpl(
    _$LyricsEvent_ErrorImpl _value,
    $Res Function(_$LyricsEvent_ErrorImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? trackKey = null, Object? message = null}) {
    return _then(
      _$LyricsEvent_ErrorImpl(
        trackKey: null == trackKey
            ? _value.trackKey
            : trackKey // ignore: cast_nullable_to_non_nullable
                  as String,
        message: null == message
            ? _value.message
            : message // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$LyricsEvent_ErrorImpl extends LyricsEvent_Error {
  const _$LyricsEvent_ErrorImpl({required this.trackKey, required this.message})
    : super._();

  @override
  final String trackKey;
  @override
  final String message;

  @override
  String toString() {
    return 'LyricsEvent.error(trackKey: $trackKey, message: $message)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$LyricsEvent_ErrorImpl &&
            (identical(other.trackKey, trackKey) ||
                other.trackKey == trackKey) &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode => Object.hash(runtimeType, trackKey, message);

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$LyricsEvent_ErrorImplCopyWith<_$LyricsEvent_ErrorImpl> get copyWith =>
      __$$LyricsEvent_ErrorImplCopyWithImpl<_$LyricsEvent_ErrorImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String trackKey) loading,
    required TResult Function(String trackKey, LyricsDoc doc) ready,
    required TResult Function(String trackKey, int lineIndex) cursor,
    required TResult Function(String trackKey) empty,
    required TResult Function(String trackKey, String message) error,
  }) {
    return error(trackKey, message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String trackKey)? loading,
    TResult? Function(String trackKey, LyricsDoc doc)? ready,
    TResult? Function(String trackKey, int lineIndex)? cursor,
    TResult? Function(String trackKey)? empty,
    TResult? Function(String trackKey, String message)? error,
  }) {
    return error?.call(trackKey, message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String trackKey)? loading,
    TResult Function(String trackKey, LyricsDoc doc)? ready,
    TResult Function(String trackKey, int lineIndex)? cursor,
    TResult Function(String trackKey)? empty,
    TResult Function(String trackKey, String message)? error,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(trackKey, message);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(LyricsEvent_Loading value) loading,
    required TResult Function(LyricsEvent_Ready value) ready,
    required TResult Function(LyricsEvent_Cursor value) cursor,
    required TResult Function(LyricsEvent_Empty value) empty,
    required TResult Function(LyricsEvent_Error value) error,
  }) {
    return error(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(LyricsEvent_Loading value)? loading,
    TResult? Function(LyricsEvent_Ready value)? ready,
    TResult? Function(LyricsEvent_Cursor value)? cursor,
    TResult? Function(LyricsEvent_Empty value)? empty,
    TResult? Function(LyricsEvent_Error value)? error,
  }) {
    return error?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(LyricsEvent_Loading value)? loading,
    TResult Function(LyricsEvent_Ready value)? ready,
    TResult Function(LyricsEvent_Cursor value)? cursor,
    TResult Function(LyricsEvent_Empty value)? empty,
    TResult Function(LyricsEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(this);
    }
    return orElse();
  }
}

abstract class LyricsEvent_Error extends LyricsEvent {
  const factory LyricsEvent_Error({
    required final String trackKey,
    required final String message,
  }) = _$LyricsEvent_ErrorImpl;
  const LyricsEvent_Error._() : super._();

  @override
  String get trackKey;
  String get message;

  /// Create a copy of LyricsEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$LyricsEvent_ErrorImplCopyWith<_$LyricsEvent_ErrorImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
