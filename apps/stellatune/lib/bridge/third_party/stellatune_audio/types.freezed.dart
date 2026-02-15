// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'types.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$Event {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'Event()';
}


}

/// @nodoc
class $EventCopyWith<$Res>  {
$EventCopyWith(Event _, $Res Function(Event) __);
}


/// Adds pattern-matching-related methods to [Event].
extension EventPatterns on Event {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( Event_StateChanged value)?  stateChanged,TResult Function( Event_Position value)?  position,TResult Function( Event_TrackChanged value)?  trackChanged,TResult Function( Event_PlaybackEnded value)?  playbackEnded,TResult Function( Event_VolumeChanged value)?  volumeChanged,TResult Function( Event_Error value)?  error,TResult Function( Event_Log value)?  log,required TResult orElse(),}){
final _that = this;
switch (_that) {
case Event_StateChanged() when stateChanged != null:
return stateChanged(_that);case Event_Position() when position != null:
return position(_that);case Event_TrackChanged() when trackChanged != null:
return trackChanged(_that);case Event_PlaybackEnded() when playbackEnded != null:
return playbackEnded(_that);case Event_VolumeChanged() when volumeChanged != null:
return volumeChanged(_that);case Event_Error() when error != null:
return error(_that);case Event_Log() when log != null:
return log(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( Event_StateChanged value)  stateChanged,required TResult Function( Event_Position value)  position,required TResult Function( Event_TrackChanged value)  trackChanged,required TResult Function( Event_PlaybackEnded value)  playbackEnded,required TResult Function( Event_VolumeChanged value)  volumeChanged,required TResult Function( Event_Error value)  error,required TResult Function( Event_Log value)  log,}){
final _that = this;
switch (_that) {
case Event_StateChanged():
return stateChanged(_that);case Event_Position():
return position(_that);case Event_TrackChanged():
return trackChanged(_that);case Event_PlaybackEnded():
return playbackEnded(_that);case Event_VolumeChanged():
return volumeChanged(_that);case Event_Error():
return error(_that);case Event_Log():
return log(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( Event_StateChanged value)?  stateChanged,TResult? Function( Event_Position value)?  position,TResult? Function( Event_TrackChanged value)?  trackChanged,TResult? Function( Event_PlaybackEnded value)?  playbackEnded,TResult? Function( Event_VolumeChanged value)?  volumeChanged,TResult? Function( Event_Error value)?  error,TResult? Function( Event_Log value)?  log,}){
final _that = this;
switch (_that) {
case Event_StateChanged() when stateChanged != null:
return stateChanged(_that);case Event_Position() when position != null:
return position(_that);case Event_TrackChanged() when trackChanged != null:
return trackChanged(_that);case Event_PlaybackEnded() when playbackEnded != null:
return playbackEnded(_that);case Event_VolumeChanged() when volumeChanged != null:
return volumeChanged(_that);case Event_Error() when error != null:
return error(_that);case Event_Log() when log != null:
return log(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( PlayerState state)?  stateChanged,TResult Function( PlatformInt64 ms,  String path,  BigInt sessionId)?  position,TResult Function( String path)?  trackChanged,TResult Function( String path)?  playbackEnded,TResult Function( double volume)?  volumeChanged,TResult Function( String message)?  error,TResult Function( String message)?  log,required TResult orElse(),}) {final _that = this;
switch (_that) {
case Event_StateChanged() when stateChanged != null:
return stateChanged(_that.state);case Event_Position() when position != null:
return position(_that.ms,_that.path,_that.sessionId);case Event_TrackChanged() when trackChanged != null:
return trackChanged(_that.path);case Event_PlaybackEnded() when playbackEnded != null:
return playbackEnded(_that.path);case Event_VolumeChanged() when volumeChanged != null:
return volumeChanged(_that.volume);case Event_Error() when error != null:
return error(_that.message);case Event_Log() when log != null:
return log(_that.message);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( PlayerState state)  stateChanged,required TResult Function( PlatformInt64 ms,  String path,  BigInt sessionId)  position,required TResult Function( String path)  trackChanged,required TResult Function( String path)  playbackEnded,required TResult Function( double volume)  volumeChanged,required TResult Function( String message)  error,required TResult Function( String message)  log,}) {final _that = this;
switch (_that) {
case Event_StateChanged():
return stateChanged(_that.state);case Event_Position():
return position(_that.ms,_that.path,_that.sessionId);case Event_TrackChanged():
return trackChanged(_that.path);case Event_PlaybackEnded():
return playbackEnded(_that.path);case Event_VolumeChanged():
return volumeChanged(_that.volume);case Event_Error():
return error(_that.message);case Event_Log():
return log(_that.message);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( PlayerState state)?  stateChanged,TResult? Function( PlatformInt64 ms,  String path,  BigInt sessionId)?  position,TResult? Function( String path)?  trackChanged,TResult? Function( String path)?  playbackEnded,TResult? Function( double volume)?  volumeChanged,TResult? Function( String message)?  error,TResult? Function( String message)?  log,}) {final _that = this;
switch (_that) {
case Event_StateChanged() when stateChanged != null:
return stateChanged(_that.state);case Event_Position() when position != null:
return position(_that.ms,_that.path,_that.sessionId);case Event_TrackChanged() when trackChanged != null:
return trackChanged(_that.path);case Event_PlaybackEnded() when playbackEnded != null:
return playbackEnded(_that.path);case Event_VolumeChanged() when volumeChanged != null:
return volumeChanged(_that.volume);case Event_Error() when error != null:
return error(_that.message);case Event_Log() when log != null:
return log(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class Event_StateChanged extends Event {
  const Event_StateChanged({required this.state}): super._();
  

 final  PlayerState state;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Event_StateChangedCopyWith<Event_StateChanged> get copyWith => _$Event_StateChangedCopyWithImpl<Event_StateChanged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event_StateChanged&&(identical(other.state, state) || other.state == state));
}


@override
int get hashCode => Object.hash(runtimeType,state);

@override
String toString() {
  return 'Event.stateChanged(state: $state)';
}


}

/// @nodoc
abstract mixin class $Event_StateChangedCopyWith<$Res> implements $EventCopyWith<$Res> {
  factory $Event_StateChangedCopyWith(Event_StateChanged value, $Res Function(Event_StateChanged) _then) = _$Event_StateChangedCopyWithImpl;
@useResult
$Res call({
 PlayerState state
});




}
/// @nodoc
class _$Event_StateChangedCopyWithImpl<$Res>
    implements $Event_StateChangedCopyWith<$Res> {
  _$Event_StateChangedCopyWithImpl(this._self, this._then);

  final Event_StateChanged _self;
  final $Res Function(Event_StateChanged) _then;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? state = null,}) {
  return _then(Event_StateChanged(
state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as PlayerState,
  ));
}


}

/// @nodoc


class Event_Position extends Event {
  const Event_Position({required this.ms, required this.path, required this.sessionId}): super._();
  

 final  PlatformInt64 ms;
 final  String path;
 final  BigInt sessionId;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Event_PositionCopyWith<Event_Position> get copyWith => _$Event_PositionCopyWithImpl<Event_Position>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event_Position&&(identical(other.ms, ms) || other.ms == ms)&&(identical(other.path, path) || other.path == path)&&(identical(other.sessionId, sessionId) || other.sessionId == sessionId));
}


@override
int get hashCode => Object.hash(runtimeType,ms,path,sessionId);

@override
String toString() {
  return 'Event.position(ms: $ms, path: $path, sessionId: $sessionId)';
}


}

