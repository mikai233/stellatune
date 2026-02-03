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
mixin _$Command {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $CommandCopyWith<$Res> {
  factory $CommandCopyWith(Command value, $Res Function(Command) then) =
      _$CommandCopyWithImpl<$Res, Command>;
}

/// @nodoc
class _$CommandCopyWithImpl<$Res, $Val extends Command>
    implements $CommandCopyWith<$Res> {
  _$CommandCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$Command_PlayImplCopyWith<$Res> {
  factory _$$Command_PlayImplCopyWith(
    _$Command_PlayImpl value,
    $Res Function(_$Command_PlayImpl) then,
  ) = __$$Command_PlayImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$Command_PlayImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_PlayImpl>
    implements _$$Command_PlayImplCopyWith<$Res> {
  __$$Command_PlayImplCopyWithImpl(
    _$Command_PlayImpl _value,
    $Res Function(_$Command_PlayImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$Command_PlayImpl extends Command_Play {
  const _$Command_PlayImpl() : super._();

  @override
  String toString() {
    return 'Command.play()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is _$Command_PlayImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return play();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return play?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (play != null) {
      return play();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return play(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return play?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (play != null) {
      return play(this);
    }
    return orElse();
  }
}

abstract class Command_Play extends Command {
  const factory Command_Play() = _$Command_PlayImpl;
  const Command_Play._() : super._();
}

/// @nodoc
abstract class _$$Command_PauseImplCopyWith<$Res> {
  factory _$$Command_PauseImplCopyWith(
    _$Command_PauseImpl value,
    $Res Function(_$Command_PauseImpl) then,
  ) = __$$Command_PauseImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$Command_PauseImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_PauseImpl>
    implements _$$Command_PauseImplCopyWith<$Res> {
  __$$Command_PauseImplCopyWithImpl(
    _$Command_PauseImpl _value,
    $Res Function(_$Command_PauseImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$Command_PauseImpl extends Command_Pause {
  const _$Command_PauseImpl() : super._();

  @override
  String toString() {
    return 'Command.pause()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is _$Command_PauseImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return pause();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return pause?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (pause != null) {
      return pause();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return pause(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return pause?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (pause != null) {
      return pause(this);
    }
    return orElse();
  }
}

abstract class Command_Pause extends Command {
  const factory Command_Pause() = _$Command_PauseImpl;
  const Command_Pause._() : super._();
}

/// @nodoc
abstract class _$$Command_StopImplCopyWith<$Res> {
  factory _$$Command_StopImplCopyWith(
    _$Command_StopImpl value,
    $Res Function(_$Command_StopImpl) then,
  ) = __$$Command_StopImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$Command_StopImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_StopImpl>
    implements _$$Command_StopImplCopyWith<$Res> {
  __$$Command_StopImplCopyWithImpl(
    _$Command_StopImpl _value,
    $Res Function(_$Command_StopImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$Command_StopImpl extends Command_Stop {
  const _$Command_StopImpl() : super._();

  @override
  String toString() {
    return 'Command.stop()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is _$Command_StopImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return stop();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return stop?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (stop != null) {
      return stop();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return stop(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return stop?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (stop != null) {
      return stop(this);
    }
    return orElse();
  }
}

abstract class Command_Stop extends Command {
  const factory Command_Stop() = _$Command_StopImpl;
  const Command_Stop._() : super._();
}

/// @nodoc
abstract class _$$Command_SeekImplCopyWith<$Res> {
  factory _$$Command_SeekImplCopyWith(
    _$Command_SeekImpl value,
    $Res Function(_$Command_SeekImpl) then,
  ) = __$$Command_SeekImplCopyWithImpl<$Res>;
  @useResult
  $Res call({int ms});
}

/// @nodoc
class __$$Command_SeekImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_SeekImpl>
    implements _$$Command_SeekImplCopyWith<$Res> {
  __$$Command_SeekImplCopyWithImpl(
    _$Command_SeekImpl _value,
    $Res Function(_$Command_SeekImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? ms = null}) {
    return _then(
      _$Command_SeekImpl(
        ms: null == ms
            ? _value.ms
            : ms // ignore: cast_nullable_to_non_nullable
                  as int,
      ),
    );
  }
}

/// @nodoc

class _$Command_SeekImpl extends Command_Seek {
  const _$Command_SeekImpl({required this.ms}) : super._();

  @override
  final int ms;

  @override
  String toString() {
    return 'Command.seek(ms: $ms)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Command_SeekImpl &&
            (identical(other.ms, ms) || other.ms == ms));
  }

  @override
  int get hashCode => Object.hash(runtimeType, ms);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Command_SeekImplCopyWith<_$Command_SeekImpl> get copyWith =>
      __$$Command_SeekImplCopyWithImpl<_$Command_SeekImpl>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return seek(ms);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return seek?.call(ms);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (seek != null) {
      return seek(ms);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return seek(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return seek?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (seek != null) {
      return seek(this);
    }
    return orElse();
  }
}

abstract class Command_Seek extends Command {
  const factory Command_Seek({required final int ms}) = _$Command_SeekImpl;
  const Command_Seek._() : super._();

  int get ms;

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Command_SeekImplCopyWith<_$Command_SeekImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Command_LoadTrackImplCopyWith<$Res> {
  factory _$$Command_LoadTrackImplCopyWith(
    _$Command_LoadTrackImpl value,
    $Res Function(_$Command_LoadTrackImpl) then,
  ) = __$$Command_LoadTrackImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String path});
}

/// @nodoc
class __$$Command_LoadTrackImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_LoadTrackImpl>
    implements _$$Command_LoadTrackImplCopyWith<$Res> {
  __$$Command_LoadTrackImplCopyWithImpl(
    _$Command_LoadTrackImpl _value,
    $Res Function(_$Command_LoadTrackImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? path = null}) {
    return _then(
      _$Command_LoadTrackImpl(
        path: null == path
            ? _value.path
            : path // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$Command_LoadTrackImpl extends Command_LoadTrack {
  const _$Command_LoadTrackImpl({required this.path}) : super._();

  @override
  final String path;

  @override
  String toString() {
    return 'Command.loadTrack(path: $path)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Command_LoadTrackImpl &&
            (identical(other.path, path) || other.path == path));
  }

  @override
  int get hashCode => Object.hash(runtimeType, path);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Command_LoadTrackImplCopyWith<_$Command_LoadTrackImpl> get copyWith =>
      __$$Command_LoadTrackImplCopyWithImpl<_$Command_LoadTrackImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return loadTrack(path);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return loadTrack?.call(path);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (loadTrack != null) {
      return loadTrack(path);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return loadTrack(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return loadTrack?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (loadTrack != null) {
      return loadTrack(this);
    }
    return orElse();
  }
}

abstract class Command_LoadTrack extends Command {
  const factory Command_LoadTrack({required final String path}) =
      _$Command_LoadTrackImpl;
  const Command_LoadTrack._() : super._();

  String get path;

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Command_LoadTrackImplCopyWith<_$Command_LoadTrackImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Command_SetVolumeImplCopyWith<$Res> {
  factory _$$Command_SetVolumeImplCopyWith(
    _$Command_SetVolumeImpl value,
    $Res Function(_$Command_SetVolumeImpl) then,
  ) = __$$Command_SetVolumeImplCopyWithImpl<$Res>;
  @useResult
  $Res call({double linear});
}

/// @nodoc
class __$$Command_SetVolumeImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_SetVolumeImpl>
    implements _$$Command_SetVolumeImplCopyWith<$Res> {
  __$$Command_SetVolumeImplCopyWithImpl(
    _$Command_SetVolumeImpl _value,
    $Res Function(_$Command_SetVolumeImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? linear = null}) {
    return _then(
      _$Command_SetVolumeImpl(
        linear: null == linear
            ? _value.linear
            : linear // ignore: cast_nullable_to_non_nullable
                  as double,
      ),
    );
  }
}

/// @nodoc

class _$Command_SetVolumeImpl extends Command_SetVolume {
  const _$Command_SetVolumeImpl({required this.linear}) : super._();

  @override
  final double linear;

  @override
  String toString() {
    return 'Command.setVolume(linear: $linear)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Command_SetVolumeImpl &&
            (identical(other.linear, linear) || other.linear == linear));
  }

  @override
  int get hashCode => Object.hash(runtimeType, linear);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Command_SetVolumeImplCopyWith<_$Command_SetVolumeImpl> get copyWith =>
      __$$Command_SetVolumeImplCopyWithImpl<_$Command_SetVolumeImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return setVolume(linear);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return setVolume?.call(linear);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (setVolume != null) {
      return setVolume(linear);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return setVolume(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return setVolume?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (setVolume != null) {
      return setVolume(this);
    }
    return orElse();
  }
}

abstract class Command_SetVolume extends Command {
  const factory Command_SetVolume({required final double linear}) =
      _$Command_SetVolumeImpl;
  const Command_SetVolume._() : super._();

  double get linear;

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Command_SetVolumeImplCopyWith<_$Command_SetVolumeImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Command_SetMutedImplCopyWith<$Res> {
  factory _$$Command_SetMutedImplCopyWith(
    _$Command_SetMutedImpl value,
    $Res Function(_$Command_SetMutedImpl) then,
  ) = __$$Command_SetMutedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({bool muted});
}

/// @nodoc
class __$$Command_SetMutedImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_SetMutedImpl>
    implements _$$Command_SetMutedImplCopyWith<$Res> {
  __$$Command_SetMutedImplCopyWithImpl(
    _$Command_SetMutedImpl _value,
    $Res Function(_$Command_SetMutedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? muted = null}) {
    return _then(
      _$Command_SetMutedImpl(
        muted: null == muted
            ? _value.muted
            : muted // ignore: cast_nullable_to_non_nullable
                  as bool,
      ),
    );
  }
}

/// @nodoc

class _$Command_SetMutedImpl extends Command_SetMuted {
  const _$Command_SetMutedImpl({required this.muted}) : super._();

  @override
  final bool muted;

  @override
  String toString() {
    return 'Command.setMuted(muted: $muted)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Command_SetMutedImpl &&
            (identical(other.muted, muted) || other.muted == muted));
  }

  @override
  int get hashCode => Object.hash(runtimeType, muted);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Command_SetMutedImplCopyWith<_$Command_SetMutedImpl> get copyWith =>
      __$$Command_SetMutedImplCopyWithImpl<_$Command_SetMutedImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return setMuted(muted);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return setMuted?.call(muted);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (setMuted != null) {
      return setMuted(muted);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return setMuted(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return setMuted?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (setMuted != null) {
      return setMuted(this);
    }
    return orElse();
  }
}

abstract class Command_SetMuted extends Command {
  const factory Command_SetMuted({required final bool muted}) =
      _$Command_SetMutedImpl;
  const Command_SetMuted._() : super._();

  bool get muted;

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Command_SetMutedImplCopyWith<_$Command_SetMutedImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Command_EnqueueImplCopyWith<$Res> {
  factory _$$Command_EnqueueImplCopyWith(
    _$Command_EnqueueImpl value,
    $Res Function(_$Command_EnqueueImpl) then,
  ) = __$$Command_EnqueueImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String path});
}

/// @nodoc
class __$$Command_EnqueueImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_EnqueueImpl>
    implements _$$Command_EnqueueImplCopyWith<$Res> {
  __$$Command_EnqueueImplCopyWithImpl(
    _$Command_EnqueueImpl _value,
    $Res Function(_$Command_EnqueueImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? path = null}) {
    return _then(
      _$Command_EnqueueImpl(
        path: null == path
            ? _value.path
            : path // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$Command_EnqueueImpl extends Command_Enqueue {
  const _$Command_EnqueueImpl({required this.path}) : super._();

  @override
  final String path;

  @override
  String toString() {
    return 'Command.enqueue(path: $path)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Command_EnqueueImpl &&
            (identical(other.path, path) || other.path == path));
  }

  @override
  int get hashCode => Object.hash(runtimeType, path);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$Command_EnqueueImplCopyWith<_$Command_EnqueueImpl> get copyWith =>
      __$$Command_EnqueueImplCopyWithImpl<_$Command_EnqueueImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return enqueue(path);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return enqueue?.call(path);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (enqueue != null) {
      return enqueue(path);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return enqueue(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return enqueue?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (enqueue != null) {
      return enqueue(this);
    }
    return orElse();
  }
}

abstract class Command_Enqueue extends Command {
  const factory Command_Enqueue({required final String path}) =
      _$Command_EnqueueImpl;
  const Command_Enqueue._() : super._();

  String get path;

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$Command_EnqueueImplCopyWith<_$Command_EnqueueImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$Command_NextImplCopyWith<$Res> {
  factory _$$Command_NextImplCopyWith(
    _$Command_NextImpl value,
    $Res Function(_$Command_NextImpl) then,
  ) = __$$Command_NextImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$Command_NextImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_NextImpl>
    implements _$$Command_NextImplCopyWith<$Res> {
  __$$Command_NextImplCopyWithImpl(
    _$Command_NextImpl _value,
    $Res Function(_$Command_NextImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$Command_NextImpl extends Command_Next {
  const _$Command_NextImpl() : super._();

  @override
  String toString() {
    return 'Command.next()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is _$Command_NextImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return next();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return next?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (next != null) {
      return next();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return next(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return next?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (next != null) {
      return next(this);
    }
    return orElse();
  }
}

abstract class Command_Next extends Command {
  const factory Command_Next() = _$Command_NextImpl;
  const Command_Next._() : super._();
}

/// @nodoc
abstract class _$$Command_PreviousImplCopyWith<$Res> {
  factory _$$Command_PreviousImplCopyWith(
    _$Command_PreviousImpl value,
    $Res Function(_$Command_PreviousImpl) then,
  ) = __$$Command_PreviousImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$Command_PreviousImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_PreviousImpl>
    implements _$$Command_PreviousImplCopyWith<$Res> {
  __$$Command_PreviousImplCopyWithImpl(
    _$Command_PreviousImpl _value,
    $Res Function(_$Command_PreviousImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$Command_PreviousImpl extends Command_Previous {
  const _$Command_PreviousImpl() : super._();

  @override
  String toString() {
    return 'Command.previous()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is _$Command_PreviousImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return previous();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return previous?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (previous != null) {
      return previous();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return previous(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return previous?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (previous != null) {
      return previous(this);
    }
    return orElse();
  }
}

abstract class Command_Previous extends Command {
  const factory Command_Previous() = _$Command_PreviousImpl;
  const Command_Previous._() : super._();
}

/// @nodoc
abstract class _$$Command_ShutdownImplCopyWith<$Res> {
  factory _$$Command_ShutdownImplCopyWith(
    _$Command_ShutdownImpl value,
    $Res Function(_$Command_ShutdownImpl) then,
  ) = __$$Command_ShutdownImplCopyWithImpl<$Res>;
}

/// @nodoc
class __$$Command_ShutdownImplCopyWithImpl<$Res>
    extends _$CommandCopyWithImpl<$Res, _$Command_ShutdownImpl>
    implements _$$Command_ShutdownImplCopyWith<$Res> {
  __$$Command_ShutdownImplCopyWithImpl(
    _$Command_ShutdownImpl _value,
    $Res Function(_$Command_ShutdownImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of Command
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc

class _$Command_ShutdownImpl extends Command_Shutdown {
  const _$Command_ShutdownImpl() : super._();

  @override
  String toString() {
    return 'Command.shutdown()';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is _$Command_ShutdownImpl);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() play,
    required TResult Function() pause,
    required TResult Function() stop,
    required TResult Function(int ms) seek,
    required TResult Function(String path) loadTrack,
    required TResult Function(double linear) setVolume,
    required TResult Function(bool muted) setMuted,
    required TResult Function(String path) enqueue,
    required TResult Function() next,
    required TResult Function() previous,
    required TResult Function() shutdown,
  }) {
    return shutdown();
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? play,
    TResult? Function()? pause,
    TResult? Function()? stop,
    TResult? Function(int ms)? seek,
    TResult? Function(String path)? loadTrack,
    TResult? Function(double linear)? setVolume,
    TResult? Function(bool muted)? setMuted,
    TResult? Function(String path)? enqueue,
    TResult? Function()? next,
    TResult? Function()? previous,
    TResult? Function()? shutdown,
  }) {
    return shutdown?.call();
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? play,
    TResult Function()? pause,
    TResult Function()? stop,
    TResult Function(int ms)? seek,
    TResult Function(String path)? loadTrack,
    TResult Function(double linear)? setVolume,
    TResult Function(bool muted)? setMuted,
    TResult Function(String path)? enqueue,
    TResult Function()? next,
    TResult Function()? previous,
    TResult Function()? shutdown,
    required TResult orElse(),
  }) {
    if (shutdown != null) {
      return shutdown();
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Command_Play value) play,
    required TResult Function(Command_Pause value) pause,
    required TResult Function(Command_Stop value) stop,
    required TResult Function(Command_Seek value) seek,
    required TResult Function(Command_LoadTrack value) loadTrack,
    required TResult Function(Command_SetVolume value) setVolume,
    required TResult Function(Command_SetMuted value) setMuted,
    required TResult Function(Command_Enqueue value) enqueue,
    required TResult Function(Command_Next value) next,
    required TResult Function(Command_Previous value) previous,
    required TResult Function(Command_Shutdown value) shutdown,
  }) {
    return shutdown(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(Command_Play value)? play,
    TResult? Function(Command_Pause value)? pause,
    TResult? Function(Command_Stop value)? stop,
    TResult? Function(Command_Seek value)? seek,
    TResult? Function(Command_LoadTrack value)? loadTrack,
    TResult? Function(Command_SetVolume value)? setVolume,
    TResult? Function(Command_SetMuted value)? setMuted,
    TResult? Function(Command_Enqueue value)? enqueue,
    TResult? Function(Command_Next value)? next,
    TResult? Function(Command_Previous value)? previous,
    TResult? Function(Command_Shutdown value)? shutdown,
  }) {
    return shutdown?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Command_Play value)? play,
    TResult Function(Command_Pause value)? pause,
    TResult Function(Command_Stop value)? stop,
    TResult Function(Command_Seek value)? seek,
    TResult Function(Command_LoadTrack value)? loadTrack,
    TResult Function(Command_SetVolume value)? setVolume,
    TResult Function(Command_SetMuted value)? setMuted,
    TResult Function(Command_Enqueue value)? enqueue,
    TResult Function(Command_Next value)? next,
    TResult Function(Command_Previous value)? previous,
    TResult Function(Command_Shutdown value)? shutdown,
    required TResult orElse(),
  }) {
    if (shutdown != null) {
      return shutdown(this);
    }
    return orElse();
  }
}

abstract class Command_Shutdown extends Command {
  const factory Command_Shutdown() = _$Command_ShutdownImpl;
  const Command_Shutdown._() : super._();
}

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