/// @nodoc
abstract mixin class $Event_PositionCopyWith<$Res> implements $EventCopyWith<$Res> {
  factory $Event_PositionCopyWith(Event_Position value, $Res Function(Event_Position) _then) = _$Event_PositionCopyWithImpl;
@useResult
$Res call({
 PlatformInt64 ms, String path, BigInt sessionId
});




}
/// @nodoc
class _$Event_PositionCopyWithImpl<$Res>
    implements $Event_PositionCopyWith<$Res> {
  _$Event_PositionCopyWithImpl(this._self, this._then);

  final Event_Position _self;
  final $Res Function(Event_Position) _then;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? ms = null,Object? path = null,Object? sessionId = null,}) {
  return _then(Event_Position(
ms: null == ms ? _self.ms : ms // ignore: cast_nullable_to_non_nullable
as PlatformInt64,path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,sessionId: null == sessionId ? _self.sessionId : sessionId // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class Event_TrackChanged extends Event {
  const Event_TrackChanged({required this.path}): super._();
  

 final  String path;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Event_TrackChangedCopyWith<Event_TrackChanged> get copyWith => _$Event_TrackChangedCopyWithImpl<Event_TrackChanged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event_TrackChanged&&(identical(other.path, path) || other.path == path));
}


@override
int get hashCode => Object.hash(runtimeType,path);

@override
String toString() {
  return 'Event.trackChanged(path: $path)';
}


}

/// @nodoc
abstract mixin class $Event_TrackChangedCopyWith<$Res> implements $EventCopyWith<$Res> {
  factory $Event_TrackChangedCopyWith(Event_TrackChanged value, $Res Function(Event_TrackChanged) _then) = _$Event_TrackChangedCopyWithImpl;
@useResult
$Res call({
 String path
});




}
/// @nodoc
class _$Event_TrackChangedCopyWithImpl<$Res>
    implements $Event_TrackChangedCopyWith<$Res> {
  _$Event_TrackChangedCopyWithImpl(this._self, this._then);

  final Event_TrackChanged _self;
  final $Res Function(Event_TrackChanged) _then;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,}) {
  return _then(Event_TrackChanged(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class Event_PlaybackEnded extends Event {
  const Event_PlaybackEnded({required this.path}): super._();
  

 final  String path;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Event_PlaybackEndedCopyWith<Event_PlaybackEnded> get copyWith => _$Event_PlaybackEndedCopyWithImpl<Event_PlaybackEnded>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event_PlaybackEnded&&(identical(other.path, path) || other.path == path));
}


@override
int get hashCode => Object.hash(runtimeType,path);

@override
String toString() {
  return 'Event.playbackEnded(path: $path)';
}


}

/// @nodoc
abstract mixin class $Event_PlaybackEndedCopyWith<$Res> implements $EventCopyWith<$Res> {
  factory $Event_PlaybackEndedCopyWith(Event_PlaybackEnded value, $Res Function(Event_PlaybackEnded) _then) = _$Event_PlaybackEndedCopyWithImpl;
@useResult
$Res call({
 String path
});




}
/// @nodoc
class _$Event_PlaybackEndedCopyWithImpl<$Res>
    implements $Event_PlaybackEndedCopyWith<$Res> {
  _$Event_PlaybackEndedCopyWithImpl(this._self, this._then);

  final Event_PlaybackEnded _self;
  final $Res Function(Event_PlaybackEnded) _then;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,}) {
  return _then(Event_PlaybackEnded(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class Event_VolumeChanged extends Event {
  const Event_VolumeChanged({required this.volume}): super._();
  

 final  double volume;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Event_VolumeChangedCopyWith<Event_VolumeChanged> get copyWith => _$Event_VolumeChangedCopyWithImpl<Event_VolumeChanged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event_VolumeChanged&&(identical(other.volume, volume) || other.volume == volume));
}


@override
int get hashCode => Object.hash(runtimeType,volume);

@override
String toString() {
  return 'Event.volumeChanged(volume: $volume)';
}


}

/// @nodoc
abstract mixin class $Event_VolumeChangedCopyWith<$Res> implements $EventCopyWith<$Res> {
  factory $Event_VolumeChangedCopyWith(Event_VolumeChanged value, $Res Function(Event_VolumeChanged) _then) = _$Event_VolumeChangedCopyWithImpl;
@useResult
$Res call({
 double volume
});




}
/// @nodoc
class _$Event_VolumeChangedCopyWithImpl<$Res>
    implements $Event_VolumeChangedCopyWith<$Res> {
  _$Event_VolumeChangedCopyWithImpl(this._self, this._then);

  final Event_VolumeChanged _self;
  final $Res Function(Event_VolumeChanged) _then;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? volume = null,}) {
  return _then(Event_VolumeChanged(
volume: null == volume ? _self.volume : volume // ignore: cast_nullable_to_non_nullable
as double,
  ));
}


}

/// @nodoc


class Event_Error extends Event {
  const Event_Error({required this.message}): super._();
  

 final  String message;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Event_ErrorCopyWith<Event_Error> get copyWith => _$Event_ErrorCopyWithImpl<Event_Error>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event_Error&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'Event.error(message: $message)';
}


}

/// @nodoc
abstract mixin class $Event_ErrorCopyWith<$Res> implements $EventCopyWith<$Res> {
  factory $Event_ErrorCopyWith(Event_Error value, $Res Function(Event_Error) _then) = _$Event_ErrorCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$Event_ErrorCopyWithImpl<$Res>
    implements $Event_ErrorCopyWith<$Res> {
  _$Event_ErrorCopyWithImpl(this._self, this._then);

  final Event_Error _self;
  final $Res Function(Event_Error) _then;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(Event_Error(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class Event_Log extends Event {
  const Event_Log({required this.message}): super._();
  

 final  String message;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Event_LogCopyWith<Event_Log> get copyWith => _$Event_LogCopyWithImpl<Event_Log>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is Event_Log&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'Event.log(message: $message)';
}


}

/// @nodoc
abstract mixin class $Event_LogCopyWith<$Res> implements $EventCopyWith<$Res> {
  factory $Event_LogCopyWith(Event_Log value, $Res Function(Event_Log) _then) = _$Event_LogCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$Event_LogCopyWithImpl<$Res>
    implements $Event_LogCopyWith<$Res> {
  _$Event_LogCopyWithImpl(this._self, this._then);

  final Event_Log _self;
  final $Res Function(Event_Log) _then;

/// Create a copy of Event
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(Event_Log(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
